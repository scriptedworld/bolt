//! Bolt's entry point.
//!
//! Command functionality does not live here. This takes the arguments the shell
//! gave and hands them to the library, so the interface is testable from an
//! external test package and this file stays the size it is.

use std::process::ExitCode;

fn main() -> ExitCode {
    bolt::cli::main(std::env::args_os().skip(1))
}
