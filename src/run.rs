//! Executing a jig's tasks and keeping what they produced.

use std::path::Path;

use crate::{Error, Outcome};

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

/// Run the jig named `jig` over `base`.
///
/// This is the whole of an invocation, by FR-2.1 and FR-2.1a: one jig, one
/// directory. FR-3.9 has the jig named rather than pathed, and FR-2.8 puts
/// `bolt.<jig>.yaml` in the config directory, which for this task is `base`.
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
/// [`Error::JigUnreadable`] when the jig is absent or will not parse, and
/// [`Error::CommandNamesBothPathForms`] when a command names both path forms,
/// which FR-4.2 makes a jig error.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
pub fn run(jig: &str, base: &Path) -> Result<Outcome, Error> {
    todo!("run {jig} over {}", base.display())
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
