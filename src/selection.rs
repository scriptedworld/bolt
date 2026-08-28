//! Narrowing a walk to what one task acts on.
//!
//! FR-3.4 and FR-3.4a are two halves of one operation: `matching` selects and
//! `excluding` removes from what was selected. Keeping them in that order is
//! the whole of the rule, because `excluding` is not a second way to select.

use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::Error;

/// Compile a task's pattern list into something a path can be tested against.
fn compile(patterns: &[String]) -> Result<GlobSet, Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        // FR-3.4 distinguishes `**`, which matches zero or more directory
        // levels, from `*`, which does not. globset's default is the opposite:
        // `*` matches `/` too, which makes the two operators the same and a
        // narrow pattern silently broad. Measured 2026-08-28 before this line
        // existed: `matching: ["*.txt"]` selected `nested/deep.txt`.
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|source| Error::JigUnreadable {
                path: PathBuf::from(pattern),
                reason: source.to_string(),
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|source| Error::JigUnreadable {
        path: PathBuf::from("<patterns>"),
        reason: source.to_string(),
    })
}

/// Apply one task's `matching` and `excluding` to a walk.
///
/// `paths` are what [`walk`](crate::walk::walk) returned, which are absolute.
/// FR-3.5 makes patterns relative to `base`, so each path is tested by its
/// position under it and a jig written for reuse says `**/*.rs` without naming
/// the subtree it was dropped into. Matching the absolute path instead would
/// leave `**/*.rs` working and every literal entry silently matching nothing,
/// which is the shape a stage 4 review measured.
///
/// Paths keep the order they arrived in, so a sorted walk gives a sorted
/// selection and FR-2.2d carries through to FR-9.2a's ordinals.
///
/// # Errors
///
/// [`Error::JigUnreadable`] when a pattern will not compile, which is a
/// property of the jig rather than of the tree.
pub fn select(
    base: &Path,
    paths: &[PathBuf],
    matching: &[String],
    excluding: &[String],
) -> Result<Vec<PathBuf>, Error> {
    let selects = compile(matching)?;
    let removes = compile(excluding)?;

    Ok(paths
        .iter()
        .filter(|path| {
            let Ok(relative) = path.strip_prefix(base) else {
                return false;
            };
            selects.is_match(relative) && !removes.is_match(relative)
        })
        .cloned()
        .collect())
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
    PATH_VARIABLES
        .iter()
        .any(|variable| command.contains(&format!("{{{variable}}}")))
}

/// The two variables that stand for the selection, without their braces.
///
/// Named once because two things read them: this, deciding whether a command
/// wants paths at all, and FR-5.13h's check that a jig task names neither. A
/// second spelling of the pair is a second place to forget one.
pub const PATH_VARIABLES: [&str; 2] = ["each_path", "all_paths"];

/// Quote a path for substitution into a command line.
///
/// FR-4.3 quotes every path bolt substitutes individually, so a path carrying a
/// space, a quote or a semicolon can neither split the command line nor inject
/// into it. Both words are load-bearing. *Individually*, because quoting the
/// joined list leaves the separators outside the quotes; and *a quote*, because
/// wrapping in single quotes is the obvious implementation and a path
/// containing one escapes it.
///
/// Single quotes with each embedded quote written as `'\''`, which is the one
/// form a POSIX shell reads back literally: close the quoting, emit an escaped
/// quote, reopen it. There is no other metacharacter to handle, because inside
/// single quotes a shell interprets nothing else.
#[must_use]
pub fn quote(path: &Path) -> String {
    quote_str(&path.to_string_lossy())
}

/// The same quoting for a value that is not a path.
///
/// A definition's value is a scalar by FR-4.16c and is quoted like a location,
/// which is what makes it one argument: a value carrying a space arrives as one
/// word rather than splitting into two.
#[must_use]
pub fn quote_str(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}
