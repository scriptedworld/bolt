//! Folding every execution's envelope into the run's one result.

use std::path::Path;

use crate::{Error, Outcome};

/// Fold every `work/*/output.yaml` under `output_dir` into one `result.yaml`.
///
/// FR-8.1 gives a run exactly one result, and has the merge read every
/// envelope, fold them mechanically, and do so repeatably over a finished
/// directory. Repeatably is testable: folding twice over one directory gives
/// the same answer.
///
/// FR-8.3 passes the merged result only when every constituent passes.
///
/// # Errors
///
/// [`Error::NoConstituents`] when the fold finds none, by FR-8.3a. FR-8.3 on
/// its own would pass such a run, because every constituent passing holds
/// vacuously when there are none, and a green result over zero checks is read
/// as checked and fine.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
pub fn merge(output_dir: &Path) -> Result<Outcome, Error> {
    todo!("fold the envelopes under {}", output_dir.display())
}
