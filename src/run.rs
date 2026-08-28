//! Executing a jig's tasks and keeping what they produced.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use serde_json::json;

use crate::jig::{self, Task};
use crate::selection::{self, consumes_paths, quote};
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
    // FR-2.5 first, and before FR-2.6c's output directory, which sits at the
    // base and would create it.
    if !base.is_dir() {
        return Err(Error::BaseMissing(base.to_path_buf()));
    }

    let jig = jig::read(base, jig)?;

    // Everything a jig can be refused for, checked before any task executes.
    // FR-3.10b makes that the shape: an incomplete jig is known before half a
    // gate has run rather than partway through it.
    let commands = validate(&jig)?;

    let walked = walk::walk(base)?;
    let output_dir = base.join(format!(".bolt-{}", stamp::iso8601(SystemTime::now())));

    // FR-2.6b. The stamp is second-granular, so two runs started in one second
    // would share a directory and each fold the other's evidence. Reproduced
    // 2026-08-28: a second jig's result reported a failing task belonging to the
    // first, and both callers were handed the same conflated file.
    if output_dir.exists() {
        return Err(Error::OutputDirectoryInUse(output_dir));
    }
    create_dir(&output_dir.join(WORK_DIR))?;

    let locations = Locations {
        project_root: base.to_path_buf(),
        base_dir: base.to_path_buf(),
        config_dir: base.to_path_buf(),
        output_dir: output_dir.clone(),
    };

    let mut executions = 0;
    for (task, command) in jig.tasks.iter().zip(commands) {
        executions += run_task(&locations, task, command, &walked)?;
    }

    merge::merge(&output_dir).map(|folded| Outcome {
        executions,
        ..folded
    })
}

/// Everything a jig is refused for, checked before any task executes.
///
/// FR-3.10b makes that the shape: an incomplete jig is known before half a gate
/// has run rather than partway through it. Returns each task's command, so the
/// run loop does not re-derive what this already proved present.
fn validate(jig: &jig::Jig) -> Result<Vec<&str>, Error> {
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
        commands.push(command);
    }

    Ok(commands)
}

/// Run one task, returning how many times its command executed.
fn run_task(
    locations: &Locations,
    task: &Task,
    command: &str,
    walked: &[PathBuf],
) -> Result<usize, Error> {
    let base = locations.base_dir.as_path();
    let output_dir = locations.output_dir.as_path();
    let wants_paths = consumes_paths(command);
    let selection = narrow(base, walked, task)?;

    if wants_paths && selection.selected.is_empty() {
        // FR-4.4c: an allowed empty selection produces no constituent at all,
        // which is what FR-4.4 alone used to mean for every task.
        if task.allow_empty {
            return Ok(0);
        }
        empty_selection(locations, task, command, &selection)?;
        return Ok(0);
    }

    let batches = if wants_paths && command.contains("{each_path}") {
        selection
            .selected
            .iter()
            .map(|path| vec![path.clone()])
            .collect()
    } else {
        vec![selection.selected.clone()]
    };

    // FR-9.6: a task naming no path variable was handed no list, so its
    // manifest claims none. Recording one would say the command saw files it
    // never received, so the key is absent rather than empty.
    let recorded = wants_paths.then_some(&selection);

    for (index, batch) in batches.iter().enumerate() {
        let ordinal = index + 1;
        let work_dir =
            output_dir
                .join(WORK_DIR)
                .join(work_dir_name(&task.name, ordinal, batches.len()));
        create_dir(&work_dir)?;

        // Substituted before the manifest is written, because FR-9.5's
        // manifest records the command AS EXECUTED and FR-9.5a writes it before
        // the command runs. Both hold only if substitution comes first.
        let execution = Execution {
            task: &task.name,
            ordinal,
            command: substitute(command, &task.name, locations, &work_dir, batch)?,
            work_dir,
        };
        write_manifest(locations, &execution, recorded)?;
        execute(base, &execution)?;
    }

    Ok(batches.len())
}

/// FR-4.4b's failing constituent for a task that matched nothing.
///
/// It has to be a constituent rather than a skip, or FR-8.3 folds a run that
/// checked nothing into a pass, which is FR-8.3a's argument one level down.
fn empty_selection(
    locations: &Locations,
    task: &Task,
    command: &str,
    selection: &Selection,
) -> Result<(), Error> {
    let execution = Execution {
        task: &task.name,
        ordinal: 1,
        command: command.to_owned(),
        work_dir: locations
            .output_dir
            .join(WORK_DIR)
            .join(work_dir_name(&task.name, 1, 1)),
    };
    create_dir(&execution.work_dir)?;
    write_manifest(locations, &execution, Some(selection))?;
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
    locations: &Locations,
    work_dir: &Path,
    paths: &[PathBuf],
) -> Result<String, Error> {
    let joined = paths
        .iter()
        .map(|path| quote(path))
        .collect::<Vec<_>>()
        .join(" ");
    let value = |name: &str| match name {
        "each_path" | "all_paths" => Some(joined.clone()),
        "work_dir" => Some(quote(work_dir)),
        "project_root" => Some(quote(&locations.project_root)),
        "base_dir" => Some(quote(&locations.base_dir)),
        "config_dir" => Some(quote(&locations.config_dir)),
        "output_dir" => Some(quote(&locations.output_dir)),
        _ => None,
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
    locations: &Locations,
    execution: &Execution,
    selection: Option<&Selection>,
) -> Result<(), Error> {
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

    // Every location is `from: "bolt"`, because all five are reserved to bolt's
    // own layer. FR-4.16's jig and file layers merge over them and are
    // `definitions/10`'s to add.
    let supplied = |path: &Path| json!({ "value": path.display().to_string(), "from": "bolt" });
    let mut manifest = json!({
        "task": task,
        "ordinal": ordinal,
        "command": command,
        "variables": {
            "project_root": supplied(&locations.project_root),
            "base_dir": supplied(&locations.base_dir),
            "work_dir": supplied(work_dir),
            "config_dir": supplied(&locations.config_dir),
            "output_dir": supplied(&locations.output_dir),
        },
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
