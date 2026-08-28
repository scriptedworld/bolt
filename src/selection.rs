//! Narrowing a walk to what one task acts on.
//!
//! FR-3.4 and FR-3.4a are two halves of one operation: `matching` selects and
//! `excluding` removes from what was selected. Keeping them in that order is
//! the whole of the rule, because `excluding` is not a second way to select.

use std::path::{Path, PathBuf};

use crate::Error;

/// Apply one task's `matching` and `excluding` to a walk.
///
/// `paths` are what [`walk`](crate::walk::walk) returned, which are absolute.
/// FR-3.5 makes patterns relative to `base`, so the patterns are matched
/// against each path's position under it and a jig written for reuse says
/// `**/*.rs` without naming the subtree it was dropped into.
///
/// `matching` is a list of patterns or literal paths, where `**` matches zero
/// or more directory levels, by FR-3.4. `excluding` takes the same list and
/// removes from what `matching` selected, by FR-3.4a.
///
/// Paths keep the order they arrived in, so a sorted walk gives a sorted
/// selection and FR-2.2d carries through to FR-9.2a's ordinals.
///
/// # Errors
///
/// [`Error::JigUnreadable`] when a pattern will not compile, which is a
/// property of the jig rather than of the tree.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
pub fn select(
    base: &Path,
    paths: &[PathBuf],
    matching: &[String],
    excluding: &[String],
) -> Result<Vec<PathBuf>, Error> {
    todo!(
        "select from {} paths under {} with {} matching and {} excluding patterns",
        paths.len(),
        base.display(),
        matching.len(),
        excluding.len(),
    )
}

/// Whether a task consumes paths at all.
///
/// FR-4.4 turns on this: a command naming `{each_path}` or `{all_paths}` does
/// not execute when its filtered selection is empty, while a command naming
/// neither always executes. FR-4.4b then makes that empty selection a failure
/// unless the task allows it, so "no paths" only means anything for a task that
/// wanted paths.
#[must_use]
pub fn consumes_paths(command: &str) -> bool {
    command.contains("{each_path}") || command.contains("{all_paths}")
}

/// Quote a path for substitution into a command line.
///
/// FR-4.3 quotes every path bolt substitutes individually, so a path carrying a
/// space, a quote or a semicolon can neither split the command line nor inject
/// into it. Both words are load-bearing. *Individually*, because quoting the
/// joined list leaves the separators outside the quotes; and *a quote*, because
/// wrapping in single quotes is the obvious implementation and a path
/// containing one escapes it.
///
/// The test for this runs the result through a shell and compares what arrives
/// with what went in, because a shape assertion cannot tell quoting from
/// wrapping.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
#[must_use]
pub fn quote(path: &Path) -> String {
    todo!("quote {}", path.display())
}
