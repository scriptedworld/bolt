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

    /// A command names a placeholder no layer supplies.
    ///
    /// FR-4.18 refuses before anything executes, with a reason naming it.
    /// Substituting nothing and handing `{requirements}` to a shell is what the
    /// row exists to prevent.
    UnknownPlaceholder {
        /// The task whose command names it.
        task: String,
        /// The placeholder, without its braces.
        placeholder: String,
    },

    /// Two tasks in one jig share a name.
    ///
    /// FR-3.3a: the name prefixes a task's work directories by FR-3.3, so a
    /// duplicate puts two tasks' executions in the same place. Reproduced
    /// 2026-08-28: the second overwrote the first's evidence, the fold saw one
    /// constituent, and a failing task vanished into a green result.
    DuplicateTaskName {
        /// The name used twice.
        task: String,
    },

    /// A task's name would not stay inside the run's work directory.
    ///
    /// The name becomes a path component by FR-3.3, so `..` in one climbs out.
    /// Reproduced 2026-08-28: a task named `../../../victim/EVIL` wrote a full
    /// evidence directory outside the base, which is FR-2.3's containment.
    UnsafeTaskName {
        /// The name that would leave the work directory.
        task: String,
    },

    /// The run's output directory already holds a run.
    ///
    /// FR-2.6b. `.bolt-<iso8601>` is second-granular, so two runs started in one
    /// second share a directory and each folds the other's evidence. Reproduced
    /// 2026-08-28: a second jig's result reported a failing task belonging to
    /// the first, and both callers were handed the same conflated file.
    OutputDirectoryInUse(PathBuf),

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

    /// A jig's `definitions` block or a definitions file names a reserved name.
    ///
    /// FR-4.19. `{base_dir}` redefined would substitute something other than
    /// where FR-4.1a stands the command, so the jig would say one thing while
    /// the process did another. FR-4.16d is why this is a refusal rather than a
    /// precedence question: bolt's layer is reserved, not merely first.
    ReservedDefinition {
        /// The reserved name the layer tried to define.
        name: String,
        /// Which layer named it, so the reason says which file to edit.
        source: String,
    },

    /// A named definitions file is absent, will not parse or will not validate.
    ///
    /// FR-4.20 validates it under FR-1.5 like everything else bolt reads as
    /// data, and says it is not taken for an absent file. Treating an
    /// unreadable one as absent would leave the jig's defaults standing and run
    /// a gate the caller thought they had overridden.
    DefinitionsUnreadable {
        /// The file that could not be read.
        path: PathBuf,
        /// What the parse or the validation said.
        reason: String,
    },

    /// The jig requires executables that are not on `PATH`, by FR-3.10b.
    ///
    /// Resolved before any task executes, so an incomplete toolchain is known
    /// in the first second rather than partway through a gate.
    ///
    /// **Every missing entry, not the first.** A caller fixing them one at a
    /// time pays a round trip per tool, which is the cost the row exists to
    /// remove.
    RequiresMissing {
        /// The entries `PATH` does not resolve, sorted.
        tools: Vec<String>,
    },

    /// The merge found no constituent to fold, by FR-8.3a.
    ///
    /// FR-8.3 alone would pass such a run, because every constituent passing
    /// holds when there are none, and a green result over zero checks is read
    /// as checked and fine.
    NoConstituents,
}

impl Error {
    /// Whether a refusal of this kind leaves a `result.yaml` behind.
    ///
    /// FR-10.7 has bolt write one whenever it is alive and in control when it
    /// stops, so a caller finding none knows the process was killed. FR-10.7a
    /// exempts refusals about the directory the result would go in, and
    /// **whether that exemption applies depends on where the directory is, not
    /// on which refusal it was.**
    ///
    /// [`Self::OutputDirectoryInUse`] is the one that never writes, whatever
    /// the caller named. The directory holds a previous run, so writing a
    /// refusal into it replaces a completed verdict with `kind: bolt-refused`
    /// while the per-task evidence still says otherwise. The Go build does
    /// exactly that, reproduced 2026-08-28 and filed at
    /// `clank/inbox/bolt.go/a-refusal-overwrites-the-run-it-refused/`. Writing
    /// it anywhere else would invent a location no caller was told about.
    ///
    /// [`Self::BaseMissing`] is the one that depends: with the default output
    /// directory it writes nothing, because that directory sits inside the base
    /// and writing there would create the thing whose absence is being refused.
    /// With `--output-dir` naming somewhere outside the base it writes a result
    /// like any other refusal, which is exactly what FR-10.7b tells a caller
    /// wanting a parseable refusal in every case to do. [`crate::run::invoke`]
    /// makes that call, since it is the only place that knows both paths.
    #[must_use]
    pub fn never_writes_a_result(&self) -> bool {
        matches!(self, Self::OutputDirectoryInUse(_))
    }
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
            Self::UnknownPlaceholder { task, placeholder } => write!(
                formatter,
                "task {task} names {{{placeholder}}}, which nothing defines",
            ),
            Self::DuplicateTaskName { task } => write!(
                formatter,
                "two tasks are named {task}; a name is a work directory prefix",
            ),
            Self::UnsafeTaskName { task } => write!(
                formatter,
                "task name {task} would leave the run's work directory",
            ),
            Self::OutputDirectoryInUse(path) => {
                write!(formatter, "{} already holds a run", path.display())
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
            Self::ReservedDefinition { name, source } => write!(
                formatter,
                "{source} defines {name}, which is reserved to bolt",
            ),
            Self::DefinitionsUnreadable { path, reason } => write!(
                formatter,
                "the definitions file {} is unreadable: {reason}",
                path.display()
            ),
            Self::RequiresMissing { tools } => write!(
                formatter,
                "the jig requires {}, which {} not on PATH",
                tools.join(", "),
                if tools.len() == 1 { "is" } else { "are" },
            ),
            Self::NoConstituents => {
                write!(formatter, "no task produced a result")
            }
        }
    }
}

impl std::error::Error for Error {}
