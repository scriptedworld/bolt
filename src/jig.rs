//! A jig, and the tasks it declares.
//!
//! FR-3.4d makes a jig YAML, read through wrench by FR-1.12 and validated
//! against its schema on the way in by FR-1.5. Bolt takes the parsed value as
//! `serde_json::Value` from wrench and derives these types off it, so a jig is
//! a struct rather than eighty lines of map digging.

use std::path::Path;

use serde::Deserialize;

use crate::Error;

/// A jig: a named set of tasks run over one directory.
#[derive(Debug, Deserialize)]
pub struct Jig {
    /// The jig's schema version.
    pub version: String,

    /// The tasks, in the order the jig declares them.
    ///
    /// FR-4.5 says they execute serially. Whether serial means *in this order*
    /// is question 38 in `NEXT_STEPS.md` and no row settles it, so nothing here
    /// promises the declaration order is the execution order.
    pub tasks: Vec<Task>,
}

/// One task in a jig.
#[derive(Debug, Deserialize)]
pub struct Task {
    /// The task's name, which names its work directories by FR-9.2.
    pub name: String,

    /// The command line, carrying whichever path form the task takes.
    ///
    /// FR-4.2 reads the shape off this rather than off a field beside it:
    /// `{each_path}` is one execution per matched path, `{all_paths}` is one
    /// execution with the selection substituted, and naming both is a jig
    /// error.
    pub command: String,

    /// Patterns or literal paths saying which files this task acts on.
    ///
    /// FR-3.4, where `**` matches zero or more directory levels. A task never
    /// sees a path its condition rejects.
    #[serde(default)]
    pub matching: Vec<String>,

    /// Patterns or literal paths removed from what `matching` selected.
    ///
    /// FR-3.4a. It removes from the selection rather than being a second way
    /// to select.
    #[serde(default)]
    pub excluding: Vec<String>,
}

/// Read a jig from disk and validate it.
///
/// # Errors
///
/// [`Error::JigUnreadable`] when the file will not parse or does not meet the
/// schema, which FR-10.5 makes a refusal rather than a failed task.
///
/// # Panics
///
/// Always, for now. Nothing is implemented.
pub fn read(path: &Path) -> Result<Jig, Error> {
    todo!("read and validate the jig at {}", path.display())
}
