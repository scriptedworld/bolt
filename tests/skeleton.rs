//! The walking skeleton: one jig, one directory, end to end.
//!
//! Stage 4 of `silo/docs/PATTERNS/how-a-change-gets-made.md`. These are written
//! to fail. Nothing in `src/` is implemented, so every one of them panics on a
//! `todo!()` rather than on an assertion, and that is the expected state: a
//! test passing here would be testing nothing the implementation will provide.
//!
//! One file rather than one per concern, because a shared `tests/common/mod.rs`
//! compiles into every test binary separately and its helpers are then dead
//! code in each binary that does not call them. Under `-D warnings` that fails
//! the gate, and the usual fix is an `allow` attribute, which hard rule 4 makes
//! a question rather than an edit.

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A jig with one task per line of `tasks`, at schema version 1.
fn jig_yaml(tasks: &str) -> String {
    format!("version: \"1.0.0\"\ntasks:\n{tasks}")
}

/// Write `contents` to `relative` under `root`, creating parents.
fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent directory");
    }
    fs::write(path, contents).expect("fixture file");
}

/// Write a jig beside the tree and return its path.
fn write_jig(root: &Path, body: &str) -> PathBuf {
    let path = root.join("jig.yaml");
    fs::write(&path, body).expect("fixture jig");
    path
}

/// An empty fixture tree.
fn tree() -> TempDir {
    TempDir::new().expect("fixture tree")
}

/// Bolt's built binary, for the tests that are about the invocation itself.
fn bolt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bolt"))
}

// COVERS: FR-2.1, FR-2.1a | positive
/// An invocation says which jig and where, and that is the whole of it.
#[test]
fn an_invocation_is_one_jig_and_one_directory() {
    let root = tree();
    write(root.path(), "a.txt", "content");
    let jig = write_jig(
        root.path(),
        &jig_yaml("  - name: check\n    command: \"true\"\n"),
    );

    let outcome = bolt()
        .arg(&jig)
        .arg(root.path())
        .output()
        .expect("bolt runs");

    assert!(
        outcome.status.success(),
        "two arguments is a complete invocation, got {:?}",
        outcome.status,
    );
}

// COVERS: FR-2.1a | negative
/// A third argument asks for an interface bolt does not have.
///
/// FR-2.1a settles one jig and one directory. Running several over one tree is
/// a jig whose tasks are nested jigs, so a second composition mechanism beside
/// that one would buy nothing.
#[test]
fn a_third_argument_is_refused() {
    let root = tree();
    let jig = write_jig(
        root.path(),
        &jig_yaml("  - name: check\n    command: \"true\"\n"),
    );

    let outcome = bolt()
        .arg(&jig)
        .arg(root.path())
        .arg(root.path())
        .output()
        .expect("bolt runs");

    // Not merely "did not succeed": a panic exits 101 and would satisfy that,
    // so the assertion could not tell a refusal from a crash. FR-10.5 makes the
    // status for a run bolt could not carry out exactly 1.
    assert_eq!(
        outcome.status.code(),
        Some(1),
        "a third argument is refused with 1, not with whatever went wrong",
    );
}

// COVERS: FR-2.2 | positive
/// The walk is the whole input: it finds the files the tasks act on.
#[test]
fn the_walk_finds_the_files_tasks_act_on() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    write(root.path(), "nested/b.txt", "two");

    let found = bolt::walk::walk(root.path()).expect("the walk succeeds");

    let names: Vec<_> = found
        .iter()
        .map(|path| path.strip_prefix(root.path()).expect("inside the base"))
        .collect();
    assert_eq!(
        names,
        [Path::new("a.txt"), Path::new("nested/b.txt")],
        "both files, and nothing else",
    );
}

// COVERS: FR-2.2a | negative
/// An ignored file is not part of the project and is not checked.
#[test]
fn an_ignored_file_is_not_walked() {
    let root = tree();
    write(root.path(), ".gitignore", "build/\n*.tmp\n");
    write(root.path(), "kept.txt", "kept");
    write(root.path(), "scratch.tmp", "ignored");
    write(root.path(), "build/output.txt", "ignored");

    let found = bolt::walk::walk(root.path()).expect("the walk succeeds");

    let ignored: Vec<_> = found
        .iter()
        .filter(|path| {
            let text = path.to_string_lossy();
            text.ends_with(".tmp") || text.contains("/build/")
        })
        .collect();
    assert!(ignored.is_empty(), "ignored paths were walked: {ignored:?}");
}

// COVERS: FR-2.2b | edge
/// A tree that is not a repository walks the same, and git's own excludes are
/// not read.
///
/// Honouring `.gitignore` means reading those files as text. Bolt does not
/// invoke git, read anything under `.git/`, or require a repository, so a
/// `.git/info/exclude` naming a file leaves that file in the walk.
#[test]
fn git_excludes_are_not_read_and_no_repository_is_required() {
    let root = tree();
    write(root.path(), ".git/info/exclude", "hidden.txt\n");
    write(root.path(), "hidden.txt", "still walked");
    write(root.path(), "plain.txt", "walked");

    let found = bolt::walk::walk(root.path()).expect("a tree with no repository walks");

    let relative: Vec<_> = found
        .iter()
        .map(|path| path.strip_prefix(root.path()).expect("inside the base"))
        .collect();
    assert!(
        relative.contains(&Path::new("hidden.txt")),
        "`.git/info/exclude` was read; FR-2.2b says it is not",
    );
    assert!(
        !relative.iter().any(|path| path.starts_with(".git")),
        "nothing under .git/ belongs in a walk",
    );
}

// COVERS: FR-2.2d | property
/// The walk returns sorted paths, so two runs over one tree agree.
///
/// FR-9.4's identical work directory names rest on this and on nothing else.
#[test]
fn the_walk_is_sorted_and_repeatable() {
    let root = tree();
    for name in ["c.txt", "a.txt", "b.txt", "nested/d.txt"] {
        write(root.path(), name, "content");
    }

    let first = bolt::walk::walk(root.path()).expect("the walk succeeds");
    let second = bolt::walk::walk(root.path()).expect("the walk succeeds twice");

    let mut sorted = first.clone();
    sorted.sort();
    assert_eq!(first, sorted, "the walk is not sorted");
    assert_eq!(first, second, "two walks over one tree disagree");
}

// COVERS: FR-2.2e | negative
/// A symlink is not followed, because following one leaves the base.
///
/// `link-jigs` leaves tracked symlinks pointing into toolbox, so a project
/// using shared jigs has them sitting in the tree being walked.
#[test]
fn a_symlink_is_not_followed() {
    let outside = tree();
    write(outside.path(), "secret.txt", "outside the base");

    let root = tree();
    write(root.path(), "inside.txt", "inside");
    unix_fs::symlink(outside.path(), root.path().join("link")).expect("fixture symlink");

    let found = bolt::walk::walk(root.path()).expect("the walk succeeds");

    assert!(
        !found.iter().any(|path| path.ends_with("secret.txt")),
        "the walk followed a symlink out of the base: {found:?}",
    );
}

// COVERS: FR-3.4, FR-3.4a | positive
/// `matching` selects and `excluding` removes from what it selected.
#[test]
fn excluding_removes_from_what_matching_selected() {
    let paths: Vec<PathBuf> = ["a.py", "b.py", "generated.py", "c.txt"]
        .iter()
        .map(PathBuf::from)
        .collect();

    let selected = bolt::selection::select(
        &paths,
        &["**/*.py".to_owned()],
        &["generated.py".to_owned()],
    )
    .expect("the patterns compile");

    assert_eq!(
        selected,
        [PathBuf::from("a.py"), PathBuf::from("b.py")],
        "matching takes the .py files and excluding removes the named one",
    );
}

// COVERS: FR-4.2 | property
/// The path form is read off the command, and decides how many executions run.
#[test]
fn each_path_runs_once_per_path_and_all_paths_runs_once() {
    assert!(
        bolt::selection::consumes_paths("check {each_path}"),
        "{{each_path}} consumes paths",
    );
    assert!(
        bolt::selection::consumes_paths("check {all_paths}"),
        "{{all_paths}} consumes paths",
    );
    assert!(
        !bolt::selection::consumes_paths("check --everything"),
        "a command naming neither does not consume paths",
    );

    let root = tree();
    write(root.path(), "a.txt", "one");
    write(root.path(), "b.txt", "two");
    let jig = write_jig(
        root.path(),
        &jig_yaml(concat!(
            "  - name: per-path\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"echo {each_path}\"\n",
            "  - name: one-shot\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"echo {all_paths}\"\n",
        )),
    );

    let outcome = bolt::run::run(&jig, root.path()).expect("the run completes");

    assert_eq!(
        outcome.executions, 3,
        "two executions for the per-path task and one for the other",
    );
}

// COVERS: FR-4.2, FR-4.3 | negative
/// Naming both path forms is a jig error, and every path is quoted alone.
#[test]
fn naming_both_path_forms_is_a_jig_error_and_paths_are_quoted_individually() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    let jig = write_jig(
        root.path(),
        &jig_yaml(concat!(
            "  - name: confused\n",
            "    matching: [\"**/*.txt\"]\n",
            "    command: \"echo {each_path} {all_paths}\"\n",
        )),
    );

    let refusal = bolt::run::run(&jig, root.path()).expect_err("naming both is a jig error");
    assert!(
        matches!(refusal, bolt::Error::CommandNamesBothPathForms { .. }),
        "wrong refusal for a command naming both forms: {refusal:?}",
    );

    let hostile = Path::new("a file; rm -rf /.txt");
    let quoted = bolt::selection::quote(hostile);
    assert!(
        quoted.starts_with('\'') && quoted.ends_with('\''),
        "a path carrying a space and a semicolon must be quoted: {quoted}",
    );
}

// COVERS: FR-4.4 | negative
/// A path-consuming task with an empty selection does not execute.
///
/// A command task naming neither form always executes, which is what makes the
/// empty selection a reason to skip only for the tasks that wanted paths.
#[test]
fn a_path_consuming_task_with_an_empty_selection_does_not_execute() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    let jig = write_jig(
        root.path(),
        &jig_yaml(concat!(
            "  - name: no-python-here\n",
            "    matching: [\"**/*.py\"]\n",
            "    command: \"echo {each_path}\"\n",
            "  - name: always\n",
            "    command: \"true\"\n",
        )),
    );

    let outcome = bolt::run::run(&jig, root.path()).expect("the run completes");

    assert_eq!(
        outcome.executions, 1,
        "only the task naming no paths executed"
    );
    assert_eq!(
        outcome.skipped,
        ["no-python-here"],
        "the skipped task is reported rather than left to be inferred",
    );
}

// COVERS: FR-4.5 | property
/// Tasks execute serially: no two executions overlap in time.
///
/// The row says serially and says nothing about order. FR-4.5a adds that serial
/// is the simplest thing that works rather than something required, and FR-4.7
/// says the merged result does not vary with the order tasks ran in. So this
/// asserts non-overlap and not the declaration order, which no row states and
/// which is question 38 in `NEXT_STEPS.md`.
#[test]
fn no_two_executions_overlap() {
    let root = tree();
    let log = root.path().join("order.log");
    let jig = write_jig(
        root.path(),
        &jig_yaml(&format!(
            concat!(
                "  - name: first\n",
                "    command: \"sh -c 'echo enter-first >> {log}; sleep 0.2; echo leave-first >> {log}'\"\n",
                "  - name: second\n",
                "    command: \"sh -c 'echo enter-second >> {log}; sleep 0.2; echo leave-second >> {log}'\"\n",
            ),
            log = log.display(),
        )),
    );

    bolt::run::run(&jig, root.path()).expect("the run completes");

    let entries: Vec<String> = fs::read_to_string(&log)
        .expect("the tasks wrote the log")
        .lines()
        .map(str::to_owned)
        .collect();
    for pair in entries.chunks(2) {
        let [enter, leave] = pair else {
            panic!("an execution logged an odd number of lines: {entries:?}");
        };
        assert_eq!(
            enter.replace("enter-", ""),
            leave.replace("leave-", ""),
            "an execution began before the previous one finished: {entries:?}",
        );
    }
}

// COVERS: FR-1.4, FR-9.2 | positive
/// Each execution keeps stdout, stderr, the exit code, a manifest and an
/// envelope, and those survive the run as evidence.
#[test]
fn an_execution_keeps_its_native_results() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    let jig = write_jig(
        root.path(),
        &jig_yaml(concat!(
            "  - name: noisy\n",
            "    command: \"sh -c 'echo out; echo err >&2'\"\n",
        )),
    );

    let outcome = bolt::run::run(&jig, root.path()).expect("the run completes");

    let work = outcome.output_dir.join("work").join("noisy-1");
    for file in [
        "stdout",
        "stderr",
        bolt::run::EXITCODE_FILE,
        bolt::run::MANIFEST_FILE,
        bolt::run::OUTPUT_FILE,
    ] {
        assert!(
            work.join(file).is_file(),
            "{file} is missing from {}",
            work.display(),
        );
    }
    assert_eq!(
        fs::read_to_string(work.join("stdout")).expect("stdout is readable"),
        "out\n",
        "stdout is captured as the command wrote it",
    );
}

// COVERS: FR-9.2a, FR-9.2b | property
/// The ordinal is the index within the task, zero-padded to the needed width.
///
/// Each task numbers its own executions from one, independently of every other
/// task, so a directory name says which task and which of its executions
/// without needing the run's order.
#[test]
fn the_ordinal_is_per_task_and_zero_padded() {
    assert_eq!(
        bolt::run::work_dir_name("lint", 1, 1),
        "lint-1",
        "one execution needs one digit",
    );
    assert_eq!(
        bolt::run::work_dir_name("lint", 2, 12),
        "lint-02",
        "twelve executions pad to two digits",
    );
    assert_eq!(
        bolt::run::work_dir_name("lint", 7, 100),
        "lint-007",
        "a hundred executions pad to three",
    );
}

// COVERS: FR-9.5, FR-9.5a | positive
/// The manifest is written before the command runs, and records what the task
/// was given and what it was kept from seeing.
#[test]
fn the_manifest_is_written_before_the_command_and_records_both_lists() {
    let root = tree();
    write(root.path(), "a.py", "kept");
    write(root.path(), "generated.py", "removed");
    let jig = write_jig(
        root.path(),
        &jig_yaml(concat!(
            "  - name: check\n",
            "    matching: [\"**/*.py\"]\n",
            "    excluding: [\"generated.py\"]\n",
            "    command: \"false\"\n",
        )),
    );

    let outcome = bolt::run::run(&jig, root.path()).expect("the run completes");

    let manifest = outcome
        .output_dir
        .join("work")
        .join("check-1")
        .join(bolt::run::MANIFEST_FILE);
    let text = fs::read_to_string(&manifest).expect("a failing command still has a manifest");
    assert!(
        text.contains("a.py"),
        "the manifest does not record what matching selected: {text}",
    );
    assert!(
        text.contains("generated.py"),
        "the manifest does not record what excluding removed: {text}",
    );
}

// COVERS: FR-6.9 | positive
/// A task naming no adapter gets the generic exit-code adapter.
#[test]
fn a_task_naming_no_adapter_gets_the_exit_code_one() {
    let root = tree();
    let jig = write_jig(
        root.path(),
        &jig_yaml(concat!(
            "  - name: passes\n",
            "    command: \"true\"\n",
            "  - name: fails\n",
            "    command: \"false\"\n",
        )),
    );

    let outcome = bolt::run::run(&jig, root.path()).expect("the run completes");

    let work = outcome.output_dir.join("work");
    let passed = fs::read_to_string(work.join("passes-1").join(bolt::run::OUTPUT_FILE))
        .expect("the passing task has an envelope");
    let failed = fs::read_to_string(work.join("fails-1").join(bolt::run::OUTPUT_FILE))
        .expect("the failing task has an envelope");

    assert!(
        passed.contains("true"),
        "a zero exit reports success: {passed}"
    );
    assert!(
        failed.contains("false"),
        "a non-zero exit reports failure: {failed}"
    );
}

// COVERS: FR-8.1, FR-8.3 | property
/// The merge folds every envelope, repeatably, and passes only when every
/// constituent passes.
#[test]
fn the_merge_folds_every_envelope_and_passes_only_when_all_do() {
    let root = tree();
    let jig = write_jig(
        root.path(),
        &jig_yaml(concat!(
            "  - name: passes\n",
            "    command: \"true\"\n",
            "  - name: fails\n",
            "    command: \"false\"\n",
        )),
    );

    let outcome = bolt::run::run(&jig, root.path()).expect("the run completes");
    assert!(
        !outcome.success,
        "one failing constituent fails the merged result",
    );
    assert!(
        outcome.output_dir.join(bolt::run::RESULT_FILE).is_file(),
        "a run has exactly one result",
    );

    let refolded = bolt::merge::merge(&outcome.output_dir).expect("a finished directory refolds");
    assert_eq!(
        refolded.success, outcome.success,
        "the fold is not repeatable over a finished directory",
    );
}

// COVERS: FR-8.3a | negative
/// A merge finding no constituent fails.
///
/// FR-8.3 on its own would pass it, because every constituent passing holds
/// when there are none, and a green result over zero checks is read as checked
/// and fine.
#[test]
fn a_merge_finding_no_constituent_fails() {
    let empty = tree();
    fs::create_dir(empty.path().join("work")).expect("an empty work directory");

    let refusal = bolt::merge::merge(empty.path()).expect_err("no constituent is a failure");

    assert!(
        matches!(refusal, bolt::Error::NoConstituents),
        "wrong refusal for an empty fold: {refusal:?}",
    );
}

// COVERS: FR-2.5, FR-2.5a | negative
/// A base that is not there is refused, and nothing is created.
///
/// The Go build created the base as a side effect of preparing the output
/// directory, so a run over a typo'd path checked an empty tree and passed.
/// This asserts the absence, not merely that the status was non-zero.
#[test]
fn a_missing_base_is_refused_and_nothing_is_created() {
    let root = tree();
    let jig = write_jig(
        root.path(),
        &jig_yaml("  - name: check\n    command: \"true\"\n"),
    );
    let absent = root.path().join("not-there");

    let refusal = bolt::run::run(&jig, &absent).expect_err("a missing base is refused");

    assert!(
        matches!(refusal, bolt::Error::BaseMissing(_)),
        "wrong refusal for a missing base: {refusal:?}",
    );
    assert!(
        !absent.exists(),
        "the base was created by the run that was refusing it",
    );
}

// COVERS: FR-10.1, FR-10.5 | property
/// Bolt exits 0 when the run completed and 1 when it could not carry it out.
///
/// FR-10.2 makes the pairing deliberate: a run in which every task executed and
/// some tools reported failures exits 0 and writes `success: false`. A caller
/// reading the exit status to learn whether the tools passed has read the wrong
/// thing.
#[test]
fn the_exit_status_says_whether_bolt_ran_not_whether_tools_passed() {
    let root = tree();
    let failing = write_jig(
        root.path(),
        &jig_yaml("  - name: fails\n    command: \"false\"\n"),
    );

    let completed = bolt()
        .arg(&failing)
        .arg(root.path())
        .output()
        .expect("bolt runs");
    assert_eq!(
        completed.status.code(),
        Some(0),
        "a completed run exits 0 whatever the tools concluded",
    );

    let refused = bolt()
        .arg(&failing)
        .arg(root.path().join("not-there"))
        .output()
        .expect("bolt runs");
    assert_eq!(
        refused.status.code(),
        Some(1),
        "a run bolt could not carry out exits 1",
    );
}
