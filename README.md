# bolt

Bolt runs a project's quality gate. One YAML file, a jig, declares the commands
and which files each of them acts on. Bolt runs them over a directory, keeps
every stream and exit status on disk, and folds the results into one verdict a
build system can read.

A gate accumulates: a formatter, a linter, a test run with a coverage
threshold, a licence check, each with its own flags and its own idea of what
failure looks like. Wired into a script, that script has to change whenever any
of the tools does, and it decays into something nobody will touch.

Bolt knows nothing about any tool it runs. It executes what the jig declares and
hands each execution's output to an adapter that turns it into a verdict, so
swapping a linter for another linter is an edit to the jig. Bolt is the part
that does not have to change.

## Where this stands, and what is not done

**Active, and in daily use.** Bolt gates its own repository and five others,
and this is the Rust implementation that replaced an earlier Go one, which has
been deleted rather than kept as a fallback.

MIT licensed because somebody else might want it, not run as an open-source
project: no release cadence and no compatibility promise. The command line is
the interface.

**Working today:** the walk and per-task filtering, both path forms, single-pass
substitution with three-layer definitions, `requires` resolved up front,
adapters and declared evidence, per-task and whole-run time limits, the depth
ceiling, short-circuit, the fold, and refusals that write a parseable result.

**Not done, and the gate reports it rather than hiding it:**

- **147 of 228 settled requirements have a test citing them.** The traceability
  task fails until the rest do, and prints the number.
- Requirements describing design constraints no test can observe need
  classifying: made testable, moved to the component that owns them, or
  exempted.
- `REQUIREMENTS.md` is one file and should be one per requirement. The checker
  reads either shape; it waits on where a retired id lives once the single file
  is gone, and 63 rows are retired.
- The repository-local quality jig should become the shared one once that
  offers the same checks.
- Definitions files and the jig's `definitions` block have no schema yet.

**Setup takes three repositories.** bolt runs the gates, toolbox supplies the
jigs and checkers, wrench validates the structured files. `docs/runbook.md`
covers cloning them as siblings and linking them, and a fresh clone has no gate
until you do.

## Running it

A jig is a file called `bolt.<name>.yaml`. Bolt looks for it in the directory
being run over, or in the directory given to `--config-dir`. A run names the jig
and the directory:

```yaml
requires: [cargo, lizard]

tasks:
  - name: format
    command: cargo fmt --check

  - name: lint
    command: cargo clippy --all-targets -- -D warnings
    time-limit: "5m"

  - name: complexity
    matching: ["**/*.rs"]
    excluding: ["**/target/**"]
    command: lizard --CCN 15 {all_paths}
```

```console
$ bolt rust-quality .
/home/you/project/.bolt-2026-08-30T03-34-55Z-1921009/result.yaml
```

Bolt prints where the result is, not what it says. The verdict belongs in the
file, and a caller that wants it reads one document either way. The path is
absolute whatever the run was given, so a caller can hold it without also
holding the directory it was run from.

Every executable a jig names goes in `requires`, and a run stops before it
starts if one of them is not on `PATH`. Half a gate run on a machine missing a
tool costs more than the check that would have caught it.

## What it leaves behind

```
.bolt-2026-08-30T03-34-55Z-1921009/
├── result.yaml              the run's one verdict, folded from below
└── work/
    ├── format-1/
    │   ├── manifest.yaml    the command as executed, and what it was handed
    │   ├── stdout, stderr   captured at the process boundary
    │   ├── exitcode
    │   └── output.yaml      the adapter's envelope: this execution's verdict
    ├── lint-1/
    └── complexity-1/
```

`result.yaml` carries `success`, and `reasons` giving a `kind` and a `message`
when it is false:

```yaml
"reasons":
  - "kind": "nonzero-exit"
    "message": "lint exited 1"
"success": false
```

Every execution gets a directory whether it passed, failed, was killed at a time
limit, or never started. The manifest is written before the command runs, so an
execution that was killed still records what it was going to attempt.

The default directory carries the time and the process id, so two runs starting
in the same second get two directories. `--output-dir` names one instead, and a
run refuses a directory that already holds a run, because writing into one would
interleave two runs' evidence.

## The exit status answers a different question

The exit status says whether bolt could carry out the run, never whether the
tools passed. A gate whose linter found problems exits 0, with `success: false`
in the result. A bolt that could not read its jig exits non-zero, and still
writes a result saying which refusal it was.

For a shell that needs to chain runs, `--result-to-exitcode` collapses the two:
0 when `success` is true and 1 otherwise, with no third case.

## How it is put together

A task is one command plus which files it acts on. `{each_path}` runs the
command once per matched file and `{all_paths}` runs it once with the whole
selection. Which of the two applies is read off the command, so there is no mode
to set and no way to set it inconsistently.

An adapter is a separate program that reads an execution's captured output and
writes an envelope. Where an adapter reaches a verdict, that verdict is the
result and bolt does not second-guess it. A task naming no adapter gets the
generic exit-code adapter, the one adapter that needs to know nothing about the
tool it is reading. `docs/PATTERNS/the-adapter-contract.md` is what you write
one against.

Definitions are values a command names as `{placeholder}`, resolved against
bolt's own locations first, then the jig's `definitions` block, then a file
named with `--definitions`. A shared jig ships defaults and an adopter overrides
one line without forking it.

A failing task does not stop the run. Stopping discards the evidence the later
tasks would have produced and leaves a reader unable to tell what else was
wrong. A task asks for the opposite with `short-circuit-failure`.

A time limit kills the process group, so a command that spawned children does
not leave them writing into a directory bolt has finished with. The killed
command keeps whatever output it gathered and its adapter still runs over it,
because a tool that reported forty problems before hanging reported forty real
problems.

`docs/jig-reference.md` has every field, every placeholder, and the full shape
of what a run writes.

## A jig is executable input

Bolt runs the commands a jig declares, with the privileges of whoever started
it. Treat a jig from somewhere else the way you would treat a shell script from
somewhere else. `SECURITY.md` states the trust boundary and how to report a
vulnerability.

## Building it

Rust 1.97 or newer.

```console
cargo build --release
```

The binary lands at `target/release/bolt`.

Bolt reads and writes every structured file through wrench, which validates each
one against a shipped schema on the way in and on the way out. `Cargo.toml`
takes wrench as a path dependency at `../wrench/rust`, so a checkout of wrench
has to sit beside this one for the build to resolve.

Nothing in the dependency tree compiles C, and `libc` is declarations only. The
binary is dynamically linked against the system `libc`, `libm` and `libgcc_s`,
so it is not a single-file image today.

The gate itself comes from a third repository, `toolbox`, which ten projects
adopt from one source. `docs/runbook.md` is the setup: what to install, the
three checkouts, and the one command that links the gate into a clone.

`CONTRIBUTING.md` has the gate, the test conventions and how a change gets made.

## What it does not do

Tasks run one at a time. There is no parallel execution and nothing schedules
them.

No task consumes another task's output. Work that needs several steps is one
script producing one exit code.

One jig over one directory per invocation. A repository of subprojects uses a
jig whose ordinary task commands invoke bolt. An adapter turns each child
result into a task envelope, and the parent folds those envelopes as ordinary
constituents. There is no separate jig task or nesting data model.

Bolt judges nothing itself beyond the exit status. Anything richer is an
adapter's verdict, and adapters are separate programs a jig names.

Unix only. Time limits kill the process group through `libc::kill`, so the
build does not target Windows.

## Composition

A task can run bolt against a subdirectory and take its verdict, which is how a
repository of several projects gates as one.

```yaml
  - name: subproject
    command: bolt inner {base_dir}/sub --output-dir {work_dir}/child
    adapter: adapters/common/bolt-result.py
```

`docs/PROJECT.md` says where the work stands and what is not done.

## Licence

Apache-2.0. See `LICENSE` and `NOTICE`.
