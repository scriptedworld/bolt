# bolt

Bolt runs a declared set of commands over a directory and records what happened.

It knows nothing about any tool it runs. A jig says which commands, over which
files; bolt executes them, keeps every stream and exit status on disk, hands each
one's output to an adapter that turns it into a verdict, and folds those verdicts
into a single result. Swapping a linter for another linter is an edit to the jig.

That separation is the whole idea. A quality gate that knows about its tools has
to change whenever they do, and gates written that way rot into shell scripts
nobody will touch. Bolt is the part that does not need to change.

## What a run looks like

A jig is `bolt.<name>.yaml`, and a run names the jig and a directory:

```yaml
version: "1.0.0"
requires: [cargo, lizard]

tasks:
  - name: format
    command: "cargo fmt --check"

  - name: lint
    command: "cargo clippy --all-targets -- -D warnings"
    time-limit: "5m"

  - name: complexity
    matching: ["**/*.rs"]
    excluding: ["**/target/**"]
    command: "lizard --CCN 15 {all_paths}"
    adapter: adapters/lizard.py
```

```console
$ bolt rust-quality .
/home/you/project/.bolt-2026-08-28T13-45-02Z/result.yaml
```

Bolt prints where the result is rather than what it says, because the verdict
belongs in the file and a caller that wants it reads one document either way.

## What it leaves behind

```
.bolt-2026-08-28T13-45-02Z/
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

Every execution gets a directory whether it passed, failed, was killed at a time
limit, or never started. The manifest is written **before** the command runs, so
an execution that was killed still records what it was going to attempt.

`result.yaml` is an envelope, the same shape every producer in this ecosystem
writes: `success`, and `reasons` carrying a `kind` and a `message` when it is
false. A consumer reads one format whatever produced it.

## The pieces

**Jig.** A named YAML file listing tasks. Spoken of by name, never by path, so a
shared jig can be distributed and a project runs `bolt go-quality .` without
knowing where the file came from.

**Task.** One command, plus which files it acts on. `{each_path}` runs it once
per matched file; `{all_paths}` runs it once with the whole selection. Which of
the two is read off the command, so there is no mode to set and no way to set it
inconsistently.

**Adapter.** A separate program that reads an execution's captured output and
writes an envelope. Where it reaches a verdict, that verdict **is** the result and
bolt does not second-guess it. A task naming no adapter gets the generic
exit-code one, which is the single adapter that needs to know nothing about the
tool it is reading.

**Definitions.** Values a jig's commands name as `{placeholder}`, resolved
against three layers: bolt's own locations, then the jig's `definitions` block,
then a file named with `--definitions`. So a shared jig ships defaults and an
adopter overrides one line without forking it.

## Things it is deliberate about

**Every substituted path is quoted, in a single left-to-right pass.** Not one
pass per variable. Chained replacement re-expands a token that appears inside an
already-substituted filename, which breaks the quoting: a file named
``p{all_paths};id #`` executed `id`. The property is the quoting *and* never
reading substituted bytes again.

**A failing task does not stop the run.** Stopping discards the evidence the
later tasks would have produced and leaves a reader unable to tell what else was
wrong. A task can ask for the opposite with `short-circuit-failure`.

**A run refuses rather than writing into a directory that already holds one.**
The default output directory is stamped to the second, so two runs starting
together would otherwise interleave their evidence and hand both callers the
same conflated file.

**The exit status says whether bolt could carry out the run, not whether the
tools passed.** A gate whose linter found problems exits 0 with
`success: false`; a bolt that could not read the jig exits non-zero. Those are
different questions and a caller usually wants them answered separately.

**A time limit kills the process group**, so a command that spawned children
does not leave them writing into a directory bolt has finished with. The killed
command keeps whatever output it gathered and its adapter still runs over it,
because a tool that reported forty problems before hanging reported forty real
problems.

## Building it

```console
cargo build --release
```

One binary, no runtime dependencies, no C toolchain. Bolt's own gate is a bolt
run over its own repository:

```console
cargo build && ./target/debug/bolt rust-quality .
```

## Status

**Rust, under active rebuild, and honest about it.** Bolt was previously written
in Go; this tree is a fresh implementation derived from the architecture
document rather than a port, which is why its requirements are renumbered and
its coverage is counted against a new document.

What works today: the walk and per-task filtering, both path forms, single-pass
substitution with three-layer definitions, `requires` resolved up front,
adapters and declared evidence, per-task and whole-run time limits, the depth
ceiling, short-circuit, the merge, and refusals that write a parseable result.

**Composition is a command line.** A jig that wants another jig run over a
subdirectory writes `bolt` in a task's command, as it writes any other tool, and
an adapter turns the child's result into that task's envelope. There is no jig
task and no nesting mechanism: bolt is a tool a jig runs, and the runner does
not know which of its commands is bolt.

    - name: subproject
      command: bolt inner {base_dir}/sub --output-dir {work_dir}/child
      adapter: bolt-result

The adapter is what carries the verdict. Bolt exits 0 whenever it carried a run
out, whatever the tools concluded, so the verdict lives in the envelope and the
adapter is how it travels between runs.

**`--result-to-exitcode` opts out of that**, for a shell that needs to compose:

    bolt --result-to-exitcode gate . && bolt --result-to-exitcode other .

The rule is `0 if success else 1` and it has no cases; a refusal is 1 like any
other failure, because a refusal is a verdict bolt reached. Off unless named, so
every caller written against the default is unaffected.

Its own gate reports eight tasks, seven passing. The eighth is traceability,
which requires every test to cite a requirement and every cited requirement to
exist; it currently reports 131 of 234 covered and fails on the rest. **That is
the state of a rebuild rather than a defect**, and it is left failing on purpose:
turning it green by marking the uncovered rows as open questions would misreport
what is settled.

## Licence

Apache-2.0. See `LICENSE` and `NOTICE`.
