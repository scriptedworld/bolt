//! Why bolt could not carry a run out.
//!
//! FR-10.5 lists what belongs here: a jig that will not parse, an unknown
//! adapter, an unwritable output directory, a depth ceiling passed, a directory
//! that is not there. A tool reporting problems is not one of them, because
//! FR-10.3 keeps the quality verdict in the envelope.

use std::fmt;
use std::path::PathBuf;

/// A refusal. Bolt could not execute the requested task ETL.
///
/// FR-2.5a has every refusal take one shape on disk: a `result.yaml` carrying
/// `success: false` and a reason, then a non-zero exit. This is the reason.
#[derive(Debug)]
pub enum Error {
    /// The directory the run was given is not there, by FR-2.5.
    ///
    /// The path is carried so the reason can name it, which FR-2.5a requires.
    BaseMissing(PathBuf),

    /// The jig could not be read or did not meet its schema, by FR-1.5.
    JigUnreadable {
        /// The jig that could not be read.
        path: PathBuf,
        /// What the parse or the validation said.
        reason: String,
    },

    /// A task names a jig rather than a command, and nested jigs are unbuilt.
    ///
    /// FR-5.x specifies them and `clank/tasks/bolt/runner/50-nested-jigs` is
    /// where they get built. Refusing by name matters because the alternative
    /// message is serde's `missing field command`, which reads as a malformed
    /// jig and invites somebody to add a command to a task that should not have
    /// one.
    NestedJigNotBuilt {
        /// The task naming a jig.
        task: String,
    },

    /// A task's command names both `{each_path}` and `{all_paths}`.
    ///
    /// FR-4.2 calls that a jig error. Which of the two shapes a task takes is
    /// read off its command, and naming both asks for both at once.
    CommandNamesBothPathForms {
        /// The task whose command names both.
        task: String,
    },

    /// Bolt could not write what a run needs on disk.
    ///
    /// FR-10.5 lists an unwritable output directory as a refusal, and this is
    /// every other filesystem failure with it: a run that cannot record what it
    /// did has not carried out the ETL, whatever the tools concluded.
    Io {
        /// What bolt was trying to write or read.
        path: PathBuf,
        /// What the operating system said.
        reason: String,
    },

    /// The merge found no constituent to fold, by FR-8.3a.
    ///
    /// FR-8.3 alone would pass such a run, because every constituent passing
    /// holds when there are none, and a green result over zero checks is read
    /// as checked and fine.
    NoConstituents,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseMissing(path) => {
                write!(formatter, "the directory {} is not there", path.display())
            }
            Self::JigUnreadable { path, reason } => {
                write!(
                    formatter,
                    "the jig {} is unreadable: {reason}",
                    path.display()
                )
            }
            Self::NestedJigNotBuilt { task } => write!(
                formatter,
                "task {task} names a jig; nested jigs are specified and not built yet",
            ),
            Self::CommandNamesBothPathForms { task } => {
                write!(formatter, "task {task} names both each_path and all_paths")
            }
            Self::Io { path, reason } => {
                write!(formatter, "{}: {reason}", path.display())
            }
            Self::NoConstituents => {
                write!(formatter, "no task produced a result")
            }
        }
    }
}

impl std::error::Error for Error {}
