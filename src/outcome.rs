//! What a finished run says about itself.

use std::path::PathBuf;

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
