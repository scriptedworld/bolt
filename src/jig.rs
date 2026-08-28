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
    /// The version of the format this jig claims to conform to.
    ///
    /// Optional, because wrench's schema requires only `tasks`. Making it
    /// mandatory here was stricter than the contract and refused six of the
    /// estate's jigs, including bolt's own, which is how it was found: the
    /// first time the Rust bolt was pointed at its own gate by NFR-12.1.
    #[serde(default)]
    pub version: Option<String>,

    /// Every executable this jig invokes, by FR-3.10.
    ///
    /// The tools its commands run, the adapters its tasks name, any checker it
    /// calls: the jig's whole inventory rather than a note about unusual tools.
    /// FR-3.10b resolves every entry before any task executes.
    #[serde(default)]
    pub requires: Vec<String>,

    /// Default values for the placeholders this jig's commands name, by FR-3.15.
    ///
    /// Optional, and so is any entry in it: a jig leaving a value to its adopter
    /// names the placeholder in a command and defines nothing. Kept as a raw
    /// value because FR-4.16c's shape is the definitions schema's, which wrench
    /// has already validated on the way in, and re-deriving it as a typed map
    /// here would be a second statement of the same thing.
    #[serde(default)]
    pub definitions: Option<serde_json::Value>,

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
    /// The task's name, which prefixes its work directories by FR-3.3.
    pub name: String,

    /// The command line, carrying whichever path form the task takes.
    ///
    /// FR-4.2 reads the shape off this rather than off a field beside it:
    /// `{each_path}` is one execution per matched path, `{all_paths}` is one
    /// execution with the selection substituted, and naming both is a jig
    /// error.
    ///
    /// Optional in the type because a JIG task has none: FR-5.13h gives it a
    /// `jig` field instead. Nested jigs are not built, and a task without a
    /// command is refused by name rather than by serde, so the reason says
    /// which feature is missing instead of which field is.
    #[serde(default)]
    pub command: Option<String>,

    /// The jig this task runs, for a jig task rather than a command task.
    #[serde(default)]
    pub jig: Option<String>,

    /// Patterns or literal paths saying which files this task acts on.
    ///
    /// FR-3.4, where `**` matches zero or more directory levels, and FR-3.5
    /// makes them relative to the run's base.
    #[serde(default)]
    pub matching: Vec<String>,

    /// Patterns or literal paths removed from what `matching` selected.
    ///
    /// FR-3.4a. It removes from the selection rather than being a second way
    /// to select.
    #[serde(default)]
    pub excluding: Vec<String>,

    /// Whether an empty selection is an acceptable result for this task.
    ///
    /// FR-4.4b makes an empty selection a failure by default, because a pattern
    /// matching nothing is usually a typo or a moved directory and a silent
    /// skip leaves it green forever. FR-4.4c is this field: a shared jig
    /// spanning languages declares it on the tasks that legitimately find
    /// nothing in a given project.
    ///
    /// FR-4.4d and FR-4.4h make it a jig error on a task naming no path
    /// variable, enforced by the schema rather than here.
    #[serde(default, rename = "allow-empty")]
    pub allow_empty: bool,

    /// Stop the run when this task fails, by FR-4.9.
    ///
    /// Defaulting to false, because FR-4.8 is the rule: a failing task does not
    /// stop the run, since stopping throws away the evidence the tasks after it
    /// would have produced and leaves a reader unable to tell what else was
    /// wrong. Stopping is what a jig asks for rather than what it gets, and
    /// this field is the asking.
    #[serde(default, rename = "short-circuit-failure")]
    pub short_circuit_failure: bool,
}

/// Read the jig named `name` from `config_dir` and validate it.
///
/// FR-3.9 makes a jig file `bolt.<name>.yaml` and has a jig spoken of by its
/// name rather than by a path. FR-2.8 says where those files are found.
///
/// # Errors
///
/// [`Error::JigUnreadable`] when the file is absent, will not parse, or does
/// not meet the schema, which FR-10.5 makes a refusal rather than a failed
/// task.
pub fn read(config_dir: &Path, name: &str) -> Result<Jig, Error> {
    let path = config_dir.join(file_name(name));
    let unreadable = |reason: String| Error::JigUnreadable {
        path: path.clone(),
        reason,
    };

    // FR-1.12: every structured file goes through wrench, which reads, decodes
    // and validates against the shipped jig schema in one call. FR-1.5 makes
    // that validation the thing a broken jig fails, so there is nothing to
    // check here that wrench has not already refused.
    let value = wrench::load_formatted_file(
        path.to_str()
            .ok_or_else(|| unreadable("the path is not utf-8".to_owned()))?,
        &wrench::JIG_SCHEMA,
        &wrench::YamlCodec,
        &wrench::LocalFileIo,
    )
    .map_err(|source| unreadable(source.to_string()))?;

    // FR-1.9: wrench hands back a `serde_json::Value`, so a jig is a derive
    // rather than eighty lines of map digging.
    serde_json::from_value(value).map_err(|source| unreadable(source.to_string()))
}

/// The file a jig named `name` is read from, by FR-3.9.
#[must_use]
pub fn file_name(name: &str) -> String {
    format!("bolt.{name}.yaml")
}
