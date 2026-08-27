//! Narrowing a walk to what one task acts on.
//!
//! FR-3.4 and FR-3.4a are two halves of one operation: `matching` selects and
//! `excluding` removes from what was selected. Keeping them in that order is
//! the whole of the rule, because `excluding` is not a second way to select.

use std::path::{Path, PathBuf};

use crate::Error;

/// Apply one task's `matching` and `excluding` to a walk.
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
    paths: &[PathBuf],
    matching: &[String],
    excluding: &[String],
) -> Result<Vec<PathBuf>, Error> {
    todo!(
        "select from {} paths with {} matching and {} excluding patterns",
        paths.len(),
        matching.len(),
        excluding.len(),
    )
}

/// Whether a task consumes paths at all.
///
/// FR-4.4 turns on this: a command naming `{each_path}` or `{all_paths}` does
/// not execute when its filtered selection is empty, while a command naming
/// neither always executes. So "no paths" is only a reason to skip for a task
/// that wanted paths.
#[must_use]
pub fn consumes_paths(command: &str) -> bool {
    command.contains("{each_path}") || command.contains("{all_paths}")
}

/// Quote a path for substitution into a command line.
///
/// FR-4.3 quotes every path bolt substitutes individually, so a path carrying a
/// space, a quote or a semicolon can neither split the command line nor inject
/// into it. Individually is the load-bearing word: quoting the joined list
/// would leave the separators outside the quotes.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
#[must_use]
pub fn quote(path: &Path) -> String {
    todo!("quote {}", path.display())
}
