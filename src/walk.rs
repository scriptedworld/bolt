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
/// require a repository. The `ignore` crate reads git's own excludes at its
/// defaults, so each is turned off against the row rather than taken as it
/// comes, and `parents` with them, because a `.gitignore` above the base is a
/// file outside it changing the run's input, which FR-2.3 forbids.
///
/// Paths come back sorted, by FR-2.2d, which is what makes the matched list the
/// same list on every run over the same tree. FR-9.4's identical work directory
/// names rest on this and on nothing else.
///
/// # Errors
///
/// [`Error::BaseMissing`] when `base` is not there, by FR-2.5.
pub fn walk(base: &Path) -> Result<Vec<PathBuf>, Error> {
    if !base.is_dir() {
        return Err(Error::BaseMissing(base.to_path_buf()));
    }

    let mut found: Vec<PathBuf> = ignore::WalkBuilder::new(base)
        // FR-2.2b, each one turning off a default that reaches outside the row.
        .require_git(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        // FR-2.2e. Following one leaves the base and breaks FR-2.3's
        // containment. This is the crate's default and is set anyway, because
        // the row is the reason rather than the default being convenient.
        .follow_links(false)
        .build()
        .filter_map(Result::ok)
        .filter(|entry| {
            // Files, not directories: FR-2.2 is about the files tasks act on.
            //
            // A symlink is excluded here too, and that is PROVISIONAL. Measured
            // 2026-08-27: with `follow_links` off, `ignore` yields the link
            // itself, so a task handed one reads through it to outside the base.
            // FR-2.2e forbids following and does not say whether returning is
            // also forbidden, which is question 38's neighbour, question 40.
            // Excluding is the containment-preserving direction and is reversed
            // by deleting this predicate once the row exists.
            entry.file_type().is_some_and(|kind| kind.is_file())
        })
        .map(ignore::DirEntry::into_path)
        .collect();

    found.sort();
    Ok(found)
}
