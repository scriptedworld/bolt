//! Executing a jig's tasks and keeping what they produced.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use serde_json::json;

use crate::definitions::{Definitions, RESERVED};
use crate::jig::{self, Task};
use crate::selection::{self, consumes_paths, quote, quote_str};
use crate::{Error, Outcome, merge, stamp, walk};

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
    })
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
}

/// Carry out an invocation.
///
/// # Errors
///
/// Everything [`run`] returns, plus [`Error::ReservedDefinition`] by FR-4.19,
/// [`Error::DefinitionsUnreadable`] by FR-4.20, and [`Error::OutputDirectoryInUse`]
/// by FR-2.6b for a named directory that already holds a run.
pub fn invoke(invocation: &Invocation) -> Result<Outcome, Error> {
    let Invocation {
        jig,
        base,
        definitions,
        output_dir,
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
    let base = &fs::canonicalize(base).map_err(|source| Error::Io {
        path: base.to_path_buf(),
        reason: source.to_string(),
    })?;

    // FR-2.4 reaches the output directory too, and it has to happen after the
    // base is canonical: the default is derived from the base, and a named one
    // is resolved against the working directory like any path on a command
    // line. `.bolt-<iso8601>` at a relative base would otherwise be recorded as
    // `./.bolt-…`, which is the defect FR-2.4 exists to prevent one level up.
    let output_dir = named.map_or_else(|| output_dir_for(base, started), absolute);

    // FR-2.6b, for a named directory as much as for the default. **This returns
    // before anything is written, and that ordering is the guarantee rather
    // than an implementation detail**; `holds_a_run` carries why.
    if holds_a_run(&output_dir) {
        return Err(Error::OutputDirectoryInUse(output_dir));
    }

    // Everything past here is bolt alive and in control, so FR-10.7 wants a
    // result for whatever goes wrong. The refusal is written into the directory
    // this run owns, which the two checks above have already established is not
    // somebody else's.
    carry_out(jig, base, &output_dir, *definitions)
        .inspect_err(|refusal| write_refusal(&output_dir, refusal))
}

/// FR-2.5's refusal, with FR-10.7a's exemption applied to it.
///
/// The exemption is about the **directory**, not the error. The default output
/// directory sits inside the base, so writing there would create the thing whose
/// absence is being refused. One named outside it has no such problem, which is
/// exactly what FR-10.7b tells a caller who wants a parseable refusal in every
/// case to do.
fn base_missing(base: &Path, named: Option<&Path>) -> Error {
    let refusal = Error::BaseMissing(base.to_path_buf());
    if wrote_a_result(&refusal, base, named) {
        // Only reachable with a named directory outside the base, so there is
        // one to write to and no default to derive from a base that is not
        // there.
        if let Some(path) = named {
            write_refusal(&absolute(path), &refusal);
        }
    }
    refusal
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
/// Exposed because the directory name is part of what a caller observes, and a
/// test that cannot predict it cannot set up the collision FR-2.6b refuses.
/// FR-2.6's filesystem-safe stamp rather than the strict ISO 8601 form, since
/// this is a path component.
#[must_use]
pub fn output_dir_for(base: &Path, at: SystemTime) -> PathBuf {
    base.join(format!(".bolt-{}", stamp::iso8601(at)))
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
        "reasons": [{ "kind": "bolt-refused", "message": refusal.to_string() }],
    });
    let _ = create_dir(output_dir);
    let _ = save(
        &output_dir.join(RESULT_FILE),
        &result,
        &wrench::ENVELOPE_SCHEMA,
    );
}

/// Carry the run out, once the output directory is known to be bolt's own.
fn carry_out(
    jig: &str,
    base: &Path,
    output_dir: &Path,
    definitions: Option<&str>,
) -> Result<Outcome, Error> {
    let output_dir = output_dir.to_path_buf();
    let jig = jig::read(base, jig)?;

    // FR-2.8 puts the config directory at the base for this task; naming it
    // separately is `runner/10`'s.
    let definitions = Definitions::build(jig.definitions.as_ref(), base, definitions)?;

    // Everything a jig can be refused for, checked before any task executes.
    // FR-3.10b makes that the shape: an incomplete jig is known before half a
    // gate has run rather than partway through it. FR-4.18a puts the unknown
    // placeholder check here for the same reason, so a jig run where nothing
    // defines what it needs refuses in the first second rather than partway
    // through a gate.
    let commands = validate(&jig, &definitions)?;
    check_requires(&jig)?;

    // Created before the walk. Creating it afterwards made FR-2.2c's exclusion
    // below true by accident, for a directory bolt had not made yet.
    create_dir(&output_dir.join(WORK_DIR))?;

    let walked = walk_excluding(base, &output_dir)?;

    let locations = Locations {
        project_root: base.to_path_buf(),
        base_dir: base.to_path_buf(),
        config_dir: base.to_path_buf(),
        output_dir: output_dir.clone(),
    };
    let scope = Scope {
        locations: &locations,
        definitions: &definitions,
    };

    let (executions, stopped) = run_tasks(&scope, &jig, commands, &walked)?;

    merge::merge(&output_dir, base).map(|folded| Outcome {
        executions,
        stopped,
        ..folded
    })
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
fn run_tasks(
    scope: &Scope,
    jig: &jig::Jig,
    commands: Vec<&str>,
    walked: &[PathBuf],
) -> Result<(usize, Vec<String>), Error> {
    let mut executions = 0;

    for (index, (task, command)) in jig.tasks.iter().zip(commands).enumerate() {
        executions += run_task(scope, task, command, walked)?;

        if task.short_circuit_failure && !task_passed(&scope.locations.output_dir, &task.name) {
            let stopped = jig.tasks[index + 1..]
                .iter()
                .map(|later| later.name.clone())
                .collect();
            return Ok((executions, stopped));
        }
    }

    Ok((executions, Vec::new()))
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
            merge::read(&path.join(OUTPUT_FILE), &wrench::ENVELOPE_SCHEMA)
                .ok()
                .and_then(|envelope| envelope.get("success").and_then(serde_json::Value::as_bool))
                .unwrap_or(true)
        })
}

/// Everything a jig is refused for, checked before any task executes.
///
/// FR-3.10b makes that the shape: an incomplete jig is known before half a gate
/// has run rather than partway through it. Returns each task's command, so the
/// run loop does not re-derive what this already proved present.
fn validate<'a>(jig: &'a jig::Jig, definitions: &Definitions) -> Result<Vec<&'a str>, Error> {
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

        let Some(command) = task.command.as_deref() else {
            return Err(Error::NestedJigNotBuilt { task: named() });
        };
        // FR-4.2's jig error.
        if command.contains("{each_path}") && command.contains("{all_paths}") {
            return Err(Error::CommandNamesBothPathForms { task: named() });
        }

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

        commands.push(command);
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

/// Run one task, returning how many times its command executed.
fn run_task(scope: &Scope, task: &Task, command: &str, walked: &[PathBuf]) -> Result<usize, Error> {
    let base = scope.locations.base_dir.as_path();
    let wants_paths = consumes_paths(command);
    let selection = narrow(base, walked, task)?;

    if wants_paths && selection.selected.is_empty() {
        // FR-4.4c: an allowed empty selection produces no constituent at all,
        // which is what FR-4.4 alone used to mean for every task.
        if task.allow_empty {
            return Ok(0);
        }
        empty_selection(scope, task, command, &selection)?;
        return Ok(0);
    }

    let batches = batches_for(command, wants_paths, &selection);

    // FR-9.6: a task naming no path variable was handed no list, so its
    // manifest claims none. Recording one would say the command saw files it
    // never received, so the key is absent rather than empty.
    let recorded = wants_paths.then_some(&selection);

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
            command: substitute(command, &task.name, scope, &work_dir, batch)?,
            work_dir,
        };
        write_manifest(scope, &execution, recorded)?;
        execute(base, &execution)?;
    }

    Ok(batches.len())
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
/// FR-4.15 runs it as a subprocess, so the streams and the status FR-9.2 keeps
/// come from the process boundary rather than from bookkeeping bolt would
/// otherwise have to trust.
fn execute(base: &Path, execution: &Execution) -> Result<(), Error> {
    let work_dir = execution.work_dir.as_path();
    let output = Command::new("sh")
        .arg("-c")
        .arg(&execution.command)
        // FR-4.1a stands the command at the base, so a relative path in a jig
        // means the same thing as a relative path in the tree.
        .current_dir(base)
        .output()
        .map_err(|source| Error::Io {
            path: work_dir.to_path_buf(),
            reason: source.to_string(),
        })?;

    write(&work_dir.join("stdout"), &output.stdout)?;
    write(&work_dir.join("stderr"), &output.stderr)?;
    let status = output.status.code().unwrap_or(-1);
    write(
        &work_dir.join(EXITCODE_FILE),
        format!("{status}\n").as_bytes(),
    )?;

    // FR-9.2 keeps whatever files the command wrote in its work directory. A
    // command is stood at the base, so anything it addressed at {work_dir} is
    // already here and nothing needs collecting.

    // FR-6.9: a task naming no adapter gets the generic exit-code one, which is
    // the only adapter that needs to know nothing about the tool it reads.
    write_envelope(
        work_dir,
        status == 0,
        &format!("{} exited {status}", execution.task),
    )
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
        &wrench::MANIFEST_SCHEMA,
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
fn write_envelope(work_dir: &Path, success: bool, message: &str) -> Result<(), Error> {
    let mut envelope = json!({ "success": success });
    if !success {
        envelope["reasons"] = json!([{ "kind": "nonzero-exit", "message": message }]);
    }
    save(
        &work_dir.join(OUTPUT_FILE),
        &envelope,
        &wrench::ENVELOPE_SCHEMA,
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
