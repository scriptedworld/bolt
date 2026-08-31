//! Bolt runs a jig over a directory and records what happened.
//!
//! An invocation says which jig and where, by FR-2.1. Bolt walks the directory,
//! filters that walk per task, executes each task's command, keeps what every
//! command produced, and folds the per-execution envelopes into one result.

pub mod adapter;
pub mod cli;
pub mod definitions;
pub mod depth;
pub mod jig;
pub mod limit;
pub mod merge;
pub mod outcome;
pub mod run;
pub mod selection;
pub mod walk;

mod error;
mod stamp;

pub use error::Error;
pub use outcome::{Outcome, Refusal};
