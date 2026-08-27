//! Finding the files a run's tasks act on.
//!
//! FR-2.2 has bolt walk the directory it was given. There is no
//! changed-since-a-ref: the walk is the whole input.

use std::path::{Path, PathBuf};

use crate::Error;

/// Walk `base`, returning the paths a task may act on.
///
/// The walk honours `.gitignore` by FR-2.2a, which keeps `.git`, `node_modules`,
/// a virtualenv and build output out of every run without a second list to
/// maintain.
///
/// FR-2.2b bounds how that is done. Honouring `.gitignore` means reading those
/// files as text: bolt does not invoke git, read anything under `.git/`, or
/// require a repository, so `.git/info/exclude` and a global excludes file are
/// not consulted. The `ignore` crate reads both at its defaults, so it is
/// configured against the row rather than taken as it comes.
///
/// Paths come back sorted, by FR-2.2d, which is what makes the matched list the
/// same list on every run over the same tree. FR-9.4's identical work directory
/// names rest on this and on nothing else.
///
/// Symlinks are not followed, by FR-2.2e. Following one leaves the base and
/// breaks FR-2.3's containment, and `link-jigs` leaves tracked symlinks pointing
/// into toolbox, so a project using shared jigs has them in the tree walked.
///
/// # Errors
///
/// [`Error::BaseMissing`] when `base` is not there, by FR-2.5.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
pub fn walk(base: &Path) -> Result<Vec<PathBuf>, Error> {
    todo!("walk {}", base.display())
}
