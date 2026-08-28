//! Turning one execution's captured output into a verdict.
//!
//! FR-6.1: an adapter is a separate process, and where it reached an
//! authoritative result that result **is** the verdict. Bolt does not
//! second-guess one.
//!
//! FR-6.1a says when bolt writes an envelope itself, and says it as a rule
//! rather than a count: only where no adapter's result is available to take.
//! FR-6.1b records why it stopped counting, which is that counting was wrong
//! twice and a list claiming completeness invites the next reader to trust the
//! number rather than the rule.

use std::path::{Path, PathBuf};

/// The name of the file holding an execution's captured standard output.
pub const STDOUT_FILE: &str = "stdout";

/// The name of the file holding an execution's captured standard error.
pub const STDERR_FILE: &str = "stderr";

/// Why bolt wrote an envelope instead of taking an adapter's, by FR-6.11.
///
/// The three are kept apart because they have different causes: a crashing
/// adapter, a silent one, and one whose output is not an envelope are three
/// different things to go and fix.
#[derive(Debug, Clone, Copy)]
pub enum Unauthoritative {
    /// The adapter ran and exited non-zero.
    Exited(i32),
    /// The adapter left no `output.yaml` where FR-6.2b says it goes.
    WroteNothing,
    /// It left one that will not parse or will not validate.
    WroteInvalid,
}

impl Unauthoritative {
    /// The reason `kind` for this case, by FR-7.9.
    ///
    /// A consumer tells one from another without reading English, which is what
    /// FR-7.10 rests on.
    #[must_use]
    pub fn kind(self) -> &'static str {
        match self {
            Self::Exited(_) => "adapter-failed",
            Self::WroteNothing => "adapter-wrote-nothing",
            Self::WroteInvalid => "adapter-wrote-invalid",
        }
    }

    /// What went wrong, in the words a person reads.
    #[must_use]
    pub fn message(self, adapter: &str) -> String {
        match self {
            Self::Exited(status) => format!("the adapter {adapter} exited {status}"),
            Self::WroteNothing => {
                format!("the adapter {adapter} wrote no {}", crate::run::OUTPUT_FILE)
            }
            Self::WroteInvalid => format!(
                "the adapter {adapter} wrote a {} that is not an envelope",
                crate::run::OUTPUT_FILE
            ),
        }
    }
}

/// Where an adapter named `name` is found, by FR-6.10.
///
/// Resolved from the config directory, where FR-2.8 already finds jigs, so a jig
/// and the adapters it names travel together and `link-jigs` places both or
/// neither.
#[must_use]
pub fn path(config_dir: &Path, name: &str) -> PathBuf {
    config_dir.join(name)
}

/// FR-6.2's default invocation, as a shell line.
///
/// Names the captured files and the locations an adapter is handed. FR-6.2a
/// gives it the same three locations every task gets; FR-6.2c has `--evidence`
/// name what the task declared and nothing it did not.
///
/// FR-6.3 hands the exit code over as a file rather than as a verdict: whether
/// that number explains anything is the adapter's judgement, not bolt's.
///
/// No flag says where the envelope goes, by FR-6.2b. The path is the work
/// directory the adapter was given and the name never varies.
#[must_use]
pub fn default_invocation(adapter: &str, evidence: &[String]) -> String {
    let mut line = format!(
        "{} --stdout {{work_dir}}/{STDOUT_FILE} --stderr {{work_dir}}/{STDERR_FILE} \
         --exitcode {{work_dir}}/{} --project-root {{project_root}} --base-dir {{base_dir}} \
         --work-dir {{work_dir}}",
        shell_word(adapter),
        crate::run::EXITCODE_FILE,
    );
    // FR-6.2c: one `--evidence` per declared file, so an adapter reading two
    // artifacts is told about both and about nothing else.
    for file in evidence {
        line.push_str(" --evidence {work_dir}/");
        line.push_str(&shell_word(file));
    }
    line
}

/// A word safe to put on a shell line without changing what it means.
///
/// The adapter name and the evidence filenames come from a jig, which is the
/// project's own file, but they still reach a shell. FR-4.3's reasoning applies
/// wherever bolt builds a command line rather than only where a *path* is
/// substituted, and the substitution pass this line goes through afterwards is
/// single-pass by design, so a quoted word stays one word.
fn shell_word(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._-/".contains(c))
    {
        return value.to_owned();
    }
    crate::selection::quote_str(value)
}
