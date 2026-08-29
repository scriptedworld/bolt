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

    /// A task carries the retired `jig` field, by FR-5.22.
    ///
    /// FR-5.18 makes composition a command line, so a jig running another jig
    /// invokes `bolt` like any other tool. The refusal names the field and says
    /// what replaced it, because a jig written against the retired mechanism is
    /// not malformed and its author needs the new spelling rather than a
    /// complaint about a missing field.
    TaskNamesAJig {
        /// The task carrying the field.
        task: String,
    },

    /// A task carries no command at all.
    ///
    /// Refused by name rather than by serde, so the reason says the task has
    /// nothing to run instead of naming a field the reader has to map back to a
    /// task.
    TaskNamesNoCommand {
        /// The task with no command.
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

    /// A `time-limit` is not a duration, by FR-4.11e.
    ///
    /// Refused before anything executes, for FR-4.18a's reason: a jig whose
    /// third task spells its limit `30` refuses in the first second rather than
    /// two tasks into a gate. Reading it as no limit is the alternative that
    /// fails silently, running unbounded exactly where somebody asked for a
    /// ceiling.
    MalformedTimeLimit {
        /// The task whose limit it is, or `None` for the jig's own.
        task: Option<String>,
        /// What was written where a duration was wanted.
        value: String,
    },

    /// The run is nested deeper than the ceiling allows, by FR-5.7.
    ///
    /// FR-5.8 makes this an ordinary refusal: a result carrying the reason, then
    /// a non-zero exit, so the run above folds a failing constituent rather than
    /// meeting a hole where one should be.
    ///
    /// A guard against accident and runaway, not against a jig trying to defeat
    /// it. FR-5.7a says a command can unset the variable and be believed
    /// outermost, and closing that needs an ancestry cross-check nothing has
    /// asked for.
    DepthExceeded {
        /// How deep this run would have been.
        level: u32,
        /// The deepest allowed, which the reason names.
        ceiling: u32,
    },

    /// The merge found no constituent to fold, by FR-8.3a.
    ///
    /// FR-8.3 alone would pass such a run, because every constituent passing
    /// holds when there are none, and a green result over zero checks is read
    /// as checked and fine.
    NoConstituents,
}

impl Error {
    /// What sort of refusal this is, for the `kind` of the reason it writes.
    ///
    /// FR-10.9. **One kind for every refusal was the defect**: a reused output
    /// directory, a base that is not there, a jig that will not parse and a task
    /// carrying a retired field are four situations with four different fixes,
    /// and a consumer that can tell them apart will.
    ///
    /// FR-10.9a is why this vocabulary is bolt's alone. wrench's envelope schema
    /// takes any non-empty string and says why it does not enumerate them: "a
    /// closed list would make a schema change the price of a new kind of
    /// failure". So a new refusal adds a name here and nothing anywhere else.
    ///
    /// FR-10.9b is why there is no wildcard arm. A refusal added without a kind
    /// does not compile, where one inheriting a neighbour's would quietly make a
    /// consumer's match wrong.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::BaseMissing(_) => "base-missing",
            Self::JigUnreadable { .. } => "jig-unreadable",
            Self::DefinitionsUnreadable { .. } => "definitions-unreadable",
            Self::TaskNamesAJig { .. } => "jig-task-retired",
            Self::TaskNamesNoCommand { .. } => "task-without-command",
            Self::UnknownPlaceholder { .. } => "unknown-placeholder",
            Self::DuplicateTaskName { .. } => "duplicate-task-name",
            Self::UnsafeTaskName { .. } => "unsafe-task-name",
            Self::CommandNamesBothPathForms { .. } => "both-path-forms",
            Self::ReservedDefinition { .. } => "reserved-definition",
            Self::MalformedTimeLimit { .. } => "malformed-time-limit",
            Self::RequiresMissing { .. } => "requires-missing",
            Self::DepthExceeded { .. } => "depth-exceeded",
            Self::NoConstituents => "no-constituents",
            Self::Io { .. } => "io-failed",
            // FR-10.9c. Named for completeness and never written: FR-2.6b
            // returns before anything is written, because the directory holds a
            // completed run and a refusal put there replaces a verdict.
            Self::OutputDirectoryInUse(_) => "output-directory-in-use",
        }
    }

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
                write!(formatter, "{}", unreadable("jig", path, reason))
            }
            Self::UnknownPlaceholder { task, placeholder } => write!(
                formatter,
                "task {task} names {{{placeholder}}}, which nothing defines",
            ),
            Self::DuplicateTaskName { task } => write!(formatter, "{}", duplicate_name(task)),
            Self::UnsafeTaskName { task } => write!(formatter, "{}", unsafe_name(task)),
            Self::OutputDirectoryInUse(path) => {
                write!(formatter, "{} already holds a run", path.display())
            }
            Self::TaskNamesAJig { task } => write!(formatter, "{}", names_a_jig(task)),
            Self::TaskNamesNoCommand { task } => {
                write!(formatter, "task {task} has no command to run")
            }
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
            Self::DefinitionsUnreadable { path, reason } => {
                write!(
                    formatter,
                    "{}",
                    unreadable("definitions file", path, reason)
                )
            }
            Self::RequiresMissing { tools } => write!(formatter, "{}", requires_missing(tools)),
            Self::MalformedTimeLimit { task, value } => {
                write!(
                    formatter,
                    "{}",
                    malformed_time_limit(task.as_deref(), value)
                )
            }
            Self::DepthExceeded { level, ceiling } => {
                write!(formatter, "{}", too_deep(*level, *ceiling))
            }
            Self::NoConstituents => {
                write!(formatter, "no task produced a result")
            }
        }
    }
}

/// FR-4.11e's reason, naming whose limit it is so the caller knows what to edit.
///
/// `task` is `None` for the jig's own limit, which is the run's by FR-4.11d.
fn malformed_time_limit(task: Option<&str>, value: &str) -> String {
    let whose = task.map_or_else(|| "the jig".to_owned(), |task| format!("task {task}"));
    format!("{whose} sets a time limit of {value}, which is not a decimal followed by s, m or h")
}

/// FR-2.3's reason, for a task name that would climb out of the work directory.
fn unsafe_name(task: &str) -> String {
    format!("task name {task} would leave the run's work directory")
}

/// FR-5.8's reason, which names the limit so a reader knows what was hit.
fn too_deep(level: u32, ceiling: u32) -> String {
    format!("this run is {level} deep and the limit is {ceiling}")
}

/// FR-3.3a's reason, which says why a duplicate matters rather than that it is.
fn duplicate_name(task: &str) -> String {
    format!("two tasks are named {task}; a name is a work directory prefix")
}

/// FR-1.5's reason for a document that would not parse or would not validate.
///
/// One sentence for both the jig and the definitions file, because a reader
/// meets them the same way and wrench produced both messages.
fn unreadable(kind: &str, path: &std::path::Path, reason: &str) -> String {
    format!("the {kind} {} is unreadable: {reason}", path.display())
}

/// FR-5.13h's reason, naming the field so a reader knows which line to edit.
fn names_a_jig(task: &str) -> String {
    format!(
        "task {task} carries the retired jig field; \
         run the jig as a command instead, bolt <jig> <directory>"
    )
}

/// FR-3.10b's reason, naming every entry `PATH` did not resolve.
fn requires_missing(tools: &[String]) -> String {
    format!(
        "the jig requires {}, which {} not on PATH",
        tools.join(", "),
        if tools.len() == 1 { "is" } else { "are" },
    )
}

impl std::error::Error for Error {}
