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

    /// Tasks that did not execute because their selection was empty.
    ///
    /// FR-4.4. Reported so a reader sees what was skipped rather than
    /// inferring it from what is absent.
    pub skipped: Vec<String>,
}
