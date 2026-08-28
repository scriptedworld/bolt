//! The command line, which is the only interface bolt has today.
//!
//! This lives here rather than in `main.rs` so that the entry point carries no
//! command functionality and the interface is reachable from an external test
//! package.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::run;

/// A complete invocation, once the arguments have been read.
struct Parsed {
    /// Which jig, by FR-2.1 and FR-3.9. A name, never a path.
    jig: String,
    /// Where, and the run's base.
    base: PathBuf,
    /// The definitions file named by FR-4.16a, if one was.
    definitions: Option<String>,
    /// Where evidence goes, by FR-2.6, if the caller said.
    output_dir: Option<PathBuf>,
    /// Where jigs are found, by FR-2.8, if the caller said.
    config_dir: Option<PathBuf>,
}

/// Read an invocation, or `None` where it is not one.
///
/// FR-2.1a keeps the positional arguments at exactly two: which jig and where.
/// Running several jigs over one tree is a jig whose tasks are nested jigs, so a
/// third positional asks for a composition mechanism bolt does not have.
///
/// FR-4.16b allows at most one definitions file, so naming it twice is refused
/// rather than the last one silently winning. There is no ordering to settle
/// between two files because there is never more than one.
fn parse(arguments: &[OsString]) -> Option<Parsed> {
    let mut positional = Vec::with_capacity(2);
    let mut definitions = None;
    let mut output_dir = None;
    let mut config_dir = None;
    let mut rest = arguments.iter();

    while let Some(argument) = rest.next() {
        match argument.to_str() {
            Some("--definitions") => {
                if definitions.is_some() {
                    return None;
                }
                definitions = Some(rest.next()?.to_string_lossy().into_owned());
            }
            // FR-2.6. Named twice is refused rather than the last one silently
            // winning, for the same reason as `--definitions`: a caller who
            // wrote it twice meant something, and neither reading is safe to
            // guess.
            Some("--output-dir") => {
                if output_dir.is_some() {
                    return None;
                }
                output_dir = Some(PathBuf::from(rest.next()?));
            }
            // FR-2.8. Where jigs live is told to bolt rather than inferred from
            // the directory being run on, which is what lets one shared jig
            // directory serve a tree it does not sit in. Refused twice for the
            // same reason as the other two.
            Some("--config-dir") => {
                if config_dir.is_some() {
                    return None;
                }
                config_dir = Some(PathBuf::from(rest.next()?));
            }
            _ => positional.push(argument),
        }
    }

    let [jig, base] = positional.as_slice() else {
        return None;
    };
    Some(Parsed {
        jig: jig.to_string_lossy().into_owned(),
        base: PathBuf::from(base),
        definitions,
        output_dir,
        config_dir,
    })
}

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
    let Some(Parsed {
        jig,
        base,
        definitions,
        output_dir,
        config_dir,
    }) = parse(&arguments)
    else {
        eprintln!(
            "usage: bolt <jig> <directory> [--definitions <name>] [--output-dir <path>] \
             [--config-dir <path>]"
        );
        return ExitCode::from(REFUSED);
    };

    match run::invoke(&run::Invocation {
        jig: &jig,
        base: &base,
        definitions: definitions.as_deref(),
        output_dir: output_dir.as_deref(),
        config_dir: config_dir.as_deref(),
    }) {
        Ok(outcome) => {
            // FR-10.3: the verdict is in the envelope, so what a caller is told
            // here is where to read it rather than what it says.
            println!("{}", outcome.output_dir.join(run::RESULT_FILE).display());
            ExitCode::from(COMPLETED)
        }
        Err(refusal) => {
            eprintln!("bolt: {refusal}");
            // FR-10.7a. A refusal that wrote nothing says so, because "no
            // result" otherwise reads as a bolt that was killed, which is
            // exactly what FR-10.7 has a caller conclude from an absent file.
            // FR-10.7b points a caller wanting one in every case at an output
            // directory outside the tree, so the advice is worth giving here
            // rather than leaving them to find the row.
            if !run::wrote_a_result(&refusal, &base, output_dir.as_deref()) {
                eprintln!(
                    "bolt: no result was written, because that is the directory in question; \
                     name --output-dir outside it for one"
                );
            }
            ExitCode::from(REFUSED)
        }
    }
}
