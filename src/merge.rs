//! Folding every execution's envelope into the run's one result.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::run::{MANIFEST_FILE, OUTPUT_FILE, RESULT_FILE, WORK_DIR};
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
/// FR-8.4 has the merged result carry the reasons its constituents produced, so
/// what failed **and why** is readable from the merged file alone. Synthesising
/// one reason per failure satisfies the first half and loses the second: every
/// failure arrives as the same kind with the same message, and a reader is sent
/// back to the work directories this exists to summarise. It also makes FR-7.10
/// unsatisfiable, since that row distinguishes a task that could not execute
/// from one that executed and failed **by the kind**, and there is only one kind
/// left to read.
///
/// A constituent that failed while carrying no reason of its own still
/// contributes one, because the envelope schema requires `reasons` whenever
/// success is false and a fold that only flipped the boolean would not
/// validate. That case is an adapter that wrote an envelope FR-6.11 would have
/// caught, so the reason says the constituent failed without saying why rather
/// than pretending to know.
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
            reasons.extend(carried(&envelope, &name));
        }

        evidence.insert(name, reference(entry));
    }

    Ok(Folded { evidence, reasons })
}

/// One execution's entry in FR-8.2's evidence mapping.
///
/// FR-8.2 wants a mapping keyed by task rather than a list of paths, each entry
/// carrying that task's args and the filepath of its own result. FR-8.2a settles
/// where each half comes from and **neither is the envelope**: the key from the
/// work directory name, which FR-3.3 prefixes with the task, and the args from
/// that execution's manifest, which FR-9.5c already records. That keeps FR-6.2's
/// adapter contract as narrow as it is, since an adapter never has to know what
/// task it was run for.
///
/// FR-8.8 makes `args` the argv **as executed, after substitution**, so the
/// merged file says what ran rather than what was written. The manifest's
/// `command` is exactly that.
///
/// A work directory with no readable manifest still gets an entry, carrying its
/// result and no args. Losing the whole merge over it would discard every other
/// constituent's evidence to report a missing field in one of them.
fn reference(entry: &Path) -> Value {
    let result = entry.join(OUTPUT_FILE).display().to_string();
    let args = load_manifest(&entry.join(MANIFEST_FILE))
        .ok()
        .and_then(|manifest| {
            manifest
                .get("command")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });

    match args {
        Some(args) => json!({ "args": args, "result": result }),
        None => json!({ "result": result }),
    }
}

/// Read an execution's manifest through wrench, validating it on the way in.
fn load_manifest(path: &Path) -> Result<Value, Error> {
    read(path, &wrench::MANIFEST_SCHEMA)
}

/// Read a structured file through wrench, by FR-1.12, validating it on the way in.
fn read(path: &Path, schema: &dyn wrench::Schema) -> Result<Value, Error> {
    let io = |reason: String| Error::Io {
        path: path.to_path_buf(),
        reason,
    };
    wrench::load_formatted_file(
        path.to_str().ok_or_else(|| io("not utf-8".to_owned()))?,
        schema,
        &wrench::YamlCodec,
        &wrench::LocalFileIo,
    )
    .map_err(|source| io(source.to_string()))
}

/// One failing constituent's reasons, as the merged result should carry them.
///
/// The constituent's own reasons where it produced any, so its `kind` and
/// `message` reach the merged file unaltered. FR-8.5 keeps the envelope on disk
/// too, so this is a copy rather than a move, and a reader who wants the
/// untouched original still has it.
fn carried(envelope: &Value, name: &str) -> Vec<Value> {
    let own: Vec<Value> = envelope
        .get("reasons")
        .and_then(Value::as_array)
        .map(|reasons| reasons.iter().filter(|r| r.is_object()).cloned().collect())
        .unwrap_or_default();

    if own.is_empty() {
        return vec![json!({
            "kind": "constituent-failed",
            "message": format!("{name} failed and gave no reason"),
        })];
    }
    own
}

/// Read an envelope through wrench, by FR-1.12, validating it on the way in.
fn load(path: &Path) -> Result<Value, Error> {
    read(path, &wrench::ENVELOPE_SCHEMA)
}
