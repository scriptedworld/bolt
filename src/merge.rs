//! Folding every execution's envelope into the run's one result.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::run::{OUTPUT_FILE, RESULT_FILE, WORK_DIR};
use crate::{Error, Outcome};

/// Fold every `work/*/output.yaml` under `output_dir` into one `result.yaml`.
///
/// FR-8.1 gives a run exactly one result, and has the merge read every
/// envelope, fold them mechanically, and do so repeatably over a finished
/// directory: folding twice gives the same file, not merely the same verdict.
///
/// FR-8.3 passes the merged result only when every constituent passes. There is
/// no constituent whose failure does not count, because a check nobody wants
/// enforced is a check not in the jig.
///
/// # Errors
///
/// [`Error::NoConstituents`] when the fold finds none, by FR-8.3a. FR-8.3 on
/// its own would pass such a run, because every constituent passing holds
/// vacuously when there are none, and a green result over zero checks is read
/// as checked and fine.
///
/// [`Error::Io`] when the result cannot be written.
pub fn merge(output_dir: &Path) -> Result<Outcome, Error> {
    let work = output_dir.join(WORK_DIR);
    let mut entries: Vec<_> = fs::read_dir(&work)
        .map_err(|source| Error::Io {
            path: work.clone(),
            reason: source.to_string(),
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join(OUTPUT_FILE).is_file())
        .collect();

    // Sorted, so the evidence mapping is the same on every fold over one
    // directory. FR-8.1's repeatability is about the file rather than about the
    // verdict, and a directory read order is not stable.
    entries.sort();

    if entries.is_empty() {
        return Err(Error::NoConstituents);
    }

    let folded = fold(&entries)?;
    let success = folded.reasons.is_empty();

    let mut result = json!({
        "success": success,
        "metadata": { "evidence": Value::Object(folded.evidence) },
    });
    if !success {
        result["reasons"] = Value::Array(folded.reasons);
    }
    let path = output_dir.join(RESULT_FILE);
    crate::run::save(&path, &result, &wrench::ENVELOPE_SCHEMA)?;

    Ok(Outcome {
        success,
        output_dir: output_dir.to_path_buf(),
        executions: entries.len(),
    })
}

/// What the fold found across every constituent.
struct Folded {
    /// FR-8.2's mapping from each execution to the envelope it produced.
    evidence: serde_json::Map<String, Value>,
    /// One entry per failing constituent, empty when every one passed.
    reasons: Vec<Value>,
}

/// Read every constituent and collect what the result is built from.
///
/// FR-8.4 has the merged result say which constituents failed, so a reader is
/// not sent to open every work directory to find out. The envelope schema
/// requires `reasons` whenever success is false, so a fold that only flipped
/// the boolean would not validate.
fn fold(entries: &[std::path::PathBuf]) -> Result<Folded, Error> {
    let mut evidence = serde_json::Map::new();
    let mut reasons = Vec::new();

    for entry in entries {
        let envelope = load(&entry.join(OUTPUT_FILE))?;
        let name = entry
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        if !envelope
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            reasons.push(json!({
                "kind": "constituent-failed",
                "message": format!("{name} did not pass"),
            }));
        }

        evidence.insert(name, json!(entry.join(OUTPUT_FILE).display().to_string()));
    }

    Ok(Folded { evidence, reasons })
}

/// Read an envelope through wrench, by FR-1.12, validating it on the way in.
fn load(path: &Path) -> Result<Value, Error> {
    let io = |reason: String| Error::Io {
        path: path.to_path_buf(),
        reason,
    };
    wrench::load_formatted_file(
        path.to_str().ok_or_else(|| io("not utf-8".to_owned()))?,
        &wrench::ENVELOPE_SCHEMA,
        &wrench::YamlCodec,
        &wrench::LocalFileIo,
    )
    .map_err(|source| io(source.to_string()))
}
