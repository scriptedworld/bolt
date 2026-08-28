//! The walking skeleton: one jig, one directory, end to end.
//!
//! Stage 4 of `silo/docs/PATTERNS/how-a-change-gets-made.md`. These are written
//! to fail. Nothing in `src/` is implemented, so every one of them panics on a
//! `todo!()` rather than on an assertion, and that is the expected state.
//!
//! One file rather than one per concern, because a shared `tests/common/mod.rs`
//! compiles into every test binary separately and its helpers are then dead
//! code in each binary that does not call them. Under `-D warnings` that fails
//! the gate, and the usual fix is an `allow` attribute, which hard rule 4 makes
//! a question rather than an edit.
//!
//! **Envelopes and manifests are read through wrench**, by FR-1.12, which also
//! validates them against their schemas on the way in. A substring check over
//! the raw text cannot tell `success: true` from a command whose name happens
//! to be `true`, and three cold reads found exactly that in the first draft.

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

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

// ---- the invocation ---------------------------------------------------------

// COVERS: FR-2.1, FR-2.1a, FR-3.9 | positive
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
        verdict(&result, &wrench::ENVELOPE_SCHEMA),
        "a jig whose only task passes produces a passing result",
    );
}

// COVERS: FR-2.1a, FR-10.5 | negative
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

// COVERS: FR-10.1, FR-10.2, FR-10.5 | property
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

// COVERS: FR-10.5 | negative
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

// COVERS: FR-1.5, FR-3.9 | edge
/// A jig with no `version` is valid, because the schema requires only `tasks`.
///
/// Bolt validates what wrench's schema says and not what it would have chosen.
/// Requiring `version` here refused six of the estate's jigs including bolt's
/// own, and nothing said so until the Rust bolt was pointed at its own gate.
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

// COVERS: FR-5.13h, FR-10.5 | negative
/// A task naming a jig is refused by name, because nested jigs are not built.
///
/// The refusal has to say which feature is missing rather than which field is.
/// Before this, the message was serde's `missing field command`, which reads as
/// a malformed jig and invites somebody to add a command to a jig task. Found
/// against wrench's real jig, whose gate has two of them.
#[test]
fn a_task_naming_a_jig_is_refused_by_name() {
    let root = tree();
    write_jig(
        root.path(),
        "nested",
        "  - name: child\n    jig: common-quality\n    in: python\n",
    );

    let refusal = bolt::run::run("nested", root.path()).expect_err("nested jigs are not built");

    match refusal {
        bolt::Error::NestedJigNotBuilt { task } => {
            assert_eq!(task, "child", "the refusal named the wrong task");
        }
        other => panic!("wrong refusal for a jig task: {other:?}"),
    }
}

// COVERS: FR-4.3, FR-2.3 | regression
/// A filename containing a template token is not re-expanded into its own
/// substitution.
///
/// **This was a working remote code execution.** Substitution chained
/// `str::replace` once per variable, so a path spliced in for `{each_path}` that
/// contained the literal text `{all_paths}` had that token expanded by the next
/// replace. The second expansion spliced a fresh quoted string into the middle
/// of the already-quoted region, broke the quoting, and put the rest of the
/// filename on the command line unquoted. A cold-read reviewer ran `id` with it
/// and escaped the base to write a file beside it, while the run reported
/// success.
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

// COVERS: FR-4.18 | negative
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

// COVERS: FR-3.3a, FR-8.3 | regression
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

// COVERS: FR-2.3, FR-9.2 | regression
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

// COVERS: FR-2.5, FR-10.7a | negative
/// A base that is not there is refused, and nothing is created.
///
/// FR-2.5a is deliberately **not** cited. It gives every refusal a
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

// ---- the walk ---------------------------------------------------------------

// COVERS: FR-2.2 | positive
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

// COVERS: FR-2.2a | negative
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

// COVERS: FR-2.2b, FR-2.7 | edge
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

// COVERS: FR-2.2b | negative
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

// COVERS: FR-2.2d | property
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

// COVERS: FR-2.2e | negative
/// A symlink is not followed, so what sits behind one is not walked.
///
/// The row says the walk does not *follow* a symlink, and this asserts exactly
/// that: the file behind a directory symlink does not appear.
///
/// **It deliberately does not assert what happens to a symlink to a FILE.**
/// Measured 2026-08-27: `ignore` with `follow_links(false)` returns the link
/// itself, so a task handed it reads through to outside the base. Whether
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

// COVERS: FR-3.4, FR-3.4a, FR-3.5 | positive
/// `matching` selects, `excluding` removes from what it selected, and both are
/// matched relative to the base.
///
/// The paths come from a real walk and are therefore absolute. The first draft
/// passed bare relative names, which hid the whole defect: measured, the glob
/// `generated.py` does not match `/abs/tmp/x/generated.py`, so a literal
/// `excluding` entry silently removes nothing while `**/*.py` keeps working.
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

// COVERS: FR-4.3 | negative
/// Every substituted path is quoted individually, against a shell.
///
/// Asserted by round trip rather than by shape. `format!("'{}'", path)` is the
/// obvious implementation and passes any starts-with/ends-with check, and a
/// path containing a single quote escapes it: an adversarial review built
/// `'a'; touch <path>/PWNED; '.txt'` against the first draft and the injected
/// command ran.
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

// COVERS: FR-4.2 | positive
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

// COVERS: FR-4.2 | negative
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

// COVERS: FR-4.4, FR-4.4b | negative
/// A path-consuming task whose selection is empty fails, and says so.
///
/// FR-4.4b changed this on 2026-08-27. It used to be a silent skip, which left
/// a typo'd pattern green forever. The task still does not execute; what is new
/// is that it produces a failing constituent rather than nothing.
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
        !verdict(&envelope, &wrench::ENVELOPE_SCHEMA),
        "the constituent for an empty selection reports success",
    );
}

// COVERS: FR-4.4c | positive
/// `allow-empty` makes an empty selection an acceptable result.
#[test]
fn allow_empty_makes_an_empty_selection_acceptable() {
    let root = tree();
    write(root.path(), "a.txt", "one");
    write_jig(
        root.path(),
        "allowed",
        concat!(
            "  - name: no-python-here\n",
            "    matching: [\"**/*.py\"]\n",
            "    allow-empty: true\n",
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

// COVERS: FR-4.4 | positive
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

// COVERS: FR-4.5 | property
/// Tasks execute serially: no two executions overlap in time.
///
/// The row says serially and says nothing about order. FR-4.5a calls serial the
/// simplest thing rather than something required, and FR-4.7 says the merged
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

// ---- evidence ---------------------------------------------------------------

// COVERS: FR-1.4, FR-9.2 | positive
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
        // write, and that is `runner/30`'s rather than the skeleton's.
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

// COVERS: FR-9.2b | property
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

// COVERS: FR-9.2a | property
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

// COVERS: FR-9.5 | positive
/// The manifest records what `matching` selected and what `excluding` removed.
///
/// The jig is valid: `matching` and `excluding` sit on a task that names a path
/// variable, which FR-3.4b requires. The first draft put them on a task whose
/// command was `false`, a jig the specification refuses, and then asserted the
/// manifest carried both paths, which is the negation of FR-9.6.
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
        &wrench::MANIFEST_SCHEMA,
    );

    // `selection.matched` and `selection.excluded` are wrench's names for the
    // two lists FR-9.5 requires. This first invented `selected` and `removed`,
    // and writing the manifest through wrench refused them, which is what
    // reading a structured file through a schema is for.
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

// COVERS: FR-9.5a | positive
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
            "    command: \"sh -c 'test -f {work_dir}/manifest.yaml'\"\n",
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

// COVERS: FR-9.6 | negative
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
        &wrench::MANIFEST_SCHEMA,
    );

    // wrench's schema says `selection` is "present for a task that consumes
    // paths", so absent is the claim rather than an empty pair of lists.
    assert!(
        manifest.get("selection").is_none(),
        "a task handed no list has a manifest claiming a selection: {manifest}",
    );
}

// ---- adapters and the merge -------------------------------------------------

// COVERS: FR-6.9 | positive
/// A task naming no adapter gets the generic exit-code adapter.
///
/// The commands are `exit 0` and `exit 3` rather than `true` and `false`, so no
/// envelope can satisfy the assertion by echoing the command line it ran, and
/// the verdict is read as a boolean rather than matched as a substring.
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
            &wrench::ENVELOPE_SCHEMA
        ),
        "a zero exit did not report success",
    );
    assert!(
        !verdict(
            &work(&outcome, "nonzero-1").join(bolt::run::OUTPUT_FILE),
            &wrench::ENVELOPE_SCHEMA
        ),
        "a non-zero exit did not report failure",
    );
}

// COVERS: FR-8.1 | property
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

    bolt::merge::merge(&outcome.output_dir).expect("a finished directory refolds");

    assert_eq!(
        first,
        fs::read(&result).expect("the result survives a refold"),
        "the fold is not repeatable over a finished directory",
    );
}

// COVERS: FR-8.3 | property
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
}

// COVERS: FR-8.3a | negative
/// A merge finding no constituent fails, with a reason saying so.
///
/// FR-8.3 alone would pass it: every constituent passing holds when there are
/// none, and a green result over zero checks reads as checked and fine.
#[test]
fn a_merge_finding_no_constituent_fails() {
    let empty = tree();
    fs::create_dir(empty.path().join(bolt::run::WORK_DIR)).expect("an empty work directory");

    let refusal = bolt::merge::merge(empty.path()).expect_err("no constituent is a failure");

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

// COVERS: FR-10.7, FR-2.5a | positive
/// A refusal writes a `result.yaml` in the shape every refusal takes.
///
/// FR-10.7 has bolt write one whenever it is alive and in control when it
/// stops, so a caller finding none knows the process was killed rather than
/// that the run never started. FR-2.5a fixes the shape: `success: false` and a
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
        !verdict(&result, &wrench::ENVELOPE_SCHEMA),
        "a refusal's result says success: false",
    );

    let envelope = read_validated(&result, &wrench::ENVELOPE_SCHEMA);
    let reasons = envelope
        .get("reasons")
        .and_then(Value::as_array)
        .expect("a refusal carries reasons");
    assert_eq!(reasons.len(), 1, "one refusal is one reason: {reasons:?}");
    assert_eq!(
        reasons[0].get("kind").and_then(Value::as_str),
        Some("bolt-refused"),
        "a refusal's reason names its kind: {reasons:?}",
    );
    assert!(
        reasons[0]
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("unreadable")),
        "the reason does not say what was refused: {reasons:?}",
    );
}

// COVERS: FR-10.7a | edge
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

// COVERS: FR-10.7, FR-10.7a, FR-2.6b | regression
/// Refusing a directory as unusable does not write into it.
///
/// **This is the guarantee FR-10.7 can be satisfied without, which is why it is
/// asserted rather than described.** The Go build writes a conformant refusal
/// into the run directory it resolved, and because the stamp is second-granular
/// two runs starting in one second resolve to the same one: the second refuses,
/// correctly, and its refusal replaces the first's completed verdict while the
/// per-task evidence still says `nonzero-exit`. Reproduced 2026-08-28, filed
/// with a `repro.sh` at
/// `clank/inbox/bolt.go/a-refusal-overwrites-the-run-it-refused/`.
///
/// A refactor satisfying FR-10.7 by writing the refusal into the resolved
/// directory passes every other test in this file.
///
/// Deterministic without landing two runs inside one second: the run directory
/// is a function of the base and the clock, so the collision is set up by
/// predicting it. The retry covers a second boundary falling between predicting
/// and running, and a failure to collide is a failure rather than a pass, since
/// a test that quietly skips its own case is worse than no test.
#[test]
fn a_refusal_does_not_write_into_the_directory_it_refused() {
    let root = tree();
    let sentinel = "success: true\n";

    for attempt in 0..20 {
        let predicted = bolt::run::output_dir_for(root.path(), SystemTime::now());
        fs::create_dir_all(&predicted).expect("the colliding run directory");
        let result = predicted.join(bolt::run::RESULT_FILE);
        fs::write(&result, sentinel).expect("the previous run's verdict");

        let refusal = bolt::run::run("check", root.path());

        match refusal {
            Err(bolt::Error::OutputDirectoryInUse(path)) => {
                assert_eq!(path, predicted, "the refusal names the wrong directory");
                assert_eq!(
                    fs::read_to_string(&result).expect("the previous verdict survives"),
                    sentinel,
                    "the refusal overwrote the verdict of the run it refused to share",
                );
                return;
            }
            // The clock crossed a second, so this run resolved elsewhere and
            // the case was not exercised. Clear up and try again.
            other => {
                assert!(other.is_err(), "a directory holding a run must be refused");
                fs::remove_dir_all(&predicted).expect("the fixture is removable");
                assert!(attempt < 19, "never landed inside one second in 20 tries");
            }
        }
    }
}

// ---- the merge carries its constituents up --------------------------------

// COVERS: FR-8.4 | positive
/// The merged result carries the reasons its constituents produced.
///
/// FR-8.4 wants what failed **and why** readable from the merged file alone.
/// Synthesising one reason per failing constituent satisfies "what failed" and
/// loses "why": every failure then arrives as the same kind with the same
/// message, and a reader is sent back to the work directories the merge exists
/// to summarise.
///
/// Asserted on the constituent's own `kind` and `message` rather than on the
/// count, because a merge that renamed its synthesised kind would satisfy a
/// count and still carry nothing.
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
    let merged = read_validated(&result, &wrench::ENVELOPE_SCHEMA);
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
