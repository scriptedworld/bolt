//! How deep a run is nested, and how deep it may go.
//!
//! FR-5.6 carries the depth in the environment of **every** process bolt spawns
//! rather than passing it to child jigs alone. That is what makes it survive
//! reparenting, backgrounding, and a task command that invokes bolt directly
//! instead of through a jig task, which is the case FR-5.7a builds on and the
//! case this can be tested against before nested jigs exist.

use std::env;

/// The variable carrying the current depth, by FR-5.6a.
pub const DEPTH: &str = "BOLT_DEPTH";

/// The variable carrying the ceiling, by FR-5.6a.
pub const CEILING: &str = "BOLT_MAX_DEPTH";

/// How deep a run may nest before it is refused, by FR-5.7.
pub const DEFAULT_CEILING: u32 = 4;

/// Where a run sits, and where the floor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Depth {
    /// This run's own depth, the outermost being 1.
    pub level: u32,
    /// The deepest level allowed.
    pub ceiling: u32,
}

impl Depth {
    /// Read this run's depth from the environment it was started in.
    ///
    /// The outermost invocation finds nothing set and is depth 1. Anything
    /// deeper finds what its parent exported and increments it, by FR-5.6.
    ///
    /// **A value that will not parse is treated as absent**, which reads the run
    /// as outermost rather than refusing it. A caller's environment is not a
    /// document bolt was asked to validate, and FR-5.7a already says the ceiling
    /// guards against accident rather than evasion, so refusing here would fail
    /// runs over stray shell state while stopping nobody who meant it.
    #[must_use]
    pub fn from_environment() -> Self {
        let inherited = read(DEPTH);
        Self {
            level: inherited.map_or(1, |level| level.saturating_add(1)),
            ceiling: read(CEILING).unwrap_or(DEFAULT_CEILING),
        }
    }

    /// Whether this run is deeper than it is allowed to be, by FR-5.7.
    #[must_use]
    pub fn exceeded(self) -> bool {
        self.level > self.ceiling
    }

    /// What every process this run spawns is told, by FR-5.6.
    ///
    /// FR-5.7 says the ceiling is read from the environment only at the
    /// outermost invocation, and **this is the mechanism rather than a check**:
    /// bolt overwrites both variables on every spawn, so a nested bolt reading
    /// them gets what the bolt above it set and never what a jig wrote. There
    /// is no branch on being outermost, and adding one would only matter for a
    /// command deliberately rewriting the variable, which FR-5.7a puts out of
    /// scope.
    #[must_use]
    pub fn exported(self) -> [(&'static str, String); 2] {
        [
            (DEPTH, self.level.to_string()),
            (CEILING, self.ceiling.to_string()),
        ]
    }
}

/// One variable, where it is set and reads as a number.
fn read(name: &str) -> Option<u32> {
    env::var(name).ok()?.trim().parse().ok()
}
