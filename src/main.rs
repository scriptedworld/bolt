//! Bolt runs a jig over a directory and records what happened.
//!
//! Nothing is implemented. `REQUIREMENTS.md` holds 225 settled rows and the
//! test plans in `clank/tasks/bolt/` say which test discharges which, both
//! written against the Go implementation that now lives in the `bolt.go`
//! repository. This crate is where they get built a second time.
//!
//! Command functionality does not live in the entry point, so this file stays
//! the size it is and the work goes in library modules beside it.

use std::process::ExitCode;

/// Bolt could not carry the run out.
///
/// FR-10.5 pairs this with 0 for a run that completed, and there is no such run
/// to return it for yet. The other status arrives with the invocation that can
/// reach it, rather than sitting here unused behind a suppression.
const REFUSED: u8 = 1;

fn main() -> ExitCode {
    eprintln!("bolt: nothing is implemented; see REQUIREMENTS.md");
    ExitCode::from(REFUSED)
}
