# What is not done

Bolt is an active rebuild with no released version or stable interface. The
command line is the only supported interface, and the crate is not published.

The remaining implementation work is:

- Add tests for settled requirements that are not yet covered. Traceability is
  intentionally incomplete while those tests are absent.
- Decide how to classify requirements that describe design constraints no test
  can observe. They must become testable, move to the component that owns them,
  or be explicitly exempted.
- Split `REQUIREMENTS.md` into one file per requirement category. The checker
  reads either shape already. What this waits on is where a retired id is
  recorded once the single file is gone, and 63 rows are retired. See
  `docs/DECISIONS/requirements-are-a-directory.md`.
- Replace the repository-local quality jig with shared jig definitions once the
  shared Rust definition provides the same checks. See
  `docs/DECISIONS/the-rust-quality-jig-is-shared.md`.
- Add and validate schemas for definitions files and for the jig's `definitions`
  block in the structured-file library.

# Decisions still needed

## Task execution

- Define whether task commands inherit the complete environment or a filtered
  environment.
- Decide whether an empty selection is represented in `result.yaml`.
- Decide whether a manifest records the complete walk or only the paths selected
  for its task.
- Decide whether a demonstrated need justifies reopening the rule that tasks do
  not consume another task's output. See
  `docs/DECISIONS/tasks-do-not-consume-other-task-output.md`.
- Decide whether commands using `{all_paths}` need a response-file mechanism to
  avoid the platform argument-length limit.
- Define how a changed-files overlay composes with the ordinary project walk.

## Results and refusals

- Decide whether `evidence-missing` should retain a command's nonzero exit
  status when both conditions apply.
- Define ownership and retention for run directories.
- Decide whether result envelopes need a schema-version field and whether
  adapter-specific metadata is open-ended.

## Composition and limits

- Define how a run based in a subdirectory learns an enclosing project root.
  Today `{project_root}` is the invocation's base directory. Composition remains
  an ordinary command line; see `docs/DECISIONS/composition-is-a-command-line.md`.
- Decide whether cycles need detection by identity in addition to the depth
  ceiling.
- Decide whether jigs can be referenced by version or always resolve from the
  files currently available.
- Decide whether the depth ceiling should be reinforced with an ancestry check.
- Decide whether live runs need a per-user cap.
- Define how a future parallel-execution budget propagates through composed
  invocations.
- Decide whether a personal policy layer exists alongside repository policy.

## Ecosystem boundaries

- Assign ownership of per-file coverage policy between bolt and its adapters.
- Define whether downstream ratchets consume `result.yaml` directly or wrap it.
- Assign responsibility for recording source-tree state.
- Decide whether bolt will expose an interface other than the command line.

Settled choices are in `docs/DECISIONS/`. Supporting design explanation is in
`docs/DESIGN-NOTES.md`.
