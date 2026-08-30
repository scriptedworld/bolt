//! Executing a jig's tasks and keeping what they produced.

use std::fs::{self, File};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};

use crate::adapter;
use crate::definitions::{Definitions, RESERVED};
use crate::depth;
use crate::jig::{self, Task};
use crate::limit;
use crate::selection::{self, consumes_paths, quote, quote_str};
use crate::{Error, Outcome, Refusal, merge, stamp, walk};

/// The directory under a run's output directory holding one entry per execution.
pub const WORK_DIR: &str = "work";

/// The name of the file holding an execution's captured exit status.
pub const EXITCODE_FILE: &str = "exitcode";

/// The name of the file holding an execution's manifest.
pub const MANIFEST_FILE: &str = "manifest.yaml";

/// The name of the file an adapter writes its envelope to.
pub const OUTPUT_FILE: &str = "output.yaml";

/// The name of the file a run's merged result is written to.
pub const RESULT_FILE: &str = "result.yaml";

/// What one task was handed, once its selection is settled.
struct Selection {
    /// What `matching` chose and `excluding` left alone.
    selected: Vec<PathBuf>,
    /// What `excluding` removed from what `matching` chose.
    removed: Vec<PathBuf>,
}

/// One execution of a task: what it is called, which of its task's executions
/// it is, the command it was given, and where its evidence goes.
struct Execution<'a> {
    /// The task this execution belongs to, by FR-9.2a.
    task: &'a str,
    /// This execution's index within that task, numbered from one.
    ordinal: usize,
    /// The command line as executed, after substitution.
    command: String,
    /// Where FR-9.2's evidence for this execution is kept.
    work_dir: PathBuf,
}

/// Everything a command's placeholders resolve against.
///
/// The two layers travel together because FR-4.16 makes them one mapping: a
/// value a jig defined and a location bolt exposed are written and read the
/// same way, so nothing that substitutes or records has one without the other.
struct Scope<'a> {
    /// Bolt's own layer, reserved by FR-4.19.
    locations: &'a Locations,
    /// The jig's block and the named file, merged by FR-4.17.
    definitions: &'a Definitions,
    /// How deep this run is, by FR-5.6, exported to everything it spawns.
    depth: depth::Depth,

    /// The run's own limit, by FR-4.13, or `None` where the jig sets none.
    ///
    /// Scope-wide because it is the one limit that reaches everything: a
    /// command, an adapter by FR-4.12c, and whether a later task starts at all.
    run_limit: Option<Limit<'a>>,
}

/// A limit that has been read: when it runs out, and how the jig spelled it.
///
/// The written form travels with the instant so a reason quotes the jig rather
/// than a rounding of it. A task told it passed `90s` when the jig says `1.5m`
/// sends a reader looking for a number that is not in the file.
#[derive(Debug, Clone, Copy)]
struct Limit<'a> {
    /// When it runs out.
    at: Instant,
    /// The limit as written in the jig.
    written: &'a str,
}

/// The limits governing one task, by FR-4.11.
#[derive(Debug, Clone, Copy)]
struct Deadlines<'a> {
    /// The run's, which outlives any one task.
    run: Option<Limit<'a>>,
    /// This task's own, measured from when the task started by FR-4.11f.
    task: Option<Limit<'a>>,
}

/// Which limit fired, and how it was written.
#[derive(Debug, Clone, Copy)]
struct Expired<'a> {
    /// Whether the run's limit or the task's, which decides what stops.
    whose: Whose,
    /// The limit as written in the jig, for the reason to quote.
    written: &'a str,
}

/// Whose limit it was.
///
/// The two differ in what they stop. FR-4.12 has a task's limit fail that task
/// and leave the run going, by FR-4.8. FR-4.13 has the run's stop the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Whose {
    /// The task's own limit.
    Task,
    /// The run's.
    Run,
}

impl<'a> Deadlines<'a> {
    /// When a command must be killed: whichever limit runs out first.
    fn command(self) -> Option<Instant> {
        limit::soonest(self.run.map(|run| run.at), self.task.map(|task| task.at))
    }

    /// When an adapter must be killed, by FR-4.11c.
    ///
    /// The run's limit alone. A task's limit does not reach its adapter, because
    /// the adapter is what records that the limit fired and a budget exhausted
    /// by the command it was killing would leave nothing to write the envelope
    /// FR-4.12d requires.
    fn adapter(self) -> Option<Instant> {
        self.run.map(|run| run.at)
    }

    /// Which limit has already passed at `now`, where one has.
    ///
    /// The run's is tested first. Where both have passed the run's is the one
    /// that matters, since it stops everything and the task's would stop one
    /// task.
    fn expired(self, now: Instant) -> Option<Expired<'a>> {
        for (whose, limit) in [(Whose::Run, self.run), (Whose::Task, self.task)] {
            if let Some(limit) = limit
                && now >= limit.at
            {
                return Some(Expired {
                    whose,
                    written: limit.written,
                });
            }
        }
        None
    }
}

impl Expired<'_> {
    /// What a reason says when this limit fired, by FR-4.12 and FR-4.13.
    ///
    /// FR-4.12f puts the unattempted count here. A per-path task cut off at path
    /// fifty leaves fifty work directories and nothing else saying the other
    /// three hundred and fifty were never tried, so the reader who wants to know
    /// how much went unchecked has only the reason to read it from.
    fn message(self, task: &str, unattempted: usize) -> String {
        let written = self.written;
        // Both name the task, because this reason sits on an execution and a
        // reader meets it there. The run's own reason, which FR-4.13 puts in the
        // result, is written separately and names no task: the two would
        // otherwise be the same sentence twice in one file.
        let passed = match self.whose {
            Whose::Task => format!("task {task} passed its time limit of {written}"),
            Whose::Run => {
                format!("task {task} was stopped when the run passed its time limit of {written}")
            }
        };
        if unattempted == 0 {
            passed
        } else {
            format!("{passed}; {unattempted} of its executions were not attempted")
        }
    }
}

/// The locations bolt exposes to every command, by FR-4.1b.
///
/// All five are reserved to bolt's own layer, which is why each is recorded
/// `from: "bolt"`. FR-4.16's jig and file layers merge over them and belong to
/// `definitions/10` rather than to the skeleton.
struct Locations {
    /// The outermost invocation's directory. No nesting here, so it is the base.
    project_root: PathBuf,
    /// The directory this run was pointed at.
    base_dir: PathBuf,
    /// Where `bolt.<name>.yaml` was found, which FR-2.8 defaults to the base.
    config_dir: PathBuf,
    /// The run directory, by FR-2.6c.
    output_dir: PathBuf,
}

/// Run the jig named `jig` over `base`.
///
/// This is the whole of an invocation, by FR-2.1 and FR-2.1a: one jig, one
/// directory. FR-3.9 has the jig named rather than pathed, and FR-2.8 puts
/// `bolt.<jig>.yaml` in the config directory, which for this task is `base`.
///
/// # Errors
///
/// [`Error::BaseMissing`] when `base` is not there, by FR-2.5. **The check runs
/// before anything is created.** The Go build made the base as a side effect of
/// preparing the output directory, so a run over a typo'd path checked an empty
/// tree and passed.
///
/// [`Error::JigUnreadable`] when the jig is absent or will not parse,
/// [`Error::CommandNamesBothPathForms`] when a command names both path forms by
/// FR-4.2, and [`Error::Io`] when the run cannot record what it did.
pub fn run(jig: &str, base: &Path) -> Result<Outcome, Error> {
    invoke(&Invocation {
        jig,
        base,
        definitions: None,
        output_dir: None,
        config_dir: None,
    })
    .map_err(Error::from)
}

/// Everything an invocation names.
///
/// Two positionals and the rest optional, by FR-2.1a: which jig and where is a
/// complete invocation, and everything here beyond those two has a default that
/// makes naming it unnecessary.
pub struct Invocation<'a> {
    /// Which jig, by FR-3.9. A name, never a path.
    pub jig: &'a str,
    /// Where, and the run's base.
    pub base: &'a Path,
    /// The definitions file named by FR-4.16a, if one was.
    pub definitions: Option<&'a str>,
    /// Where evidence goes, by FR-2.6. `None` is `.bolt-<iso8601>` at the base.
    pub output_dir: Option<&'a Path>,
    /// Where jigs are found, by FR-2.8. `None` is the base.
    ///
    /// FR-2.8 has where jigs live told to bolt rather than inferred from the
    /// directory being run on, so one shared jig directory can serve a tree it
    /// does not sit in. The default stays the base, which is what makes naming
    /// it unnecessary for a project keeping its own jigs.
    pub config_dir: Option<&'a Path>,
}

/// Carry out an invocation.
///
/// # Errors
///
/// Everything [`run`] returns, plus [`Error::ReservedDefinition`] by FR-4.19,
/// [`Error::DefinitionsUnreadable`] by FR-4.20, and [`Error::OutputDirectoryInUse`]
/// by FR-2.6b for a named directory that already holds a run.
///
/// Each arrives inside a [`Refusal`], which carries where the reason was
/// written as well as what it was. FR-10.3a has the command line print that
/// path, and this is the only place it is known: FR-2.6c derives the default
/// from a stamp taken below.
pub fn invoke(invocation: &Invocation) -> Result<Outcome, Refusal> {
    let Invocation {
        jig,
        base,
        definitions,
        output_dir,
        config_dir,
    } = invocation;

    // One stamp for the whole invocation. Taking it twice would let a second
    // boundary fall between them and put a refusal somewhere the run would not
    // have written.
    let started = SystemTime::now();
    let named = *output_dir;

    if !base.is_dir() {
        return Err(base_missing(base, named));
    }

    // FR-2.4, and this is the only place it has to happen. Every path bolt
    // records or substitutes descends from the base, so resolving it once here
    // resolves all of them, and `bolt gate .` stops recording `"value": "."` in
    // manifests that a reader standing somewhere else cannot use.
    //
    // FR-4.17b keeps this away from definitions: nothing distinguishes
    // `../REQUIREMENTS.md` from `100`, so a definition's value is left alone and
    // reaches its command as written.
    let base = &fs::canonicalize(base).map_err(|source| {
        // Nothing is written: the base is what could not be resolved, so there
        // is no run directory yet to write into.
        wrote_nothing(Error::Io {
            path: base.to_path_buf(),
            reason: source.to_string(),
        })
    })?;

    let output_dir = output_dir_of(base, named, started)?;

    // Everything past here is bolt alive and in control, so FR-10.7 wants a
    // result for whatever goes wrong. The refusal is written into the directory
    // this run owns, which the two checks above have already established is not
    // somebody else's.
    // FR-2.8 defaults to the base, absolute like every path a caller writes.
    let config_dir = config_dir.map_or_else(|| base.clone(), absolute);

    carry_out(jig, base, &output_dir, *definitions, &config_dir).map_err(|error| {
        write_refusal(&output_dir, &error);
        Refusal {
            error,
            result: Some(output_dir.join(RESULT_FILE)),
        }
    })
}

/// Where this run's evidence goes, refusing a directory already holding one.
///
/// FR-2.4 reaches the output directory as it reaches the base, and it has to
/// happen after the base is canonical: the default is derived from the base,
/// and a named one is resolved against the working directory like any path on a
/// command line. `.bolt-<iso8601>` at a relative base would otherwise be
/// recorded as `./.bolt-…`, which is the defect FR-2.4 exists to prevent one
/// level up.
///
/// # Errors
///
/// [`Error::OutputDirectoryInUse`] by FR-2.6b, for a named directory as much as
/// for the default. **It returns before anything is written, and that ordering
/// is the guarantee rather than an implementation detail**; `holds_a_run`
/// carries why.
fn output_dir_of(
    base: &Path,
    named: Option<&Path>,
    started: SystemTime,
) -> Result<PathBuf, Refusal> {
    let output_dir = named.map_or_else(|| output_dir_for(base, started), absolute);
    if holds_a_run(&output_dir) {
        return Err(wrote_nothing(Error::OutputDirectoryInUse(output_dir)));
    }
    Ok(output_dir)
}

/// A refusal that wrote nothing, by FR-10.7a.
///
/// Named rather than written inline at each site so that "nothing was written"
/// is one decision with one spelling. The two callers are FR-2.6b's occupied
/// directory and a base that would not resolve, and both return before this run
/// owns anywhere to write.
fn wrote_nothing(error: Error) -> Refusal {
    Refusal {
        error,
        result: None,
    }
}

/// FR-2.5's refusal, with FR-10.7a's exemption applied to it.
///
/// The exemption is about the **directory**, not the error. The default output
/// directory sits inside the base, so writing there would create the thing whose
/// absence is being refused. One named outside it has no such problem, which is
/// exactly what FR-10.7b tells a caller who wants a parseable refusal in every
/// case to do.
fn base_missing(base: &Path, named: Option<&Path>) -> Refusal {
    let error = Error::BaseMissing(base.to_path_buf());
    if wrote_a_result(&error, base, named) {
        // Only reachable with a named directory outside the base, so there is
        // one to write to and no default to derive from a base that is not
        // there.
        if let Some(path) = named {
            let output_dir = absolute(path);
            write_refusal(&output_dir, &error);
            return Refusal {
                error,
                result: Some(output_dir.join(RESULT_FILE)),
            };
        }
    }
    wrote_nothing(error)
}

/// A path made absolute without touching the filesystem, by FR-2.4.
///
/// `canonicalize` is wrong here: an output directory need not exist yet, since
/// FR-2.6a has bolt create it. This resolves against the working directory and
/// leaves symlinks alone, which is what a path on a command line means.
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Whether a refusal leaves a `result.yaml` behind.
///
/// Stated once and consulted twice: [`invoke`] to decide whether to write one,
/// and the command line to decide whether to tell a caller that none was
/// written. Those two answers disagreeing is how "no result" comes to mean two
/// different things, and FR-10.7 has a caller read an absent result as a bolt
/// that was killed.
///
/// `output_dir` is what the caller named, not what a run resolved, so `None`
/// means the FR-2.6c default at the base.
#[must_use]
pub fn wrote_a_result(refusal: &Error, base: &Path, output_dir: Option<&Path>) -> bool {
    if refusal.never_writes_a_result() {
        return false;
    }
    // FR-10.7a and FR-10.7b together: the missing base is exempt only while the
    // result would land inside it.
    if matches!(refusal, Error::BaseMissing(_)) {
        return output_dir.is_some_and(|named| !named.starts_with(base));
    }
    true
}

/// Resolve every `requires` entry against `PATH`, by FR-3.10b.
///
/// An incomplete toolchain is known in the first second rather than partway
/// through a gate, which is the whole of what this buys. It names **every**
/// missing entry rather than the first, so a caller fixing them does not pay a
/// round trip per tool.
///
/// FR-3.10c keeps this narrow: it is a guarantee about `requires`, not about
/// every way a process fails to launch. A command invoking something the jig
/// never declared still fails its own task by FR-4.10, and FR-4.10a says the
/// reason names what the shell reported rather than a `requires` entry, because
/// a declared tool cannot be the one that failed to start.
///
/// # Errors
///
/// [`Error::RequiresMissing`], naming every entry `PATH` does not resolve.
fn check_requires(jig: &jig::Jig) -> Result<(), Error> {
    let mut missing: Vec<String> = jig
        .requires
        .iter()
        .filter(|entry| !on_path(entry))
        .cloned()
        .collect();

    if missing.is_empty() {
        return Ok(());
    }
    // Sorted so a jig missing three tools names them the same way every run,
    // which is what makes the message diffable between two runs of a gate.
    missing.sort();
    missing.dedup();
    Err(Error::RequiresMissing { tools: missing })
}

/// Whether `PATH` resolves `entry` to something executable.
///
/// An entry carrying a separator is a path rather than a name, and is taken as
/// written: `requires` lists executables a jig invokes, and a command may
/// invoke one by path.
fn on_path(entry: &str) -> bool {
    let candidate = Path::new(entry);
    if candidate.components().count() > 1 {
        return executable(candidate);
    }
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|directory| executable(&directory.join(entry)))
    })
}

/// Whether a path names a file somebody could execute.
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// The walk, with FR-2.2c's exclusion applied.
///
/// A run never walks its own output directory, whatever it was named and
/// whatever `.gitignore` says, which is knowable because the run created it.
/// The default `.bolt-<iso8601>` is hidden and `ignore` would skip it anyway; a
/// named `build/qa` is not, so the exclusion is explicit rather than inherited
/// from a default that happens to cover one case.
///
/// Another run's output directory is not recognisable by name, which is why
/// FR-2.6b refuses one that already holds a run rather than pretending this can
/// spot it.
fn walk_excluding(base: &Path, output_dir: &Path) -> Result<Vec<PathBuf>, Error> {
    Ok(walk::walk(base)?
        .into_iter()
        .filter(|path| !path.starts_with(output_dir))
        .collect())
}

/// Whether a directory already holds a run, by FR-2.6b.
///
/// A directory bolt would create is not one that holds a run, and FR-2.6a has
/// `--output-dir` created if it is not there. So the question is whether there
/// is a result or a work directory in it rather than whether the path exists: a
/// caller pointing two runs at an existing empty `build/qa` is doing what
/// FR-2.6a describes, not what FR-2.6b refuses.
///
/// **Refusing here rather than writing is the whole guarantee.** Writing into
/// one interleaves two runs' evidence, and the default stamp is second-granular
/// so two runs started in one second resolve to the same directory. Reproduced
/// 2026-08-28 against the Go build: a second jig's refusal replaced the first's
/// completed verdict while its per-task evidence still said otherwise.
/// `a_refusal_does_not_write_into_the_directory_it_refused` holds it, and
/// removing the directory is the caller's decision rather than bolt's.
fn holds_a_run(output_dir: &Path) -> bool {
    output_dir.join(RESULT_FILE).exists() || output_dir.join(WORK_DIR).exists()
}

/// Where a run over `base` starting at `at` writes, by FR-2.6c.
///
/// FR-2.6's filesystem-safe stamp and not the strict ISO 8601 form, since this
/// is a path component.
///
/// FR-2.6e appends the process id. The stamp is second-granular, so two
/// invocations starting in one second resolve to one directory and FR-2.6b
/// refuses the second; one invocation is one process, so the id separates them
/// and says which run wrote a directory that is still there.
#[must_use]
fn output_dir_for(base: &Path, at: SystemTime) -> PathBuf {
    base.join(format!(
        ".bolt-{}-{}",
        stamp::iso8601(at),
        std::process::id(),
    ))
}

/// Write a refusal's `result.yaml`, by FR-10.7, in FR-2.5a's shape.
///
/// Best effort by design. A filesystem failure here leaves the message on
/// stderr as the only record, which is the same state a killed bolt leaves and
/// is already what FR-10.7 tells a caller to read that way. Returning this
/// error instead would replace the reason the run was refused with the reason
/// the note about it could not be written, which is the less useful of the two.
fn write_refusal(output_dir: &Path, refusal: &Error) {
    debug_assert!(
        !refusal.never_writes_a_result(),
        "a refusal exempt from FR-10.7 reached the writer: {refusal:?}",
    );

    let result = json!({
        "success": false,
        // FR-10.9. The kind says which sort of refusal this was, so a consumer
        // tells a missing base from an unreadable jig without reading English.
        "reasons": [{ "kind": refusal.kind(), "message": refusal.to_string() }],
    });
    let _ = create_dir(output_dir);
    let _ = save(
        &output_dir.join(RESULT_FILE),
        &result,
        &wrench::schemas::ENVELOPE,
    );
}

/// Carry the run out, once the output directory is known to be bolt's own.
fn carry_out(
    jig: &str,
    base: &Path,
    output_dir: &Path,
    definitions: Option<&str>,
    config_dir: &Path,
) -> Result<Outcome, Error> {
    let output_dir = output_dir.to_path_buf();

    let depth = within_ceiling()?;
    let jig = jig::read(config_dir, jig)?;

    // FR-4.16a reads a definitions file from the config directory, where FR-3.9
    // puts the jig, so `--config-dir` moves a jig and its adjustments together.
    let definitions = Definitions::build(jig.definitions.as_ref(), config_dir, definitions)?;

    // Everything a jig can be refused for, checked before any task executes.
    // FR-3.10b makes that the shape: an incomplete jig is known before half a
    // gate has run rather than partway through it. FR-4.18a puts the unknown
    // placeholder check here for the same reason, so a jig run where nothing
    // defines what it needs refuses in the first second rather than partway
    // through a gate. FR-4.11e joins them: a limit that is not a duration is a
    // jig error, and finding it two tasks in would waste the run it was meant
    // to bound.
    let plans = validate(&jig, &definitions)?;
    let run_limit = read_limit(jig.time_limit.as_deref(), None)?;
    check_requires(&jig)?;

    // FR-4.11f: the run's clock starts once the jig is known good. A refused jig
    // spends none of the budget, and the walk, which is real work over a real
    // tree, spends it like anything else does.
    let started = Instant::now();

    // Created before the walk. Creating it afterwards made FR-2.2c's exclusion
    // below true by accident, for a directory bolt had not made yet.
    create_dir(&output_dir.join(WORK_DIR))?;

    let walked = walk_excluding(base, &output_dir)?;

    let locations = locations_for(base, config_dir, &output_dir);
    let scope = Scope {
        locations: &locations,
        definitions: &definitions,
        depth,
        run_limit: run_limit.map(|(duration, written)| Limit {
            at: started + duration,
            written,
        }),
    };

    let progress = run_tasks(&scope, &jig, plans, &walked)?;

    merge::merge(&output_dir, base, &run_reasons(progress.expired)).map(|folded| Outcome {
        executions: progress.executions,
        stopped: progress.stopped,
        ..folded
    })
}

/// The reasons a run carries in its own right, by FR-4.13.
///
/// Handed to the merge rather than left on disk for it to find, because a passed
/// run limit is a property of the run and no constituent can carry it. FR-4.14
/// is why the merge still runs at all: a run that times out carries what
/// completed.
///
/// Named without a task, unlike the reason each stopped execution carries, so
/// one file does not say the same sentence twice.
fn run_reasons(expired: Option<&str>) -> Vec<Value> {
    expired
        .map(|written| {
            json!({
                "kind": limit::KIND,
                "message": format!("the run passed its time limit of {written}"),
            })
        })
        .into_iter()
        .collect()
}

/// The five locations this run exposes, by FR-4.1b.
///
/// `project_root` and `base_dir` are the same directory while nothing nests:
/// FR-5.13 narrows the base for a child and leaves the root alone, so the two
/// separate only once `50b` runs one. That is also why `50d` waits, since
/// `needs-repository-root` has nothing to reach for until they differ.
fn locations_for(base: &Path, config_dir: &Path, output_dir: &Path) -> Locations {
    Locations {
        project_root: base.to_path_buf(),
        base_dir: base.to_path_buf(),
        config_dir: config_dir.to_path_buf(),
        output_dir: output_dir.to_path_buf(),
    }
}

/// This run's depth, refused by FR-5.7 where it is past the ceiling.
///
/// Checked before the jig is read, so a run too deep does no work and opens no
/// files. FR-5.8 still has it write a result, which [`invoke`] arranges for
/// every refusal, so the run that spawned it folds an ordinary failure rather
/// than meeting a hole.
fn within_ceiling() -> Result<depth::Depth, Error> {
    let depth = depth::Depth::from_environment();
    if depth.exceeded() {
        return Err(Error::DepthExceeded {
            level: depth.level,
            ceiling: depth.ceiling,
        });
    }
    Ok(depth)
}

/// Read a `time-limit` as written, refusing by FR-4.11e where it is not one.
///
/// `task` names the task whose limit it is, or `None` for the jig's own, so the
/// reason says which line to edit.
///
/// Reading an unparseable limit as no limit is the alternative, and it fails
/// silently: the run goes unbounded exactly where somebody asked for a ceiling,
/// and the jig looks like it has one.
fn read_limit<'a>(
    written: Option<&'a str>,
    task: Option<&str>,
) -> Result<Option<(Duration, &'a str)>, Error> {
    let Some(written) = written else {
        return Ok(None);
    };
    let Some(duration) = limit::parse(written) else {
        return Err(Error::MalformedTimeLimit {
            task: task.map(str::to_owned),
            value: written.to_owned(),
        });
    };
    Ok(Some((duration, written)))
}

/// Execute a jig's tasks in order, returning how many ran and what did not.
///
/// FR-4.5 executes them serially. FR-4.8 is the default and stays it: a failing
/// task does not stop the run, because stopping throws away the evidence the
/// tasks after it would have produced and leaves a reader unable to tell what
/// else was wrong. FR-4.9 is the exception a jig asks for.
///
/// The verdict a short-circuit reads is taken back off the envelope the task
/// just wrote rather than tracked alongside. The envelope is the authoritative
/// result by FR-6.1, and a second copy in bookkeeping is a second thing that can
/// disagree with the evidence on disk.
fn run_tasks<'a>(
    scope: &Scope<'a>,
    jig: &'a jig::Jig,
    plans: Vec<Plan<'a>>,
    walked: &[PathBuf],
) -> Result<Progress<'a>, Error> {
    let mut executions = 0;
    let names_from = |index: usize| -> Vec<String> {
        jig.tasks[index..]
            .iter()
            .map(|later| later.name.clone())
            .collect()
    };

    for (index, (task, plan)) in jig.tasks.iter().zip(plans).enumerate() {
        // FR-4.13's limit stops tasks starting, the same way FR-4.11b's stops
        // executions starting. Checked before the task rather than after, so a
        // run already over its budget does not begin work it will have to kill.
        if let Some(expired) = scope.run_limit.filter(|run| Instant::now() >= run.at) {
            return Ok(Progress {
                executions,
                stopped: names_from(index),
                expired: Some(expired.written),
            });
        }

        let ran = run_task(scope, task, &plan, walked)?;
        executions += ran.executions;

        // A task's own limit fails that task and leaves the run going, by
        // FR-4.12 and FR-4.8. Only the run's stops everything.
        if let Some(expired) = ran.expired.filter(|it| it.whose == Whose::Run) {
            return Ok(Progress {
                executions,
                stopped: names_from(index + 1),
                expired: Some(expired.written),
            });
        }

        if task.short_circuit_failure && !task_passed(&scope.locations.output_dir, &task.name) {
            return Ok(Progress {
                executions,
                stopped: names_from(index + 1),
                expired: None,
            });
        }
    }

    Ok(Progress {
        executions,
        stopped: Vec::new(),
        expired: None,
    })
}

/// How far a run got.
struct Progress<'a> {
    /// How many executions ran, across every task.
    executions: usize,
    /// Tasks that did not run, in declaration order.
    stopped: Vec<String>,
    /// The run's limit as written, where it was the run's limit that stopped it.
    expired: Option<&'a str>,
}

/// How far one task got.
struct TaskRun<'a> {
    /// How many times its command executed.
    executions: usize,
    /// Which limit fired during it, where one did.
    expired: Option<Expired<'a>>,
}

/// Whether every execution of `task` passed, read back off what it wrote.
///
/// FR-4.9's short-circuit needs a verdict, and this takes it from the envelopes
/// on disk rather than from a boolean carried alongside the run. The envelope is
/// the authoritative result by FR-6.1, so a second copy in bookkeeping is a
/// second thing that can disagree with the evidence, and the evidence is what a
/// reader will have.
///
/// A task that produced no envelope at all has not failed: FR-4.4c's allowed
/// empty selection produces no constituent, and stopping a run on a task that
/// legitimately found nothing would be the opposite of what that field asks for.
fn task_passed(output_dir: &Path, task: &str) -> bool {
    let work = output_dir.join(WORK_DIR);
    let Ok(entries) = fs::read_dir(&work) else {
        return true;
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&format!("{task}-")))
        })
        .all(|path| {
            merge::read(&path.join(OUTPUT_FILE), &wrench::schemas::ENVELOPE)
                .ok()
                .and_then(|envelope| envelope.get("success").and_then(serde_json::Value::as_bool))
                .unwrap_or(true)
        })
}

/// The command a task will run, once the ways of not having one are refused.
///
/// # Errors
///
/// [`Error::TaskNamesAJig`] by FR-5.22, **checked first**, so a jig written
/// against the retired nesting mechanism is told what replaced it rather than
/// told it forgot a field. [`Error::TaskNamesNoCommand`] for a task with
/// nothing to run at all, and [`Error::CommandNamesBothPathForms`] by FR-4.2,
/// since a command cannot be one execution per path and one execution over all
/// of them at once.
fn command_of<'a>(task: &'a Task, named: &impl Fn() -> String) -> Result<&'a str, Error> {
    if task.names_a_jig() {
        return Err(Error::TaskNamesAJig { task: named() });
    }
    let Some(command) = task.command.as_deref() else {
        return Err(Error::TaskNamesNoCommand { task: named() });
    };
    if command.contains("{each_path}") && command.contains("{all_paths}") {
        return Err(Error::CommandNamesBothPathForms { task: named() });
    }
    Ok(command)
}

/// What validation settled about one task, so the run does not re-derive it.
///
/// Everything here was proved before any task executed, which is what FR-3.10b
/// and FR-4.18a ask for and what FR-4.11e joins.
struct Plan<'a> {
    /// The command line as written, which validation proved present.
    command: &'a str,
    /// Whether the command names a path variable, by FR-4.2.
    wants_paths: bool,
    /// This task's limit and how the jig spelled it, proved a duration by
    /// FR-4.11e. It becomes a deadline when the task starts, by FR-4.11f.
    limit: Option<(Duration, &'a str)>,
}

/// Everything a jig is refused for, checked before any task executes.
///
/// FR-3.10b makes that the shape: an incomplete jig is known before half a gate
/// has run rather than partway through it. Returns each task's plan, so the run
/// loop does not re-derive what this already proved.
fn validate<'a>(jig: &'a jig::Jig, definitions: &Definitions) -> Result<Vec<Plan<'a>>, Error> {
    let mut commands = Vec::with_capacity(jig.tasks.len());
    let mut seen: Vec<&str> = Vec::with_capacity(jig.tasks.len());

    for task in &jig.tasks {
        let named = || task.name.clone();

        // FR-3.3a. The name prefixes this task's work directories, so a
        // duplicate puts two tasks' executions in one place: the second
        // overwrites the first's evidence and the fold sees one constituent, so
        // a failing task disappears into a green result.
        if seen.contains(&task.name.as_str()) {
            return Err(Error::DuplicateTaskName { task: named() });
        }
        seen.push(&task.name);

        // The name becomes a path component, so it must stay one. Without this
        // a task named `../../victim` writes a full evidence directory outside
        // the base, which is FR-2.3's containment rather than a naming nicety.
        if Path::new(&task.name).components().count() != 1
            || task.name.contains(std::path::MAIN_SEPARATOR)
        {
            return Err(Error::UnsafeTaskName { task: named() });
        }

        let command = command_of(task, &named)?;

        // FR-4.18a. Checked here rather than at substitution, which happens per
        // execution, so a jig whose second task names a placeholder nothing
        // defines refuses before the first task runs instead of partway through
        // a gate.
        for placeholder in placeholders(command) {
            if !RESERVED.contains(&placeholder) && definitions.get(placeholder).is_none() {
                return Err(Error::UnknownPlaceholder {
                    task: named(),
                    placeholder: placeholder.to_owned(),
                });
            }
        }

        commands.push(Plan {
            command,
            wants_paths: consumes_paths(command),
            limit: read_limit(task.time_limit.as_deref(), Some(&task.name))?,
        });
    }

    Ok(commands)
}

/// Every `{name}` a command line spells, in the order they appear.
///
/// An unmatched brace is literal text rather than a placeholder, which is the
/// same reading `substitute` takes, and the two have to agree or a command
/// passes validation and then fails to substitute.
fn placeholders(command: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = command;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            return found;
        };
        found.push(&after[..close]);
        rest = &after[close + 1..];
    }
    found
}

/// Run one task, returning how far it got.
fn run_task<'a>(
    scope: &Scope<'a>,
    task: &Task,
    plan: &Plan<'a>,
    walked: &[PathBuf],
) -> Result<TaskRun<'a>, Error> {
    let base = scope.locations.base_dir.as_path();
    let selection = narrow(base, walked, task)?;

    if plan.wants_paths && selection.selected.is_empty() {
        // FR-4.4c: an allowed empty selection produces no constituent at all,
        // which is what FR-4.4 alone used to mean for every task.
        if task.optional {
            return Ok(TaskRun {
                executions: 0,
                expired: None,
            });
        }
        empty_selection(scope, task, plan.command, &selection)?;
        return Ok(TaskRun {
            executions: 0,
            expired: None,
        });
    }

    execute_batches(scope, task, plan, &selection)
}

/// Execute a task's batches in turn, stopping at whichever limit fires first.
///
/// FR-4.11f measures the task's limit from here, which is wall clock from the
/// moment the task starts. FR-4.11c keeps that budget off the adapters, so what
/// it charges for is the task's own commands and the short gaps between them,
/// never a tool bolt promised would get to record the kill.
fn execute_batches<'a>(
    scope: &Scope<'a>,
    task: &Task,
    plan: &Plan<'a>,
    selection: &Selection,
) -> Result<TaskRun<'a>, Error> {
    let batches = batches_for(plan.command, plan.wants_paths, selection);

    // FR-9.6: a task naming no path variable was handed no list, so its
    // manifest claims none. Recording one would say the command saw files it
    // never received, so the key is absent rather than empty.
    let recorded = plan.wants_paths.then_some(selection);

    let deadlines = deadlines_for(scope, plan);

    for (index, batch) in batches.iter().enumerate() {
        let work_dir = scope
            .locations
            .output_dir
            .join(WORK_DIR)
            .join(work_dir_name(&task.name, index + 1, batches.len()));
        create_dir(&work_dir)?;

        // Substituted before the manifest is written, because FR-9.5's
        // manifest records the command AS EXECUTED and FR-9.5a writes it before
        // the command runs. Both hold only if substitution comes first.
        let execution = Execution {
            task: &task.name,
            ordinal: index + 1,
            command: substitute(plan.command, &task.name, scope, &work_dir, batch)?,
            work_dir,
        };
        write_manifest(scope, &execution, recorded)?;

        // FR-4.11b: the executions after a killed one do not start. This is the
        // case where the limit fell between two of them rather than during one,
        // so nothing was killed and nothing would otherwise record that the
        // task ran out. FR-9.5a's manifest is already written above, which is
        // that row's "never got started" clause doing its work.
        if let Some(expired) = deadlines.expired(Instant::now()) {
            return stopped_at(&execution, expired, index, batches.len());
        }

        if let Some(expired) = execute(scope, task, &execution, deadlines)? {
            return stopped_at(&execution, expired, index + 1, batches.len());
        }
    }

    Ok(TaskRun {
        executions: batches.len(),
        expired: None,
    })
}

/// The limits governing one task, taken as it starts.
///
/// FR-4.11f measures the task's from here, so it is wall clock from the moment
/// the task starts rather than a total of what its commands spent.
fn deadlines_for<'a>(scope: &Scope<'a>, plan: &Plan<'a>) -> Deadlines<'a> {
    Deadlines {
        run: scope.run_limit,
        task: plan.limit.map(|(duration, written)| Limit {
            at: Instant::now() + duration,
            written,
        }),
    }
}

/// What a task returns when a limit stopped it after `executions` of `total`.
///
/// The two ways a task stops share this, because they differ only in whether the
/// execution holding the reason ran: one was killed partway, the other never
/// started. Either way what is left unattempted is the rest of the list, and
/// FR-4.12f wants that count in the reason.
fn stopped_at<'a>(
    execution: &Execution,
    expired: Expired<'a>,
    executions: usize,
    total: usize,
) -> Result<TaskRun<'a>, Error> {
    timed_out(execution, expired, total - executions)?;
    Ok(TaskRun {
        executions,
        expired: Some(expired),
    })
}

/// Record on one execution that a limit fired.
///
/// FR-4.12b: it fails whatever its adapter concluded, and its reasons carry at
/// least the limit being passed. The adapter's own reasons are kept beside it,
/// because a tool that reported forty problems before hanging reported forty
/// real problems and FR-4.12a keeps them.
///
/// FR-4.12d holds either way. An adapter that wrote an envelope has it amended;
/// an execution that never started gets one written here. So a timed-out
/// execution always has a valid envelope, which is what distinguishes it from
/// one whose adapter died of its own accord and left none.
fn timed_out(execution: &Execution, expired: Expired, unattempted: usize) -> Result<(), Error> {
    let path = execution.work_dir.join(OUTPUT_FILE);

    // The limit first, because it is why the rest is partial.
    let mut reasons = vec![json!({
        "kind": limit::KIND,
        "message": expired.message(execution.task, unattempted),
    })];
    if let Ok(envelope) = merge::read(&path, &wrench::schemas::ENVELOPE) {
        reasons.extend(
            envelope
                .get("reasons")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        );
    }

    save(
        &path,
        &json!({ "success": false, "reasons": reasons }),
        &wrench::schemas::ENVELOPE,
    )
}

/// How a task's selection divides into executions, by FR-4.2.
///
/// `{each_path}` is one execution per matched path; anything else is one
/// execution, whether or not it was handed a list.
fn batches_for(command: &str, wants_paths: bool, selection: &Selection) -> Vec<Vec<PathBuf>> {
    if wants_paths && command.contains("{each_path}") {
        selection
            .selected
            .iter()
            .map(|path| vec![path.clone()])
            .collect()
    } else {
        vec![selection.selected.clone()]
    }
}

/// FR-4.4b's failing constituent for a task that matched nothing.
///
/// It has to be a constituent rather than a skip, or FR-8.3 folds a run that
/// checked nothing into a pass, which is FR-8.3a's argument one level down.
fn empty_selection(
    scope: &Scope,
    task: &Task,
    command: &str,
    selection: &Selection,
) -> Result<(), Error> {
    let execution = Execution {
        task: &task.name,
        ordinal: 1,
        command: command.to_owned(),
        work_dir: scope
            .locations
            .output_dir
            .join(WORK_DIR)
            .join(work_dir_name(&task.name, 1, 1)),
    };
    create_dir(&execution.work_dir)?;
    write_manifest(scope, &execution, Some(selection))?;
    write_envelope(
        &execution.work_dir,
        false,
        "empty-selection",
        &format!(
            "{} matched no paths, and does not allow an empty selection",
            task.name
        ),
    )
}

/// Apply FR-3.4's `matching` and FR-3.4a's `excluding` to the walk.
fn narrow(base: &Path, walked: &[PathBuf], task: &Task) -> Result<Selection, Error> {
    let matched = selection::select(base, walked, &task.matching, &[])?;
    let selected = selection::select(base, walked, &task.matching, &task.excluding)?;
    let removed = matched
        .iter()
        .filter(|path| !selected.contains(path))
        .cloned()
        .collect();
    Ok(Selection { selected, removed })
}

/// Run one execution's command and capture what it produced.
///
/// Returns which limit fired, where one did, so the task loop knows whether to
/// stop this task or the whole run.
fn execute<'a>(
    scope: &Scope<'a>,
    task: &Task,
    execution: &Execution,
    deadlines: Deadlines<'a>,
) -> Result<Option<Expired<'a>>, Error> {
    let work_dir = execution.work_dir.as_path();
    let ran = spawn_and_wait(
        &execution.command,
        scope.locations.base_dir.as_path(),
        Output {
            work_dir,
            kept: true,
            depth: scope.depth,
        },
        deadlines.command(),
    )?;

    // Read at the kill rather than after the adapter, because both limits keep
    // running while the adapter does and the answer would drift.
    let expired = if ran.killed {
        deadlines.expired(Instant::now())
    } else {
        None
    };

    write(
        &work_dir.join(EXITCODE_FILE),
        format!("{}\n", ran.status).as_bytes(),
    )?;

    // FR-9.2 keeps whatever files the command wrote in its work directory. A
    // command is stood at the base, so anything it addressed at {work_dir} is
    // already here and nothing needs collecting.
    //
    // FR-4.12a: a killed command keeps whatever output it gathered and its
    // adapter runs over that. The streams went to their files as the command
    // wrote them, so a partial capture is already there and there is nothing to
    // recover from a pipe the kill closed.
    let adapter_expired = adapt(scope, task, execution, &ran, deadlines)?;

    // The adapter's answer wins where there is one, because only the run's limit
    // can reach an adapter and the run's limit is what stops everything.
    Ok(adapter_expired.or(expired))
}

/// Where an execution's streams go, and where a failure to start is reported.
#[derive(Debug, Clone, Copy)]
struct Output<'a> {
    /// The work directory this execution owns.
    work_dir: &'a Path,
    /// Whether the streams are kept as evidence or discarded.
    ///
    /// A command's are kept, by FR-9.2, and FR-6.2 hands them to the adapter.
    /// An adapter's own are discarded: FR-6.2b puts its result in the envelope,
    /// and writing its chatter into the work directory would overwrite the
    /// capture it was called to read.
    kept: bool,
    /// How deep this run is, exported to whatever it spawns by FR-5.6.
    depth: depth::Depth,
}

/// What running a command came to.
struct Ran {
    /// Its exit status, or -1 where a signal ended it.
    status: i32,
    /// Whether a limit killed it rather than it finishing on its own.
    killed: bool,
}

/// How long a poll waits before looking again, at most.
///
/// A limit set in tens of milliseconds is still observed promptly, because the
/// interval starts far below this and doubles up to it. A gate running for
/// minutes is not woken hundreds of times a second to be told nothing changed.
const POLL_CEILING: Duration = Duration::from_millis(50);

/// Run a command line to completion, or to `deadline` where one is set.
///
/// FR-4.15 runs it as a subprocess, so the streams and the status FR-9.2 keeps
/// come from the process boundary rather than from bookkeeping bolt would
/// otherwise have to trust.
///
/// **The streams go straight to their files rather than through a pipe**, which
/// is what makes FR-4.12a hold: a killed command's partial output is already on
/// disk, with nothing left to drain from a pipe its own death closed. It also
/// removes the deadlock a polled wait would otherwise invite, where a child
/// blocks filling a pipe nobody is reading while bolt waits for it to exit.
fn spawn_and_wait(
    command: &str,
    base: &Path,
    output: Output,
    deadline: Option<Instant>,
) -> Result<Ran, Error> {
    let io = |source: std::io::Error| Error::Io {
        path: output.work_dir.to_path_buf(),
        reason: source.to_string(),
    };
    let (stdout, stderr) = streams(&output)?;

    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        // FR-4.1a stands the command at the base, so a relative path in a jig
        // means the same thing as a relative path in the tree.
        .current_dir(base)
        // As `Command::output` left it. A gate command that inherited a terminal
        // could block on a read nobody is going to answer.
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        // FR-5.6: every process bolt spawns, not every child jig. A task command
        // that runs bolt itself is at depth too, which is what makes the ceiling
        // reachable without a jig task and is the case FR-5.7a describes.
        .envs(output.depth.exported())
        // FR-4.12e wants the descendants killed with the child, and this is what
        // makes that possible: the child leads a group of its own, so a signal
        // to that group reaches what it spawned and nothing else.
        .process_group(0)
        .spawn()
        .map_err(io)?;

    wait_for(&mut child, deadline).map_err(io)
}

/// The two stdio handles an execution's streams go to.
fn streams(output: &Output) -> Result<(Stdio, Stdio), Error> {
    if !output.kept {
        return Ok((Stdio::null(), Stdio::null()));
    }
    let create = |name: &str| -> Result<Stdio, Error> {
        let path = output.work_dir.join(name);
        File::create(&path)
            .map(Stdio::from)
            .map_err(|source| Error::Io {
                path,
                reason: source.to_string(),
            })
    };
    Ok((create(adapter::STDOUT_FILE)?, create(adapter::STDERR_FILE)?))
}

/// Wait for a child, killing it and its group where `deadline` passes first.
///
/// Polled rather than blocked, because a blocking wait cannot be interrupted by
/// a clock and bolt has no other thread to hold one.
fn wait_for(child: &mut Child, deadline: Option<Instant>) -> std::io::Result<Ran> {
    let code = |status: std::process::ExitStatus| status.code().unwrap_or(-1);

    let Some(deadline) = deadline else {
        return Ok(Ran {
            status: code(child.wait()?),
            killed: false,
        });
    };

    let mut interval = Duration::from_millis(1);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Ran {
                status: code(status),
                killed: false,
            });
        }
        let now = Instant::now();
        if now >= deadline {
            kill_group(child);
            return Ok(Ran {
                status: code(child.wait()?),
                killed: true,
            });
        }
        std::thread::sleep(interval.min(deadline - now));
        interval = (interval * 2).min(POLL_CEILING);
    }
}

/// Kill a child and everything it started, by FR-4.12e.
///
/// `SIGKILL` to the process group rather than to the child alone. A command that
/// spawned its own children leaves them running when only the child is
/// signalled, and they go on writing into a work directory bolt has finished
/// with and into the streams an adapter is about to read under FR-4.12a.
///
/// No grace period and no `SIGTERM` first. The limit is the grace period: it was
/// declared in the jig, the command has had all of it, and a second countdown
/// would make the declared number mean something other than what it says.
///
/// The group is the child's own, established by `process_group(0)` at spawn, so
/// the negative pid reaches what bolt started and nothing else. Signalling
/// bolt's own group would include bolt.
///
/// The child is not reaped before this, so its pid is still its own and cannot
/// have been reused by an unrelated process.
fn kill_group(child: &Child) {
    let Ok(leader) = i32::try_from(child.id()) else {
        return;
    };
    // SAFETY: `kill` takes two integers and touches no memory bolt owns. The
    // negative pid names the group this child leads, which the spawn
    // established, so nothing bolt did not start is in it.
    unsafe {
        libc::kill(-leader, libc::SIGKILL);
    }
}

/// Reach a verdict for one execution, by FR-6.1.
///
/// FR-6.14 first: a declared evidence file that was not produced fails the task
/// with a reason naming the path. A task declaring evidence it did not write did
/// not do what it said, and FR-6.2c's refusal to discover means nothing else
/// notices. Checked before the adapter runs, since an adapter handed a path that
/// is not there can only guess.
///
/// FR-6.9 then: a task naming no adapter gets the generic exit-code one, which
/// is the one adapter that needs to know nothing about the tool it reads.
fn adapt<'a>(
    scope: &Scope<'a>,
    task: &Task,
    execution: &Execution,
    ran: &Ran,
    deadlines: Deadlines<'a>,
) -> Result<Option<Expired<'a>>, Error> {
    let work_dir = execution.work_dir.as_path();

    if let Some(missing) = task
        .evidence
        .iter()
        .find(|file| !work_dir.join(file).exists())
    {
        return write_envelope(
            work_dir,
            false,
            "evidence-missing",
            &format!("{} declared {missing} and did not write it", task.name),
        )
        .map(|()| None);
    }

    let Some(name) = task.adapter.as_deref() else {
        // FR-6.9a: the generic exit-code adapter does not run on a command a
        // limit killed. That status is bolt's own signal rather than an answer
        // the tool gave, so `timed_out` writes the envelope and FR-4.12b's
        // reason is the verdict.
        if ran.killed {
            return Ok(None);
        }
        return write_envelope(
            work_dir,
            ran.status == 0,
            "nonzero-exit",
            &format!("{} exited {}", execution.task, ran.status),
        )
        .map(|()| None);
    };

    run_adapter(scope, task, execution, name, deadlines)
}

/// Run a named adapter and take its verdict, or say why bolt could not.
///
/// **The envelope is removed before the adapter runs.** An `output.yaml` left by
/// an earlier fold would otherwise satisfy "the adapter wrote one", and a silent
/// adapter would inherit the previous run's verdict. Carried over from the Go
/// build, which found it.
///
/// FR-6.11's three cases are kept apart because they have different causes, and
/// FR-6.12 leaves canonical form to the adapter: bolt validates on the way in
/// and does not reparse to compare, which FR-6.13 says would fail every jig that
/// documents itself.
fn run_adapter<'a>(
    scope: &Scope<'a>,
    task: &Task,
    execution: &Execution,
    name: &str,
    deadlines: Deadlines<'a>,
) -> Result<Option<Expired<'a>>, Error> {
    let work_dir = execution.work_dir.as_path();
    let envelope = work_dir.join(OUTPUT_FILE);
    let _ = fs::remove_file(&envelope);

    // FR-4.12c: the run's limit is the one that reaches an adapter, and bolt
    // writes that envelope itself because nothing else is left to. An adapter
    // whose budget has already gone is not started at all: spawning it to kill
    // it a moment later would let it write half of something first, and the
    // observable outcome is the same either way.
    let out_of_time = |limit: Limit<'a>| Expired {
        whose: Whose::Run,
        written: limit.written,
    };
    if let Some(run) = scope.run_limit.filter(|run| Instant::now() >= run.at) {
        return Ok(Some(out_of_time(run)));
    }

    let line = substitute(
        &invocation(scope, task, name),
        &task.name,
        scope,
        work_dir,
        &[],
    )?;

    let ran = spawn_and_wait(
        &line,
        &scope.locations.base_dir,
        Output {
            work_dir,
            kept: false,
            depth: scope.depth,
        },
        deadlines.adapter(),
    )?;

    if ran.killed {
        return Ok(scope.run_limit.map(out_of_time));
    }

    // FR-6.1: where the adapter reached an authoritative result, that result is
    // the verdict and bolt does not second-guess it.
    match unauthoritative(&ran, &envelope) {
        None => Ok(None),
        Some(why) => write_envelope(work_dir, false, why.kind(), &why.message(name)).map(|()| None),
    }
}

/// The adapter invocation, as written before substitution.
///
/// FR-6.2d: an explicit one gets the same substitutions a command gets, so it
/// names the locations and the captures the same way. Two spellings would make
/// the jig format teach itself twice.
fn invocation(scope: &Scope, task: &Task, name: &str) -> String {
    task.adapter_command.clone().unwrap_or_else(|| {
        adapter::default_invocation(
            &adapter::path(&scope.locations.config_dir, name)
                .display()
                .to_string(),
            &task.evidence,
        )
    })
}

/// Why an adapter's result cannot be taken, where it cannot, by FR-6.11.
///
/// The three are kept apart because they have different causes: a crashing
/// adapter, a silent one, and one whose output is not an envelope are three
/// different things to go and fix.
fn unauthoritative(ran: &Ran, envelope: &Path) -> Option<adapter::Unauthoritative> {
    if ran.status != 0 {
        Some(adapter::Unauthoritative::Exited(ran.status))
    } else if !envelope.is_file() {
        Some(adapter::Unauthoritative::WroteNothing)
    } else if merge::read(envelope, &wrench::schemas::ENVELOPE).is_err() {
        Some(adapter::Unauthoritative::WroteInvalid)
    } else {
        None
    }
}

/// Substitute a command's template variables, in ONE left-to-right pass.
///
/// **Chained `str::replace` is a command injection, and it was one here.**
/// Measured 2026-08-28 against the built binary by a cold-read reviewer: a file
/// named `p{all_paths};id #`, selected by a `{each_path}` task, was quoted
/// correctly by [`quote`] and then had the literal `{all_paths}` *inside its own
/// name* expanded by the next `replace`. That spliced a fresh `'…'` string into
/// the middle of the already-quoted region, broke the quoting, and put the rest
/// of the filename on the command line unquoted. `id` executed. A second fixture
/// escaped the base and created a file beside it while the run reported success.
///
/// So FR-4.3's guarantee is not a property of the quoting alone. It is a
/// property of the quoting AND of never reading substituted bytes again, which
/// is what a single pass gives and what chaining cannot.
///
/// # Errors
///
/// [`Error::UnknownPlaceholder`] for a `{name}` no layer supplies, by FR-4.18.
/// Chained replace left an unknown placeholder in the string and handed it to
/// the shell; the row wants a refusal naming it before anything executes.
fn substitute(
    command: &str,
    task: &str,
    scope: &Scope,
    work_dir: &Path,
    paths: &[PathBuf],
) -> Result<String, Error> {
    let Scope {
        locations,
        definitions,
        ..
    } = scope;
    let joined = paths
        .iter()
        .map(|path| quote(path))
        .collect::<Vec<_>>()
        .join(" ");
    // Bolt's layer first and unconditionally, by FR-4.16d: the locations and
    // path variables are reserved rather than overridable, so nothing above
    // them can win. FR-4.19 already refused any layer that named one, so this
    // ordering and that refusal say the same thing twice on purpose.
    //
    // A defined value is quoted like a location, which is what makes it one
    // argument. FR-4.16c settles it as a scalar, so a value carrying a space
    // arrives as one word rather than splitting into two.
    let value = |name: &str| match name {
        "each_path" | "all_paths" => Some(joined.clone()),
        "work_dir" => Some(quote(work_dir)),
        "project_root" => Some(quote(&locations.project_root)),
        "base_dir" => Some(quote(&locations.base_dir)),
        "config_dir" => Some(quote(&locations.config_dir)),
        "output_dir" => Some(quote(&locations.output_dir)),
        _ => definitions.get(name).map(quote_str),
    };

    let mut out = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            // An unmatched brace is literal text rather than a placeholder.
            out.push_str(&rest[open..]);
            return Ok(out);
        };
        let name = &after[..close];
        let Some(substituted) = value(name) else {
            return Err(Error::UnknownPlaceholder {
                task: task.to_owned(),
                placeholder: name.to_owned(),
            });
        };
        out.push_str(&substituted);
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The directory name for one execution of a task.
///
/// FR-9.2a makes the ordinal the execution index within the task, numbered from
/// one and independently of every other task, so a name says which task and
/// which of its executions without needing the run's order.
///
/// FR-9.2b zero-pads it to the width that task's execution count needs, so a
/// listing sorts correctly with no arbitrary cap and no wasted digits. The
/// count is known before the first execution, because the matched list is
/// settled before any of it runs.
#[must_use]
pub fn work_dir_name(task: &str, ordinal: usize, executions: usize) -> String {
    let width = executions.to_string().len();
    format!("{task}-{ordinal:0width$}")
}

/// Write an execution's manifest.
///
/// FR-9.5 records which paths `matching` selected and which `excluding`
/// removed, for a task that consumes paths, so what the task saw and what it
/// was kept from seeing sit on disk beside what it did.
///
/// FR-9.5a writes it **before** the command runs, so an execution that was
/// killed, or that never got started, still records what was going to be
/// attempted. FR-9.6 has a task naming no path variable claim none, because
/// recording one would say the command saw files it never received.
fn write_manifest(
    scope: &Scope,
    execution: &Execution,
    selection: Option<&Selection>,
) -> Result<(), Error> {
    let Scope {
        locations,
        definitions,
        ..
    } = scope;
    let Execution {
        task,
        ordinal,
        command,
        work_dir,
    } = execution;
    let names = |paths: &[PathBuf]| -> Vec<String> {
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    };

    let mut manifest = json!({
        "task": task,
        "ordinal": ordinal,
        "command": command,
        "variables": variables(locations, definitions, work_dir),
    });

    // The key names are wrench's, not bolt's. FR-9.5 says a manifest records
    // what was selected and what was removed; `selection.matched` and
    // `selection.excluded` are what the shipped schema calls them, and writing
    // through wrench refused the pair this first invented.
    if let Some(selection) = selection {
        manifest["selection"] = json!({
            "matched": names(&selection.selected),
            "excluded": names(&selection.removed),
        });
    }

    save(
        &work_dir.join(MANIFEST_FILE),
        &manifest,
        &wrench::schemas::MANIFEST,
    )
}

/// Every template variable this execution was given, and where each came from.
///
/// FR-9.5c puts the locations here. FR-9.5g adds the other two layers, because
/// the same key means different things depending on which file won and the
/// command line alone does not say.
///
/// Every location is `from: "bolt"`, since all five are reserved to bolt's own
/// layer. FR-4.19 refused any jig or file that named one, so the definitions
/// below cannot overwrite a location and the insertion order is not load
/// bearing.
fn variables(
    locations: &Locations,
    definitions: &Definitions,
    work_dir: &Path,
) -> serde_json::Value {
    let supplied = |path: &Path| json!({ "value": path.display().to_string(), "from": "bolt" });
    let mut variables = json!({
        "project_root": supplied(&locations.project_root),
        "base_dir": supplied(&locations.base_dir),
        "work_dir": supplied(work_dir),
        "config_dir": supplied(&locations.config_dir),
        "output_dir": supplied(&locations.output_dir),
    });

    if let Some(map) = variables.as_object_mut() {
        for (name, definition) in definitions.entries() {
            map.insert(
                name.clone(),
                json!({ "value": definition.value, "from": definition.from }),
            );
        }
    }
    variables
}

/// Write an execution's envelope.
fn write_envelope(work_dir: &Path, success: bool, kind: &str, message: &str) -> Result<(), Error> {
    let mut envelope = json!({ "success": success });
    if !success {
        // FR-7.9's kind, so a consumer tells one sort of failure from another
        // without reading English, and FR-7.8's message, which every reason
        // carries so one consumer can render every reason it meets.
        envelope["reasons"] = json!([{ "kind": kind, "message": message }]);
    }
    save(
        &work_dir.join(OUTPUT_FILE),
        &envelope,
        &wrench::schemas::ENVELOPE,
    )
}

/// Write a structured file through wrench, by FR-1.12.
pub(crate) fn save(
    path: &Path,
    value: &serde_json::Value,
    schema: &dyn wrench::Schema,
) -> Result<(), Error> {
    let io = |reason: String| Error::Io {
        path: path.to_path_buf(),
        reason,
    };
    wrench::save_formatted_file(
        value,
        path.to_str().ok_or_else(|| io("not utf-8".to_owned()))?,
        schema,
        &wrench::YamlCodec,
        &wrench::LocalFileIo,
    )
    .map_err(|source| io(source.to_string()))
}

/// Create a directory and its parents, reporting where it failed.
fn create_dir(path: &Path) -> Result<(), Error> {
    fs::create_dir_all(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })
}

/// Write bytes, reporting where it failed.
fn write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    fs::write(path, bytes).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })
}
