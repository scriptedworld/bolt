//! Executing a jig's tasks and keeping what they produced.

use std::path::{Path, PathBuf};

use crate::{Error, Outcome};

/// The name of the file holding an execution's captured exit status.
pub const EXITCODE_FILE: &str = "exitcode";

/// The name of the file holding an execution's manifest.
pub const MANIFEST_FILE: &str = "manifest.yaml";

/// The name of the file an adapter writes its envelope to.
pub const OUTPUT_FILE: &str = "output.yaml";

/// The name of the file a run's merged result is written to.
pub const RESULT_FILE: &str = "result.yaml";

/// Run `jig` over `base`.
///
/// This is the whole of an invocation, by FR-2.1 and FR-2.1a: one jig, one
/// directory. Running several jigs over one tree is a jig whose tasks are
/// nested jigs, which FR-5.x specifies and which this task does not build.
///
/// The run writes a `.bolt-<iso8601>` directory at `base`, holding a work
/// directory per execution by FR-9.2 and one `result.yaml` by FR-8.1.
///
/// # Errors
///
/// [`Error::BaseMissing`] when `base` is not there, by FR-2.5. **Nothing is
/// created before that check.** The Go build made the base as a side effect of
/// preparing the output directory, so a run over a typo'd path checked an empty
/// tree and passed.
///
/// [`Error::JigUnreadable`] when the jig will not parse, and
/// [`Error::CommandNamesBothPathForms`] when a command names both path forms,
/// which FR-4.2 makes a jig error.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
pub fn run(jig: &Path, base: &Path) -> Result<Outcome, Error> {
    todo!("run {} over {}", jig.display(), base.display())
}

/// The directory name for one execution of a task.
///
/// FR-9.2a makes the ordinal the execution index within the task, numbered from
/// one and independently of every other task, so a name says which task and
/// which of its executions without needing the run's order. For a per-path task
/// the index is the position in the matched list, which FR-9.5's manifest
/// records, so an execution traces back to the path it was handed.
///
/// FR-9.2b zero-pads it to the width that task's execution count needs, so a
/// listing sorts correctly with no arbitrary cap and no wasted digits. The
/// count is known before the first execution, because the matched list is
/// settled before any of it runs.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
#[must_use]
pub fn work_dir_name(task: &str, ordinal: usize, executions: usize) -> String {
    todo!("name execution {ordinal} of {executions} for {task}")
}

/// Write an execution's manifest.
///
/// FR-9.5 records which paths `matching` selected and which `excluding`
/// removed, for a task that consumes paths, so what the task saw and what it
/// was kept from seeing sit on disk beside what it did.
///
/// FR-9.5a writes it **before** the command runs, so an execution that was
/// killed, or that never got started, still records what was going to be
/// attempted. The case that most needs a record is the one that would otherwise
/// have none.
///
/// # Errors
///
/// [`Error::JigUnreadable`] carries a write failure for now; the refusal
/// taxonomy for an unwritable output directory is `runner/10`'s.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
pub fn write_manifest(
    work_dir: &Path,
    selected: &[PathBuf],
    removed: &[PathBuf],
) -> Result<(), Error> {
    todo!(
        "write a manifest to {} for {} selected and {} removed",
        work_dir.display(),
        selected.len(),
        removed.len(),
    )
}
