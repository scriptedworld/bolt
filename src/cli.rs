//! The command line, which is the only interface bolt has today.
//!
//! This lives here rather than in `main.rs` so that the entry point carries no
//! command functionality and the interface is reachable from an external test
//! package.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::run;

/// Bolt could not carry the run out.
///
/// FR-10.5 pairs this with 0 for a run that completed, whatever the tools
/// concluded, because FR-10.1 has the exit status answer whether bolt could
/// execute the ETL and FR-10.3 keeps the quality verdict in the envelope.
pub const REFUSED: u8 = 1;

/// A run bolt carried out, whatever the tools concluded.
pub const COMPLETED: u8 = 0;

/// Run bolt from a command line, returning the status the shell sees.
///
/// `arguments` excludes the program name. FR-2.1 and FR-2.1a make a complete
/// invocation exactly two: which jig, and where. Running several jigs over one
/// tree is a jig whose tasks are nested jigs, so a third argument asks for a
/// composition mechanism bolt does not have.
///
/// FR-10.6 leaves one status unchosen: a bolt killed by a signal exits 128 plus
/// the signal number, which is the shell's convention and never reaches here.
#[must_use]
pub fn main<I>(arguments: I) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
{
    let arguments: Vec<OsString> = arguments.into_iter().collect();
    let [jig, base] = arguments.as_slice() else {
        eprintln!("usage: bolt <jig> <directory>");
        return ExitCode::from(REFUSED);
    };

    match run::run(&jig.to_string_lossy(), &PathBuf::from(base)) {
        Ok(outcome) => {
            // FR-10.3: the verdict is in the envelope, so what a caller is told
            // here is where to read it rather than what it says.
            println!("{}", outcome.output_dir.join(run::RESULT_FILE).display());
            ExitCode::from(COMPLETED)
        }
        Err(refusal) => {
            eprintln!("bolt: {refusal}");
            ExitCode::from(REFUSED)
        }
    }
}
