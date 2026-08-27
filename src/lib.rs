//! Bolt runs a jig over a directory and records what happened.
//!
//! An invocation says which jig and where, by FR-2.1. Bolt walks the directory,
//! filters that walk per task, executes each task's command, keeps what every
//! command produced, and folds the per-execution envelopes into one result.
//!
//! Nothing here is implemented. The tests in `tests/` are written against this
//! surface and fail against it deliberately, per stage 4 of
//! `silo/docs/PATTERNS/how-a-change-gets-made.md`.

pub mod cli;
pub mod jig;
pub mod merge;
pub mod outcome;
pub mod run;
pub mod selection;
pub mod walk;

mod error;

pub use error::Error;
pub use outcome::Outcome;
