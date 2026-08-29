//! What a finished run says about itself.

use std::path::PathBuf;

use crate::Error;

/// A run bolt could not carry out, and where it recorded that.
///
/// FR-10.7 has bolt write a `result.yaml` whenever it is alive and in control
/// when it stops, and FR-10.3a has it print where that is. Carrying the path
/// beside the reason is what makes the second possible: the directory a default
/// run resolves to comes from FR-2.6c's stamp, taken inside
/// [`invoke`](crate::run::invoke), so no caller can reconstruct it afterwards.
///
/// `result` is `None` for a refusal that deliberately wrote nothing. FR-10.7a's
/// missing base and FR-2.6b's occupied directory are both that case, and a
/// caller is told so rather than left to read an absent file as a bolt that
/// died.
#[derive(Debug)]
pub struct Refusal {
    /// Why the run was refused.
    pub error: Error,

    /// The `result.yaml` carrying that reason, where one was written.
    pub result: Option<PathBuf>,
}

impl From<Refusal> for Error {
    fn from(refusal: Refusal) -> Self {
        refusal.error
    }
}

/// The result of a run that bolt was able to carry out.
///
/// A run that bolt could *not* carry out is an [`Error`](crate::Error) instead.
/// FR-10.2 pairs the two deliberately: a run in which every task executed and
/// some tools reported failures exits 0 and writes `success: false`, because
/// FR-10.1 has the exit status answer whether bolt could execute the ETL and
/// FR-10.3 keeps the quality verdict in the envelope.
#[derive(Debug)]
pub struct Outcome {
    /// Whether every constituent envelope passed.
    ///
    /// FR-8.3: there is no constituent whose failure does not count. A check
    /// nobody wants enforced is a check not in the jig.
    pub success: bool,

    /// The run directory, holding one work directory per execution.
    pub output_dir: PathBuf,

    /// How many executions ran, across every task.
    pub executions: usize,

    /// Tasks a short-circuit kept from running, in declaration order.
    ///
    /// FR-4.9. A reader sees what was not attempted rather than inferring it
    /// from what is absent, which is not the same thing: a task missing from
    /// the evidence could equally have skipped an empty selection under
    /// FR-4.4c.
    ///
    /// Empty for every run that was not stopped, which FR-4.8 makes the
    /// ordinary case.
    pub stopped: Vec<String>,
}
