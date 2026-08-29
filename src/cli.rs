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
    /// Whether FR-10.8's flag was named, making the envelope the exit code.
    result_to_exitcode: bool,
}

/// Read an invocation, or `None` where it is not one.
///
/// FR-2.1a keeps the positional arguments at exactly two: which jig and where.
/// Running several jigs over one tree is a jig whose tasks invoke bolt by
/// FR-5.18, so a third positional asks for a composition mechanism bolt does not
/// have and does not need.
///
/// FR-4.16b allows at most one definitions file, so naming it twice is refused
/// rather than the last one silently winning. There is no ordering to settle
/// between two files because there is never more than one.
fn parse(arguments: &[OsString]) -> Option<Parsed> {
    let mut positional = Vec::with_capacity(2);
    let mut definitions = None;
    let mut output_dir = None;
    let mut config_dir = None;
    let mut result_to_exitcode = false;
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
            // FR-10.8. Named twice is not refused, unlike the three above: a
            // flag says one thing however many times it is written, where a
            // value written twice leaves two readings and no way to choose.
            Some("--result-to-exitcode") => result_to_exitcode = true,
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
        result_to_exitcode,
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

/// Under FR-10.8's flag, the envelope said the run did not pass.
///
/// **The same number as [`REFUSED`], and deliberately so.** FR-10.8c makes a
/// refusal a verdict bolt reached rather than a question left open, so there is
/// nothing for a third status to mean and the refusal path needs no branch on
/// the flag at all.
pub const ENVELOPE_FAILED: u8 = 1;

/// What a run bolt carried out exits with, by FR-10.5 and FR-10.8.
///
/// Without the flag the answer is FR-10.1's and does not depend on the verdict
/// at all: bolt executed the ETL, so 0. With it, `0 if success else 1`.
///
/// **FR-10.8b has no third case and this has no third branch.** A task set
/// always resolves, by FR-10.8d: one that matched nothing and was declared
/// optional is satisfied, and a required one that never ran has failed. Neither
/// is an absent verdict, so there is nothing a third status could mean.
const fn completed_status(success: bool, result_to_exitcode: bool) -> u8 {
    if !result_to_exitcode {
        return COMPLETED;
    }
    if success { COMPLETED } else { ENVELOPE_FAILED }
}

/// Run bolt from a command line, returning the status the shell sees.
///
/// `arguments` excludes the program name. FR-2.1 and FR-2.1a make a complete
/// invocation exactly two: which jig, and where. Running several jigs over one
/// tree is a jig whose tasks invoke bolt by FR-5.18, so a third argument asks
/// for a composition mechanism bolt does not have and does not need.
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
        result_to_exitcode,
    }) = parse(&arguments)
    else {
        eprintln!(
            "usage: bolt <jig> <directory> [--definitions <name>] [--output-dir <path>] \
             [--config-dir <path>] [--result-to-exitcode]"
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
            // here is where to read it rather than what it says. FR-10.8e keeps
            // that true under the flag: the number changes and the line does
            // not, so a caller gets both readings from one run.
            println!("{}", outcome.output_dir.join(run::RESULT_FILE).display());
            ExitCode::from(completed_status(outcome.success, result_to_exitcode))
        }
        Err(refusal) => report_refusal(&refusal),
    }
}

/// Tell a caller a run was refused, and with what status.
///
/// FR-10.7a. A refusal that wrote nothing says so, because "no result"
/// otherwise reads as a bolt that was killed, which is exactly what FR-10.7 has
/// a caller conclude from an absent file. FR-10.7b points a caller wanting one
/// in every case at an output directory outside the tree, so the advice is
/// worth giving here rather than leaving them to find the row.
///
/// **FR-10.8's flag is not a parameter here, and that is the row rather than an
/// omission.** FR-10.8c makes a refusal a verdict bolt reached, so
/// `bolt-refused` with `success: false` is 1 whether or not the caller asked
/// for the envelope to decide. Reading `kind` to call it "no verdict" would
/// overrule an authoritative field with its neighbour.
fn report_refusal(refusal: &crate::Refusal) -> ExitCode {
    eprintln!("bolt: {}", refusal.error);
    if let Some(result) = &refusal.result {
        // FR-10.3a. Stdout is where the result is, on every path that wrote
        // one. FR-5.19's adapter reads this line, and a refusal going quiet
        // would reach it as an empty stdout, which is what FR-10.7 has a caller
        // read as a bolt that died.
        println!("{}", result.display());
    } else {
        eprintln!(
            "bolt: no result was written, because that is the directory in question; \
             name --output-dir outside it for one"
        );
    }
    ExitCode::from(REFUSED)
}
