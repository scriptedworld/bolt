//! The walking skeleton: one jig, one directory, end to end.
//!
//! One file, not one per concern. A shared `tests/common/mod.rs` compiles into
//! every test binary separately, and its helpers are then dead code in each
//! binary that does not call them. Under `-D warnings` that fails the gate, and
//! the usual fix is an `allow` attribute, which hard rule 4 turns into a
//! question instead of an edit.
//!
//! Envelopes and manifests are read through wrench, by FR-1.12, which also
//! validates them against their schemas on the way in. A substring check over
//! the raw text cannot tell `success: true` from a command whose name happens
//! to be `true`.

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;

// ---- fixtures ---------------------------------------------------------------

/// An empty fixture tree.
fn tree() -> TempDir {
    TempDir::new().expect("fixture tree")
}

/// Write `contents` to `relative` under `root`, creating parents.
fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent directory");
    }
    fs::write(path, contents).expect("fixture file");
}

/// Write `bolt.<name>.yaml` into `root`, which is the config directory.
///
/// FR-3.9 names a jig file that way and has a jig spoken of by its name, so
/// every test here passes a name to `run` and never a path.
fn write_jig(root: &Path, name: &str, tasks: &str) {
    write(
        root,
        &bolt::jig::file_name(name),
        &format!("version: \"1.0.0\"\ntasks:\n{tasks}"),
    );
}

/// Bolt's built binary, for the tests that are about the invocation itself.
fn bolt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bolt"))
}

/// Paths relative to `root`, for comparing against a fixture's own names.
fn under(root: &Path, paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| {
            path.strip_prefix(root)
                .expect("the walk stays inside the base")
                .display()
                .to_string()
        })
        .collect()
}

/// Read a structured file through wrench, validating it against `schema`.
fn read_validated(path: &Path, schema: &dyn wrench::Schema) -> Value {
    wrench::load_formatted_file(
        path.to_str().expect("a utf-8 fixture path"),
        schema,
        &wrench::YamlCodec,
        &wrench::LocalFileIo,
    )
    .unwrap_or_else(|error| panic!("{} is not a valid document: {error}", path.display()))
}

/// The `success` field of an envelope or a result, as a boolean.
fn verdict(path: &Path, schema: &dyn wrench::Schema) -> bool {
    read_validated(path, schema)
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("{} has no boolean success", path.display()))
}

/// The work directory for one execution of `task`.
fn work(outcome: &bolt::Outcome, entry: &str) -> PathBuf {
    outcome.output_dir.join(bolt::run::WORK_DIR).join(entry)
}

// ---- runs and files ---------------------------------------------------------

// COVERS FR-1.1 | property
/// A failing run's outcome is in its files, and stdout says only where.
///
/// Asserted through the binary, since the streams are the binary's. Stdout
/// carrying anything beyond the result path would be something a consumer could
/// only learn by reading it, and stderr is empty for a run bolt carried out.
#[test]
fn what_a_run_concluded_is_in_its_files_and_not_its_streams() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: fails\n    command: \"sh -c 'echo broken; exit 3'\"\n",
    );

    let finished = bolt()
        .arg("check")
        .arg(root.path())
        .output()
        .expect("bolt runs");
    let stdout = String::from_utf8_lossy(&finished.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    assert_eq!(lines.len(), 1, "stdout is one line: {stdout:?}");
    assert!(
        finished.stderr.is_empty(),
        "a run bolt carried out writes nothing to stderr: {}",
        String::from_utf8_lossy(&finished.stderr),
    );

    let result = Path::new(lines[0]);
    assert!(
        !verdict(result, &wrench::schemas::ENVELOPE),
        "the result says the run failed",
    );
    let reasons = reasons_in(result);
    assert!(
        !reasons.is_empty(),
        "the result says why the run failed: {reasons:?}",
    );
    let captured = fs::read_to_string(
        result
            .with_file_name(bolt::run::WORK_DIR)
            .join("fails-1/stdout"),
    )
    .expect("the command's own output is on disk");
    assert_eq!(captured, "broken\n", "the command's stdout was not kept");
}

// COVERS FR-1.3 | positive
/// A jig that transforms files and judges nothing is an ordinary run.
#[test]
fn a_jig_reaching_no_verdict_about_code_is_a_run() {
    let root = tree();
    write(root.path(), "a.txt", "one\ntwo\n");
    write(root.path(), "b.txt", "three\n");
    write_jig(
        root.path(),
        "count",
        "  - name: lines\n    command: \"sh -c 'cat \\\"$@\\\" | wc -l' sh {all_paths}\"\n    matching: [\"*.txt\"]\n",
    );

    let outcome = bolt::run::run("count", root.path()).expect("the run completes");

    assert!(
        outcome.success,
        "a run producing data and no verdict passes"
    );
    let counted = fs::read_to_string(work(&outcome, "lines-1").join("stdout"))
        .expect("the extracted data is kept");
    assert_eq!(counted.trim(), "3", "the data is what the command produced");
}

// COVERS FR-1.6 | negative
/// A jig that will not parse and a jig of the wrong shape fail different steps.
///
/// Both are refused as unreadable, and the reason says which step refused it: a
/// parse failure names where in the text it stopped, and a schema failure names
/// where in the decoded structure the mismatch is.
#[test]
fn parsing_and_validating_are_two_refusals() {
    let unparseable = tree();
    write(unparseable.path(), &bolt::jig::file_name("x"), "tasks: [\n");
    let misshapen = tree();
    write(misshapen.path(), &bolt::jig::file_name("x"), "tasks: 5\n");

    let reason = |base: &Path| match bolt::run::run("x", base) {
        Err(bolt::Error::JigUnreadable { reason, .. }) => reason,
        other => panic!("a bad jig was not refused as unreadable: {other:?}"),
    };

    let parsing = reason(unparseable.path());
    let validating = reason(misshapen.path());
    assert!(
        parsing.contains("parsing") && parsing.contains("line"),
        "the parse step did not refuse the unparseable jig: {parsing}",
    );
    assert!(
        validating.contains("validating") && validating.contains("'/tasks'"),
        "the schema step did not refuse the misshapen jig: {validating}",
    );
}

// COVERS FR-1.7 | edge
/// A command that will not parse as shell is still a string, and the jig loads.
///
/// The unbalanced quote is the shell's to reject when the task executes. The
/// schema passes it, so the run is carried out and the task fails.
#[test]
fn a_command_the_shell_cannot_parse_still_validates() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: unbalanced\n    command: \"sh -c 'echo unterminated\"\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the jig validates and runs");

    assert_eq!(outcome.executions, 1, "the task executed");
    assert!(
        !outcome.success,
        "the shell rejected the command at run time"
    );
}

// COVERS FR-1.8 | property
/// Every structured file a run writes reads back through its schema.
///
/// A passing task, a failing one, and one whose adapter wrote nothing, so the
/// envelopes bolt writes for itself are among what is read, not only the ones an
/// adapter produced.
#[test]
fn every_structured_file_a_run_writes_reads_back() {
    let root = tree();
    write(root.path(), "a.txt", "content");
    write_adapter(root.path(), "silent", "exit 0");
    write_jig(
        root.path(),
        "mixed",
        concat!(
            "  - name: passes\n    command: \"sh -c 'exit 0' {each_path}\"\n    matching: [\"*.txt\"]\n",
            "  - name: fails\n    command: \"sh -c 'exit 1'\"\n",
            "  - name: quiet\n    command: \"sh -c 'exit 0'\"\n    adapter: silent\n",
        ),
    );

    let outcome = bolt::run::run("mixed", root.path()).expect("the run completes");

    let mut read = 0;
    read_validated(
        &outcome.output_dir.join(bolt::run::RESULT_FILE),
        &wrench::schemas::ENVELOPE,
    );
    for entry in fs::read_dir(outcome.output_dir.join(bolt::run::WORK_DIR)).expect("work") {
        let dir = entry.expect("a work directory").path();
        read_validated(
            &dir.join(bolt::run::OUTPUT_FILE),
            &wrench::schemas::ENVELOPE,
        );
        read_validated(
            &dir.join(bolt::run::MANIFEST_FILE),
            &wrench::schemas::MANIFEST,
        );
        read += 1;
    }
    assert_eq!(read, 3, "every execution's files were read");
}

// COVERS FR-1.9a | edge
/// What a command writes is kept as it was, with no schema applied to it.
///
/// Stdout that is broken YAML and an artifact of raw bytes would each be
/// refused if bolt read them as data. The run passes and both are kept byte for
/// byte.
#[test]
fn captured_output_and_artifacts_are_not_read_as_data() {
    let root = tree();
    write_jig(
        root.path(),
        "raw",
        concat!(
            "  - name: noisy\n",
            "    command: \"sh -c 'printf \\\"tasks: [\\\\n\\\"; printf \\\"\\\\377\\\\000\\\" > {work_dir}/blob.bin'\"\n",
            "    evidence: [\"blob.bin\"]\n",
        ),
    );

    let outcome = bolt::run::run("raw", root.path()).expect("the run completes");

    assert!(outcome.success, "unstructured output failed the run");
    let dir = work(&outcome, "noisy-1");
    assert_eq!(
        fs::read(dir.join("stdout")).expect("stdout kept"),
        b"tasks: [\n",
        "stdout was altered",
    );
    assert_eq!(
        fs::read(dir.join("blob.bin")).expect("the artifact kept"),
        [0o377, 0],
        "the artifact was altered",
    );
}

// COVERS FR-1.10 | property
/// Strings that look like other types are quoted, and booleans are not.
///
/// `no`, `1.20` and `null` pass through the manifest as definitions values, so
/// they reach disk in a file bolt wrote. The raw text is read as well as the
/// decoded structure, because a reader that forgives an unquoted `no` would
/// pass a file another reader takes for `false`.
#[test]
fn canonical_yaml_keeps_every_scalar_its_type() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("types"),
        concat!(
            "definitions:\n  answer: \"no\"\n  release: \"1.20\"\n  nothing: \"null\"\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0' {answer} {release} {nothing}\"\n",
        ),
    );

    let outcome = bolt::run::run("types", root.path()).expect("the run completes");

    let path = work(&outcome, "alpha-1").join(bolt::run::MANIFEST_FILE);
    let text = fs::read_to_string(&path).expect("the manifest");
    for quoted in ["\"no\"", "\"1.20\"", "\"null\""] {
        assert!(text.contains(quoted), "{quoted} is not quoted:\n{text}");
    }
    assert!(
        !text.contains('{'),
        "the manifest is not block style:\n{text}"
    );

    let variables = read_validated(&path, &wrench::schemas::MANIFEST)["variables"].clone();
    for (key, value) in [("answer", "no"), ("release", "1.20"), ("nothing", "null")] {
        assert_eq!(
            variables[key]["value"].as_str(),
            Some(value),
            "{key} did not survive as a string",
        );
    }

    let result =
        fs::read_to_string(outcome.output_dir.join(bolt::run::RESULT_FILE)).expect("the result");
    assert!(
        result.lines().any(|line| line == "\"success\": true"),
        "success is not a bare boolean:\n{result}",
    );
}

// COVERS FR-1.13 | positive
/// Validation needs nothing from the machine: no `PATH`, no home, no network.
///
/// The schema refusal proves the schema was there to apply. Bolt ships its
/// schemas inside the binary, so an empty environment reaches the same
/// validating step as a full one.
#[test]
fn validation_needs_nothing_installed() {
    let root = tree();
    write(root.path(), &bolt::jig::file_name("x"), "tasks: 5\n");

    let finished = bolt()
        .env_clear()
        .current_dir(root.path())
        .arg("x")
        .arg(root.path())
        .output()
        .expect("bolt runs");

    let stderr = String::from_utf8_lossy(&finished.stderr);
    assert_eq!(finished.status.code(), Some(1), "the jig was not refused");
    assert!(
        stderr.contains("validating") && stderr.contains("'/tasks'"),
        "the schema was not applied: {stderr}",
    );
}

// COVERS FR-1.9, FR-1.12 | property
/// Bolt reads and writes no structured file itself, so wrench is the only route.
///
/// Two halves. The manifest takes wrench from the path FR-1.12 names and no
/// YAML crate of bolt's own, and no source file reaches a JSON codec's text,
/// byte or stream entry points. `serde_json::from_value` stays allowed: it
/// derives a type off a value wrench already validated, and reads no file.
#[test]
fn every_structured_file_goes_through_wrench() {
    let manifest = fs::read_to_string(repository().join("Cargo.toml")).expect("Cargo.toml");
    let wrench = manifest
        .lines()
        .find(|line| line.starts_with("wrench = "))
        .expect("the manifest names wrench");
    assert!(
        wrench.contains("path = \"../wrench/rust\""),
        "wrench is not taken from ../wrench/rust: {wrench}",
    );
    let yaml: Vec<&str> = manifest
        .lines()
        .filter(|line| {
            !line.starts_with('#')
                && line
                    .split('=')
                    .next()
                    .is_some_and(|name| name.contains("yaml"))
        })
        .collect();
    assert!(
        yaml.is_empty(),
        "bolt names a YAML crate of its own: {yaml:?}"
    );

    let codec = [
        "from_str",
        "from_slice",
        "from_reader",
        "to_string",
        "to_string_pretty",
        "to_vec",
        "to_writer",
    ];
    for entry in fs::read_dir(repository().join("src"))
        .expect("src")
        .filter_map(Result::ok)
    {
        let source = fs::read_to_string(entry.path()).expect("a source file");
        for name in codec {
            let called = source.contains(&format!("serde_json::{name}("));
            let imported = source
                .lines()
                .filter(|line| line.trim_start().starts_with("use serde_json"))
                .any(|line| {
                    line.split(|c: char| !c.is_alphanumeric() && c != '_')
                        .any(|word| word == name)
                });
            assert!(
                !called && !imported,
                "{} reaches serde_json::{name} without wrench",
                entry.path().display(),
            );
        }
    }
}

// ---- the invocation ---------------------------------------------------------

// COVERS FR-2.1, FR-2.1a, FR-3.9 | positive
/// An invocation names a jig and a directory, and that is the whole of it.
///
/// This asserts the run actually happened, not merely that the process was
/// happy: a `cli::main` that checked its argument count and returned would
/// satisfy an exit-status assertion while never reading the jig.
#[test]
fn an_invocation_is_a_jig_name_and_a_directory() {
    let root = tree();
    write(root.path(), "a.txt", "content");
    write_jig(
        root.path(),
        "check",
        "  - name: passes\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt()
        .arg("check")
        .arg(root.path())
        .output()
        .expect("bolt runs");

    assert_eq!(
        outcome.status.code(),
        Some(0),
        "two arguments is a complete invocation: {}",
        String::from_utf8_lossy(&outcome.stderr),
    );

    let runs: Vec<_> = fs::read_dir(root.path())
        .expect("the base is readable")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(".bolt-"))
        .collect();
    assert_eq!(runs.len(), 1, "a run writes exactly one run directory");

    let result = runs[0].path().join(bolt::run::RESULT_FILE);
    assert!(
        verdict(&result, &wrench::schemas::ENVELOPE),
        "a jig whose only task passes produces a passing result",
    );
}

// COVERS FR-2.1a, FR-10.5 | negative
/// A third argument asks for an interface bolt does not have.
///
/// The status is asserted as exactly 1, not as "not success": a `todo!()` panic
/// exits 101 and would satisfy the weaker form, so it could not tell a refusal
/// from a crash. The 1 is FR-10.5's, which is why that row is cited alongside.
#[test]
fn a_third_argument_is_refused() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: passes\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt()
        .arg("check")
        .arg(root.path())
        .arg(root.path())
        .output()
        .expect("bolt runs");

    assert_eq!(
        outcome.status.code(),
        Some(1),
        "a third argument is refused with 1, not with whatever went wrong",
    );
}

// COVERS FR-10.1, FR-10.2, FR-10.5 | property
/// The exit status says whether bolt ran, not whether the tools passed.
#[test]
fn the_exit_status_says_whether_bolt_ran_not_whether_tools_passed() {
    let root = tree();
    write_jig(
        root.path(),
        "failing",
        "  - name: fails\n    command: \"sh -c 'exit 3'\"\n",
    );

    let completed = bolt()
        .arg("failing")
        .arg(root.path())
        .output()
        .expect("bolt runs");
    assert_eq!(
        completed.status.code(),
        Some(0),
        "a completed run exits 0 whatever the tools concluded",
    );

    let refused = bolt()
        .arg("failing")
        .arg(root.path().join("not-there"))
        .output()
        .expect("bolt runs");
    assert_eq!(
        refused.status.code(),
        Some(1),
        "a run bolt could not carry out exits 1",
    );
}

// COVERS FR-10.5 | negative
/// A jig that will not parse is a refusal, not a crash.
#[test]
fn a_jig_that_will_not_parse_is_refused() {
    let root = tree();
    write(root.path(), &bolt::jig::file_name("broken"), "tasks: [\n");

    let refusal = bolt::run::run("broken", root.path()).expect_err("a broken jig is refused");

    assert!(
        matches!(refusal, bolt::Error::JigUnreadable { .. }),
        "wrong refusal for an unparseable jig: {refusal:?}",
    );
}

// COVERS FR-1.5, FR-3.9 | edge
/// A jig with no `version` is valid, because the schema requires only `tasks`.
///
/// Bolt validates what wrench's schema says, not what bolt would have chosen.
/// Requiring `version` here refuses six of the estate's jigs, bolt's own among
/// them.
#[test]
fn a_jig_without_a_version_is_read() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("bare"),
        "tasks:\n  - name: passes\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("bare", root.path()).expect("a jig with no version is valid");

    assert!(outcome.success, "the run did not complete");
    assert_eq!(outcome.executions, 1, "the task did not execute");
}

// COVERS FR-5.22, FR-10.5 | negative
/// A task carrying the retired `jig` field is refused by name, and told what
/// replaced it.
///
/// The message is the whole point of the row. Serde's `missing field command`
/// reads as a malformed task and invites somebody to add a command to one that
/// already meant to run a jig, which is the wrong repair. FR-5.18 makes the
/// right one a command line, so the refusal spells it out.
///
/// Asserted on the text as well as the variant. A reader meeting this has a jig
/// written against a mechanism that no longer exists, and the variant name
/// reaches nobody. Wrench's real jig has two of these in its gate.
#[test]
fn a_task_carrying_the_retired_jig_field_is_refused_by_name() {
    let root = tree();
    write_jig(
        root.path(),
        "nested",
        "  - name: child\n    jig: common-quality\n",
    );

    let refusal = bolt::run::run("nested", root.path()).expect_err("the jig field is retired");

    let bolt::Error::TaskNamesAJig { task } = &refusal else {
        panic!("wrong refusal for a task naming a jig: {refusal:?}");
    };
    assert_eq!(task, "child", "the refusal named the wrong task");

    let said = refusal.to_string();
    assert!(
        said.contains("retired") && said.contains("bolt <jig> <directory>"),
        "the refusal did not say what replaced the field: {said}",
    );
}

// COVERS FR-4.3, FR-2.3 | regression
/// A filename containing a template token is not re-expanded into its own
/// substitution.
///
/// Bolt had a working remote code execution here, from substitution chaining
/// `str::replace` once per variable. A path spliced in for `{each_path}` that
/// contained the literal text `{all_paths}` had that token expanded by the next
/// replace. The second expansion spliced a fresh quoted string into the middle
/// of the already-quoted region, broke the quoting, and put the rest of the
/// filename on the command line unquoted. A filename built that way ran `id`
/// and escaped the base to write a file beside it, while the run reported
/// success.
/// `docs/LESSONS/chained-substitution-is-a-command-injection.md` has the rest.
///
/// `quote` was correct throughout. FR-4.3 is not a property of the quoting
/// alone; it needs substituted bytes never to be read again.
#[test]
fn a_filename_containing_a_template_token_is_not_re_expanded() {
    let root = tree();
    let canary = root.path().join("PWNED");
    write(root.path(), "p{all_paths};id > PWNED #", "x");
    write_jig(
        root.path(),
        "inject",
        "  - name: t\n    matching: [\"p*\"]\n    command: \"echo {each_path}\"\n",
    );

    let outcome = bolt::run::run("inject", root.path()).expect("the run completes");

    let stdout =
        fs::read_to_string(work(&outcome, "t-1").join("stdout")).expect("the task wrote stdout");
    assert_eq!(
        stdout.trim(),
        root.path()
            .join("p{all_paths};id > PWNED #")
            .display()
            .to_string(),
        "the path did not reach the command intact",
    );
    assert!(!canary.exists(), "the filename injected a command");
}

// COVERS FR-4.18 | negative
/// A placeholder nothing defines is refused before anything executes.
#[test]
fn an_unknown_placeholder_is_refused() {
    let root = tree();
    write_jig(
        root.path(),
        "undefined",
        "  - name: t\n    command: \"check {requirements}\"\n",
    );

    let refusal = bolt::run::run("undefined", root.path()).expect_err("nothing defines it");

    match refusal {
        bolt::Error::UnknownPlaceholder { task, placeholder } => {
            assert_eq!(task, "t");
            assert_eq!(placeholder, "requirements", "the reason must name it");
        }
        other => panic!("wrong refusal for an unknown placeholder: {other:?}"),
    }
}

// COVERS FR-3.3a, FR-8.3 | regression
/// Two tasks sharing a name are refused, because the second would erase the
/// first's evidence and its failure with it.
///
/// Reproduced before this check existed: a jig with two tasks named `lint`, the
/// first failing and the second passing, produced `success: true` and exit 0.
/// The failing task vanished from the fold entirely.
#[test]
fn a_duplicate_task_name_is_refused() {
    let root = tree();
    write_jig(
        root.path(),
        "twice",
        concat!(
            "  - name: lint\n    command: \"sh -c 'exit 1'\"\n",
            "  - name: lint\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let refusal = bolt::run::run("twice", root.path()).expect_err("a duplicate name is refused");

    assert!(
        matches!(refusal, bolt::Error::DuplicateTaskName { .. }),
        "wrong refusal for a duplicate task name: {refusal:?}",
    );
}

// COVERS FR-2.3, FR-9.2 | regression
/// A task name that would climb out of the work directory is refused.
///
/// The name is a path component by FR-3.3. Reproduced before this check: a task
/// named `../../../victim/EVIL` wrote a complete evidence directory outside the
/// base and outside the run's output directory.
#[test]
fn a_task_name_that_leaves_the_work_directory_is_refused() {
    let root = tree();
    write_jig(
        root.path(),
        "escape",
        "  - name: ../../escaped\n    command: \"sh -c 'exit 0'\"\n",
    );

    let refusal = bolt::run::run("escape", root.path()).expect_err("the name would climb out");

    assert!(
        matches!(refusal, bolt::Error::UnsafeTaskName { .. }),
        "wrong refusal for a name leaving the work directory: {refusal:?}",
    );
}

// COVERS FR-2.5, FR-10.7a | negative
/// A base that is not there is refused, and nothing is created.
///
/// FR-2.5a is deliberately not cited. It gives every refusal a
/// `result.yaml`, and FR-10.7a exempts exactly this case: the default output
/// directory sits at the base, so writing the result would create the base
/// whose absence is being refused. Bolt says so on stderr and writes none.
#[test]
fn a_missing_base_is_refused_and_nothing_is_created() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: passes\n    command: \"sh -c 'exit 0'\"\n",
    );
    let absent = root.path().join("not-there");

    let refusal = bolt::run::run("check", &absent).expect_err("a missing base is refused");

    assert!(
        matches!(refusal, bolt::Error::BaseMissing(_)),
        "wrong refusal for a missing base: {refusal:?}",
    );
    assert!(
        !absent.exists(),
        "the base was created by the run that was refusing it",
    );
    let strays: Vec<_> = fs::read_dir(root.path())
        .expect("the fixture root is readable")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(".bolt-"))
        .collect();
    assert!(
        strays.is_empty(),
        "a refused run left a run directory behind: {strays:?}",
    );
}

// ---- jigs and tasks ---------------------------------------------------------

// COVERS FR-3.2 | positive
/// A task using every field it may declare loads and runs as each one says.
///
/// The adapter passes only if the evidence file lists the kept path and not the
/// excluded one, so `matching`, `excluding`, `evidence` and `adapter` each have
/// to have done their part for the task to pass.
#[test]
fn a_task_using_every_field_at_once_runs() {
    let root = tree();
    write(root.path(), "keep.txt", "k");
    write(root.path(), "skip.txt", "s");
    write_adapter(
        root.path(),
        "listed",
        concat!(
            "for a in \"$@\"; do case $prev in --work-dir) w=$a;; esac; prev=$a; done\n",
            "if grep -q keep.txt \"$w/list.txt\" && ! grep -q skip.txt \"$w/list.txt\"; then\n",
            "  printf '\"success\": true\\n' > \"$w/output.yaml\"\n",
            "else\n",
            "  printf '\"success\": false\\n\"reasons\":\\n  - \"kind\": \"wrong-list\"\\n    \"message\": \"x\"\\n' > \"$w/output.yaml\"\n",
            "fi\n",
        ),
    );
    write_jig(
        root.path(),
        "full",
        concat!(
            "  - name: full\n",
            "    description: every field a task may declare\n",
            "    command: \"sh -c 'echo \\\"$@\\\" > {work_dir}/list.txt' sh {all_paths}\"\n",
            "    matching: [\"*.txt\"]\n",
            "    excluding: [\"skip.txt\"]\n",
            "    adapter: listed\n",
            "    evidence: [\"list.txt\"]\n",
            "    short-circuit-failure: true\n",
        ),
    );

    let outcome = bolt::run::run("full", root.path()).expect("the run completes");

    assert_eq!(outcome.executions, 1, "the task executed once");
    assert!(
        outcome.success,
        "a field did not do its part: {:?}",
        reasons_in(&work(&outcome, "full-1").join(bolt::run::OUTPUT_FILE)),
    );
}

// COVERS FR-3.3 | property
/// Every work directory starts with the name of the task that made it.
#[test]
fn a_tasks_name_prefixes_its_work_directories() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write(root.path(), "b.txt", "b");
    write_jig(
        root.path(),
        "named",
        concat!(
            "  - name: lint\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
            "  - name: fmt\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let outcome = bolt::run::run("named", root.path()).expect("the run completes");

    let mut names: Vec<String> = fs::read_dir(outcome.output_dir.join(bolt::run::WORK_DIR))
        .expect("work")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["fmt-1", "lint-1", "lint-2"], "work directories");
}

// COVERS FR-3.4c | positive
/// A jig carries comments beside its entries, and they change nothing.
///
/// One sits inside the `excluding` list, which is where the row says the reason
/// for an exclusion belongs.
#[test]
fn a_jig_carries_comments_beside_its_entries() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write(root.path(), "vendored.txt", "v");
    write(
        root.path(),
        &bolt::jig::file_name("commented"),
        concat!(
            "# The whole-tree check.\n",
            "tasks:\n",
            "  - name: read # one execution per file\n",
            "    command: \"cat {each_path}\"\n",
            "    matching: [\"*.txt\"]\n",
            "    excluding:\n",
            "      # Vendored, and checked upstream.\n",
            "      - vendored.txt\n",
        ),
    );

    let outcome = bolt::run::run("commented", root.path()).expect("a commented jig runs");

    assert!(outcome.success, "the run failed");
    assert_eq!(
        outcome.executions, 1,
        "the excluded file was read, or nothing was"
    );
}

// COVERS FR-3.4e | edge
/// A tool that finds its own files and finds none passes, and bolt cannot tell.
///
/// The same empty selection made by bolt fails, by FR-4.4b. The pair is the
/// row: bolt's guarantee reaches only the selections bolt makes.
#[test]
fn an_empty_selection_is_only_caught_where_bolt_selects() {
    let run = |task: &str| {
        let root = tree();
        write(root.path(), "a.txt", "a");
        write_jig(root.path(), "check", task);
        bolt::run::run("check", root.path()).expect("the run completes")
    };

    let opaque = run("  - name: finds\n    command: \"sh -c 'find . -name \\\"*.go\\\"'\"\n");
    let selected =
        run("  - name: finds\n    command: \"cat {each_path}\"\n    matching: [\"**/*.go\"]\n");

    assert!(
        opaque.success,
        "a tool that matched nothing on its own was noticed"
    );
    assert!(
        !selected.success,
        "bolt's own empty selection was not caught"
    );
}

// COVERS FR-3.7 | positive
/// A jig kept outside the tree and linked in runs without being copied.
///
/// The link's target is edited after it is made, and the run sees the edit, so
/// what ran is the file outside the tree and not a copy of it.
#[test]
fn a_linked_jig_runs_from_where_it_lives() {
    let shared = tree();
    let root = tree();
    write_jig(
        shared.path(),
        "shared",
        "  - name: old\n    command: \"sh -c 'exit 1'\"\n",
    );
    let name = bolt::jig::file_name("shared");
    unix_fs::symlink(shared.path().join(&name), root.path().join(&name)).expect("the link");
    write_jig(
        shared.path(),
        "shared",
        "  - name: new\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("shared", root.path()).expect("a linked jig runs");

    assert!(
        outcome.success,
        "the run did not read the jig through the link"
    );
    assert!(
        work(&outcome, "new-1").exists(),
        "the edit made after linking was not seen"
    );
}

// COVERS FR-3.10e | property
/// A composing jig's `requires` is its own; the child checks its own list.
///
/// The child needs a tool that is absent and the parent does not list it. The
/// parent runs, so it did not gather the child's list, and the child refuses
/// when it runs, so the list was checked where it belongs.
#[test]
fn requires_belongs_to_the_jig_that_declares_it() {
    let root = tree();
    write(root.path(), "sub/a.txt", "a");
    write(
        root.path(),
        &bolt::jig::file_name("inner"),
        concat!(
            "requires: [\"sh\", \"no-such-tool-594a\"]\n",
            "tasks:\n  - name: inner-check\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );
    write(
        root.path(),
        &bolt::jig::file_name("outer"),
        &format!(
            "requires: [\"env\"]\ntasks:\n  - name: subproject\n    command: \"env -u {} {}=3 {} inner \
             {{base_dir}}/sub --config-dir {{config_dir}} --output-dir {{work_dir}}/child\"\n",
            bolt::depth::DEPTH,
            bolt::depth::CEILING,
            env!("CARGO_BIN_EXE_bolt"),
        ),
    );

    let outcome = bolt::run::run("outer", root.path()).expect("the parent is not refused");

    assert_eq!(outcome.executions, 1, "the parent's task executed");
    let child = work(&outcome, "subproject-1")
        .join("child")
        .join(bolt::run::RESULT_FILE);
    let kinds: Vec<String> = reasons_in(&child)
        .into_iter()
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(
        kinds,
        ["requires-missing"],
        "the child did not check its own list"
    );
}

// COVERS FR-3.12 | negative
/// A broken jig beside the one being run is not read, and does not fail it.
#[test]
fn a_broken_sibling_jig_does_not_fail_the_run() {
    let root = tree();
    write(root.path(), &bolt::jig::file_name("broken"), "tasks: [\n");
    write_jig(
        root.path(),
        "good",
        "  - name: passes\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("good", root.path()).expect("a broken sibling is not read");

    assert!(outcome.success, "the run failed");
}

// COVERS FR-3.14 | property
/// Two runs of one jig over one tree show the same tasks.
///
/// The second runs with the tree touched and bolt's environment changed, which
/// is the state a condition read at run time would see. The work directories,
/// which are the tasks as executed, match.
#[test]
fn two_runs_of_one_jig_show_the_same_tasks() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write_jig(
        root.path(),
        "fixed",
        concat!(
            "  - name: each\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
            "  - name: whole\n    command: \"sh -c 'exit 0'\"\n",
            "  - name: maybe\n    command: \"cat {all_paths}\"\n",
            "    matching: [\"*.go\"]\n    optional: true\n",
        ),
    );
    let tasks = |environment: &str| {
        let out = tree();
        bolt()
            .env("CI", environment)
            .arg("fixed")
            .arg(root.path())
            .arg("--output-dir")
            .arg(out.path().join("run"))
            .output()
            .expect("bolt runs");
        let mut names: Vec<String> = fs::read_dir(out.path().join("run").join(bolt::run::WORK_DIR))
            .expect("work")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };

    let first = tasks("");
    write(root.path(), "a.txt", "changed");
    let second = tasks("true");

    assert_eq!(first, ["each-1", "whole-1"], "the first run's tasks");
    assert_eq!(first, second, "the task set moved between runs");
}

/// The work directory names one run wrote, sorted: the tasks as executed.
fn executed(outcome: &bolt::Outcome) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(outcome.output_dir.join(bolt::run::WORK_DIR))
        .expect("work")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// COVERS FR-3.1 | property
/// What bolt executes over a tree is exactly the jig it was handed.
///
/// Two jigs over one unchanged tree. Each run executes its own jig's tasks and
/// none of the other's, so nothing about the tree chose what ran.
#[test]
fn what_runs_is_the_jig_and_nothing_else() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write_jig(
        root.path(),
        "one",
        "  - name: first\n    command: \"sh -c 'exit 0'\"\n  - name: second\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
    );
    write_jig(
        root.path(),
        "two",
        "  - name: third\n    command: \"sh -c 'exit 0'\"\n",
    );

    let out = tree();
    let one = run_into("one", root.path(), &out.path().join("one")).expect("one runs");
    let two = run_into("two", root.path(), &out.path().join("two")).expect("two runs");

    assert_eq!(executed(&one), ["first-1", "second-1"], "jig one");
    assert_eq!(executed(&two), ["third-1"], "jig two");
}

// COVERS FR-3.8 | property
/// A jig run from a shared config directory and the same jig in the project
/// run alike.
///
/// The file is byte for byte the same in both places, so any difference in the
/// tasks executed or the verdict would be bolt treating a shared jig as a
/// different kind of thing from a project's own.
#[test]
fn a_shared_jig_and_a_project_jig_run_alike() {
    let tasks = concat!(
        "  - name: pass\n    command: \"sh -c 'exit 0'\"\n",
        "  - name: fail\n    command: \"sh -c 'exit 3'\"\n",
        "  - name: each\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
    );
    let root = tree();
    let shared = tree();
    let out = tree();
    write(root.path(), "a.txt", "a");
    write(root.path(), "b.txt", "b");
    write_jig(root.path(), "gate", tasks);
    write_jig(shared.path(), "gate", tasks);

    let local =
        run_into("gate", root.path(), &out.path().join("local")).expect("the project jig runs");
    let remote = bolt::run::invoke(&bolt::run::Invocation {
        jig: "gate",
        base: root.path(),
        definitions: None,
        output_dir: Some(&out.path().join("shared")),
        config_dir: Some(shared.path()),
    })
    .expect("the shared jig runs");

    assert_eq!(
        executed(&local),
        ["each-1", "each-2", "fail-1", "pass-1"],
        "the project jig's tasks",
    );
    assert_eq!(
        executed(&local),
        executed(&remote),
        "the two placements ran different tasks"
    );
    assert!(
        !local.success,
        "the failing task did not fail the project jig"
    );
    assert_eq!(local.success, remote.success, "the two placements disagree");
}

// ---- the walk ---------------------------------------------------------------

// COVERS FR-2.2 | positive
/// The walk is the whole input: it finds the files the tasks act on.
///
/// Compared as a set. Sorted order is FR-2.2d's claim and has its own test, so
/// asserting it here would mark FR-2.2 covered by something it does not say.
#[test]
fn the_walk_finds_the_files_tasks_act_on() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    write(root.path(), "nested/b.txt", "two");

    let found = bolt::walk::walk(root.path()).expect("the walk succeeds");

    let mut names = under(root.path(), &found);
    names.sort();
    assert_eq!(
        names,
        ["a.txt", "nested/b.txt"],
        "both files, and nothing else"
    );
}

// COVERS FR-2.2a | negative
/// An ignored file is not part of the project and is not checked.
///
/// Asserts what survives as well as what does not. An empty walk satisfies the
/// absence half on its own.
#[test]
fn an_ignored_file_is_not_walked() {
    let root = tree();
    write(root.path(), ".gitignore", "build/\n*.tmp\n");
    write(root.path(), "kept.txt", "kept");
    write(root.path(), "scratch.tmp", "ignored");
    write(root.path(), "build/output.txt", "ignored");

    let found = bolt::walk::walk(root.path()).expect("the walk succeeds");
    let names = under(root.path(), &found);

    assert!(
        names.contains(&"kept.txt".to_owned()),
        "the walk found nothing: {names:?}"
    );
    assert!(
        !names
            .iter()
            .any(|name| Path::new(name).extension().is_some_and(|ext| ext == "tmp")),
        "an ignored file was walked: {names:?}",
    );
    assert!(
        !names.iter().any(|name| name.starts_with("build/")),
        "an ignored directory was walked: {names:?}",
    );
}

// COVERS FR-2.2f | negative
/// Naming an ignored file in `matching` does not bring it back.
///
/// The literal path is the strongest way a jig can ask for a file, so if any
/// spelling reached it this one would. A second task names a file that is not
/// ignored, so the first matching nothing is shown to be the walk and not the
/// pattern.
#[test]
fn matching_cannot_reach_an_ignored_file() {
    let root = tree();
    write(root.path(), ".gitignore", "generated.txt\n");
    write(root.path(), "generated.txt", "ignored");
    write(root.path(), "kept.txt", "kept");
    write_jig(
        root.path(),
        "reach",
        concat!(
            "  - name: ignored\n    command: \"cat {each_path}\"\n",
            "    matching: [\"generated.txt\"]\n    optional: true\n",
            "  - name: kept\n    command: \"cat {each_path}\"\n",
            "    matching: [\"kept.txt\"]\n",
        ),
    );

    let outcome = bolt::run::run("reach", root.path()).expect("the run completes");

    assert!(
        work(&outcome, "kept-1").exists(),
        "the literal path matched nothing"
    );
    assert!(
        !work(&outcome, "ignored-1").exists(),
        "a task reached a file .gitignore excludes",
    );
    assert_eq!(
        outcome.executions, 1,
        "only the task naming a kept file executed"
    );
}

// COVERS FR-2.9 | property
/// A relative path means the base, wherever it was written and wherever bolt
/// was started.
///
/// Bolt is started one directory up, where a decoy `a.txt` sits, and given the
/// base as a relative argument. The pattern, the definitions value and the path
/// written into the command are all relative, and each reaches the base's own
/// file and never the decoy.
#[test]
fn every_relative_path_resolves_against_the_base() {
    let root = tree();
    write(root.path(), "a.txt", "decoy\n");
    write(root.path(), "sub/a.txt", "found\n");
    write(
        &root.path().join("sub"),
        &bolt::jig::file_name("paths"),
        concat!(
            "definitions:\n  file: a.txt\n",
            "tasks:\n  - name: read\n",
            "    command: \"sh -c 'cat \\\"$1\\\" {file} a.txt' sh {each_path}\"\n",
            "    matching: [\"a.txt\"]\n",
        ),
    );

    let finished = bolt()
        .current_dir(root.path())
        .arg("paths")
        .arg("sub")
        .output()
        .expect("bolt runs");

    let result = String::from_utf8_lossy(&finished.stdout).trim().to_owned();
    let stdout = Path::new(&result)
        .with_file_name(bolt::run::WORK_DIR)
        .join("read-1/stdout");
    assert_eq!(
        fs::read_to_string(&stdout).expect("the task ran"),
        "found\nfound\nfound\n",
        "a relative path reached something other than the base",
    );
}

// COVERS FR-2.2b, FR-2.7 | edge
/// A tree that is not a repository walks the same as one that is.
///
/// The fixture has no `.git` at all, which is what "does not require a
/// repository" means. `ignore`'s `require_git` defaults to true, so at its
/// defaults this tree's `.gitignore` would be inert and `scratch.tmp` would be
/// walked.
#[test]
fn a_tree_that_is_not_a_repository_walks_the_same() {
    let root = tree();
    write(root.path(), ".gitignore", "*.tmp\n");
    write(root.path(), "kept.txt", "kept");
    write(root.path(), "scratch.tmp", "ignored");
    assert!(
        !root.path().join(".git").exists(),
        "the fixture has no repository"
    );

    let found = bolt::walk::walk(root.path()).expect("a tree with no repository walks");
    let names = under(root.path(), &found);

    assert!(
        names.contains(&"kept.txt".to_owned()),
        "the walk found nothing: {names:?}"
    );
    assert!(
        !names.contains(&"scratch.tmp".to_owned()),
        ".gitignore was inert without a repository: {names:?}",
    );
}

// COVERS FR-2.2b | negative
/// Git's own excludes are not read, because bolt reads nothing under `.git/`.
///
/// Only the exclude file is asserted. Whether the walk *returns* paths under
/// `.git/` is a separate question that no row settles, and asserting it here
/// would put an unsupported claim under this row's marker.
#[test]
fn a_git_excludes_file_is_not_read() {
    let root = tree();
    write(root.path(), ".git/info/exclude", "hidden.txt\n");
    write(root.path(), "hidden.txt", "still walked");

    let found = bolt::walk::walk(root.path()).expect("the walk succeeds");
    let names = under(root.path(), &found);

    assert!(
        names.contains(&"hidden.txt".to_owned()),
        "`.git/info/exclude` was read; FR-2.2b says it is not: {names:?}",
    );
}

// COVERS FR-2.2d | property
/// The walk returns sorted paths, so two runs over one tree agree.
///
/// The fixture pins component order against byte order: `nested.txt` and
/// `nested/d.txt` sort differently depending on which is used, so a walk that
/// happens to be sorted by raw string does not pass by accident.
#[test]
fn the_walk_is_sorted_and_repeatable() {
    let root = tree();
    for name in ["c.txt", "a.txt", "nested.txt", "nested/d.txt"] {
        write(root.path(), name, "content");
    }

    let first = bolt::walk::walk(root.path()).expect("the walk succeeds");
    let second = bolt::walk::walk(root.path()).expect("the walk succeeds twice");

    assert_eq!(first.len(), 4, "the walk found the wrong number of files");
    let mut sorted = first.clone();
    sorted.sort();
    assert_eq!(first, sorted, "the walk is not sorted");
    assert_eq!(first, second, "two walks over one tree disagree");
}

// COVERS FR-2.2e | negative
/// A symlink is not followed, so what sits behind one is not walked.
///
/// The row says the walk does not *follow* a symlink, and this asserts exactly
/// that: the file behind a directory symlink does not appear.
///
/// It deliberately does not assert what happens to a symlink to a file.
/// Measured: `ignore` with `follow_links(false)` returns the link itself, so a
/// task handed it reads through to outside the base. Whether
/// FR-2.2e forbids that is question 40 and is open, and a test here would
/// answer it by accident.
#[test]
fn a_symlink_is_not_followed() {
    let outside = tree();
    write(outside.path(), "secret.txt", "outside the base");

    let root = tree();
    write(root.path(), "inside.txt", "inside");
    unix_fs::symlink(outside.path(), root.path().join("dirlink")).expect("fixture symlink");

    let found = bolt::walk::walk(root.path()).expect("the walk succeeds");
    let names = under(root.path(), &found);

    assert!(
        names.contains(&"inside.txt".to_owned()),
        "the walk found nothing: {names:?}"
    );
    assert!(
        !names.iter().any(|name| name.contains("secret.txt")),
        "the walk followed a symlink out of the base: {names:?}",
    );
}

// ---- selection --------------------------------------------------------------

// COVERS FR-3.4, FR-3.4a, FR-3.5 | positive
/// `matching` selects, `excluding` removes from what it selected, and both are
/// matched relative to the base.
///
/// The paths come from a real walk and are therefore absolute. Bare relative
/// names hide the whole defect: measured, the glob `generated.py` does not
/// match `/abs/tmp/x/generated.py`, so a literal `excluding` entry silently
/// removes nothing while `**/*.py` keeps working.
#[test]
fn matching_selects_and_excluding_removes_relative_to_the_base() {
    let root = tree();
    for name in [
        "a.py",
        "generated.py",
        "nested/c.py",
        "nested/generated.py",
        "d.txt",
    ] {
        write(root.path(), name, "content");
    }
    let walked = bolt::walk::walk(root.path()).expect("the walk succeeds");

    let selected = bolt::selection::select(
        root.path(),
        &walked,
        &["**/*.py".to_owned()],
        &["generated.py".to_owned()],
    )
    .expect("the patterns compile");

    assert_eq!(
        under(root.path(), &selected),
        ["a.py", "nested/c.py", "nested/generated.py"],
        "`**` crosses directories, and a literal exclusion removes one path and not its namesake",
    );
}

// COVERS FR-4.3 | negative
/// Every substituted path is quoted individually, against a shell.
///
/// Asserted by round trip, not by shape. `format!("'{}'", path)` is the obvious
/// implementation and passes any starts-with/ends-with check, and a path
/// containing a single quote escapes it: against that implementation, a file
/// named `'a'; touch <path>/PWNED; '.txt'` runs the injected command.
#[test]
fn every_substituted_path_is_quoted_individually() {
    let root = tree();
    let canary = root.path().join("PWNED");
    let hostile = root
        .path()
        .join(format!("a'; touch {}; '.txt", canary.display()));

    let quoted = bolt::selection::quote(&hostile);
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("printf %s {quoted}"))
        .output()
        .expect("the shell runs");

    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        hostile.display().to_string(),
        "the path did not survive the shell unchanged",
    );
    assert!(!canary.exists(), "the quoted path injected a command");
}

// ---- how a task runs --------------------------------------------------------

// COVERS FR-4.2 | positive
/// `{each_path}` runs once per matched path and `{all_paths}` runs once.
///
/// Asserted by which work directories exist, not by a total. Swapping the two
/// forms leaves the count at three and passes a sum-based check.
#[test]
fn each_path_runs_once_per_path_and_all_paths_runs_once() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    write(root.path(), "b.txt", "two");
    write_jig(
        root.path(),
        "forms",
        concat!(
            "  - name: per-path\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"sh -c 'echo {each_path}'\"\n",
            "  - name: one-shot\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"sh -c 'echo {all_paths}'\"\n",
        ),
    );

    let outcome = bolt::run::run("forms", root.path()).expect("the run completes");

    assert!(
        work(&outcome, "per-path-1").is_dir(),
        "the per-path task ran once"
    );
    assert!(
        work(&outcome, "per-path-2").is_dir(),
        "the per-path task ran twice"
    );
    assert!(
        work(&outcome, "one-shot-1").is_dir(),
        "the one-shot task ran"
    );
    assert!(
        !work(&outcome, "one-shot-2").is_dir(),
        "{{all_paths}} ran more than once",
    );

    let one_shot = fs::read_to_string(work(&outcome, "one-shot-1").join("stdout"))
        .expect("the one-shot task wrote stdout");
    assert!(
        one_shot.contains("a.txt") && one_shot.contains("b.txt"),
        "{{all_paths}} did not receive the whole selection: {one_shot}",
    );
}

// COVERS FR-4.2 | negative
/// A command naming both path forms is a jig error, and the reason names it.
#[test]
fn a_command_naming_both_path_forms_is_a_jig_error() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    write_jig(
        root.path(),
        "confused",
        concat!(
            "  - name: names-both\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"sh -c 'echo {each_path} {all_paths}'\"\n",
        ),
    );

    let refusal = bolt::run::run("confused", root.path()).expect_err("naming both is a jig error");

    match refusal {
        bolt::Error::CommandNamesBothPathForms { task } => {
            assert_eq!(task, "names-both", "the refusal named the wrong task");
        }
        other => panic!("wrong refusal for a command naming both forms: {other:?}"),
    }
}

// COVERS FR-4.4, FR-4.4b, FR-4.4e | negative
/// A path-consuming task whose selection is empty fails, and says so.
///
/// FR-4.4b makes this a failure. A silent skip would leave a typo'd pattern
/// green forever. The task still does not execute; it produces a failing
/// constituent instead of nothing.
#[test]
fn a_path_consuming_task_with_an_empty_selection_fails() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    write_jig(
        root.path(),
        "empty",
        concat!(
            "  - name: no-python-here\n",
            "    matching: [\"**/*.py\"]\n",
            "    command: \"sh -c 'echo {each_path}'\"\n",
        ),
    );

    let outcome = bolt::run::run("empty", root.path()).expect("the run completes");

    assert!(!outcome.success, "an empty selection fails the run");
    assert_eq!(outcome.executions, 0, "the task did not execute");
    let envelope = work(&outcome, "no-python-here-1").join(bolt::run::OUTPUT_FILE);
    assert!(
        envelope.is_file(),
        "an empty selection produced no constituent, so nothing folds into the verdict",
    );
    assert!(
        !verdict(&envelope, &wrench::schemas::ENVELOPE),
        "the constituent for an empty selection reports success",
    );
}

// COVERS FR-4.4c | positive
/// `optional` makes an empty selection an acceptable result.
#[test]
fn optional_makes_an_empty_selection_acceptable() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    write_jig(
        root.path(),
        "allowed",
        concat!(
            "  - name: no-python-here\n",
            "    matching: [\"**/*.py\"]\n",
            "    optional: true\n",
            "    command: \"sh -c 'echo {each_path}'\"\n",
            "  - name: always\n",
            "    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let outcome = bolt::run::run("allowed", root.path()).expect("the run completes");

    assert!(
        outcome.success,
        "an allowed empty selection does not fail the run"
    );
    assert!(
        !work(&outcome, "no-python-here-1").exists(),
        "an allowed empty selection produced a constituent",
    );
    assert!(
        work(&outcome, "always-1").is_dir(),
        "the other task still ran"
    );
}

// COVERS FR-4.4 | positive
/// A command naming neither path form always executes.
///
/// The tree is empty, so the walk finds nothing. A task that consumed paths
/// would have an empty selection here; this one never asked for any.
#[test]
fn a_command_naming_no_path_variable_always_executes() {
    let root = tree();
    write_jig(
        root.path(),
        "whole",
        "  - name: always\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("whole", root.path()).expect("the run completes");

    assert_eq!(
        outcome.executions, 1,
        "a task naming no paths executed over an empty tree"
    );
    assert!(outcome.success, "and it passed");
}

// COVERS FR-4.5 | property
/// Tasks execute serially: no two executions overlap in time.
///
/// The row says serially and says nothing about order. FR-4.5a calls serial the
/// simplest thing, not something required, and FR-4.7 says the merged
/// result does not vary with the order tasks ran in, so this asserts non-overlap
/// and not declaration order, which no row states and which is question 38.
///
/// The length assertion is load-bearing: without it a run that executed one task
/// and stopped logs a single matched pair and reports no overlap.
#[test]
fn no_two_executions_overlap() {
    let root = tree();
    let log = root.path().join("order.log");
    write_jig(
        root.path(),
        "serial",
        &format!(
            concat!(
                "  - name: first\n",
                "    command: \"sh -c 'echo enter-first >> {log}; sleep 0.2; echo leave-first >> {log}'\"\n",
                "  - name: second\n",
                "    command: \"sh -c 'echo enter-second >> {log}; sleep 0.2; echo leave-second >> {log}'\"\n",
            ),
            log = log.display(),
        ),
    );

    let outcome = bolt::run::run("serial", root.path()).expect("the run completes");
    assert_eq!(outcome.executions, 2, "both tasks executed");

    let entries: Vec<String> = fs::read_to_string(&log)
        .expect("the tasks wrote the log")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        entries.len(),
        4,
        "an execution did not log both ends: {entries:?}"
    );
    for pair in entries.chunks(2) {
        let [enter, leave] = pair else {
            unreachable!("a length of 4 chunks evenly");
        };
        assert_eq!(
            enter.replace("enter-", ""),
            leave.replace("leave-", ""),
            "an execution began before the previous one finished: {entries:?}",
        );
    }
}

// ---- locations and selection ------------------------------------------------

/// Run `jig` at `base` and return what its one task wrote to stdout.
fn stdout_of(jig: &str, base: &Path, entry: &str) -> (bolt::Outcome, String) {
    let outcome = bolt::run::run(jig, base).expect("the run completes");
    let stdout = fs::read_to_string(work(&outcome, entry).join("stdout")).expect("stdout kept");
    (outcome, stdout)
}

// COVERS FR-4.1, FR-4.1c | positive
/// Every location a task can name is substituted, and each is its own place.
///
/// The outermost run sits at the project root, so `{project_root}` is the base
/// here. The config directory defaults to the base too, and the other two are
/// the run's own.
#[test]
fn every_location_is_a_template_variable() {
    let root = tree();
    write_jig(
        root.path(),
        "where",
        concat!(
            "  - name: show\n    command: \"printf '%s\\\\n' ",
            "{project_root} {base_dir} {work_dir} {config_dir} {output_dir}\"\n",
        ),
    );

    let (outcome, stdout) = stdout_of("where", root.path(), "show-1");

    let base = fs::canonicalize(root.path()).expect("the base");
    let expected = [
        base.clone(),
        base.clone(),
        work(&outcome, "show-1"),
        base,
        outcome.output_dir.clone(),
    ]
    .map(|path| path.display().to_string());
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        expected,
        "project root, base, work, config and output, in that order",
    );
}

// COVERS FR-4.1d | property
/// Template variables are underscored, flags are hyphenated, and a flag and a
/// variable spelled alike name one place.
///
/// Every variable bolt reserves is lower case with underscores. Each location
/// flag maps to the variable its name becomes once hyphens are underscores,
/// and that variable substitutes to the value the flag was given.
#[test]
fn a_flag_and_its_variable_are_one_name_in_two_shapes() {
    for name in bolt::definitions::RESERVED {
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "{{{name}}} is not lower case with underscores",
        );
    }

    let root = tree();
    let config = tree();
    let out = tree();
    write(root.path(), "a.txt", "a");
    write_jig(
        config.path(),
        "say",
        "  - name: say\n    command: \"printf '%s\\\\n' {config_dir} {output_dir}\"\n",
    );
    let run = out.path().join("run");

    let finished = bolt()
        .arg("--config-dir")
        .arg(config.path())
        .arg("--output-dir")
        .arg(&run)
        .arg("say")
        .arg(root.path())
        .output()
        .expect("bolt runs");
    assert!(
        finished.status.success(),
        "{}",
        String::from_utf8_lossy(&finished.stderr),
    );

    let said = fs::read_to_string(run.join(bolt::run::WORK_DIR).join("say-1").join("stdout"))
        .expect("stdout");
    let flags = [("--config-dir", config.path()), ("--output-dir", &run)];
    assert_eq!(
        said.lines().count(),
        flags.len(),
        "one line per flag: {said}"
    );
    for ((flag, given), line) in flags.into_iter().zip(said.lines()) {
        let variable = flag.trim_start_matches("--").replace('-', "_");
        assert!(
            bolt::definitions::RESERVED.contains(&variable.as_str()),
            "{flag} has no {{{variable}}}",
        );
        assert_eq!(
            line,
            fs::canonicalize(given)
                .expect("the flag's path")
                .display()
                .to_string(),
            "{{{variable}}} is not where {flag} pointed",
        );
    }
}

// COVERS FR-4.1a | positive
/// A command stands at the base.
#[test]
fn a_command_runs_at_the_base_directory() {
    let root = tree();
    write_jig(
        root.path(),
        "here",
        "  - name: pwd\n    command: \"pwd -P\"\n",
    );

    let (_, stdout) = stdout_of("here", root.path(), "pwd-1");

    assert_eq!(
        stdout.trim(),
        fs::canonicalize(root.path())
            .expect("the base")
            .display()
            .to_string(),
        "the command stood somewhere other than the base",
    );
}

// COVERS FR-4.2a | edge
/// A command naming no path variable runs once, however many files there are.
#[test]
fn a_command_naming_no_path_variable_runs_once_over_many_files() {
    let root = tree();
    for name in ["a.txt", "b.txt", "c.txt"] {
        write(root.path(), name, name);
    }
    write_jig(
        root.path(),
        "once",
        "  - name: whole\n    command: \"ls\"\n",
    );

    let (outcome, stdout) = stdout_of("once", root.path(), "whole-1");

    assert_eq!(outcome.executions, 1, "the command ran more than once");
    assert!(stdout.contains("c.txt"), "the tree was not there: {stdout}");
}

// COVERS FR-4.4a | edge
/// A task composing bolt obeys the empty-selection rule like any other.
///
/// Its command is bolt, over a subproject that is not there. Required, it fails
/// with bolt's empty-selection reason and bolt never starts; optional, it is
/// satisfied.
#[test]
fn a_composing_task_over_a_missing_subproject_matches_nothing() {
    let run = |optional: &str| {
        let root = tree();
        write(root.path(), "a.txt", "a");
        write_jig(
            root.path(),
            "outer",
            &format!(
                "  - name: subproject\n    command: \"sh -c '{} inner {{base_dir}}/sub' {{all_paths}}\"\n    \
                 matching: [\"sub/**\"]\n{optional}  - name: here\n    command: \"sh -c 'exit 0'\"\n",
                env!("CARGO_BIN_EXE_bolt"),
            ),
        );
        let outcome = bolt::run::run("outer", root.path()).expect("the run completes");
        (root, outcome)
    };

    let (_kept, required) = run("");
    let (_also_kept, optional) = run("    optional: true\n");

    assert_eq!(required.executions, 1, "bolt was started over nothing");
    let kinds: Vec<String> = reasons_in(&required.output_dir.join(bolt::run::RESULT_FILE))
        .into_iter()
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(kinds, ["empty-selection"], "the composing task's reason");
    assert!(
        optional.success,
        "an expected missing subproject failed the run"
    );
    assert_eq!(optional.executions, 1, "bolt was started over nothing");
}

// COVERS FR-4.4d | negative
/// `optional` on a command with no selection is refused before anything runs.
#[test]
fn optional_without_a_path_variable_is_a_jig_error() {
    let root = tree();
    write_jig(
        root.path(),
        "pointless",
        "  - name: whole\n    command: \"sh -c 'exit 0'\"\n    optional: true\n",
    );

    let refusal = bolt::run::run("pointless", root.path()).expect_err("the jig is refused");

    match refusal {
        bolt::Error::JigUnreadable { reason, .. } => assert!(
            reason.contains("validating") && reason.contains("'/tasks/0'"),
            "not refused by the schema at the task: {reason}",
        ),
        other => panic!("wrong refusal: {other:?}"),
    }
}

// COVERS FR-3.4b, FR-4.4h | negative
/// `matching`, `excluding` and `optional` on a command naming no path variable
/// are each refused by the jig schema, before bolt reads a task.
///
/// The reason is the schema's `validating` at the task, not a refusal of bolt's
/// own, which is FR-4.4h's half: the runner has no rule of its own to restate.
#[test]
fn selection_fields_without_a_path_variable_are_refused_by_the_schema() {
    for field in [
        "matching: [\"*.txt\"]",
        "excluding: [\"*.txt\"]",
        "optional: true",
    ] {
        let root = tree();
        write_jig(
            root.path(),
            "whole",
            &format!("  - name: whole\n    command: \"sh -c 'exit 0'\"\n    {field}\n"),
        );

        match bolt::run::run("whole", root.path()) {
            Err(bolt::Error::JigUnreadable { reason, .. }) => assert!(
                reason.contains("validating") && reason.contains("'/tasks/0'"),
                "{field}: not refused by the schema at the task: {reason}",
            ),
            other => panic!("{field} was not refused as unreadable: {other:?}"),
        }
    }
}

// COVERS FR-4.4f | edge
/// An empty selection is a verdict, so bolt still exits 0.
#[test]
fn an_empty_selection_leaves_the_exit_status_alone() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write_jig(
        root.path(),
        "empty",
        "  - name: none\n    command: \"cat {each_path}\"\n    matching: [\"*.py\"]\n",
    );

    let finished = bolt()
        .arg("empty")
        .arg(root.path())
        .output()
        .expect("bolt runs");

    assert_eq!(finished.status.code(), Some(0), "bolt carried the run out");
    let result = String::from_utf8_lossy(&finished.stdout).trim().to_owned();
    assert!(
        !verdict(Path::new(&result), &wrench::schemas::ENVELOPE),
        "the empty selection did not fail the result",
    );
}

// COVERS FR-4.7 | property
/// Declaring the same tasks in the opposite order gives the same result.
///
/// Both tasks fail with different messages, so the reasons have an order to get
/// wrong. The two runs share a base and differ only in their output directory,
/// which is replaced before comparing.
#[test]
fn task_order_does_not_change_the_result() {
    let root = tree();
    let alpha = "  - name: alpha\n    command: \"sh -c 'exit 3'\"\n";
    let beta = "  - name: beta\n    command: \"sh -c 'exit 4'\"\n";
    write_jig(root.path(), "forward", &format!("{alpha}{beta}"));
    write_jig(root.path(), "backward", &format!("{beta}{alpha}"));
    let out = tree();

    let result = |jig: &str| {
        let outcome = run_into(jig, root.path(), &out.path().join(jig)).expect("the run completes");
        fs::read_to_string(outcome.output_dir.join(bolt::run::RESULT_FILE))
            .expect("the result")
            .replace(&outcome.output_dir.display().to_string(), "OUT")
    };

    assert_eq!(
        result("forward"),
        result("backward"),
        "the order leaked into the result"
    );
}

// COVERS FR-4.16c | negative
/// A definitions value is a scalar, in a jig's block and in a file alike.
#[test]
fn a_definitions_value_that_is_not_a_scalar_is_refused() {
    let in_jig = tree();
    write(
        in_jig.path(),
        &bolt::jig::file_name("nested"),
        concat!(
            "definitions:\n  lint:\n    deny: warnings\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );
    let in_file = tree();
    write_jig(
        in_file.path(),
        "plain",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );
    write_definitions(in_file.path(), "listed", "deny: [warnings, clippy]\n");

    let from_jig = bolt::run::run("nested", in_jig.path()).expect_err("a nested block refuses");
    let from_file = run_with("plain", in_file.path(), "listed").expect_err("a list refuses");

    assert!(
        matches!(from_jig, bolt::Error::JigUnreadable { .. }),
        "a nested value in a jig: {from_jig:?}",
    );
    assert!(
        matches!(from_file, bolt::Error::DefinitionsUnreadable { .. }),
        "a list in a file: {from_file:?}",
    );
}

// COVERS FR-4.17b | positive
/// A relative definitions value reaches above the base, because the command
/// stands at the base.
#[test]
fn a_relative_definition_resolves_against_the_base() {
    let root = tree();
    write(root.path(), "REQUIREMENTS.md", "the root's\n");
    write(root.path(), "go/REQUIREMENTS.md", "the pack's\n");
    write(
        &root.path().join("go"),
        &bolt::jig::file_name("check"),
        concat!(
            "definitions:\n  requirements: ../REQUIREMENTS.md\n",
            "tasks:\n  - name: read\n    command: \"cat {requirements}\"\n",
        ),
    );

    let (_, stdout) = stdout_of("check", &root.path().join("go"), "read-1");

    assert_eq!(
        stdout, "the root's\n",
        "the value did not resolve against the base"
    );
}

// ---- evidence ---------------------------------------------------------------

// COVERS FR-1.4, FR-9.2, FR-4.15 | positive
/// Each execution keeps its native results, including files the command wrote.
///
/// Every kept file's contents are asserted. `is_file()` alone is satisfied by a
/// zero-byte file, and an empty `exitcode` breaks every adapter by FR-6.3.
#[test]
fn an_execution_keeps_its_native_results() {
    let root = tree();
    write_jig(
        root.path(),
        "noisy",
        // The artifact is addressed at {work_dir}. FR-4.1a stands a command at
        // the base, and FR-9.2 keeps what the command wrote *there*, meaning in
        // its work directory, so a command wanting an artifact kept says where.
        // Declaring `evidence` is how a task names files bolt did not see it
        // write, and that belongs to `runner/30`, not the skeleton.
        concat!(
            "  - name: noisy\n",
            "    command: \"sh -c 'echo out; echo err >&2; echo made > {work_dir}/artifact.txt'\"\n",
        ),
    );

    let outcome = bolt::run::run("noisy", root.path()).expect("the run completes");
    let dir = work(&outcome, "noisy-1");

    assert_eq!(
        fs::read_to_string(dir.join("stdout")).expect("stdout is kept"),
        "out\n",
        "stdout is captured as the command wrote it",
    );
    assert_eq!(
        fs::read_to_string(dir.join("stderr")).expect("stderr is kept"),
        "err\n",
        "stderr is captured as the command wrote it",
    );
    assert_eq!(
        fs::read_to_string(dir.join(bolt::run::EXITCODE_FILE))
            .expect("the exit code is kept")
            .trim(),
        "0",
        "the exit code is recorded as a number an adapter can read",
    );
    assert_eq!(
        fs::read_to_string(dir.join("artifact.txt")).expect("the artifact is kept"),
        "made\n",
        "a file the command wrote survives the run as evidence",
    );
    assert!(
        dir.join(bolt::run::MANIFEST_FILE).is_file() && dir.join(bolt::run::OUTPUT_FILE).is_file(),
        "the manifest and the envelope are both kept",
    );
}

// COVERS FR-7.5, FR-7.5a, FR-7.5b | property
/// Nothing bolt writes as a unit leaves a temporary behind.
///
/// FR-7.5a writes to a temporary and renames, so a killed bolt leaves the file
/// absent instead of half written, and FR-7.5b keeps the temporary beside its
/// target because a rename across filesystems is a copy.
///
/// Asserted as the absence of a leftover. Killing a run midway cannot be made
/// deterministic. A write that skipped the rename fails
/// this twice over: the temporary survives and the target is not there.
///
/// Every structured file goes through wrench, which renames for itself. What
/// this covers is bolt's own writing, which reaches only the `exitcode` file.
#[test]
fn a_written_file_leaves_no_temporary_beside_it() {
    let root = tree();
    write_jig(
        root.path(),
        "atomic",
        "  - name: alpha\n    command: \"sh -c 'exit 3'\"\n",
    );

    let outcome = bolt::run::run("atomic", root.path()).expect("the run completes");
    let dir = work(&outcome, "alpha-1");
    let leftovers: Vec<String> = fs::read_dir(&dir)
        .expect("the work directory is readable")
        .filter_map(|entry| {
            entry
                .ok()
                .map(|it| it.file_name().to_string_lossy().into_owned())
        })
        .filter(|name| name.contains(".tmp-"))
        .collect();

    assert!(
        leftovers.is_empty(),
        "a temporary survived the write: {leftovers:?}",
    );
    assert_eq!(
        fs::read_to_string(dir.join(bolt::run::EXITCODE_FILE))
            .expect("the exit code is at its target")
            .trim(),
        "3",
        "the renamed file is not the one the run wrote",
    );
}

// COVERS FR-9.2b | property
/// The ordinal is zero-padded to the width that task's execution count needs.
#[test]
fn the_ordinal_is_zero_padded_to_the_width_needed() {
    assert_eq!(
        bolt::run::work_dir_name("lint", 1, 1),
        "lint-1",
        "one needs one digit"
    );
    assert_eq!(
        bolt::run::work_dir_name("lint", 9, 9),
        "lint-9",
        "nine needs one"
    );
    assert_eq!(
        bolt::run::work_dir_name("lint", 9, 10),
        "lint-09",
        "ten needs two"
    );
    assert_eq!(
        bolt::run::work_dir_name("lint", 7, 100),
        "lint-007",
        "a hundred needs three"
    );
}

// COVERS FR-9.2a | property
/// Each task numbers its own executions from one, independently of the others.
///
/// A unit test over the naming function cannot show this: it is handed an index
/// and cannot say where the index came from. This runs two path-consuming tasks
/// and asserts both start at one, and that the first execution got the first
/// path in the walk's sorted order.
#[test]
fn each_task_numbers_its_own_executions_from_one() {
    let root = tree();
    write(root.path(), "a.txt", "first");
    write(root.path(), "b.txt", "second");
    write_jig(
        root.path(),
        "twice",
        concat!(
            "  - name: alpha\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"sh -c 'echo {each_path}'\"\n",
            "  - name: beta\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"sh -c 'echo {each_path}'\"\n",
        ),
    );

    let outcome = bolt::run::run("twice", root.path()).expect("the run completes");

    for task in ["alpha", "beta"] {
        assert!(
            work(&outcome, &format!("{task}-1")).is_dir(),
            "{task} does not number from one",
        );
        assert!(
            work(&outcome, &format!("{task}-2")).is_dir(),
            "{task} has no second execution",
        );
    }
    let first = fs::read_to_string(work(&outcome, "alpha-1").join("stdout"))
        .expect("the first execution wrote stdout");
    assert!(
        first.contains("a.txt"),
        "ordinal 1 is not the first path in sorted order: {first}",
    );
}

// COVERS FR-9.5 | positive
/// The manifest records what `matching` selected and what `excluding` removed.
///
/// The jig is valid: `matching` and `excluding` sit on a task that names a path
/// variable, which FR-3.4b requires. Putting them on a task whose command is
/// `false` builds a jig the specification refuses, and asserting that such a
/// manifest carries both paths is the negation of FR-9.6.
#[test]
fn the_manifest_records_what_was_selected_and_removed() {
    let root = tree();
    write(root.path(), "a.py", "kept");
    write(root.path(), "generated.py", "removed");
    write_jig(
        root.path(),
        "manifested",
        concat!(
            "  - name: check\n",
            "    matching: [\"**/*.py\"]\n",
            "    excluding: [\"generated.py\"]\n",
            "    command: \"sh -c 'echo {all_paths}'\"\n",
        ),
    );

    let outcome = bolt::run::run("manifested", root.path()).expect("the run completes");
    let manifest = read_validated(
        &work(&outcome, "check-1").join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );

    // `selection.matched` and `selection.excluded` are wrench's names for the
    // two lists FR-9.5 requires. Invented names such as `selected` and
    // `removed` are refused when the manifest is written through wrench, which
    // is what reading a structured file through a schema is for.
    let listed = |key: &str| -> Vec<String> {
        manifest
            .get("selection")
            .and_then(|selection| selection.get(key))
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("the manifest has no selection.{key} list: {manifest}"))
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .map(|path| {
                Path::new(&path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    };

    assert_eq!(
        listed("matched"),
        ["a.py"],
        "the manifest's matched list is wrong"
    );
    assert_eq!(
        listed("excluded"),
        ["generated.py"],
        "the manifest's excluded list is wrong",
    );
}

// COVERS FR-9.5a | positive
/// A manifest is written before its command runs.
///
/// The command reads its own manifest, which is the only way this suite can
/// observe ordering. A test that merely finds a manifest after a failing
/// command passes just as well against one written afterwards.
#[test]
fn a_manifest_exists_before_the_command_runs() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    write_jig(
        root.path(),
        "early",
        concat!(
            "  - name: reads-its-own\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"sh -c 'test -f {work_dir}/manifest.yaml' sh {each_path}\"\n",
        ),
    );

    let outcome = bolt::run::run("early", root.path()).expect("the run completes");

    assert_eq!(
        fs::read_to_string(work(&outcome, "reads-its-own-1").join(bolt::run::EXITCODE_FILE))
            .expect("the exit code is kept")
            .trim(),
        "0",
        "the manifest did not exist when the command ran",
    );
}

// COVERS FR-9.6 | negative
/// A task naming no path variable has a manifest claiming no paths.
///
/// Recording one would say the command saw files it never received.
#[test]
fn a_task_naming_no_path_variable_claims_no_paths() {
    let root = tree();
    write(root.path(), "a.py", "present");
    write_jig(
        root.path(),
        "whole",
        "  - name: everything\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("whole", root.path()).expect("the run completes");
    let manifest = read_validated(
        &work(&outcome, "everything-1").join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );

    // wrench's schema says `selection` is "present for a task that consumes
    // paths", so absent is the claim, not an empty pair of lists.
    assert!(
        manifest.get("selection").is_none(),
        "a task handed no list has a manifest claiming a selection: {manifest}",
    );
}

/// Every path under `root`, relative to it and sorted.
fn listing(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("readable").filter_map(Result::ok) {
            let path = entry.path();
            found.push(
                path.strip_prefix(root)
                    .expect("under the root")
                    .display()
                    .to_string(),
            );
            if path.is_dir() {
                walk(root, &path, found);
            }
        }
    }
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found.sort();
    found
}

// COVERS FR-9.1 | property
/// Everything a run writes is inside its one run directory.
#[test]
fn a_runs_whole_output_is_one_directory() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write_jig(
        root.path(),
        "check",
        "  - name: read\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
    );
    let before = listing(root.path());

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    let run = outcome
        .output_dir
        .strip_prefix(fs::canonicalize(root.path()).expect("the base"))
        .expect("the default run directory is at the base")
        .display()
        .to_string();
    let added: Vec<String> = listing(root.path())
        .into_iter()
        .filter(|path| !before.contains(path))
        .collect();
    assert!(
        added.iter().all(|path| path.starts_with(&run)),
        "the run wrote outside {run}: {added:?}",
    );
}

// COVERS FR-9.2c, FR-9.2d | edge
/// An artifact is kept only where it was addressed, and one written at the base
/// stays in the tree.
#[test]
fn an_artifact_not_written_to_the_work_directory_stays_where_it_landed() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: writes\n    command: \"sh -c 'echo x > stray.txt; echo y > {work_dir}/kept.txt'\"\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    let dir = work(&outcome, "writes-1");
    assert!(
        dir.join("kept.txt").is_file(),
        "the addressed artifact was lost"
    );
    assert!(
        !dir.join("stray.txt").exists(),
        "bolt went looking for an artifact it was not handed",
    );
    assert!(
        root.path().join("stray.txt").is_file(),
        "the artifact the tool wrote into the tree was removed",
    );
}

// COVERS FR-9.3 | property
/// One execution's directory holds what ran, what it said and how it ended.
///
/// Copied somewhere else first, so nothing the assertions read can be reaching
/// back into the rest of the run.
#[test]
fn one_executions_evidence_is_complete_in_its_directory() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write_jig(
        root.path(),
        "check",
        "  - name: read\n    command: \"sh -c 'cat \\\"$1\\\"; echo e >&2; exit 2' sh {each_path}\"\n    matching: [\"*.txt\"]\n",
    );
    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    let elsewhere = tree();
    for name in [
        "manifest.yaml",
        "stdout",
        "stderr",
        "exitcode",
        "output.yaml",
    ] {
        fs::copy(
            work(&outcome, "read-1").join(name),
            elsewhere.path().join(name),
        )
        .unwrap_or_else(|error| panic!("{name} is not in the work directory: {error}"));
    }
    let dir = elsewhere.path();

    let manifest = read_validated(
        &dir.join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );
    let a = fs::canonicalize(root.path().join("a.txt")).expect("a.txt");
    assert!(
        manifest["command"]
            .as_str()
            .is_some_and(|command| command.contains(&a.display().to_string())),
        "the command as executed: {manifest}",
    );
    assert_eq!(fs::read_to_string(dir.join("stdout")).expect("stdout"), "a");
    assert_eq!(
        fs::read_to_string(dir.join("stderr")).expect("stderr"),
        "e\n"
    );
    assert_eq!(
        fs::read_to_string(dir.join("exitcode"))
            .expect("exitcode")
            .trim(),
        "2"
    );
    assert!(!verdict(
        &dir.join(bolt::run::OUTPUT_FILE),
        &wrench::schemas::ENVELOPE
    ));
}

// COVERS FR-9.4 | property
/// Two runs over one tree name every file the same.
#[test]
fn two_runs_over_one_tree_line_up_file_for_file() {
    let root = tree();
    for name in ["a.txt", "b.txt", "c.txt"] {
        write(root.path(), name, name);
    }
    write_jig(
        root.path(),
        "check",
        concat!(
            "  - name: each\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
            "  - name: whole\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );
    let out = tree();

    let first = run_into("check", root.path(), &out.path().join("one")).expect("first run");
    let second = run_into("check", root.path(), &out.path().join("two")).expect("second run");

    assert_eq!(
        listing(&first.output_dir),
        listing(&second.output_dir),
        "the two runs' files do not line up",
    );
}

// COVERS FR-9.5b | negative
/// A manifest is not touched once its command starts.
///
/// The command copies its own manifest as it runs. After the run the manifest
/// is byte for byte what the command saw, so nothing the run learned by
/// finishing went into it.
#[test]
fn a_manifest_holds_only_what_was_known_beforehand() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: copies\n    command: \"sh -c 'cp {work_dir}/manifest.yaml {work_dir}/during.yaml; exit 5'\"\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    let dir = work(&outcome, "copies-1");
    assert_eq!(
        fs::read(dir.join(bolt::run::MANIFEST_FILE)).expect("the manifest"),
        fs::read(dir.join("during.yaml")).expect("the copy taken while running"),
        "the manifest changed after its command started",
    );
}

// COVERS FR-9.5c | property
/// The manifest names every variable the execution was given, the path
/// variable included.
#[test]
fn the_manifest_records_every_template_variable() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write(root.path(), "b.txt", "b");
    write_jig(
        root.path(),
        "check",
        concat!(
            "  - name: each\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
            "  - name: all\n    command: \"cat {all_paths}\"\n    matching: [\"*.txt\"]\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    let variables = |entry: &str| {
        read_validated(
            &work(&outcome, entry).join(bolt::run::MANIFEST_FILE),
            &wrench::schemas::MANIFEST,
        )["variables"]
            .clone()
    };
    let base = fs::canonicalize(root.path()).expect("the base");
    let path = |name: &str| base.join(name).display().to_string();

    let each = variables("each-2");
    for location in [
        "project_root",
        "base_dir",
        "work_dir",
        "config_dir",
        "output_dir",
    ] {
        assert_eq!(
            each[location]["from"], "bolt",
            "{location} is missing: {each}"
        );
    }
    assert_eq!(
        each["each_path"]["value"],
        path("b.txt"),
        "the path this execution got"
    );
    assert_eq!(
        variables("all-1")["all_paths"]["value"],
        serde_json::json!([path("a.txt"), path("b.txt")]),
        "the list this execution got",
    );
}

// COVERS FR-9.5e | negative
/// The environment bolt ran in is not written into any manifest.
#[test]
fn the_environment_is_not_in_the_manifest() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );

    let finished = bolt()
        .env("BOLT_TEST_MARKER", "marker-from-the-environment")
        .arg("check")
        .arg(root.path())
        .output()
        .expect("bolt runs");

    let result = String::from_utf8_lossy(&finished.stdout).trim().to_owned();
    let manifest = fs::read_to_string(
        Path::new(&result)
            .with_file_name(bolt::run::WORK_DIR)
            .join("alpha-1")
            .join(bolt::run::MANIFEST_FILE),
    )
    .expect("the manifest");
    assert!(
        !manifest.contains("marker-from-the-environment") && !manifest.contains("BOLT_TEST_MARKER"),
        "the environment reached the manifest:\n{manifest}",
    );
}

// COVERS FR-9.8 | property
/// Each per-path execution's manifest carries the whole matched list.
#[test]
fn every_per_path_manifest_carries_the_whole_selection() {
    let root = tree();
    for name in ["a.txt", "b.txt", "c.txt"] {
        write(root.path(), name, name);
    }
    write_jig(
        root.path(),
        "check",
        "  - name: each\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    let base = fs::canonicalize(root.path()).expect("the base");
    let all =
        serde_json::json!(["a.txt", "b.txt", "c.txt"].map(|n| base.join(n).display().to_string()));
    for entry in ["each-1", "each-2", "each-3"] {
        let manifest = read_validated(
            &work(&outcome, entry).join(bolt::run::MANIFEST_FILE),
            &wrench::schemas::MANIFEST,
        );
        assert_eq!(manifest["selection"]["matched"], all, "{entry}'s selection");
    }
}

// COVERS FR-9.9 | edge
/// A task that runs once still has an ordinal in its directory name.
#[test]
fn a_single_execution_still_carries_an_ordinal() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: once\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    assert!(work(&outcome, "once-1").is_dir(), "no once-1");
    assert!(
        !work(&outcome, "once").exists(),
        "a bare task name was used"
    );
}

// COVERS FR-8.5 | property
/// Each execution's envelope is still on disk, untouched, beside the result.
#[test]
fn constituent_envelopes_survive_the_merge() {
    let root = tree();
    let envelope = "\"success\": false\n\"reasons\":\n- \"kind\": \"k\"\n  \"message\": \"m\"\n";
    write(root.path(), "envelope.yaml", envelope);
    write_adapter(
        root.path(),
        "fixed",
        &format!(
            "for a in \"$@\"; do case $prev in --work-dir) w=$a;; esac; prev=$a; done\ncp {} \"$w/output.yaml\"\n",
            root.path().join("envelope.yaml").display(),
        ),
    );
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n    adapter: fixed\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    assert!(
        outcome.output_dir.join(bolt::run::RESULT_FILE).is_file(),
        "no result"
    );
    assert_eq!(
        fs::read_to_string(envelope_of(&outcome, "alpha-1")).expect("the envelope"),
        envelope,
        "the constituent envelope was changed or removed by the merge",
    );
}

// ---- adapters and the merge -------------------------------------------------

// COVERS FR-6.9 | positive
/// A task naming no adapter gets the generic exit-code adapter.
///
/// The commands are `exit 0` and `exit 3`, not `true` and `false`, so no
/// envelope can satisfy the assertion by echoing the command line it ran, and
/// the verdict is read as a boolean instead of matched as a substring.
#[test]
fn a_task_naming_no_adapter_gets_the_exit_code_one() {
    let root = tree();
    write_jig(
        root.path(),
        "verdicts",
        concat!(
            "  - name: zero\n",
            "    command: \"sh -c 'exit 0'\"\n",
            "  - name: nonzero\n",
            "    command: \"sh -c 'exit 3'\"\n",
        ),
    );

    let outcome = bolt::run::run("verdicts", root.path()).expect("the run completes");

    assert!(
        verdict(
            &work(&outcome, "zero-1").join(bolt::run::OUTPUT_FILE),
            &wrench::schemas::ENVELOPE
        ),
        "a zero exit did not report success",
    );
    assert!(
        !verdict(
            &work(&outcome, "nonzero-1").join(bolt::run::OUTPUT_FILE),
            &wrench::schemas::ENVELOPE
        ),
        "a non-zero exit did not report failure",
    );
}

// COVERS FR-8.1 | property
/// The merge folds every envelope into one result, repeatably.
///
/// Repeatability is asserted on the file's bytes, not on one field. A merge that
/// appended to the result, or rebuilt its evidence mapping differently on the
/// second pass, keeps the verdict stable and changes everything else.
#[test]
fn the_merge_folds_every_envelope_repeatably() {
    let root = tree();
    write_jig(
        root.path(),
        "two",
        concat!(
            "  - name: alpha\n",
            "    command: \"sh -c 'exit 0'\"\n",
            "  - name: beta\n",
            "    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let outcome = bolt::run::run("two", root.path()).expect("the run completes");
    let result = outcome.output_dir.join(bolt::run::RESULT_FILE);
    let first = fs::read(&result).expect("a run has a result");

    bolt::merge::merge(&outcome.output_dir, root.path(), &[])
        .expect("a finished directory refolds");

    assert_eq!(
        first,
        fs::read(&result).expect("the result survives a refold"),
        "the fold is not repeatable over a finished directory",
    );
}

// COVERS FR-8.3 | property
/// The merged result passes only when every constituent passes.
///
/// Both directions. Without the passing case, a merge hardcoding failure is
/// invisible to the whole suite, because FR-10.2 keeps the exit status green
/// either way.
#[test]
fn the_merge_passes_only_when_every_constituent_passes() {
    let all_pass = tree();
    write_jig(
        all_pass.path(),
        "good",
        concat!(
            "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
            "  - name: beta\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );
    let passing = bolt::run::run("good", all_pass.path()).expect("the run completes");
    assert!(
        passing.success,
        "every constituent passing did not pass the run"
    );

    let one_fails = tree();
    write_jig(
        one_fails.path(),
        "mixed",
        concat!(
            "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
            "  - name: beta\n    command: \"sh -c 'exit 3'\"\n",
        ),
    );
    let failing = bolt::run::run("mixed", one_fails.path()).expect("the run completes");
    assert!(
        !failing.success,
        "one failing constituent did not fail the run"
    );

    // Asserted on `result.yaml` and not only on the returned struct. FR-8.3 is
    // about the merged result, FR-10.3 makes the envelope the verdict, and
    // every gate in the estate reads the file. The two are computed from one
    // value, so a mutation writing `success: true` into the document while the
    // struct stays false passes both assertions above.
    for (outcome, expected) in [(&passing, true), (&failing, false)] {
        let written = read_validated(
            &outcome.output_dir.join(bolt::run::RESULT_FILE),
            &wrench::schemas::ENVELOPE,
        );
        assert_eq!(
            written.get("success").and_then(Value::as_bool),
            Some(expected),
            "the written result disagrees with the run it folded: {written}",
        );
    }
}

// COVERS FR-6.7 | property
/// Every shape of merged result validates against the envelope schema.
///
/// All passing, a mix, a constituent whose adapter wrote nothing, an empty
/// selection, and a run stopped by its own limit. Each reaches the merge by a
/// different route, and the last carries a reason no constituent produced.
#[test]
fn every_merged_result_validates() {
    let shapes = [
        "  - name: a\n    command: \"sh -c 'exit 0'\"\n",
        "  - name: a\n    command: \"sh -c 'exit 0'\"\n  - name: b\n    command: \"sh -c 'exit 2'\"\n",
        "  - name: a\n    command: \"sh -c 'exit 0'\"\n    adapter: silent\n",
        "  - name: a\n    command: \"cat {each_path}\"\n    matching: [\"*.py\"]\n",
        "  - name: a\n    command: \"sh -c 'exit 0'\"\n  - name: b\n    command: \"sleep 5\"\n",
    ];

    for (index, tasks) in shapes.into_iter().enumerate() {
        let root = tree();
        write_adapter(root.path(), "silent", "exit 0");
        let limit = if index == 4 {
            "time-limit: \"0.5s\"\n"
        } else {
            ""
        };
        write(
            root.path(),
            &bolt::jig::file_name("shape"),
            &format!("{limit}tasks:\n{tasks}"),
        );

        let outcome = bolt::run::run("shape", root.path()).expect("the run completes");

        let result = read_validated(
            &outcome.output_dir.join(bolt::run::RESULT_FILE),
            &wrench::schemas::ENVELOPE,
        );
        assert_eq!(
            result["success"].as_bool(),
            Some(index == 0),
            "shape {index}: {result}",
        );
    }
}

// COVERS FR-8.3a | negative
/// A merge finding no constituent fails, with a reason saying so.
///
/// FR-8.3 alone would pass it: every constituent passing holds when there are
/// none, and a green result over zero checks reads as checked and fine.
#[test]
fn a_merge_finding_no_constituent_fails() {
    let empty = tree();
    fs::create_dir(empty.path().join(bolt::run::WORK_DIR)).expect("an empty work directory");

    let refusal = bolt::merge::merge(empty.path(), empty.path(), &[])
        .expect_err("no constituent is a failure");

    assert!(
        matches!(refusal, bolt::Error::NoConstituents),
        "wrong refusal for an empty fold: {refusal:?}",
    );
    assert!(
        refusal.to_string().contains("no task produced a result"),
        "the reason does not say why: {refusal}",
    );
}

// ---- refusals write a result, and never over somebody else's ----------------

// COVERS FR-10.7, FR-2.5a | positive
/// A refusal writes a `result.yaml` in the shape every refusal takes.
///
/// FR-10.7 has bolt write one whenever it is alive and in control when it
/// stops, so a caller finding none knows the process was killed, not that the
/// run never started. FR-2.5a fixes the shape: `success: false` and a
/// reason, then a non-zero exit.
///
/// The refusal chosen is an unparseable jig, because it happens once the base
/// is known to be there and so is not FR-10.7a's exempt case.
#[test]
fn a_refusal_writes_a_result() {
    let root = tree();
    write(root.path(), &bolt::jig::file_name("broken"), "tasks: [\n");

    let refused = bolt()
        .arg("broken")
        .arg(root.path())
        .output()
        .expect("bolt runs");

    assert_eq!(
        refused.status.code(),
        Some(1),
        "a refusal exits 1: {}",
        String::from_utf8_lossy(&refused.stderr),
    );

    let runs: Vec<_> = fs::read_dir(root.path())
        .expect("the base is readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".bolt-"))
        })
        .collect();
    assert_eq!(
        runs.len(),
        1,
        "a refusal writes one run directory: {runs:?}"
    );

    let result = runs[0].join(bolt::run::RESULT_FILE);
    assert!(
        result.is_file(),
        "a refusal wrote no result.yaml, so a caller cannot tell it from a kill",
    );
    assert!(
        !verdict(&result, &wrench::schemas::ENVELOPE),
        "a refusal's result says success: false",
    );

    assert_refusal_shape(&result, "jig-unreadable", "unreadable");
}

/// FR-2.5a's shape, asserted where a refusal wrote a result.
///
/// One reason, the `kind` this sort of refusal carries by FR-10.9, and a message
/// carrying `says`. One helper, not a copy per caller, since several tests want
/// the same three assertions and differ only in which refusal produced the file
/// and therefore in what it should say.
///
/// `kind` is a parameter because FR-10.9 gives each refusal its own. With
/// `bolt-refused` for every refusal, a helper asserting one string whatever
/// produced the file could not notice two situations sharing one name.
fn assert_refusal_shape(result: &Path, kind: &str, says: &str) {
    let envelope = read_validated(result, &wrench::schemas::ENVELOPE);
    let reasons = envelope
        .get("reasons")
        .and_then(Value::as_array)
        .expect("a refusal carries reasons");

    assert_eq!(reasons.len(), 1, "one refusal is one reason: {reasons:?}");
    assert_eq!(
        reasons[0].get("kind").and_then(Value::as_str),
        Some(kind),
        "a refusal's reason names its kind: {reasons:?}",
    );
    assert!(
        reasons[0]
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains(says)),
        "the reason does not say what was refused: {reasons:?}",
    );
}

// COVERS FR-10.7a | edge
/// The refusal that writes nothing is the base not being there, and it says so.
///
/// FR-10.7a: the default output directory sits inside the base, so writing the
/// result would create the very thing whose absence is being refused. Bolt says
/// on stderr that it wrote none. FR-10.7b points a caller wanting a parseable
/// refusal in every case at `--output-dir`, which is `runner/10`.
#[test]
fn the_missing_base_refusal_writes_nothing_and_says_so() {
    let root = tree();
    let absent = root.path().join("not-there");

    let refused = bolt()
        .arg("check")
        .arg(&absent)
        .output()
        .expect("bolt runs");

    assert_eq!(refused.status.code(), Some(1), "a missing base is refused");
    assert!(
        !absent.exists(),
        "refusing a missing base created it, which is what the refusal was about",
    );

    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("not there"),
        "stderr does not say why: {stderr}",
    );
    assert!(
        stderr.contains("no result was written"),
        "stderr does not say that no result was written: {stderr}",
    );
}

// COVERS FR-10.7, FR-10.7a, FR-2.6b | regression
/// Refusing a directory as unusable does not write into it.
///
/// FR-10.7 can be satisfied without this guarantee, which is why it is asserted
/// and not just described. An earlier Go implementation of bolt writes a
/// conformant refusal into the run directory it resolved, and because the stamp
/// is second-granular two runs starting in one second resolve to the same one:
/// the second refuses, correctly, and its refusal replaces the first's completed
/// verdict while the per-task evidence still says `nonzero-exit`. Reproduced
/// against that build.
///
/// A refactor satisfying FR-10.7 by writing the refusal into the resolved
/// directory passes every other test in this file.
///
/// The directory is named instead of resolved from the stamp, which with
/// FR-2.6e's nanoseconds does not collide by clock. Both cases reach the
/// same refusal through `wrote_a_result`, and a named directory constructs it
/// without a retry loop that could pass by never exercising its own case.
#[test]
fn a_refusal_does_not_write_into_the_directory_it_refused() {
    let root = tree();
    let sentinel = "success: true\n";
    let named = root.path().join("qa");
    fs::create_dir_all(&named).expect("the colliding run directory");
    let result = named.join(bolt::run::RESULT_FILE);
    fs::write(&result, sentinel).expect("the previous run's verdict");

    let refusal = run_into("check", root.path(), &named).expect_err("the directory is in use");

    assert!(
        matches!(&refusal, bolt::Error::OutputDirectoryInUse(path) if *path == named),
        "the refusal names the wrong directory: {refusal:?}",
    );
    assert_eq!(
        fs::read_to_string(&result).expect("the previous verdict survives"),
        sentinel,
        "the refusal overwrote the verdict of the run it refused to share",
    );
}

// ---- the merge carries its constituents up --------------------------------

// COVERS FR-8.4 | positive
/// The merged result carries the reasons its constituents produced.
///
/// FR-8.4 wants what failed and why readable from the merged file alone.
/// Synthesising one reason per failing constituent satisfies "what failed" and
/// loses "why": every failure then arrives as the same kind with the same
/// message, and a reader is sent back to the work directories the merge exists
/// to summarise.
///
/// Asserted on the constituent's own `kind` and `message`, not on the count,
/// because a merge that renamed its synthesised kind would satisfy a count and
/// still carry nothing.
#[test]
fn the_merge_carries_its_constituents_reasons() {
    let root = tree();
    write_jig(
        root.path(),
        "mixed",
        concat!(
            "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
            "  - name: beta\n    command: \"sh -c 'exit 3'\"\n",
        ),
    );

    let outcome = bolt::run::run("mixed", root.path()).expect("the run completes");
    assert!(!outcome.success, "a failing constituent fails the run");

    let result = outcome.output_dir.join(bolt::run::RESULT_FILE);
    let merged = read_validated(&result, &wrench::schemas::ENVELOPE);
    let reasons = merged
        .get("reasons")
        .and_then(Value::as_array)
        .expect("a failing merge carries reasons");

    assert_eq!(
        reasons.len(),
        1,
        "one failing constituent is one reason: {reasons:?}",
    );

    let kinds: Vec<&str> = reasons
        .iter()
        .filter_map(|reason| reason.get("kind").and_then(Value::as_str))
        .collect();
    assert!(
        kinds.contains(&"nonzero-exit"),
        "the constituent's own kind did not survive the fold: {kinds:?}",
    );

    let messages: Vec<&str> = reasons
        .iter()
        .filter_map(|reason| reason.get("message").and_then(Value::as_str))
        .collect();
    assert!(
        messages.iter().any(|message| message.contains("beta")),
        "no reason names the task that failed: {messages:?}",
    );
    assert!(
        messages.iter().any(|message| message.contains('3')),
        "no reason says what happened, only that something did: {messages:?}",
    );
    assert!(
        !messages.iter().any(|message| message.contains("alpha")),
        "a passing constituent contributed a reason: {messages:?}",
    );
}

// ---- paths, and what the merge says a result rests on ----------------------

// COVERS FR-2.4 | positive
/// Paths are resolved to absolute before anything runs.
///
/// Driven through the built binary with a relative base, because that is the
/// case the row is about and the one an in-process call cannot reach: a test
/// passing `root.path()` hands bolt an absolute path already and would pass
/// against an implementation that resolves nothing.
///
/// Without resolution, `bolt gate .` records `"base_dir": {"value": "."}` in
/// every manifest and substitutes relative paths into command lines.
/// `strip_prefix` survives that, so nothing fails; every recorded path is
/// simply wrong for a reader not standing where bolt stood.
#[test]
fn paths_are_resolved_to_absolute_before_anything_runs() {
    let root = tree();
    write(root.path(), "a.txt", "content");
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0' {each_path}\"\n    matching: [\"**/*.txt\"]\n",
    );

    let outcome = bolt()
        .arg("check")
        .arg(".")
        .current_dir(root.path())
        .output()
        .expect("bolt runs");
    assert_eq!(
        outcome.status.code(),
        Some(0),
        "a relative base is a complete invocation: {}",
        String::from_utf8_lossy(&outcome.stderr),
    );

    let run = fs::read_dir(root.path())
        .expect("the base is readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".bolt-"))
        })
        .expect("a run directory");

    let manifest = read_validated(
        &run.join(bolt::run::WORK_DIR)
            .join("alpha-1")
            .join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );
    let variables = manifest
        .get("variables")
        .and_then(Value::as_object)
        .expect("a manifest records its variables");

    for name in [
        "project_root",
        "base_dir",
        "config_dir",
        "work_dir",
        "output_dir",
    ] {
        let value = variables
            .get(name)
            .and_then(|entry| entry.get("value"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{name} is not in the manifest"));
        assert!(
            Path::new(value).is_absolute(),
            "{name} was recorded relative, as {value:?}",
        );
    }
}

// COVERS FR-8.2, FR-8.2a, FR-8.8 | positive
/// Evidence is a mapping keyed by execution, each entry carrying args and result.
///
/// FR-8.2 wants the merge to rewrite `evidence` from a list of paths into a
/// mapping whose entries each carry that task's args and the filepath of its own
/// result. Bare strings leave `args` absent entirely.
///
/// FR-8.2a settles where each half comes from and neither is the envelope: the
/// key from the work directory name, the args from that execution's manifest.
/// That keeps FR-6.2's adapter contract narrow, since an adapter never has to
/// know what task it was run for.
///
/// FR-8.8 makes `args` the argv as executed, after substitution, so the merged
/// file says what ran and not what was written. Asserted by putting a path
/// variable in the command and requiring the substituted filename to appear.
///
/// The envelope schema does not constrain `metadata.evidence`, so nothing
/// catches the shape on the way out and this test is the only check.
#[test]
fn evidence_is_keyed_by_execution_and_carries_args_and_result() {
    let root = tree();
    write(root.path(), "a.txt", "content");
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0' {each_path}\"\n    matching: [\"**/*.txt\"]\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    let merged = read_validated(
        &outcome.output_dir.join(bolt::run::RESULT_FILE),
        &wrench::schemas::ENVELOPE,
    );

    let evidence = merged
        .get("metadata")
        .and_then(|metadata| metadata.get("evidence"))
        .and_then(Value::as_object)
        .expect("evidence is a mapping, not a list of paths");

    let entry = evidence
        .get("alpha-1")
        .unwrap_or_else(|| panic!("no entry keyed by the work directory: {evidence:?}"));

    let result = entry
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("the entry carries no result filepath: {entry:?}"));
    assert!(
        Path::new(result).is_file(),
        "the result filepath does not name a file that exists: {result:?}",
    );

    let args = entry
        .get("args")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("the entry carries no args: {entry:?}"));
    assert!(
        args.contains("a.txt"),
        "args is not the argv as executed; the path variable is unsubstituted: {args:?}",
    );
    assert!(
        !args.contains("{each_path}"),
        "args is what was written rather than what ran: {args:?}",
    );
}

// ---- adapters ----------------------------------------------------------------

/// Write an executable adapter script into `root`, the config directory.
///
/// FR-6.10 resolves an adapter by name from there, where FR-2.8 already finds
/// jigs, so a jig and its adapters travel together.
fn write_adapter(root: &Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    let path = root.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}")).expect("the adapter script");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
}

// COVERS FR-6.1, FR-6.4, FR-6.10, FR-6.12 | positive
/// An adapter's verdict is the verdict, whatever the exit status said.
///
/// FR-6.1: where an adapter reached an authoritative result, that result is the
/// verdict and bolt does not second-guess it. Asserted the hard way round, with
/// a command that exits 0 and an adapter that reads its output and says the run
/// failed. Under FR-6.9's exit-code adapter that task passes, so a bolt
/// ignoring the adapter would produce the opposite verdict.
///
/// FR-6.4: the adapter is chosen by the format it reads, not by the tool. This
/// one reads a count off stdout and would serve any tool emitting it.
#[test]
fn an_adapters_verdict_is_the_verdict() {
    let root = tree();
    write_adapter(
        root.path(),
        "counting-adapter",
        concat!(
            "for a in \"$@\"; do case $prev in --stdout) out=$a;; --work-dir) w=$a;; esac; prev=$a; done\n",
            "n=$(cat \"$out\")\n",
            "if [ \"$n\" -gt 0 ]; then\n",
            "  printf '\"success\": false\\n\"reasons\":\\n  - \"kind\": \"findings\"\\n    \"message\": \"%s problems\"\\n' \"$n\" > \"$w/output.yaml\"\n",
            "else\n",
            "  printf '\"success\": true\\n' > \"$w/output.yaml\"\n",
            "fi\n",
        ),
    );
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'echo 3'\"\n    adapter: counting-adapter\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    assert!(
        !outcome.success,
        "the adapter said the tool found problems and bolt overrode it",
    );
    let envelope = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::OUTPUT_FILE),
        &wrench::schemas::ENVELOPE,
    );
    let reasons = envelope
        .get("reasons")
        .and_then(Value::as_array)
        .expect("the adapter's envelope carries its reasons");
    assert_eq!(
        reasons[0].get("kind").and_then(Value::as_str),
        Some("findings"),
        "the adapter's own kind did not survive: {reasons:?}",
    );
    assert_eq!(
        reasons[0].get("message").and_then(Value::as_str),
        Some("3 problems"),
        "the adapter's own message did not survive: {reasons:?}",
    );
}

// COVERS FR-6.2, FR-6.2a, FR-6.2c, FR-6.3 | positive
/// The default invocation names the captures, the locations and the evidence.
///
/// FR-6.2 fixes the flags. FR-6.2a hands over the same locations every task
/// gets. FR-6.3 passes the exit code as a file, because whether that number
/// explains anything is the adapter's judgement, not bolt's.
///
/// FR-6.2c has `--evidence` name what the task declared and nothing else: an
/// artifact nobody declared still sits in the work directory, it is simply not
/// passed. Asserted by writing two files and declaring one.
#[test]
fn the_default_invocation_names_the_captures_and_only_declared_evidence() {
    let root = tree();
    write_adapter(
        root.path(),
        "recording-adapter",
        concat!(
            "for a in \"$@\"; do case $prev in --work-dir) w=$a;; esac; prev=$a; done\n",
            "printf '%s\\n' \"$@\" > \"$w/argv\"\n",
            "printf '\"success\": true\\n' > \"$w/output.yaml\"\n",
        ),
    );
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "tasks:\n  - name: alpha\n",
            "    command: \"sh -c 'echo declared > {work_dir}/report.json; echo stray > {work_dir}/scratch.tmp'\"\n",
            "    adapter: recording-adapter\n",
            "    evidence: [\"report.json\"]\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    let argv = fs::read_to_string(work(&outcome, "alpha-1").join("argv"))
        .expect("the adapter recorded its argv");

    for flag in [
        "--stdout",
        "--stderr",
        "--exitcode",
        "--project-root",
        "--base-dir",
        "--work-dir",
    ] {
        assert!(argv.contains(flag), "the invocation omits {flag}: {argv}");
    }
    assert!(
        argv.contains("report.json"),
        "the declared evidence was not passed: {argv}",
    );
    assert!(
        !argv.contains("scratch.tmp"),
        "an undeclared artifact was discovered and passed: {argv}",
    );
    // FR-6.3: the exit code arrives as a file, so the adapter reads it or does
    // not, and bolt records no verdict of its own from it.
    assert!(
        work(&outcome, "alpha-1")
            .join(bolt::run::EXITCODE_FILE)
            .is_file(),
        "the exit code was not left as a file for the adapter",
    );
}

// COVERS FR-6.2b, FR-6.2d, FR-6.2e | positive
/// An explicit invocation gets the same substitutions and the same envelope path.
///
/// FR-6.2d: two spellings of a substitution would make the jig format teach
/// itself twice. FR-6.2e: it is still expected to leave the envelope where the
/// default would, because FR-6.2b's name never varies and no flag says where it
/// goes.
#[test]
fn an_explicit_adapter_invocation_is_substituted_like_a_command() {
    let root = tree();
    write_adapter(
        root.path(),
        "plain-adapter",
        "printf '\"success\": true\\n' > \"$1/output.yaml\"\n",
    );
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
            "    adapter: plain-adapter\n",
            "    adapter-command: \"{config_dir}/plain-adapter {work_dir}\"\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    assert!(
        outcome.success,
        "the explicit invocation did not produce a verdict"
    );
    assert!(
        work(&outcome, "alpha-1")
            .join(bolt::run::OUTPUT_FILE)
            .is_file(),
        "the envelope is not where FR-6.2b says it goes",
    );
}

// COVERS FR-6.1a, FR-6.11, FR-7.6, FR-7.9 | negative
/// Each of the three broken-adapter cases gets its own kind.
///
/// FR-6.11 keeps them apart because they have different causes: a crashing
/// adapter, a silent one, and one whose output is not an envelope are three
/// different things to go and fix. FR-7.6 is what makes the second and third
/// two different conditions.
///
/// FR-7.9's kind is what lets a consumer tell them apart without reading
/// English, so this asserts on the kinds and not on the messages.
#[test]
fn each_broken_adapter_case_has_its_own_kind() {
    let cases = [
        ("exits", "exit 7\n", "adapter-failed"),
        ("silent", "exit 0\n", "adapter-wrote-nothing"),
        (
            "garbage",
            "for a in \"$@\"; do case $prev in --work-dir) w=$a;; esac; prev=$a; done\nprintf 'not an envelope\\n' > \"$w/output.yaml\"\n",
            "adapter-wrote-invalid",
        ),
    ];

    for (name, body, expected) in cases {
        let root = tree();
        write_adapter(root.path(), "broken-adapter", body);
        write(
            root.path(),
            &bolt::jig::file_name("check"),
            concat!(
                "version: \"1.0.0\"\n",
                "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0'\"\n    adapter: broken-adapter\n",
            ),
        );

        let outcome = bolt::run::run("check", root.path()).expect("the run completes");
        assert!(
            !outcome.success,
            "{name}: a broken adapter did not fail the task"
        );

        let envelope = read_validated(
            &work(&outcome, "alpha-1").join(bolt::run::OUTPUT_FILE),
            &wrench::schemas::ENVELOPE,
        );
        let kind = envelope
            .get("reasons")
            .and_then(Value::as_array)
            .and_then(|reasons| reasons.first())
            .and_then(|reason| reason.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{name}: no kind on the reason"));
        assert_eq!(kind, expected, "{name}: the wrong case was reported");
    }
}

// COVERS FR-7.1, FR-7.7, FR-6.7a | property
/// What makes an adapter's envelope valid, checked on the way in.
///
/// `success` alone is a complete envelope, and a failure needs reasons each
/// carrying `message` and `kind`. The invalid cases all parse, so what refuses
/// them is validation on read, which bolt's own checks on write cannot reach
/// because the adapter never wrote through bolt.
#[test]
fn an_adapters_envelope_is_valid_by_the_schema_alone() {
    let cases = [
        ("\"success\": true", Ok(true)),
        (
            "\"success\": false\n\"reasons\":\n  - \"kind\": \"k\"\n    \"message\": \"m\"",
            Ok(false),
        ),
        ("\"success\": \"true\"", Err("a string for success")),
        ("\"success\": false", Err("a failure with no reasons")),
        (
            "\"success\": false\n\"reasons\":\n  - \"message\": \"m\"",
            Err("a reason with no kind"),
        ),
        ("\"metadata\": {}", Err("no success at all")),
    ];

    for (envelope, expected) in cases {
        let root = tree();
        write(root.path(), "envelope.yaml", &format!("{envelope}\n"));
        write_adapter(
            root.path(),
            "fixed",
            &format!(
                "for a in \"$@\"; do case $prev in --work-dir) w=$a;; esac; prev=$a; done\ncp {} \"$w/output.yaml\"\n",
                root.path().join("envelope.yaml").display(),
            ),
        );
        write_jig(
            root.path(),
            "check",
            "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n    adapter: fixed\n",
        );

        let outcome = bolt::run::run("check", root.path()).expect("the run completes");
        let kinds: Vec<String> =
            reasons_in(&work(&outcome, "alpha-1").join(bolt::run::OUTPUT_FILE))
                .into_iter()
                .map(|(kind, _)| kind)
                .collect();
        match expected {
            Ok(passed) => {
                assert_eq!(
                    outcome.success, passed,
                    "{envelope:?} was not authoritative"
                );
                if !passed {
                    assert_eq!(kinds, ["k"], "{envelope:?}: the adapter's reason");
                }
            }
            Err(why) => {
                assert!(!outcome.success, "{why} was accepted");
                assert_eq!(kinds, ["adapter-wrote-invalid"], "{why}");
            }
        }
    }
}

// COVERS FR-7.4 | property
/// A task's envelope, a run's result and a refusal's result are one shape.
///
/// All three are read by one consumer against wrench's one envelope schema,
/// with no case for which of bolt's writers produced the file.
#[test]
fn every_envelope_bolt_writes_is_read_one_way() {
    let root = tree();
    let out = tree();
    write_jig(
        root.path(),
        "gate",
        "  - name: fail\n    command: \"sh -c 'exit 2'\"\n",
    );
    write(root.path(), &bolt::jig::file_name("broken"), "tasks: [\n");

    let ran = run_into("gate", root.path(), &out.path().join("ran")).expect("the run completes");
    let refused = out.path().join("refused");
    let finished = bolt()
        .arg("--output-dir")
        .arg(&refused)
        .arg("broken")
        .arg(root.path())
        .output()
        .expect("bolt runs");
    assert_eq!(
        finished.status.code(),
        Some(1),
        "the broken jig was not refused"
    );

    let written = [
        envelope_of(&ran, "fail-1"),
        ran.output_dir.join(bolt::run::RESULT_FILE),
        refused.join(bolt::run::RESULT_FILE),
    ];
    for path in &written {
        assert!(
            !verdict(path, &wrench::schemas::ENVELOPE),
            "{} does not read as a failure",
            path.display(),
        );
        let reasons = read_validated(path, &wrench::schemas::ENVELOPE)["reasons"].clone();
        assert!(
            reasons.as_array().is_some_and(|all| !all.is_empty()),
            "{} carries no reasons: {reasons}",
            path.display(),
        );
    }
}

// COVERS FR-7.3 | positive
/// `metadata` carrying `statistics` and `evidence` is accepted, and so is its
/// absence.
#[test]
fn metadata_is_optional_and_carries_statistics_and_evidence() {
    let root = tree();
    write_adapter(
        root.path(),
        "rich",
        concat!(
            "for a in \"$@\"; do case $prev in --work-dir) w=$a;; esac; prev=$a; done\n",
            "printf '\"success\": true\\n\"metadata\":\\n  \"statistics\":\\n    \"files\": 3\\n",
            "  \"evidence\":\\n    - \"report.json\"\\n' > \"$w/output.yaml\"\n",
        ),
    );
    write_jig(
        root.path(),
        "check",
        concat!(
            "  - name: rich\n    command: \"sh -c 'exit 0'\"\n    adapter: rich\n",
            "  - name: bare\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    assert!(
        outcome.success,
        "an envelope with or without metadata failed"
    );
    let rich = read_validated(&envelope_of(&outcome, "rich-1"), &wrench::schemas::ENVELOPE);
    assert_eq!(rich["metadata"]["statistics"]["files"], 3, "statistics");
    let bare = read_validated(&envelope_of(&outcome, "bare-1"), &wrench::schemas::ENVELOPE);
    assert!(
        bare.get("metadata").is_none(),
        "metadata was supplied: {bare}"
    );
}

// COVERS FR-7.3a | negative
/// The default envelope carries no exit status; the `exitcode` file does.
#[test]
fn the_exit_status_is_not_in_the_envelope_by_default() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    let envelope = read_validated(
        &envelope_of(&outcome, "alpha-1"),
        &wrench::schemas::ENVELOPE,
    );
    assert_eq!(
        envelope,
        serde_json::json!({ "success": true }),
        "the envelope carries more than the verdict",
    );
    assert_eq!(
        fs::read_to_string(work(&outcome, "alpha-1").join(bolt::run::EXITCODE_FILE))
            .expect("the exitcode file")
            .trim(),
        "0",
        "the raw status is not on disk",
    );
}

// COVERS FR-7.3c | negative
/// No envelope or result bolt writes carries a timing.
#[test]
fn nothing_bolt_writes_carries_a_timing() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write_jig(
        root.path(),
        "check",
        concat!(
            "  - name: passes\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
            "  - name: fails\n    command: \"sh -c 'exit 1'\"\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    let mut documents = vec![outcome.output_dir.join(bolt::run::RESULT_FILE)];
    documents.extend(["passes-1", "fails-1"].map(|entry| envelope_of(&outcome, entry)));
    for path in documents {
        let document = read_validated(&path, &wrench::schemas::ENVELOPE);
        let metadata: Vec<&String> = document
            .get("metadata")
            .and_then(Value::as_object)
            .map(|metadata| metadata.keys().collect())
            .unwrap_or_default();
        assert!(
            metadata
                .iter()
                .all(|key| ["base", "evidence"].contains(&key.as_str())),
            "{}: metadata carries {metadata:?}",
            path.display(),
        );
    }
}

// COVERS FR-6.11 | regression
/// A silent adapter does not inherit an earlier fold's envelope.
///
/// An `output.yaml` already in the work directory would satisfy "the adapter
/// wrote one", so a silent adapter would be handed a verdict it did not reach and
/// FR-6.11's `adapter-wrote-nothing` would never fire.
///
/// The command plants the envelope, not an earlier run, which reaches the
/// same condition inside one run: FR-2.6b refuses a second run into the same
/// directory, so two runs cannot set this up.
#[test]
fn a_silent_adapter_does_not_inherit_an_envelope_it_did_not_write() {
    let root = tree();
    write_adapter(root.path(), "silent-adapter", "exit 0\n");
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "tasks:\n  - name: alpha\n",
            "    command: \"sh -c 'printf \\\"success: true\\\\n\\\" > {work_dir}/output.yaml'\"\n",
            "    adapter: silent-adapter\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    assert!(
        !outcome.success,
        "a silent adapter inherited an envelope it did not write",
    );
    let kind = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::OUTPUT_FILE),
        &wrench::schemas::ENVELOPE,
    )
    .get("reasons")
    .and_then(Value::as_array)
    .and_then(|reasons| reasons.first())
    .and_then(|reason| reason.get("kind"))
    .and_then(Value::as_str)
    .map(str::to_owned)
    .expect("a reason saying why");
    assert_eq!(
        kind, "adapter-wrote-nothing",
        "the stale envelope was taken as the adapter's own",
    );
}

// COVERS FR-6.14, FR-7.8 | negative
/// A declared evidence file that was not produced fails the task, naming it.
///
/// FR-6.2c's refusal to discover means nothing else notices: a task declaring
/// evidence it did not write did not do what it said.
#[test]
fn declared_evidence_that_was_not_produced_fails_the_task() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
            "    evidence: [\"report.json\"]\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    assert!(
        !outcome.success,
        "undeclared evidence did not fail the task"
    );

    let envelope = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::OUTPUT_FILE),
        &wrench::schemas::ENVELOPE,
    );
    let message = envelope
        .get("reasons")
        .and_then(Value::as_array)
        .and_then(|reasons| reasons.first())
        .and_then(|reason| reason.get("message"))
        .and_then(Value::as_str)
        .expect("a reason saying why");
    assert!(
        message.contains("report.json"),
        "the reason does not name the path: {message}",
    );
}

// COVERS FR-6.14, FR-6.9, FR-7.2 | regression
/// Missing evidence and a non-zero exit are two reasons, not one.
///
/// The evidence check returns before the exit-code path, so the status is easy
/// to lose whenever both apply. It is the more diagnostic of the two: a
/// `pytest` exit of 4 is a usage error, is distinct from 1, and is usually
/// *why* the declared file is absent at all, so reporting only the symptom
/// sends a reader to their coverage configuration when the command line is
/// what is wrong.
///
/// The task declares no adapter, which is what makes the status bolt's to
/// report under FR-6.9. Where an adapter is declared FR-6.3 keeps that
/// judgement the adapter's, and the status stays out.
#[test]
fn missing_evidence_carries_the_exit_status_beside_it() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 4'\"\n",
            "    evidence: [\"report.json\"]\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    let carried = reasons_in(&work(&outcome, "alpha-1").join(bolt::run::OUTPUT_FILE));

    assert!(
        carried
            .iter()
            .any(|(kind, message)| kind == "evidence-missing" && message.contains("report.json")),
        "no reason names the evidence that was not produced: {carried:?}",
    );
    assert!(
        carried
            .iter()
            .any(|(kind, message)| kind == "nonzero-exit" && message.contains("exited 4")),
        "the exit status was dropped: {carried:?}",
    );
}

// COVERS FR-6.6, FR-6.13 | property
/// Re-folding a finished run costs no re-execution.
///
/// FR-6.6: every input an adapter reads is already on disk, so fixing an adapter
/// and folding again is free. FR-6.13 is why bolt does not reparse and compare
/// to check canonical form: comments do not survive a round trip, so that check
/// would fail every jig documenting itself.
#[test]
fn refolding_a_finished_run_costs_no_re_execution() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    let ran = fs::read_to_string(work(&outcome, "alpha-1").join(bolt::run::EXITCODE_FILE))
        .expect("the exit code is on disk");

    bolt::merge::merge(&outcome.output_dir, root.path(), &[])
        .expect("a finished directory refolds");

    assert_eq!(
        fs::read_to_string(work(&outcome, "alpha-1").join(bolt::run::EXITCODE_FILE))
            .expect("still on disk"),
        ran,
        "re-folding re-executed the command",
    );
}

// COVERS FR-7.10 | property
/// A task that could not execute is distinguishable in the merged result.
///
/// FR-7.10 is about the merged file, not the per-execution envelope: the kind
/// says which, and FR-8.4 carries reasons up. So a reader with only
/// `result.yaml` tells a tool that found problems from a task that never got
/// far enough to have findings.
///
/// The row belongs to `runner/30`, not `runner/20`, because telling them apart
/// needs more than one kind to exist, which only real adapters give.
///
/// Two tasks, two different failures: one whose command ran and reported
/// problems, one whose adapter produced nothing authoritative.
#[test]
fn a_task_that_could_not_execute_is_distinguishable_in_the_merged_result() {
    let root = tree();
    write_adapter(root.path(), "silent-adapter", "exit 0\n");
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "tasks:\n",
            "  - name: found-problems\n    command: \"sh -c 'exit 3'\"\n",
            "  - name: never-concluded\n    command: \"sh -c 'exit 0'\"\n    adapter: silent-adapter\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    let merged = read_validated(
        &outcome.output_dir.join(bolt::run::RESULT_FILE),
        &wrench::schemas::ENVELOPE,
    );
    let kinds: Vec<&str> = merged
        .get("reasons")
        .and_then(Value::as_array)
        .expect("a failing merge carries reasons")
        .iter()
        .filter_map(|reason| reason.get("kind").and_then(Value::as_str))
        .collect();

    assert!(
        kinds.contains(&"nonzero-exit"),
        "the tool that reported problems is not distinguishable: {kinds:?}",
    );
    assert!(
        kinds.contains(&"adapter-wrote-nothing"),
        "the task that reached no verdict is not distinguishable: {kinds:?}",
    );
    assert_ne!(
        kinds[0], kinds[1],
        "both failures arrived as one kind, so a reader cannot tell them apart",
    );
}

// ---- requires, and stopping when a jig asks ---------------------------------

// COVERS FR-3.10, FR-3.10b, FR-3.10d | negative
/// A jig requiring a tool that is not there refuses before anything executes.
///
/// FR-3.10b: an incomplete toolchain is known before a gate starts, not partway
/// through it. FR-3.10d is the same check from the other side, for a project
/// jig naming a tool the base image lacks.
///
/// Asserted on evidence, not on the status, because a refusal that
/// happened *after* the first task ran would exit 1 just the same. Nothing may
/// have executed.
#[test]
fn a_jig_requiring_a_missing_tool_refuses_before_executing() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "requires: [\"sh\", \"definitely-not-a-real-tool-8f3a\"]\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let refusal = bolt::run::run("check", root.path()).expect_err("a missing tool refuses");
    assert!(
        matches!(refusal, bolt::Error::RequiresMissing { .. }),
        "wrong refusal for a missing tool: {refusal:?}",
    );
    assert!(
        refusal
            .to_string()
            .contains("definitely-not-a-real-tool-8f3a"),
        "the reason does not name the tool: {refusal}",
    );
    assert!(
        !refusal.to_string().contains("\"sh\""),
        "the reason names a tool that is present: {refusal}",
    );

    let ran = fs::read_dir(root.path())
        .expect("the base is readable")
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .path()
                .join(bolt::run::WORK_DIR)
                .join("alpha-1")
                .exists()
        });
    assert!(!ran, "a task executed before the missing tool was found");
}

// COVERS FR-3.10a | negative
/// Every missing entry is named, not the first.
///
/// A caller fixing them one at a time pays a round trip per tool, which is the
/// cost the row exists to remove. FR-3.10a is the consistency this makes
/// checkable: an adapter no entry covers is found before a run, not when the
/// task reaches it, and that only helps if the whole list is resolved.
#[test]
fn a_refusal_names_every_missing_tool() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "requires: [\"missing-tool-zeta\", \"sh\", \"missing-tool-alpha\"]\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let refusal = bolt::run::run("check", root.path()).expect_err("missing tools refuse");
    let said = refusal.to_string();

    assert!(
        said.contains("missing-tool-alpha") && said.contains("missing-tool-zeta"),
        "the reason does not name both missing tools: {said}",
    );
    // Sorted, so a jig missing three tools names them the same way every run
    // and two runs of a gate produce a diffable message.
    assert!(
        said.find("missing-tool-alpha") < said.find("missing-tool-zeta"),
        "the missing tools are not in a stable order: {said}",
    );
}

// COVERS FR-3.10 | positive
/// A jig whose `requires` are all present runs as before.
#[test]
fn a_jig_requiring_only_present_tools_runs() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "requires: [\"sh\"]\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("a satisfied jig runs");
    assert!(outcome.success, "the run did not pass");
}

// COVERS FR-4.8 | positive
/// A failing task does not stop the run.
///
/// FR-4.8 is the default and the reason for it: a run that stops early throws
/// away the evidence the later tasks would have produced and leaves a reader
/// unable to tell what else was wrong. Asserted by requiring the task *after*
/// the failure to have left its own evidence behind.
#[test]
fn a_failing_task_does_not_stop_the_run() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        concat!(
            "  - name: alpha\n    command: \"sh -c 'exit 3'\"\n",
            "  - name: beta\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    assert!(!outcome.success, "a failing task did not fail the run");
    assert!(
        outcome.stopped.is_empty(),
        "nothing asked to stop, yet something was reported unreached: {:?}",
        outcome.stopped,
    );
    assert!(
        work(&outcome, "beta-1")
            .join(bolt::run::OUTPUT_FILE)
            .is_file(),
        "the task after the failure did not execute",
    );
}

// COVERS FR-4.9 | positive
/// A task carrying `short-circuit-failure` stops the run when it fails.
///
/// Stopping is something a jig asks for; no jig gets it by default. The tasks
/// after it are reported as not reached, so a reader sees what was not
/// attempted instead of inferring it from what is absent. The two differ: a
/// task missing from the evidence could equally have skipped an empty
/// selection.
#[test]
fn short_circuit_failure_stops_the_run_and_says_what_was_not_reached() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        concat!(
            "  - name: alpha\n    command: \"sh -c 'exit 3'\"\n    short-circuit-failure: true\n",
            "  - name: beta\n    command: \"sh -c 'exit 0'\"\n",
            "  - name: gamma\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    assert!(!outcome.success, "a short-circuited run did not fail");
    assert_eq!(
        outcome.stopped,
        vec!["beta".to_owned(), "gamma".to_owned()],
        "the tasks not reached are not reported, or not in declaration order",
    );
    assert!(
        !work(&outcome, "beta-1").exists(),
        "a task after the short-circuit executed anyway",
    );
}

// COVERS FR-4.9 | edge
/// `short-circuit-failure` on a task that passes stops nothing.
///
/// The field asks for stopping *on failure*, so a run where the carrying task
/// passes is an ordinary run. A bolt reading the field and ignoring the verdict
/// would pass every other test here.
#[test]
fn short_circuit_failure_stops_nothing_when_the_task_passes() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        concat!(
            "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n    short-circuit-failure: true\n",
            "  - name: beta\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");

    assert!(outcome.success, "the run did not pass");
    assert!(
        outcome.stopped.is_empty(),
        "a passing task stopped the run: {:?}",
        outcome.stopped,
    );
    assert!(
        work(&outcome, "beta-1")
            .join(bolt::run::OUTPUT_FILE)
            .is_file(),
        "the task after a passing short-circuit did not execute",
    );
}

// COVERS FR-4.10, FR-4.10a, FR-4.10b, FR-3.10c | negative
/// A command invoking an undeclared tool fails its task, and the run carries on.
///
/// FR-3.10c keeps FR-3.10b narrow: checking `requires` up front is a guarantee
/// about `requires`, not about every way a process fails to launch.
///
/// FR-4.10a settles what the reason may say. Once every declared entry is
/// resolved before anything executes, a declared tool cannot be the one that
/// failed to start, so the reachable case is a command invoking something the
/// jig never declared and there is no entry to name. The reason carries what the
/// shell reported instead.
///
/// FR-4.10b is the consequence: an under-declared jig is visible only as a task
/// that failed, which FR-3.10's inventory rule is what closes. Bolt does not
/// read a command to work out what it invokes.
#[test]
fn a_command_that_cannot_start_fails_its_task_and_the_run_carries_on() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        concat!(
            "  - name: alpha\n    command: \"definitely-not-a-real-tool-9c2b\"\n",
            "  - name: beta\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    // No `requires`, so nothing is declared and FR-3.10b has nothing to catch.
    let outcome =
        bolt::run::run("check", root.path()).expect("this is a failing run, not a refusal");

    assert!(!outcome.success, "a command that cannot start did not fail");
    assert!(
        work(&outcome, "beta-1")
            .join(bolt::run::OUTPUT_FILE)
            .is_file(),
        "the run did not carry on past a task that could not start",
    );

    let envelope = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::OUTPUT_FILE),
        &wrench::schemas::ENVELOPE,
    );
    let reasons = envelope
        .get("reasons")
        .and_then(Value::as_array)
        .expect("a failing execution carries reasons");
    assert!(
        !reasons.is_empty(),
        "the task failed with no reason saying why: {reasons:?}",
    );
}

// ---- the output directory ---------------------------------------------------

// COVERS FR-2.6, FR-2.6a | positive
/// `--output-dir` names where a run writes, and is created with its parents.
///
/// FR-2.6a: a graph node's `.ephemera/` may not exist yet, and making the caller
/// create it first buys nothing. Asserted two levels deep, so a single
/// `create_dir` would fail where `create_dir_all` succeeds.
#[test]
fn a_named_output_directory_is_created_with_its_parents() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );
    let named = root.path().join("build").join("qa");

    let outcome = run_into("check", root.path(), &named).expect("the run completes");

    assert_eq!(
        outcome.output_dir, named,
        "the run did not write where it was told",
    );
    assert!(
        named.join(bolt::run::RESULT_FILE).is_file(),
        "no result in the named directory",
    );
    assert!(
        !fs::read_dir(root.path())
            .expect("the base is readable")
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with(".bolt-")),
        "a named output directory did not stop the default one being written",
    );
}

// COVERS FR-2.6c, FR-2.6d | positive
/// Given no `--output-dir`, a run writes `.bolt-<iso8601>` at its base.
///
/// FR-2.6d wants the filesystem-safe spelling: hyphens where the strict form
/// has colons. A directory name is a path on every platform, and the strict
/// form's colons are legal here and hostile to a Windows checkout.
#[test]
fn the_default_output_directory_is_a_filesystem_safe_stamp_at_the_base() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    let name = outcome
        .output_dir
        .file_name()
        .expect("the run directory has a name")
        .to_string_lossy()
        .into_owned();

    assert!(
        name.starts_with(".bolt-"),
        "the default run directory is not named for bolt: {name}",
    );
    assert!(
        !name.contains(':'),
        "the stamp carries colons, which are hostile to a Windows checkout: {name}",
    );
    assert_eq!(
        outcome.output_dir.parent(),
        Some(
            fs::canonicalize(root.path())
                .expect("the base resolves")
                .as_path()
        ),
        "the default run directory is not at the base",
    );
}

// COVERS FR-2.6e | regression
/// The default run directory ends in the process id of the run that wrote it.
///
/// The stamp is second-granular, so without the id two invocations starting in
/// one second resolve to one directory and FR-2.6b refuses the second. Whether
/// they collide depends on where the wall clock falls, which is why the id is
/// asserted from the child's own pid: a pair straddling a second boundary would
/// pass on difference alone, against a bolt with no id as well as this one.
///
/// Through the binary, because one invocation is one process and the process is
/// what the id separates.
#[test]
fn a_run_directory_is_named_for_the_process_that_wrote_it() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );

    let (first, first_pid) = run_the_binary(root.path());
    let (second, second_pid) = run_the_binary(root.path());

    assert!(
        first.ends_with(&format!("-{first_pid}")),
        "the run directory is not named for the process that wrote it: {first}",
    );
    assert_ne!(
        first_pid, second_pid,
        "the two invocations shared a process, so the case was not exercised",
    );
    assert_ne!(
        first, second,
        "two invocations back to back shared one output directory",
    );
}

/// One invocation of the built binary over `base`, and the pid it ran under.
///
/// The run directory is read back off the result path bolt prints, which is the
/// only place a caller learns it when `--output-dir` was not named.
fn run_the_binary(base: &Path) -> (String, u32) {
    let child = bolt()
        .arg("check")
        .arg(base)
        .stdout(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    let pid = child.id();
    let output = child.wait_with_output().expect("the run finishes");

    assert!(
        output.status.success(),
        "the run did not complete: {output:?}"
    );
    let printed = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let directory = Path::new(&printed)
        .parent()
        .and_then(Path::file_name)
        .expect("the result sits in a run directory")
        .to_string_lossy()
        .into_owned();

    (directory, pid)
}

// COVERS FR-2.6b | negative
/// A named output directory that already holds a run is refused.
///
/// FR-2.6b: writing into one interleaves two runs' evidence, and FR-2.2c's
/// exclusion cannot recognise a directory it did not name. Removing it is the
/// caller's decision, so bolt refuses instead of clearing it.
///
/// An existing but empty directory is not one that holds a run. FR-2.6a
/// describes that case, and FR-2.6b does not refuse it.
#[test]
fn a_named_output_directory_holding_a_run_is_refused() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );
    let named = root.path().join("qa");
    fs::create_dir(&named).expect("an empty directory the caller made");

    run_into("check", root.path(), &named).expect("an empty named directory is not in use");

    let refusal =
        run_into("check", root.path(), &named).expect_err("a second run into it is refused");
    assert!(
        matches!(refusal, bolt::Error::OutputDirectoryInUse(_)),
        "wrong refusal for a directory already holding a run: {refusal:?}",
    );
}

// COVERS FR-2.2c | property
/// A run never walks its own output directory, whatever it was named.
///
/// Knowable because the run created it. The default `.bolt-<iso8601>` is hidden
/// and would be skipped anyway, so this names one that is not: `evidence/` sits
/// in the base, is not hidden, and is not in any `.gitignore`.
///
/// Asserted through the manifest's selection, not the exit status, because a
/// task that matched its own evidence would still pass.
#[test]
fn a_run_does_not_walk_its_own_output_directory() {
    let root = tree();
    write(root.path(), "a.txt", "content");
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0' {all_paths}\"\n    matching: [\"**/*\"]\n",
    );
    // Populated before the run, so the walk would find it if nothing excluded
    // it. Without this the directory does not exist when the walk happens and
    // the exclusion is true by accident, so the test passes against an
    // implementation that excludes nothing.
    let named = root.path().join("evidence");
    write(
        root.path(),
        "evidence/stale.txt",
        "a previous run's leavings",
    );

    let outcome = run_into("check", root.path(), &named).expect("the run completes");
    let manifest = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );
    let matched: Vec<String> = manifest
        .get("selection")
        .and_then(|selection| selection.get("matched"))
        .and_then(Value::as_array)
        .expect("a path-consuming task records what it matched")
        .iter()
        .filter_map(|path| path.as_str().map(str::to_owned))
        .collect();

    assert!(
        !matched.iter().any(|path| path.contains("evidence")),
        "the run walked into its own output directory: {matched:?}",
    );
    assert!(
        matched.iter().any(|path| path.ends_with("a.txt")),
        "the exclusion took the rest of the tree with it: {matched:?}",
    );
}

// COVERS FR-8.9 | positive
/// `result.yaml` records the base the run was pointed at.
///
/// FR-8.9: it is the first thing a reader asks of a result, and FR-9.5c's
/// per-execution manifests answer it only for somebody already inside the run
/// directory. That matters most when the run directory is somewhere else
/// entirely, which `--output-dir` makes ordinary.
#[test]
fn the_result_records_the_base_the_run_was_pointed_at() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );
    let elsewhere = tree();
    let named = elsewhere.path().join("qa");

    let outcome = run_into("check", root.path(), &named).expect("the run completes");
    let result = read_validated(
        &outcome.output_dir.join(bolt::run::RESULT_FILE),
        &wrench::schemas::ENVELOPE,
    );

    let recorded = result
        .get("metadata")
        .and_then(|metadata| metadata.get("base"))
        .and_then(Value::as_str)
        .expect("the result records its base");
    assert_eq!(
        Path::new(recorded),
        fs::canonicalize(root.path()).expect("the base resolves"),
        "the result names the wrong base",
    );
}

// COVERS FR-10.7b, FR-10.3, FR-10.4 | edge
/// Naming an output directory outside the tree gets a result for every refusal.
///
/// FR-10.7a exempts the missing base only while the result would land inside
/// it. FR-10.7b is the way out: a caller wanting a parseable refusal in every
/// case names a directory outside the tree being checked, which a graph node
/// already does by FR-2.6a's `.ephemera/`.
///
/// So this is the same refusal as
/// `the_missing_base_refusal_writes_nothing_and_says_so`, with the one thing
/// changed that the row says changes it.
#[test]
fn a_named_directory_outside_the_base_gets_a_result_for_a_missing_base() {
    let root = tree();
    let absent = root.path().join("not-there");
    let elsewhere = tree();
    let named = elsewhere.path().join("qa");

    let refusal = run_into("check", &absent, &named).expect_err("a missing base is refused");
    assert!(
        matches!(refusal, bolt::Error::BaseMissing(_)),
        "wrong refusal for a missing base: {refusal:?}",
    );
    assert!(
        !absent.exists(),
        "refusing a missing base created it, which is what the refusal was about",
    );

    let result = named.join(bolt::run::RESULT_FILE);
    assert!(
        result.is_file(),
        "FR-10.7b's way out wrote no result, so there is no way out",
    );
    // FR-10.3 and FR-10.4: the verdict is in the envelope and the status says
    // only whether bolt could run, so a refusal reads false here and 1 there.
    assert!(
        !verdict(&result, &wrench::schemas::ENVELOPE),
        "a refusal's result says success: false",
    );
    assert_refusal_shape(&result, "base-missing", "not there");
}

// COVERS FR-10.6 | edge
/// A bolt killed by a signal dies of the signal instead of choosing a status.
///
/// FR-10.6 is the one case where bolt does not pick its own exit status: the
/// shell's convention is 128 plus the signal number, and it is the shell that
/// applies it. So bolt must not intercept the signal and exit a number of its
/// own, which is what this asserts.
///
/// The row is about what a shell sees, so a test calling the entry point in
/// process cannot reach it: a signal delivered to the test harness kills the
/// harness. This spawns the real binary on a jig that blocks, signals it, and
/// reads the wait status.
///
/// `status.code()` is `None` for a signalled process and `signal()` carries the
/// number. Asserting `code() == Some(143)` would be wrong and would pass only
/// against a bolt that had caught the signal, which is the defect.
#[test]
fn a_bolt_killed_by_a_signal_dies_of_the_signal() {
    use std::os::unix::process::ExitStatusExt;

    let root = tree();
    write_jig(
        root.path(),
        "slow",
        "  - name: blocks\n    command: \"sh -c 'sleep 30'\"\n",
    );

    let mut child = bolt()
        .arg("slow")
        .arg(root.path())
        .spawn()
        .expect("bolt starts");

    // Wait for the command to be running, not for a fixed sleep: the
    // work directory appears once the task has started, so polling for it makes
    // the test wait exactly as long as it needs to.
    // A timeout here would leave the rest of the test signalling a bolt that had
    // not started its command, which still dies of the signal and would pass for
    // the wrong reason.
    assert!(
        wait_for_execution(root.path(), "blocks-1"),
        "bolt never reached the blocking task, so the signal proves nothing",
    );

    let killed = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()
        .expect("kill runs");
    assert!(killed.success(), "could not signal bolt");

    let status = child.wait().expect("bolt is reaped");

    assert_eq!(
        status.code(),
        None,
        "bolt chose an exit status where the signal should have carried it",
    );
    assert_eq!(
        status.signal(),
        Some(15),
        "bolt did not die of the signal it was sent",
    );
}

// ---- the three-layer definitions mapping -----------------------------------

/// Write `bolt.<name>.definitions.yaml` into `root`, the config directory.
fn write_definitions(root: &Path, name: &str, body: &str) {
    write(root, &bolt::definitions::file_name(name), body);
}

/// A run naming a definitions file, for the tests that are about the layers.
fn run_with(jig: &str, base: &Path, definitions: &str) -> Result<bolt::Outcome, bolt::Error> {
    bolt::run::invoke(&bolt::run::Invocation {
        jig,
        base,
        definitions: Some(definitions),
        output_dir: None,
        config_dir: None,
    })
    .map_err(bolt::Error::from)
}

/// Wait until `entry`'s work directory appears under a run at `base`.
///
/// Polls instead of sleeping a fixed time, so a test waits exactly as long as
/// it needs to. Returns whether it appeared, so a caller can fail and not carry
/// on against a run that never started.
fn wait_for_execution(base: &Path, entry: &str) -> bool {
    let began = std::time::Instant::now();
    while began.elapsed() < std::time::Duration::from_secs(10) {
        let started = fs::read_dir(base)
            .expect("the base is readable")
            .filter_map(Result::ok)
            .any(|run| {
                run.file_name().to_string_lossy().starts_with(".bolt-")
                    && run.path().join(bolt::run::WORK_DIR).join(entry).exists()
            });
        if started {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    false
}

/// A run naming an output directory, for the tests that are about FR-2.6.
fn run_into(jig: &str, base: &Path, output_dir: &Path) -> Result<bolt::Outcome, bolt::Error> {
    bolt::run::invoke(&bolt::run::Invocation {
        jig,
        base,
        definitions: None,
        output_dir: Some(output_dir),
        config_dir: None,
    })
    .map_err(bolt::Error::from)
}

// COVERS FR-3.15, FR-4.16 | positive
/// A jig's `definitions` block supplies its own placeholders.
///
/// FR-4.16 builds one mapping in three layers. This is the middle one on its
/// own: no file named, so a jig whose defaults cover its placeholders runs
/// without one, which FR-4.16b calls the ordinary case.
///
/// Asserted through the recorded argv, not the exit status, because a command
/// whose placeholder vanished would still exit 0.
#[test]
fn a_jigs_definitions_block_supplies_its_placeholders() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0' {deny}\"\n",
    );
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        "version: \"1.0.0\"\ndefinitions:\n  deny: warnings\ntasks:\n  - name: alpha\n    command: \"sh -c 'exit 0' {deny}\"\n",
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    let manifest = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );

    let command = manifest
        .get("command")
        .and_then(Value::as_str)
        .expect("a manifest records the command as executed");
    assert!(
        command.contains("warnings"),
        "the jig's own definition did not reach the command: {command:?}",
    );
}

// COVERS FR-4.16a, FR-4.16b, FR-4.17 | positive
/// A definitions file merges over the jig's block, key by key.
///
/// FR-4.17 is successive replacement: the file replaces the keys it names and
/// leaves every other one standing, so a project overriding one detail writes
/// that one line and inherits the rest.
#[test]
fn a_definitions_file_replaces_only_the_keys_it_names() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "definitions:\n  deny: warnings\n  requirements: REQUIREMENTS.md\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0' {deny} {requirements}\"\n",
        ),
    );
    write_definitions(root.path(), "override", "deny: clippy::all\n");

    let outcome = run_with("check", root.path(), "override").expect("the run completes");
    let manifest = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );
    let command = manifest
        .get("command")
        .and_then(Value::as_str)
        .expect("a manifest records the command as executed");

    assert!(
        command.contains("clippy::all"),
        "the file did not replace the jig's value: {command:?}",
    );
    assert!(
        !command.contains("warnings"),
        "the jig's replaced value survived: {command:?}",
    );
    assert!(
        command.contains("REQUIREMENTS.md"),
        "a key the file did not name was not left standing: {command:?}",
    );
}

// COVERS FR-9.5g | positive
/// The manifest says which layer supplied each value.
///
/// FR-9.5g: the same key means different things depending on which file won,
/// and the command line alone does not say. Bolt's own locations are already
/// `from: "bolt"`; this adds the other two layers.
#[test]
fn the_manifest_records_which_layer_supplied_each_value() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "definitions:\n  deny: warnings\n  requirements: REQUIREMENTS.md\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0' {deny} {requirements}\"\n",
        ),
    );
    write_definitions(root.path(), "override", "deny: clippy::all\n");

    let outcome = run_with("check", root.path(), "override").expect("the run completes");
    let manifest = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );
    let variables = manifest
        .get("variables")
        .and_then(Value::as_object)
        .expect("a manifest records its variables");

    let layer = |key: &str| {
        variables
            .get(key)
            .and_then(|entry| entry.get("from"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{key} is not in the manifest: {variables:?}"))
    };

    assert_eq!(layer("base_dir"), "bolt", "a location is bolt's layer");
    assert_eq!(
        layer("requirements"),
        "jig",
        "a key only the jig defined is the jig's layer",
    );
    assert_eq!(
        layer("deny"),
        "file",
        "a key the file replaced is the file's layer",
    );
}

// COVERS FR-4.19, FR-4.16d | negative
/// A jig or a file naming a reserved variable refuses the run.
///
/// FR-4.19: `{base_dir}` redefined would substitute something other than where
/// FR-4.1a stands the command, so the jig would say one thing while the process
/// did another. Both layers are checked, because a file can name one the jig did
/// not.
#[test]
fn a_definition_naming_a_reserved_variable_is_refused() {
    let from_jig = tree();
    write(
        from_jig.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "definitions:\n  base_dir: /somewhere/else\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );
    let refusal =
        bolt::run::run("check", from_jig.path()).expect_err("a jig redefining a location refuses");
    assert!(
        matches!(refusal, bolt::Error::ReservedDefinition { .. }),
        "wrong refusal for a jig redefining a location: {refusal:?}",
    );
    assert!(
        refusal.to_string().contains("base_dir"),
        "the reason does not name the variable: {refusal}",
    );

    let from_file = tree();
    write_jig(
        from_file.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );
    write_definitions(from_file.path(), "bad", "each_path: nonsense\n");
    let refusal = run_with("check", from_file.path(), "bad")
        .expect_err("a file redefining a path variable refuses");
    assert!(
        matches!(refusal, bolt::Error::ReservedDefinition { .. }),
        "wrong refusal for a file redefining a path variable: {refusal:?}",
    );
}

// COVERS FR-4.18b | edge
/// A definition holding an empty value is defined.
///
/// FR-4.18 refuses a placeholder no layer holds at all, which is a different
/// state from a layer holding the empty string. A jig wanting a flag to carry
/// nothing says so by defining it, not by leaving it out.
#[test]
fn a_definition_holding_an_empty_value_is_defined() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "definitions:\n  extra: \"\"\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0' {extra}\"\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("an empty definition is defined");
    assert!(outcome.success, "the run did not pass");
}

// COVERS FR-4.17a, FR-4.17c | property
/// A definition's value is a literal and is never re-read as a template.
///
/// FR-4.17a settles every value on reading the file. FR-4.17c is what rests on
/// it: a definition cannot introduce `{each_path}`, so FR-4.2 still reads how a
/// task runs off the command as written, and substitution changes what a
/// command says, never how many times it runs.
///
/// This is the same single-pass property `7e3198f` fixed for paths, reached from
/// the other side. A definition whose value spells a path variable must arrive
/// as those characters.
#[test]
fn a_definition_value_is_a_literal_and_is_not_re_expanded() {
    let root = tree();
    write(root.path(), "a.txt", "content");
    write(
        root.path(),
        &bolt::jig::file_name("check"),
        concat!(
            "version: \"1.0.0\"\n",
            "definitions:\n  sneaky: \"{each_path}\"\n",
            "tasks:\n  - name: alpha\n    command: \"sh -c 'exit 0' {sneaky}\"\n",
        ),
    );

    let outcome = bolt::run::run("check", root.path()).expect("the run completes");
    assert_eq!(
        outcome.executions, 1,
        "a definition naming a path variable changed how many times the task ran",
    );

    let manifest = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );
    let command = manifest
        .get("command")
        .and_then(Value::as_str)
        .expect("a manifest records the command as executed");
    assert!(
        command.contains("{each_path}"),
        "the definition's value was re-expanded rather than kept literal: {command:?}",
    );
    assert!(
        !command.contains("a.txt"),
        "a definition introduced a path variable: {command:?}",
    );
}

// COVERS FR-4.20 | negative
/// A definitions file that will not validate refuses, and is not taken as absent.
///
/// FR-4.20: schema-validated under FR-1.5 like everything else bolt reads as
/// data. Treating an unreadable one as absent would leave the jig's defaults
/// standing and run a gate the caller thought they had overridden.
#[test]
fn a_definitions_file_that_will_not_validate_is_refused() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );
    // Nested, where the schema allows one level of scalars.
    write_definitions(root.path(), "bad", "deny:\n  nested: warnings\n");

    let refusal =
        run_with("check", root.path(), "bad").expect_err("an invalid definitions file refuses");
    assert!(
        matches!(refusal, bolt::Error::DefinitionsUnreadable { .. }),
        "wrong refusal for an invalid definitions file: {refusal:?}",
    );

    let absent = tree();
    write_jig(
        absent.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );
    let refusal = run_with("check", absent.path(), "missing")
        .expect_err("a named file that is not there refuses");
    assert!(
        matches!(refusal, bolt::Error::DefinitionsUnreadable { .. }),
        "a named definitions file that is absent is still a refusal: {refusal:?}",
    );
}

// COVERS FR-4.18a | negative
/// An unknown placeholder refuses before any task executes.
///
/// FR-4.18a puts the check where `requires` is, under FR-3.10b, so a jig run
/// where nothing defines what it needs refuses in the first second, not
/// partway through a gate. Asserted by putting the offending task second and
/// requiring that the first one left no evidence behind.
#[test]
fn an_unknown_placeholder_refuses_before_anything_executes() {
    let root = tree();
    write_jig(
        root.path(),
        "check",
        concat!(
            "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
            "  - name: beta\n    command: \"sh -c 'exit 0' {undefined}\"\n",
        ),
    );

    let refusal = bolt::run::run("check", root.path()).expect_err("an unknown placeholder refuses");
    assert!(
        matches!(refusal, bolt::Error::UnknownPlaceholder { .. }),
        "wrong refusal for an unknown placeholder: {refusal:?}",
    );
    assert!(
        refusal.to_string().contains("undefined"),
        "the reason does not name the placeholder: {refusal}",
    );

    let ran: Vec<_> = fs::read_dir(root.path())
        .expect("the base is readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".bolt-"))
        })
        .filter(|run| run.join(bolt::run::WORK_DIR).join("alpha-1").exists())
        .collect();
    assert!(
        ran.is_empty(),
        "the first task executed before the second was refused: {ran:?}",
    );
}

// ---- time limits -------------------------------------------------------------

/// Write a jig carrying a run-wide `time-limit`, by FR-4.11d.
///
/// The run's limit sits on the jig, which is the one document describing the run
/// as a whole. A task's sits on the task and needs no helper.
fn write_limited_jig(root: &Path, name: &str, limit: &str, tasks: &str) {
    write(
        root,
        &bolt::jig::file_name(name),
        &format!("version: \"1.0.0\"\ntime-limit: \"{limit}\"\ntasks:\n{tasks}"),
    );
}

/// Every reason an envelope or a result carries, as `(kind, message)`.
///
/// Read through `read_validated`, so a caller asking what an envelope says has
/// already asserted that it is one. FR-4.12d is that property, and this is where
/// most of the tests below happen to check it.
fn reasons_in(path: &Path) -> Vec<(String, String)> {
    read_validated(path, &wrench::schemas::ENVELOPE)
        .get("reasons")
        .and_then(Value::as_array)
        .map(|reasons| {
            reasons
                .iter()
                .map(|reason| {
                    let field = |name: &str| {
                        reason
                            .get(name)
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned()
                    };
                    (field("kind"), field("message"))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The envelope one execution wrote.
fn envelope_of(outcome: &bolt::Outcome, entry: &str) -> PathBuf {
    work(outcome, entry).join(bolt::run::OUTPUT_FILE)
}

/// Whether any run under `base` got as far as creating a work directory.
fn executed_anything(base: &Path) -> bool {
    fs::read_dir(base)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".bolt-"))
        })
        .any(|run| {
            fs::read_dir(run.join(bolt::run::WORK_DIR))
                .into_iter()
                .flatten()
                .next()
                .is_some()
        })
}

// COVERS FR-4.11 | positive
/// Both limits are options, so a jig setting neither lets a tool finish.
///
/// The command sleeps for longer than every limit this file sets, so a bolt that
/// applied a ceiling of its own, or that read an absent field as zero, kills it
/// here. Asserted on the command's last line, not on the verdict, because a
/// killed `sleep` also reports failure and the two would be indistinguishable.
#[test]
fn a_jig_setting_no_limit_lets_a_slow_command_finish() {
    let root = tree();
    write_jig(
        root.path(),
        "unbounded",
        "  - name: slow\n    command: \"sh -c 'sleep 0.4; echo done'\"\n",
    );

    let outcome = bolt::run::run("unbounded", root.path()).expect("the run completes");

    assert!(outcome.success, "an unlimited run killed its own command");
    let stdout =
        fs::read_to_string(work(&outcome, "slow-1").join("stdout")).expect("the task wrote stdout");
    assert!(
        stdout.contains("done"),
        "the command did not reach its own last line: {stdout}",
    );
}

// COVERS FR-4.11a, FR-4.11b, FR-4.12f, FR-6.9a | property
/// A task's limit covers all of its executions taken together.
///
/// Each command finishes well inside the limit and the task still runs out,
/// which is the whole of the difference between the two readings. Four paths at
/// 0.3s each against 0.7s: under a per-execution budget every path has more
/// than twice what it needs and all four pass, and
/// under FR-4.11a the third is killed partway and FR-4.11b stops the fourth.
///
/// Commands that each outrun the limit on their own cannot separate the two. A
/// bolt that restarts the budget every execution passes that version, because
/// the first execution is killed either way and FR-4.11b then stops the rest,
/// so nothing downstream can tell the two apart.
#[test]
fn a_tasks_limit_covers_all_its_executions_together() {
    let root = tree();
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        write(root.path(), name, "content");
    }
    write_jig(
        root.path(),
        "budget",
        concat!(
            "  - name: slow\n",
            // Four executions of 0.3s against a budget of 0.7s. Each finishes
            // inside the limit on its own, which is what makes the two readings
            // separable, and four of them cannot. 0.1s against 0.25s is flaky
            // under a loaded suite, because a tenth of a second is the same
            // order as process startup when eighty tests are running.
            "    time-limit: \"0.7s\"\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"sh -c 'sleep 0.3; echo {each_path}'\"\n",
        ),
    );

    let outcome = bolt::run::run("budget", root.path()).expect("the run completes");

    assert!(
        outcome.executions < 4,
        "every execution got the limit to itself: {} ran",
        outcome.executions,
    );
    assert!(
        !work(&outcome, "slow-4").exists(),
        "FR-4.11b: an execution started after the task's limit had passed",
    );
    assert!(!outcome.success, "a task that ran out of budget passed");

    // The killed execution is whichever one the budget ran out under, so the
    // reason is read off the last work directory, not a fixed one.
    let last = format!("slow-{}", outcome.executions);
    let carried = reasons_in(&envelope_of(&outcome, &last));
    // FR-6.9a: the limit is the only reason. Bolt's exit-code adapter does not
    // add `exited -1` beside it, which is bolt's own signal reported as though
    // the tool had chosen it.
    assert_eq!(
        carried
            .iter()
            .map(|(kind, _)| kind.as_str())
            .collect::<Vec<_>>(),
        vec!["time-limit"],
        "the killed execution does not say a limit killed it, and only that: {carried:?}",
    );
    assert!(
        carried[0].1.contains("were not attempted"),
        "FR-4.12f: the reason does not say how many were not attempted: {carried:?}",
    );
}

// COVERS FR-4.13, FR-4.14a | edge
/// A run whose budget is gone before it starts runs nothing, and says so.
///
/// A limit of `0s` puts the deadline in the past at the first check, which is
/// the reachable case for the test bolt makes before each task as well as after
/// one. Without that check the first task starts and is killed a moment later.
/// That looks similar and is not, because it leaves a work directory, an
/// execution and a killed process behind.
///
/// It is also the one shape where the merge finds no constituent at all and must
/// still write a result, so it is where FR-4.14a is separable from FR-8.3a. That
/// row refuses a run with nothing folded, because a green result over zero
/// checks reads as checked and fine; a run carrying its own reason is not green.
///
/// And it is the only place the run's own reason can be told apart from the ones
/// its executions carry, since there are no executions. A test for FR-4.13 with
/// a killed execution passes against a merge that drops the run's reason
/// entirely, because it finds the same words on that execution's envelope.
#[test]
fn a_run_with_no_budget_left_executes_nothing_and_still_writes_a_result() {
    let root = tree();
    write_limited_jig(
        root.path(),
        "spent",
        "0s",
        concat!(
            "  - name: first\n    command: \"sh -c 'echo never'\"\n",
            "  - name: second\n    command: \"sh -c 'echo never'\"\n",
        ),
    );

    let outcome = bolt::run::run("spent", root.path()).expect("the run completes");

    assert_eq!(
        outcome.executions, 0,
        "a run with no budget left executed something",
    );
    assert_eq!(
        outcome.stopped,
        vec!["first".to_owned(), "second".to_owned()],
        "the tasks that never ran are not recorded",
    );
    assert!(
        !outcome.success,
        "a run that checked nothing reported success"
    );

    let carried = reasons_in(&outcome.output_dir.join(bolt::run::RESULT_FILE));
    assert_eq!(
        carried.len(),
        1,
        "the result carries something other than the run's own reason: {carried:?}",
    );
    assert!(
        carried[0].0 == "time-limit" && carried[0].1.contains("the run passed its time limit"),
        "FR-4.14a: the result does not say why nothing ran: {carried:?}",
    );
}

// COVERS FR-4.11c, FR-4.12a, FR-4.12b, FR-7.5c | positive
/// The adapter runs after the limit fired, over what the command had gathered.
///
/// FR-4.11c: the limit governs commands, and the adapter is what records that it
/// fired, so a budget the command exhausted does not also kill the adapter.
/// FR-4.12a: a tool that reported a problem before it hung reported a real
/// problem. FR-4.12b: the execution fails anyway.
///
/// The adapter concludes success and leaves a note of what it read, so the
/// two halves are separable. The note proves it ran and saw the partial output;
/// the verdict proves bolt overrode what it concluded. A test asserting only the
/// verdict cannot tell those apart, because a bolt that never ran the adapter
/// also fails the execution.
#[test]
fn a_killed_command_keeps_its_output_and_its_adapter_still_runs() {
    let root = tree();
    write_adapter(
        root.path(),
        "noting-adapter",
        concat!(
            "for a in \"$@\"; do case $prev in --stdout) out=$a;; --work-dir) w=$a;; esac; prev=$a; done\n",
            "cp \"$out\" \"$w/adapter-saw\"\n",
            "printf '\"success\": true\\n' > \"$w/output.yaml\"\n",
        ),
    );
    write_jig(
        root.path(),
        "hangs",
        concat!(
            "  - name: reporting\n",
            // Half a second. The command has to reach its `echo` before the
            // limit fires, and under a suite running eighty tests at once fifty
            // milliseconds is not enough for process startup: measured flaky,
            // roughly one run in five, reporting an empty capture. The sleep is
            // a hundredfold the limit either way, so the choice does not change
            // what is being tested.
            "    time-limit: \"0.5s\"\n",
            "    adapter: noting-adapter\n",
            "    command: \"sh -c 'echo forty-problems; sleep 5'\"\n",
        ),
    );

    let outcome = bolt::run::run("hangs", root.path()).expect("the run completes");

    let saw = fs::read_to_string(work(&outcome, "reporting-1").join("adapter-saw"))
        .expect("FR-4.11c: the adapter did not run once the limit had fired");
    assert!(
        saw.contains("forty-problems"),
        "FR-4.12a: the adapter did not see what the command gathered: {saw}",
    );

    assert!(!outcome.success, "a timed-out task passed the run");
    let carried = reasons_in(&envelope_of(&outcome, "reporting-1"));
    assert!(
        carried.iter().any(|(kind, _)| kind == "time-limit"),
        "FR-4.12b: the adapter's success stood: {carried:?}",
    );
}

// COVERS FR-4.12, FR-4.11d | negative
/// A task that passes its limit fails, and the run carries on past it.
///
/// FR-4.8's rule holds for a slow task exactly as it does for a failing one: the
/// tasks after it still execute, because stopping throws away the evidence they
/// would have produced.
///
/// Asserted on `stopped` and on the following task's envelope, not on a count of
/// executions. A 0.05s limit can expire during the task's own setup, and
/// FR-4.11b's check then reports the task as having run nothing where a limit
/// expiring during the command reports one. Both discharge FR-4.12, so a count
/// asserts which of two legal paths the clock took. Measured, it gives 1 under
/// an instrumented build inside a gate run and 2 everywhere else.
#[test]
fn a_slow_task_fails_and_the_run_carries_on() {
    let root = tree();
    write_jig(
        root.path(),
        "carries-on",
        concat!(
            "  - name: hangs\n",
            "    time-limit: \"0.05s\"\n",
            "    command: \"sh -c 'sleep 5'\"\n",
            "  - name: after\n",
            "    command: \"sh -c 'echo reached'\"\n",
        ),
    );

    let outcome = bolt::run::run("carries-on", root.path()).expect("the run completes");

    assert!(
        !outcome.success,
        "a task that passed its limit did not fail"
    );
    assert!(
        outcome.stopped.is_empty(),
        "a task's limit stopped the whole run: {:?}",
        outcome.stopped,
    );
    assert!(
        verdict(
            &envelope_of(&outcome, "after-1"),
            &wrench::schemas::ENVELOPE
        ),
        "the task after the slow one did not pass",
    );

    let carried = reasons_in(&envelope_of(&outcome, "hangs-1"));
    assert!(
        carried
            .iter()
            .any(|(kind, message)| kind == "time-limit" && message.contains("0.05s")),
        "no reason names the limit that was passed: {carried:?}",
    );
}

// COVERS FR-4.12c, FR-4.12d | edge
/// Where the run's limit catches the adapter, bolt writes that envelope itself.
///
/// FR-4.11c keeps a task's limit off its adapter; the run's is the one that can
/// still reach it, and then nothing else is left to write an envelope. FR-4.12d
/// is what that guarantees: a timed-out execution has a valid one whichever of
/// the two was running, which is what distinguishes it from an adapter that died
/// of its own accord and left none.
#[test]
fn the_runs_limit_catching_an_adapter_leaves_bolt_to_write_the_envelope() {
    let root = tree();
    write_adapter(root.path(), "hanging-adapter", "sleep 5\n");
    write_limited_jig(
        root.path(),
        "slow-adapter",
        "0.5s",
        concat!(
            "  - name: quick\n",
            "    adapter: hanging-adapter\n",
            "    command: \"sh -c 'echo done'\"\n",
        ),
    );

    let outcome = bolt::run::run("slow-adapter", root.path()).expect("the run completes");

    let envelope = envelope_of(&outcome, "quick-1");
    assert!(
        envelope.is_file(),
        "FR-4.12d: the killed adapter left no envelope and bolt wrote none",
    );
    assert!(
        !verdict(&envelope, &wrench::schemas::ENVELOPE),
        "a timed-out execution reported success",
    );
    let carried = reasons_in(&envelope);
    assert!(
        carried
            .iter()
            .any(|(kind, message)| kind == "time-limit" && message.contains("0.5s")),
        "bolt's envelope does not say a limit caught the adapter: {carried:?}",
    );
}

// COVERS FR-4.13, FR-4.14, FR-4.14a, FR-4.11d | negative
/// A run that passes its limit fails, and still writes what it managed.
///
/// FR-4.14a is the half that most needs asserting: the task that finished before
/// the limit is still in the evidence mapping with its own verdict. A run that
/// reported only the timeout would discard evidence already written and paid
/// for.
#[test]
fn a_run_that_times_out_writes_a_result_carrying_what_completed() {
    let root = tree();
    write_limited_jig(
        root.path(),
        "bounded",
        "0.5s",
        concat!(
            "  - name: first\n    command: \"sh -c 'echo quick'\"\n",
            "  - name: hangs\n    command: \"sh -c 'sleep 5'\"\n",
            "  - name: third\n    command: \"sh -c 'echo never'\"\n",
        ),
    );

    let outcome = bolt::run::run("bounded", root.path()).expect("the run completes");

    assert!(!outcome.success, "a run that passed its limit did not fail");
    assert_eq!(
        outcome.stopped,
        vec!["third".to_owned()],
        "the tasks the limit kept from running are not recorded",
    );

    let result = outcome.output_dir.join(bolt::run::RESULT_FILE);
    let carried = reasons_in(&result);
    assert!(
        carried.iter().any(|(kind, message)| kind == "time-limit"
            && message.contains("the run")
            && message.contains("0.5s")),
        "FR-4.13: the result does not say the run passed its limit: {carried:?}",
    );

    let keys: Vec<String> = read_validated(&result, &wrench::schemas::ENVELOPE)
        .get("metadata")
        .and_then(|metadata| metadata.get("evidence"))
        .and_then(Value::as_object)
        .map(|evidence| evidence.keys().cloned().collect())
        .expect("the result carries an evidence mapping");
    assert!(
        keys.contains(&"first-1".to_owned()),
        "FR-4.14a: what completed before the limit is missing from the result: {keys:?}",
    );
    assert!(
        verdict(
            &envelope_of(&outcome, "first-1"),
            &wrench::schemas::ENVELOPE
        ),
        "the completed task's own verdict did not survive the timeout",
    );
}

// COVERS FR-4.12e | regression
/// A killed command takes the children it spawned with it.
///
/// The command backgrounds a loop appending to a file every twenty milliseconds
/// and then blocks. Signalling the child alone leaves that loop running, writing
/// into the tree after bolt has finished with it, so the file goes on growing
/// once the run has returned. `SIGKILL` to the process group stops it.
///
/// Asserted twice. The file not growing is the property the row is about, and a
/// pid check alone would pass against a child that was reparented and still
/// writing. The group being empty catches the opposite case: a survivor whose
/// directory has gone stops writing, since its `>>` fails, and goes on forking
/// with nothing to show for it.
#[test]
fn a_timed_out_command_leaves_no_children_running() {
    let root = tree();
    write_jig(
        root.path(),
        "spawner",
        concat!(
            "  - name: forks\n",
            "    time-limit: \"0.5s\"\n",
            "    command: \"sh -c 'read _ _ _ _ g _ < /proc/self/stat; echo $g > group.txt; while : ; do echo tick >> ticks.txt; sleep 0.02; done & sleep 5'\"\n",
        ),
    );

    let outcome = bolt::run::run("spawner", root.path()).expect("the run completes");
    assert!(!outcome.success, "the timed-out task passed");

    let ticks = root.path().join("ticks.txt");
    let size = || fs::metadata(&ticks).map_or(0, |meta| meta.len());

    let when_the_run_returned = size();
    assert!(
        when_the_run_returned > 0,
        "the child never ran, so this proves nothing about killing it",
    );

    std::thread::sleep(std::time::Duration::from_millis(300));
    let after_waiting = size();

    let group = fs::read_to_string(root.path().join("group.txt"))
        .expect("the command recorded its process group");
    let group = group.trim();
    let survivors = members_of_group(group);
    if !survivors.is_empty() {
        // Killed before either assertion, so a leak fails the suite once and
        // leaves nothing forking after it.
        let _ = std::process::Command::new("kill")
            .args(["-9", "--", &format!("-{group}")])
            .status();
    }

    assert!(
        survivors.is_empty(),
        "processes in the killed group outlived it: {survivors:?}",
    );
    assert_eq!(
        after_waiting, when_the_run_returned,
        "an orphaned child was still writing after the run returned",
    );
}

/// The pids of live processes in `group`, read from `/proc`.
///
/// A zombie is left out: it has exited and waits only to be reaped by whoever
/// inherited it. The command name in `stat` is parenthesised and may hold
/// spaces, so fields are counted from its closing parenthesis: state, parent,
/// group.
fn members_of_group(group: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| fs::read_to_string(entry.path().join("stat")).ok())
        .filter_map(|stat| {
            let (pid, rest) = stat.split_once(' ')?;
            let (_, fields) = rest.rsplit_once(')')?;
            let fields: Vec<&str> = fields.split_whitespace().collect();
            (fields.first() != Some(&"Z") && fields.get(2) == Some(&group)).then(|| pid.to_owned())
        })
        .collect()
}

// COVERS FR-4.11e | negative
/// A limit that is not a duration refuses the run before anything executes.
///
/// Declared on the second task, so a bolt reading limits as it reached them
/// would run the first one first. The assertion is that no work directory
/// exists, which is what separates a check made up front from one made in
/// passing; the exit status is the same either way.
///
/// The refusal arrives from wrench's schema, not from bolt. The `time-limit`
/// pattern is in `jig.schema.json` from wrench `dbc3570`, so a jig writing `30`
/// is refused at validation one layer before the runner reads it. That is
/// FR-1.5 doing its job and is the better place for it: bolt never sees a jig
/// the schema would not accept.
///
/// `Error::MalformedTimeLimit` is kept and is not dead. It is what catches
/// wrench's pattern and `limit::parse` drifting apart, and the two are the same
/// regex by agreement, not by construction. The grammar half is
/// asserted directly against `limit::parse` by the property test below, so the
/// pair still covers both layers: that one checks bolt agrees, this one checks
/// the document is stopped.
#[test]
fn a_time_limit_that_is_not_a_duration_refuses_the_run() {
    let root = tree();
    write_jig(
        root.path(),
        "malformed",
        concat!(
            "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
            "  - name: beta\n    time-limit: \"30\"\n    command: \"sh -c 'exit 0'\"\n",
        ),
    );

    let refusal = bolt::run::run("malformed", root.path()).expect_err("a malformed limit refuses");
    assert!(
        matches!(refusal, bolt::Error::JigUnreadable { .. }),
        "wrong refusal for a malformed limit: {refusal:?}",
    );
    let said = refusal.to_string();
    assert!(
        said.contains("time-limit") && said.contains("30"),
        "the reason names neither the field nor what was written: {said}",
    );
    // Bolt's own guard still refuses the same string, so the two layers agree.
    // A pattern loosened in wrench without this moving would show up here.
    assert!(
        bolt::limit::parse("30").is_none(),
        "bolt's grammar accepts what the schema refuses",
    );
    assert!(
        !executed_anything(root.path()),
        "a task executed before the malformed limit was refused",
    );

    // The jig's own limit is refused the same way, and the reason points at the
    // jig's field instead of a task's. In a tree of its own, because the
    // refusal above wrote its result into this one's default output directory
    // and FR-2.6b refuses a second run that lands on the same second.
    let other = tree();
    write_limited_jig(
        other.path(),
        "malformed-run",
        "soon",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );
    let refusal =
        bolt::run::run("malformed-run", other.path()).expect_err("a malformed run limit refuses");
    let said = refusal.to_string();
    assert!(
        said.contains("'/time-limit'"),
        "the jig's own limit was not located at the document root: {said}",
    );
    assert!(
        !said.contains("/tasks/"),
        "the jig's own limit was reported as a task's: {said}",
    );
}

// COVERS FR-4.11e | property
/// A limit is a decimal followed by `s`, `m` or `h`, and nothing else is one.
///
/// The rejections are what this tests. `f64` parsing on its own takes `1e3`, `+5`,
/// `inf` and `NaN`, none of which anybody writes in a jig on purpose, and
/// accepting them would make the grammar something a second implementation has
/// to discover instead of read.
#[test]
fn a_time_limit_is_a_decimal_and_a_unit() {
    use std::time::Duration;

    for (written, expected) in [
        ("30s", Duration::from_secs(30)),
        ("0.05s", Duration::from_millis(50)),
        (".5s", Duration::from_millis(500)),
        ("1.5m", Duration::from_secs(90)),
        ("2h", Duration::from_hours(2)),
        ("0s", Duration::ZERO),
    ] {
        assert_eq!(
            bolt::limit::parse(written),
            Some(expected),
            "{written} did not read as a duration",
        );
    }

    // `5.s` is the one Rust would parse and this does not. The whole grammar is
    // then `^[0-9]*\.?[0-9]+[smh]$`, which a schema can carry in one line
    // without restating the reference implementation's judgement.
    for written in [
        "30", "", "s", "30x", "1e3s", "+5s", "-1s", "1.2.3s", "infs", "NaNs", " 30s", "30 s",
        "5.s", ".s",
    ] {
        assert_eq!(
            bolt::limit::parse(written),
            None,
            "{written} was taken for a duration",
        );
    }
}

// COVERS FR-4.11f | property
/// A task's limit is wall clock from when the task started.
///
/// So the adapters between its executions spend it, even though FR-4.11c keeps
/// the limit from killing one. Three paths whose commands are instant and whose
/// adapter takes a fifth of a second each: under wall-clock accounting the
/// budget is gone before the third starts, and under an accounting that charged
/// only the commands all three would run with almost the whole limit unspent.
#[test]
fn a_tasks_limit_is_wall_clock_and_its_adapters_spend_it() {
    let root = tree();
    for name in ["a.txt", "b.txt", "c.txt"] {
        write(root.path(), name, "content");
    }
    write_adapter(
        root.path(),
        "slow-adapter",
        concat!(
            "for a in \"$@\"; do case $prev in --work-dir) w=$a;; esac; prev=$a; done\n",
            "sleep 0.3\n",
            "printf '\"success\": true\\n' > \"$w/output.yaml\"\n",
        ),
    );
    write_jig(
        root.path(),
        "adapted",
        concat!(
            "  - name: each\n",
            "    time-limit: \"0.5s\"\n",
            "    adapter: slow-adapter\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"sh -c 'echo {each_path}'\"\n",
        ),
    );

    let outcome = bolt::run::run("adapted", root.path()).expect("the run completes");

    assert_eq!(
        outcome.executions, 2,
        "the task's adapters did not spend its budget",
    );

    // FR-9.5a and FR-4.12f together: the execution that never started still
    // records what it was going to attempt, and says how many were left.
    assert!(
        work(&outcome, "each-3")
            .join(bolt::run::MANIFEST_FILE)
            .is_file(),
        "an execution that never started recorded nothing it was going to do",
    );
    let carried = reasons_in(&envelope_of(&outcome, "each-3"));
    assert!(
        carried.iter().any(|(kind, message)| kind == "time-limit"
            && message.contains("1 of its executions were not attempted")),
        "the unattempted execution carries no count: {carried:?}",
    );
}

// COVERS FR-4.12d | property
/// Every timed-out execution has a valid envelope, however the limit caught it.
///
/// Two shapes in one run: an execution killed mid-command, and one that never
/// started because the budget had already gone. A limit of `0s` gives the second
/// deterministically, with no sleep to race against.
#[test]
fn every_timed_out_execution_has_a_valid_envelope() {
    let root = tree();
    write(root.path(), "a.txt", "content");
    write(root.path(), "b.txt", "content");
    write_jig(
        root.path(),
        "shapes",
        concat!(
            "  - name: killed\n",
            "    time-limit: \"0.05s\"\n",
            "    command: \"sh -c 'sleep 5'\"\n",
            "  - name: never\n",
            "    time-limit: \"0s\"\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"sh -c 'echo {each_path}'\"\n",
        ),
    );

    let outcome = bolt::run::run("shapes", root.path()).expect("the run completes");

    for entry in ["killed-1", "never-1"] {
        let envelope = envelope_of(&outcome, entry);
        assert!(envelope.is_file(), "{entry} left no envelope");
        assert!(
            !verdict(&envelope, &wrench::schemas::ENVELOPE),
            "{entry} timed out and reported success",
        );
        let carried = reasons_in(&envelope);
        assert!(
            carried.iter().any(|(kind, _)| kind == "time-limit"),
            "{entry} does not say a limit caught it: {carried:?}",
        );
    }

    assert_eq!(
        outcome.executions, 1,
        "the task whose budget was already gone executed something",
    );
}

// ---- the depth ceiling ------------------------------------------------------

/// Write a jig whose one task runs bolt again on the jig named `next`.
///
/// The chain FR-5.6 and FR-5.7a describe: a task command invoking bolt directly,
/// not through a jig task. The binary's own path comes from Cargo, and
/// each link names its own output directory so the runs do not collide under
/// FR-2.6b when two land in the same second.
fn write_recursing_jig(root: &Path, name: &str, next: &str) {
    write(
        root,
        &bolt::jig::file_name(name),
        &format!(
            "version: \"1.0.0\"\ntasks:\n  - name: deeper\n    command: \"{} {next} {{base_dir}} \
             --output-dir {{work_dir}}/nested\"\n",
            env!("CARGO_BIN_EXE_bolt"),
        ),
    );
}

// COVERS FR-5.6, FR-5.7, FR-5.7b, FR-5.8 | negative
/// Bolt inside bolt is stopped at the ceiling, and the refusal writes a result.
///
/// A chain of bolts, each running the next, until one is past the ceiling. It
/// never reads a jig: it refuses, writes its own result, and exits non-zero.
///
/// This needs no nested jigs. FR-5.6 carries the depth in the environment of
/// every process bolt spawns, not only of child jigs, precisely so a task
/// command invoking bolt is at depth too. A test waiting for jig tasks would be
/// testing a narrower rule than the one written.
///
/// The first link clears the depth and sets the ceiling to two, so the chain is
/// the same length however deep this suite is already running. Without that it
/// passes for a person and fails under `bolt rust-quality .`, because the gate
/// exports its own depth into the `tests` command and every link shifts by one.
/// NFR-12.1 makes the suite reachable from the gate, and the Go build's
/// `runner/60` discharge records the trap in that: any test that invokes the
/// gate is reachable from the gate.
///
/// Setting the ceiling also asserts more than the default would: FR-5.7 has it
/// read from the environment, and a hard-coded four would pass against a bolt
/// that ignored the variable.
///
/// The failure travels exactly one level, which the test plan explains.
#[test]
fn bolt_inside_bolt_is_stopped_at_the_ceiling() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("one"),
        &format!(
            "version: \"1.0.0\"\ntasks:\n  - name: deeper\n    command: \"env -u {} {}=2 {} two \
             {{base_dir}} --output-dir {{work_dir}}/nested\"\n",
            bolt::depth::DEPTH,
            bolt::depth::CEILING,
            env!("CARGO_BIN_EXE_bolt"),
        ),
    );
    write_recursing_jig(root.path(), "two", "three");
    write_recursing_jig(root.path(), "three", "four");
    write_jig(root.path(), "four", "  - name: n\n    command: \"true\"\n");

    let outcome = bolt::run::run("one", root.path()).expect("the outermost run completes");

    // Each link's output directory sits in the previous one's work directory,
    // named by the `--output-dir` its command carried. `two` is depth 1 and
    // `four` is depth 3, one past the ceiling the first link set.
    let nested = |at: &Path| at.join(bolt::run::WORK_DIR).join("deeper-1").join("nested");
    let three = nested(&work(&outcome, "deeper-1").join("nested"));
    let refused = nested(&three);

    let carried = reasons_in(&refused.join(bolt::run::RESULT_FILE));
    assert!(
        carried
            .iter()
            .any(|(kind, message)| kind == "depth-exceeded" && message.contains("limit is 2")),
        "FR-5.8: the refused run's reason does not name the limit: {carried:?}",
    );
    assert!(
        !refused.join(bolt::run::WORK_DIR).exists(),
        "the refused run executed a task before refusing",
    );
    assert!(
        !verdict(
            &three.join(bolt::run::RESULT_FILE),
            &wrench::schemas::ENVELOPE
        ),
        "the run that invoked the refused one did not fail",
    );
    assert!(
        outcome.success,
        "FR-10.1: a bolt whose task failed still exits 0, so the run above sees \
         a command that succeeded. If this fails, that rule changed.",
    );
}

// COVERS FR-5.7a, FR-5.7c | edge
/// The ceiling is a guard against accident, and the row says so.
///
/// A command can unset the variable and be believed outermost. Asserted, not
/// left implied, because a reader meeting the depth code could reasonably
/// take it for a security boundary and build on it as though it held.
///
/// Closing this needs the ancestry cross-check, which is question 24 and not a
/// row. The test exists to record that the hole is known and deliberate: if
/// somebody closes it, this fails and they find the reasoning.
#[test]
fn unsetting_the_depth_is_believed_because_the_guard_is_against_accident() {
    let root = tree();
    write(
        root.path(),
        &bolt::jig::file_name("launder"),
        &format!(
            "version: \"1.0.0\"\ntasks:\n  - name: resets\n    command: \"env -u {} -u {} {} inner \
             {{base_dir}} --output-dir {{work_dir}}/nested\"\n",
            bolt::depth::DEPTH,
            bolt::depth::CEILING,
            env!("CARGO_BIN_EXE_bolt"),
        ),
    );
    write_jig(
        root.path(),
        "inner",
        "  - name: runs\n    command: \"sh -c 'exit 0'\"\n",
    );

    let outcome = bolt::run::run("launder", root.path()).expect("the run completes");

    assert!(
        outcome.success,
        "a child that cleared the depth was not believed outermost, which would \
         mean the guard has become something the row says it is not",
    );

    // A value that will not parse is treated as absent for the same reason. A
    // caller's environment is not a document bolt was asked to validate, and
    // refusing on stray shell state would fail runs while stopping nobody who
    // meant it.
    let garbled = tree();
    write(
        garbled.path(),
        &bolt::jig::file_name("garbled"),
        &format!(
            "version: \"1.0.0\"\ntasks:\n  - name: nonsense\n    command: \"env {}=deep {} inner \
             {{base_dir}} --output-dir {{work_dir}}/nested\"\n",
            bolt::depth::DEPTH,
            env!("CARGO_BIN_EXE_bolt"),
        ),
    );
    write_jig(
        garbled.path(),
        "inner",
        "  - name: runs\n    command: \"true\"\n",
    );

    let outcome = bolt::run::run("garbled", garbled.path()).expect("the run completes");
    assert!(
        outcome.success,
        "a depth that will not parse was not read as absent",
    );
}

// COVERS FR-5.6a | positive
/// The depth reaches every process bolt spawns, under the agreed names.
///
/// Not only the ones that are bolt. FR-5.6 says every process, which is what
/// makes the depth survive a command that backgrounds something or invokes bolt
/// three layers into a shell script.
#[test]
fn every_spawned_process_is_told_the_depth() {
    let root = tree();
    write_jig(
        root.path(),
        "reports",
        &format!(
            "  - name: says\n    command: \"sh -c 'echo ${}/${}'\"\n",
            bolt::depth::DEPTH,
            bolt::depth::CEILING,
        ),
    );

    let outcome = bolt::run::run("reports", root.path()).expect("the run completes");

    // Against what this run's own depth is, not against 1. The suite is
    // reachable from the gate: bolt running its own jig exports the variables
    // to the `tests` command, so the depth here is 1 for a person and 2 under
    // `bolt rust-quality .`. NFR-12.1 makes that the ordinary case, and a test
    // hard-coding 1 fails only in the run that matters.
    let mine = bolt::depth::Depth::from_environment();
    let said = fs::read_to_string(work(&outcome, "says-1").join("stdout")).expect("stdout");
    assert_eq!(
        said.trim(),
        format!("{}/{}", mine.level, mine.ceiling),
        "an ordinary command was not told the depth and the ceiling",
    );
}

// ---- the jig task, and where jigs are found ---------------------------------

// COVERS FR-2.8 | positive
/// A config directory says where jigs are found, so nothing has to infer it.
///
/// Asserted with the jig in a tree the run never touches, so a bolt still
/// deriving the config directory from the base cannot find it at all. The
/// substituted `{config_dir}` is checked too: finding the jig and telling a
/// command where it came from are two things, and a bolt that hard-coded the
/// base into the locations would pass the first half.
///
/// Driven through the built binary as well as through the call, because the flag
/// and the field are separate work and a test of one is not a test of the other.
#[test]
fn a_config_directory_says_where_jigs_are_found() {
    let root = tree();
    let elsewhere = tree();
    write(root.path(), "a.txt", "content");
    write_jig(
        elsewhere.path(),
        "remote",
        "  - name: says\n    command: \"sh -c 'echo {config_dir}'\"\n",
    );

    let outcome = bolt::run::invoke(&bolt::run::Invocation {
        jig: "remote",
        base: root.path(),
        definitions: None,
        output_dir: None,
        config_dir: Some(elsewhere.path()),
    })
    .expect("a jig in the config directory is found");

    assert!(outcome.success, "the run did not pass");
    let said = fs::read_to_string(work(&outcome, "says-1").join("stdout")).expect("stdout");
    assert_eq!(
        said.trim(),
        elsewhere
            .path()
            .canonicalize()
            .expect("the config directory resolves")
            .display()
            .to_string(),
        "{{config_dir}} did not substitute to where the jig was found",
    );

    // The flag, which is the other half. A second tree because FR-2.6b refuses
    // a second run landing on the same second-granular output directory.
    let again = tree();
    write(again.path(), "a.txt", "content");
    let ran = bolt()
        .arg("--config-dir")
        .arg(elsewhere.path())
        .arg("remote")
        .arg(again.path())
        .output()
        .expect("bolt runs");
    assert!(
        ran.status.success(),
        "--config-dir was not accepted: {}",
        String::from_utf8_lossy(&ran.stderr),
    );
}

/// A tree holding a subproject, its jig, and an adapter that carries a child's
/// verdict up.
///
/// Shared by the two composition tests so that the thing under test is the same
/// tree in both: one asserts the fold, the other asserts that the same jig run
/// by hand reaches the same verdict, and the claim only means something if
/// neither gets its own slightly different fixture.
///
/// The adapter is FR-5.19 in nine lines of shell. It reads the result path off
/// the stdout bolt printed, reads the child's verdict from there, and writes an
/// envelope. Nothing about it is bolt-specific beyond knowing that the first
/// line of stdout is a path to a result.
fn composing_tree() -> TempDir {
    let root = tree();
    write(root.path(), "sub/a.txt", "content");
    write_jig(
        root.path(),
        "inner",
        "  - name: inner-check\n    command: \"sh -c 'exit 3'\"\n",
    );
    write_adapter(
        root.path(),
        "result-adapter",
        concat!(
            "for a in \"$@\"; do case $prev in --stdout) out=$a;; --work-dir) w=$a;; esac; prev=$a; done\n",
            "child=$(cat \"$out\")\n",
            "if grep -q '\"success\": true' \"$child\"; then\n",
            "  printf '\"success\": true\\n' > \"$w/output.yaml\"\n",
            "else\n",
            "  printf '\"success\": false\\n\"reasons\":\\n  - \"kind\": \"child-failed\"\\n    \"message\": \"%s\"\\n' \"$child\" > \"$w/output.yaml\"\n",
            "fi\n",
        ),
    );
    write_jig(
        root.path(),
        "outer",
        &format!(
            "  - name: subproject\n    command: \"env -u {} {}=3 {} inner {{base_dir}}/sub \
             --config-dir {{config_dir}} --output-dir {{work_dir}}/child\"\n    adapter: \
             result-adapter\n",
            bolt::depth::DEPTH,
            bolt::depth::CEILING,
            env!("CARGO_BIN_EXE_bolt"),
        ),
    );
    root
}

// COVERS FR-5.1a, FR-5.1b | property
/// The same jig on the same directory reaches the same verdict, composed or by
/// hand.
///
/// That is the whole of "a child run is not a mode". Composition puts a bolt
/// invocation on a command line, and a person putting the same invocation in a
/// terminal is doing the identical thing, so there is one code path because
/// there was never a second one to keep in step.
///
/// The composed run's child result and this one
/// are compared on the reason text, so a bolt that treated an invocation from a
/// jig differently would show it here and not only in a comment.
///
/// Run into a directory of its own, because FR-2.6c would otherwise put a
/// second `.bolt-…` inside the tree the first run walked.
#[test]
fn a_jig_run_by_hand_reaches_what_composition_reached() {
    let root = composing_tree();
    let alone = tree();
    let out = alone.path().join("out");

    let direct = bolt::run::invoke(&bolt::run::Invocation {
        jig: "inner",
        base: &root.path().join("sub"),
        definitions: None,
        output_dir: Some(&out),
        config_dir: Some(root.path()),
    })
    .expect("the same jig run by hand completes");

    assert!(!direct.success, "the child jig was supposed to fail");
    let by_hand = fs::read_to_string(direct.output_dir.join("result.yaml")).expect("the result");
    assert!(
        by_hand.contains("inner-check exited 3"),
        "the jig run directly reached a different verdict: {by_hand}",
    );
}

// COVERS FR-5.9 | property
/// A child's paths are absolute even when its parent wrote them relative.
///
/// The parent's command hands the child a relative base, config directory and
/// output directory. The child's result and manifest name them absolutely, so
/// they mean the same thing read from the parent's work directory as from the
/// child's own.
#[test]
fn a_childs_paths_are_absolute_whatever_its_parent_wrote() {
    let root = tree();
    write(root.path(), "sub/a.txt", "a");
    write_jig(
        root.path(),
        "inner",
        "  - name: inner-check\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
    );
    write_jig(
        root.path(),
        "outer",
        &format!(
            "  - name: subproject\n    command: \"env -u {} {}=3 {} inner sub --config-dir . \
             --output-dir child-run\"\n",
            bolt::depth::DEPTH,
            bolt::depth::CEILING,
            env!("CARGO_BIN_EXE_bolt"),
        ),
    );

    bolt::run::run("outer", root.path()).expect("the parent completes");

    let child = root.path().join("child-run");
    let result = read_validated(
        &child.join(bolt::run::RESULT_FILE),
        &wrench::schemas::ENVELOPE,
    );
    let base = result["metadata"]["base"]
        .as_str()
        .expect("the child's base");
    assert!(Path::new(base).is_absolute(), "the child's base: {base}");
    let evidence = result["metadata"]["evidence"]["inner-check-1"]["result"]
        .as_str()
        .expect("the child's evidence");
    assert!(
        Path::new(evidence).is_absolute(),
        "the child's evidence: {evidence}"
    );

    let manifest = read_validated(
        &child
            .join(bolt::run::WORK_DIR)
            .join("inner-check-1")
            .join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );
    for (key, entry) in manifest["variables"].as_object().expect("variables") {
        if key.ends_with("_dir") || key == "project_root" || key == "each_path" {
            let value = entry["value"].as_str().expect("a string value");
            assert!(
                Path::new(value.trim_matches('\'')).is_absolute(),
                "{key} was recorded as {value}",
            );
        }
    }
}

// COVERS FR-5.18, FR-5.19, FR-5.20 | positive
/// Bolt composes with itself as a command, and the child's verdict folds in.
///
/// This is the whole of composition. A task runs `bolt` the way it runs any
/// tool, an adapter reads the result path bolt printed and turns the child's
/// verdict into an envelope, and the merge folds it as a constituent like any
/// other. Nothing in the runner knows one command is bolt, which is FR-5.18,
/// and the way to see it is that this test adds no bolt code at all.
///
/// The child fails and the parent's command succeeds, which is the pairing that
/// matters. FR-10.1 has bolt exit 0 whenever it carried the run out, so a
/// parent reading the exit status would call this green. The adapter is what
/// makes the verdict travel, and a bolt whose composition rested on `&&` would
/// pass a weaker test than this one.
///
/// The child's tree lands under the parent's work directory because the
/// command says `--output-dir {work_dir}/child`, which is FR-5.20: a line in a
/// jig, not a rule in the runner.
///
/// The depth is cleared and the ceiling set, as
/// `bolt_inside_bolt_is_stopped_at_the_ceiling` does and for the reason it
/// records: this suite is reachable from `bolt rust-quality .`, so a chain
/// measured from the ambient depth is a different length under the gate than
/// under `cargo test`.
#[test]
fn bolt_composes_as_a_command_and_the_childs_verdict_folds_in() {
    let root = composing_tree();

    let outcome = bolt::run::run("outer", root.path()).expect("the outer run completes");

    assert!(
        !outcome.success,
        "the child failed and the parent folded a pass",
    );

    // FR-5.20: the child's evidence is where the command put it.
    let child_result = work(&outcome, "subproject-1").join("child/result.yaml");
    assert!(
        child_result.is_file(),
        "the child's result is not under the parent's work directory: {}",
        child_result.display(),
    );

    // FR-5.19: what the adapter carried up is the child's own reason, reached
    // through the path bolt printed and not through an exit status.
    let folded = fs::read_to_string(outcome.output_dir.join("result.yaml")).expect("the result");
    assert!(
        folded.contains("child-failed"),
        "the adapter's verdict did not reach the fold: {folded}",
    );
    let child = fs::read_to_string(&child_result).expect("the child's result");
    assert!(
        child.contains("inner-check exited 3"),
        "the child did not record its own task's failure: {child}",
    );
}

// COVERS FR-10.3a | negative
/// A refusal prints where it recorded itself, on stdout, like any other run.
///
/// FR-10.3 tells a caller where to read the verdict, not what it says, and
/// FR-10.3a makes that unconditional. Asserted on a refusal, because that is
/// the case that goes quiet without it: the reason goes to stderr, stdout stays
/// empty, and FR-5.19's adapter reads an empty file. FR-10.7 has a caller read
/// an absent result as a bolt that died, so a silent refusal is the one failure
/// that misreports itself.
///
/// Without FR-10.3a, `bolt nosuchjig <dir> --output-dir <out>` writes
/// `out/result.yaml` and prints zero bytes on stdout.
#[test]
fn a_refusal_prints_where_it_recorded_itself() {
    let root = tree();
    let out = root.path().join("elsewhere");

    let ran = bolt()
        .arg("nosuchjig")
        .arg(root.path())
        .arg("--output-dir")
        .arg(&out)
        .output()
        .expect("bolt runs");

    assert!(!ran.status.success(), "an absent jig is a refusal");
    let said = String::from_utf8_lossy(&ran.stdout);
    let printed = said.trim();
    assert_eq!(
        Path::new(printed),
        out.join("result.yaml"),
        "stdout did not name the result the refusal wrote",
    );
    assert!(
        Path::new(printed).is_file(),
        "the path printed does not exist",
    );
}

/// A tree with one jig whose single task exits with `code`.
///
/// The three exit-status tests want a run that passed, a run that failed, and a
/// refusal, differing in nothing else. Sharing the fixture is what makes the
/// comparison between them mean something.
fn exiting_tree(code: u8) -> TempDir {
    let root = tree();
    write_jig(
        root.path(),
        "verdict",
        &format!("  - name: only\n    command: \"sh -c 'exit {code}'\"\n"),
    );
    root
}

// COVERS FR-10.8, FR-10.8b, FR-10.8e | positive
/// The envelope becomes the exit code only when the flag asks, and the result
/// line is printed either way.
///
/// FR-10.8's default is the whole of its safety: every caller written against
/// FR-10.1 sees what it always saw, so nothing already in the estate changes
/// meaning when the flag arrives. Asserted as a pair on one failing run,
/// because a test of the flag alone would pass against a bolt that had simply
/// changed its default.
///
/// FR-10.8e is the other half: the flag changes one number. The run still writes
/// its result and still names it on stdout, so a caller gets the verdict and
/// where to read it from the same invocation.
#[test]
fn the_envelope_becomes_the_exit_code_only_when_asked() {
    let root = exiting_tree(1);

    let without = bolt()
        .arg("verdict")
        .arg(root.path())
        .arg("--output-dir")
        .arg(root.path().join("a"))
        .output()
        .expect("bolt runs");
    assert_eq!(
        without.status.code(),
        Some(0),
        "FR-10.1's default moved: a run bolt carried out did not exit 0",
    );

    let with = bolt()
        .arg("--result-to-exitcode")
        .arg("verdict")
        .arg(root.path())
        .arg("--output-dir")
        .arg(root.path().join("b"))
        .output()
        .expect("bolt runs");
    assert_eq!(
        with.status.code(),
        Some(1),
        "the flag did not carry the envelope's failure to the exit code",
    );

    // FR-10.8e: the line is unchanged, so both readings come from one run.
    let printed = String::from_utf8_lossy(&with.stdout);
    let result = Path::new(printed.trim());
    assert_eq!(
        result,
        root.path().join("b").join("result.yaml"),
        "the flag changed what is printed as well as the status",
    );
    let envelope = fs::read_to_string(result).expect("the result");
    assert!(
        envelope.contains("\"success\": false"),
        "the exit code and the envelope disagree: {envelope}",
    );
}

// COVERS FR-10.8b | positive
/// A passing run under the flag is 0, which is the half that makes 1 mean
/// something.
///
/// Same jig, same flag, one digit different in the task's command. Without this
/// a bolt that returned 1 under the flag whatever happened would pass the
/// failing test, and "the envelope decides" would be indistinguishable from "the
/// flag means failure".
#[test]
fn a_passing_run_under_the_flag_is_zero() {
    let root = exiting_tree(0);

    let ran = bolt()
        .arg("--result-to-exitcode")
        .arg("verdict")
        .arg(root.path())
        .arg("--output-dir")
        .arg(root.path().join("out"))
        .output()
        .expect("bolt runs");

    assert_eq!(
        ran.status.code(),
        Some(0),
        "a passing envelope did not exit 0 under the flag",
    );
}

// COVERS FR-10.8c, FR-10.8d | negative
/// A refusal under the flag is 1, the same as without it, because a refusal is
/// a verdict.
///
/// This row is the one most likely to be undone, because the wrong answer is
/// attractive. Bolt writes `kind: bolt-refused` alongside `success: false`, so
/// reading the kind and reporting "no check ran, so nothing was found wrong" as
/// a third status looks like extra care. The envelope schema calls `success` the
/// authoritative verdict, though, and a neighbouring field does not overrule it.
///
/// The deeper reason there is no third status is that there is no third state.
/// A task set always resolves. A task that matched nothing and was declared
/// optional is satisfied; a required one that never ran has failed. Neither is
/// an absent verdict, so nothing is being collapsed.
///
/// It is kept as a test, not a comment, because the reasoning for a third
/// status is genuinely persuasive.
///
/// Asserted as a pair on one refusal, flag and no flag, which makes it a claim
/// about the flag and not about refusals: the numbers being equal is the
/// assertion.
#[test]
fn a_refusal_under_the_flag_is_still_one() {
    let root = tree();
    write_jig(root.path(), "retired", "  - name: child\n    jig: inner\n");

    let plain = bolt()
        .arg("retired")
        .arg(root.path())
        .arg("--output-dir")
        .arg(root.path().join("a"))
        .output()
        .expect("bolt runs");
    assert_eq!(
        plain.status.code(),
        Some(1),
        "a refusal without the flag is FR-10.5's 1",
    );

    let flagged = bolt()
        .arg("--result-to-exitcode")
        .arg("retired")
        .arg(root.path())
        .arg("--output-dir")
        .arg(root.path().join("b"))
        .output()
        .expect("bolt runs");
    assert_eq!(
        flagged.status.code(),
        plain.status.code(),
        "the flag invented a status for a refusal instead of reporting its verdict",
    );
    assert_eq!(
        flagged.status.code(),
        Some(1),
        "a refusal under the flag did not report the envelope's failure",
    );

    // The refusal still wrote and still named its result, so a caller reads why
    // from the same place as any other run.
    let printed = String::from_utf8_lossy(&flagged.stdout);
    let envelope = fs::read_to_string(Path::new(printed.trim())).expect("the result");
    assert!(
        envelope.contains("jig-task-retired"),
        "the result does not say bolt refused: {envelope}",
    );
}

// COVERS FR-10.9, FR-10.9a, FR-10.9b | property
/// Four refusals with four different fixes carry four different kinds.
///
/// The claim is that they differ, so the test is that they differ, not that
/// each equals a string. Without FR-10.9 every one of these carries
/// `bolt-refused`, so a per-refusal assertion would pass against the defect for
/// three of the four while looking thorough. Collecting them and counting the
/// distinct values cannot.
///
/// FR-10.9a: each is validated against wrench's envelope schema on the way out,
/// which is what makes the open vocabulary usable. A closed list would have made
/// this change a schema change in another repository.
#[test]
fn refusals_that_need_different_fixes_carry_different_kinds() {
    let root = tree();
    write(root.path(), "a.txt", "content");
    write_jig(root.path(), "retired", "  - name: child\n    jig: inner\n");
    write_jig(
        root.path(),
        "twice",
        "  - name: same\n    command: \"true\"\n  - name: same\n    command: \"true\"\n",
    );
    write(
        root.path(),
        &bolt::jig::file_name("broken"),
        "version: nope\n",
    );

    let mut seen = Vec::new();
    for (jig, base) in [
        ("retired", root.path().to_path_buf()),
        ("twice", root.path().to_path_buf()),
        ("broken", root.path().to_path_buf()),
        // FR-2.5's missing base, which needs an output directory outside it by
        // FR-10.7a for a result to exist at all.
        ("retired", root.path().join("absent")),
    ] {
        let out = root.path().join(format!("out-{}-{}", jig, seen.len()));
        let refused = bolt::run::invoke(&bolt::run::Invocation {
            jig,
            base: &base,
            definitions: None,
            output_dir: Some(&out),
            config_dir: Some(root.path()),
        })
        .expect_err("each of these is a refusal");

        let result = refused.result.expect("each of these wrote a result");
        let reasons = reasons_in(&result);
        assert_eq!(reasons.len(), 1, "one refusal is one reason: {reasons:?}");
        seen.push(reasons[0].0.clone());
    }

    let mut distinct = seen.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        seen.len(),
        "refusals needing different fixes shared a kind: {seen:?}",
    );
    assert!(
        !seen.contains(&"bolt-refused".to_owned()),
        "the one-kind-for-everything value survived: {seen:?}",
    );
}

// COVERS FR-10.9c, FR-10.7c | edge
/// A reused output directory writes no kind, and the file already there is not
/// its refusal.
///
/// FR-10.7c makes that the rule, not a gap: a bolt declining to start returns 1
/// with its reason on stderr, because it has nothing to report about a tree it
/// did not read and the file it would write to is somebody else's. A change
/// making this case write a result would reintroduce the overwrite the Go build
/// performs, measured there by checksum, so this test is what such a change has
/// to get past.
///
/// Here a refusal's kind cannot be read at all. FR-2.6b returns before writing, because the directory holds a
/// completed run and a refusal put there would replace a verdict. So a caller
/// testing whether a `result.yaml` exists gets `true` about the previous run,
/// and one reading its `success` gets that run's answer.
///
/// Asserted the way it would actually mislead: the file is present, and it says
/// the earlier run passed.
#[test]
fn a_reused_output_directory_leaves_the_earlier_result_alone() {
    let root = tree();
    write_jig(
        root.path(),
        "fine",
        "  - name: only\n    command: \"true\"\n",
    );
    let out = root.path().join("out");

    let first = run_into("fine", root.path(), &out).expect("the first run completes");
    assert!(first.success, "the first run was supposed to pass");

    let refused = bolt::run::invoke(&bolt::run::Invocation {
        jig: "fine",
        base: root.path(),
        definitions: None,
        output_dir: Some(&out),
        config_dir: None,
    })
    .expect_err("a directory holding a run is refused");

    assert!(
        refused.result.is_none(),
        "the refusal claimed to have written a result into an occupied directory",
    );
    let standing = reasons_in(&out.join(bolt::run::RESULT_FILE));
    assert!(
        standing.is_empty(),
        "the earlier run's result was overwritten by the refusal: {standing:?}",
    );
}

// ---- the project's own claims about itself ----------------------------------

/// This repository's root, for the tests that assert something about bolt
/// itself instead of about a run.
///
/// `CARGO_MANIFEST_DIR`, not the working directory, because a test's cwd
/// is not promised and FR-4.1a moves it for anything running under a gate.
fn repository() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

// COVERS NFR-12.3 | property
/// Every place that names bolt's licence names the same one.
///
/// The assertion is agreement, not a value. Four files state the licence and
/// each can be edited alone, so checking each against a literal `Apache-2.0`
/// would pass with three correct and one stale. What has to hold is that no
/// place naming a licence names a different one, which is why the manifest is
/// read first and everything else is compared against what it declares.
#[test]
fn every_statement_of_the_licence_agrees() {
    let manifest = fs::read_to_string(repository().join("Cargo.toml")).expect("Cargo.toml");
    let declared = manifest
        .lines()
        .find_map(|line| line.strip_prefix("license = "))
        .map(|value| value.trim_matches('"').to_owned())
        .expect("the manifest declares a licence");

    // The manifest carries an SPDX identifier and the prose files spell it out,
    // so the comparison is on both halves and not only the identifier. A
    // NOTICE reading "Apache License, Version 2.0" agrees with `Apache-2.0` and
    // does not contain it.
    let (family, version) = declared.split_once('-').expect("an SPDX identifier");

    let notice = fs::read_to_string(repository().join("NOTICE")).expect("NOTICE");
    assert!(
        notice.contains(family) && notice.contains(version),
        "NOTICE does not name {declared}, in either spelling: {notice}",
    );
    assert!(
        notice.to_lowercase().contains("copyright"),
        "NFR-12.3 wants a NOTICE naming the copyright holder: {notice}",
    );

    let licence = fs::read_to_string(repository().join("LICENSE")).expect("LICENSE");
    assert!(
        licence.contains(family) && licence.contains(version),
        "LICENSE is not the {declared} text",
    );

    // `deny.toml` is prose about the licence as well as configuration, and the
    // prose is the part that goes stale. The discriminator is citing the
    // requirement: a comment naming NFR-12.3 is making a claim about what that
    // row says, so it has to agree with it. Matching on "bolt is" instead
    // catches "the targets bolt is built for", the kind of false positive that
    // gets a check deleted instead of fixed.
    let deny = fs::read_to_string(repository().join("deny.toml")).expect("deny.toml");
    let citing: Vec<&str> = deny
        .lines()
        .filter(|line| line.trim_start().starts_with('#') && line.contains("NFR-12.3"))
        .collect();
    assert!(
        !citing.is_empty(),
        "deny.toml no longer cites NFR-12.3, so this half of the test checks nothing",
    );
    for line in citing {
        assert!(
            line.contains(&declared),
            "a comment cites NFR-12.3 and does not name {declared}: {line}",
        );
    }
}

// COVERS NFR-12.1 | positive
/// Bolt runs over its own repository and walks it the way its gate does.
///
/// The gate's jigs are toolbox's and not in this tree, so the jig here is a
/// stand-in kept outside it. What is asserted is that bolt can be pointed at
/// itself: the walk honours bolt's own `.gitignore`, so the build output under
/// `target/` is not among what a gate would check.
#[test]
fn bolt_runs_over_its_own_repository() {
    let config = tree();
    let out = tree();
    write_jig(
        config.path(),
        "self",
        "  - name: sources\n    command: \"sh -c 'printf \\\"%s\\\\n\\\" \\\"$@\\\"' sh {all_paths}\"\n    matching: [\"**/*.rs\"]\n",
    );

    let outcome = bolt::run::invoke(&bolt::run::Invocation {
        jig: "self",
        base: repository(),
        definitions: None,
        output_dir: Some(&out.path().join("run")),
        config_dir: Some(config.path()),
    })
    .expect("bolt runs over its own repository");

    assert!(outcome.success, "the run failed");
    let listed = fs::read_to_string(work(&outcome, "sources-1").join("stdout")).expect("stdout");
    assert!(
        listed.contains("/src/run.rs"),
        "bolt's own source was not walked"
    );
    assert!(
        !listed.contains("/target/"),
        "bolt's build output was walked"
    );
}

// COVERS NFR-12.4 | property
/// Nothing compiles C, and the binary links only the system C runtime.
///
/// `cc` is how a Rust build compiles C, so its absence from the lockfile is the
/// dependency tree's half. `ldd` is the binary's half.
#[test]
fn bolt_builds_without_a_c_toolchain() {
    let lock =
        fs::read_to_string(repository().join("Cargo.lock")).expect("Cargo.lock after a build");
    for crate_name in ["cc", "cmake"] {
        assert!(
            !lock.contains(&format!("name = \"{crate_name}\"\n")),
            "{crate_name} is in the dependency tree",
        );
    }

    let linked = Command::new("ldd")
        .arg(env!("CARGO_BIN_EXE_bolt"))
        .output()
        .expect("ldd runs");
    let mut libraries: Vec<String> = String::from_utf8_lossy(&linked.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| !name.starts_with("linux-vdso") && !name.contains("ld-linux"))
        .map(|name| name.split(".so").next().unwrap_or(name).to_owned())
        .collect();
    libraries.sort();
    assert_eq!(libraries, ["libc", "libgcc_s", "libm"], "linked against");
}

// COVERS FR-11.1 | property
/// A run needs the jig, the directory and nothing else from the machine.
///
/// No environment beyond a `PATH` for the shell, started from an unrelated
/// directory, with the tree copied somewhere nothing else knows about.
#[test]
fn a_run_needs_only_the_jig_and_the_directory() {
    let root = tree();
    write(root.path(), "a.txt", "a");
    write_jig(
        root.path(),
        "check",
        "  - name: read\n    command: \"cat {each_path}\"\n    matching: [\"*.txt\"]\n",
    );
    let elsewhere = tree();

    let finished = bolt()
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(elsewhere.path())
        .arg("check")
        .arg(root.path())
        .output()
        .expect("bolt runs");

    assert_eq!(
        finished.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&finished.stderr),
    );
    let result = String::from_utf8_lossy(&finished.stdout).trim().to_owned();
    assert!(
        verdict(Path::new(&result), &wrench::schemas::ENVELOPE),
        "the run failed"
    );
}

// COVERS FR-11.3 | positive
/// One jig runs against a throwaway copy of the tree with a change in it, and
/// the two runs differ only where the trees did.
#[test]
fn the_same_jig_runs_against_a_throwaway_copy() {
    let original = tree();
    write(original.path(), "good.txt", "fine\n");
    write(original.path(), "other.txt", "fine\n");
    write_jig(
        original.path(),
        "check",
        "  - name: clean\n    command: \"sh -c '! grep -q broken \\\"$1\\\"' sh {each_path}\"\n    matching: [\"*.txt\"]\n",
    );
    let copy = tree();
    for name in ["good.txt", "other.txt", &bolt::jig::file_name("check")] {
        fs::copy(original.path().join(name), copy.path().join(name)).expect("the copy");
    }
    write(copy.path(), "other.txt", "broken\n");

    let before = bolt::run::run("check", original.path()).expect("the original runs");
    let after = bolt::run::run("check", copy.path()).expect("the copy runs");

    assert!(before.success, "the original tree failed");
    assert!(!after.success, "the change in the copy went unnoticed");
    assert!(
        verdict(&envelope_of(&after, "clean-1"), &wrench::schemas::ENVELOPE),
        "the unchanged file failed in the copy",
    );
}

// COVERS FR-13.7 | positive
/// A commit SHA handed in through a definitions file is in every manifest,
/// marked as the file's.
///
/// The command does not name it, and it is recorded anyway, because a manifest
/// records every key the layers hold.
#[test]
fn a_commit_sha_from_the_caller_reaches_the_manifest() {
    // Forty hex digits, built here so the secrets scanners see no literal.
    let sha = "ab".repeat(20);
    let root = tree();
    write_jig(
        root.path(),
        "check",
        "  - name: alpha\n    command: \"sh -c 'exit 0'\"\n",
    );
    write_definitions(root.path(), "tree", &format!("commit: \"{sha}\"\n"));

    let outcome = run_with("check", root.path(), "tree").expect("the run completes");

    let manifest = read_validated(
        &work(&outcome, "alpha-1").join(bolt::run::MANIFEST_FILE),
        &wrench::schemas::MANIFEST,
    );
    assert_eq!(
        manifest["variables"]["commit"],
        serde_json::json!({ "value": sha, "from": "file" }),
        "the SHA did not reach the manifest",
    );
}

// COVERS FR-4.12 | property
/// The deadline arithmetic, which every timed task depends on and no other test reaches.
///
/// These are three pure functions and the run path exercises them only through a
/// task that actually times out, which is slow and asserts something else.
/// Without these tests the arithmetic itself goes unmeasured, and
/// `src/limit.rs` falls to 77.4% of lines, under the gate's per-file coverage
/// line.
///
/// `soonest` taking a set deadline over none is the arm that most needs a test.
/// A run's budget and a task's own limit are each optional, and the wrong fold
/// there (`None` winning, or the later of the two) is a limit that silently
/// does not bind, and nothing would report it.
#[test]
fn a_deadline_is_the_soonest_of_the_limits_that_are_set() {
    let now = Instant::now();
    let second = Duration::from_secs(1);
    let minute = Duration::from_secs(60);

    assert_eq!(
        bolt::limit::deadline(None, now),
        None,
        "no limit sets no deadline"
    );
    assert_eq!(
        bolt::limit::deadline(Some(second), now),
        Some(now + second),
        "a limit set now runs out a limit from now",
    );

    let soon = now + second;
    let later = now + minute;
    assert_eq!(bolt::limit::soonest(Some(soon), Some(later)), Some(soon));
    assert_eq!(bolt::limit::soonest(Some(later), Some(soon)), Some(soon));
    assert_eq!(
        bolt::limit::soonest(Some(soon), None),
        Some(soon),
        "a set deadline wins over none, in either position",
    );
    assert_eq!(bolt::limit::soonest(None, Some(soon)), Some(soon));
    assert_eq!(bolt::limit::soonest(None, None), None);

    assert!(
        !bolt::limit::passed(None, later),
        "no deadline has not passed"
    );
    assert!(!bolt::limit::passed(Some(later), now));
    assert!(
        bolt::limit::passed(Some(now), now),
        "a deadline reached has passed"
    );
    assert!(bolt::limit::passed(Some(soon), later));
}

// COVERS FR-10.9c | edge
/// The one refusal bolt names and never writes.
///
/// `OutputDirectoryInUse` is documented in `src/error.rs` as "named for
/// completeness and never written", because FR-2.6b returns before anything is
/// written: the directory holds a completed run, and a refusal put there
/// replaces a verdict with `kind: bolt-refused` while the per-task evidence
/// still says otherwise.
///
/// A variant nothing constructs still has a contract. Its kind is in the open
/// vocabulary consumers match on and its message is what a person reads, and
/// no refusal exercises either: both arms stay uncovered while the rest of the
/// enum is reached through real refusals. Constructed directly here, which is
/// the only way to reach a case the runner is built not to produce.
#[test]
fn the_refusal_that_never_writes_still_names_itself() {
    let refusal = bolt::Error::OutputDirectoryInUse(PathBuf::from("/somewhere/.bolt-gate"));

    assert_eq!(refusal.kind(), "output-directory-in-use");
    assert!(
        refusal.never_writes_a_result(),
        "the one refusal that must not write is the one this test is about",
    );
    assert_eq!(
        refusal.to_string(),
        "/somewhere/.bolt-gate already holds a run",
        "the message names the directory, because a refusal that does not is one nobody can act on",
    );
}
