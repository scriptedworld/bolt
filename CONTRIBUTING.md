# Contributing to bolt

## Prerequisites

`docs/runbook.md` carries all of it: the tools, the three sibling checkouts
bolt needs to build and gate, and the one command that links the shared gate
from toolbox. A clone that has not run that step fails `traceability` on its
checker being absent, which reads like a coverage failure until you look at
`stderr`.

A jig names every executable it invokes in `requires`, so a missing one refuses
the run up front instead of failing a task halfway through.

## The loops

`cargo test` is the fast one. The suite runs in under a second; `cargo test` prints how many.

The gate is a bolt run over bolt's own repository, so the binary under test is
the binary doing the testing:

```console
$ cargo build --release
$ ./target/release/bolt common-quality . --output-dir .bolt-gate
$ ./target/release/bolt rust-std-quality . --output-dir .bolt-gate-rust
/home/you/bolt/.bolt-gate/result.yaml
```

**Two runs, and neither jig is bolt's.** `bolt.rust-quality.yaml` lived here
while toolbox shipped no Rust jig; it moved to toolbox on 2026-09-03 as
`bolt.rust-std-quality.yaml`, and the tasks it marked as belonging to the common
jig were already there. This repository now carries no jig and no definitions
file: every value it set is the shared default.

`rust-std-quality` is six tasks: format, lint, build, tests, vuln, licences.
`common-quality` is three: traceability, suppressions, secrets. Complexity is no
longer a task of its own — the four numbers it produced now come from clippy
inside `lint`, which is why that task's description says so.

Read the verdict in `result.yaml` and each task's own output under `work/`. A
run exits 0 whenever it could be carried out, so the exit status is not the
answer; `success` in the result is.

**The tests task judges that profile per file at 80% of lines**, as of
toolbox's 2026-09-04 change. `coverage.lcov` lands in the work directory as
evidence and `adapters/rust/coverage.py` reads it, so a file falling below the
line fails the run and names itself. There is no aggregate threshold, because an
aggregate is what lets a well-tested file carry an untested one.

**Lines and not branches, and that is the toolchain rather than a choice.**
cargo-llvm-cov writes `BRF:0` and no `BRDA` records at all without `--branch`,
which is unstable and needs a nightly compiler. The adapter reads branch records
where they exist and reports `branch_measured: false` where they do not, so
nothing here passes a threshold that had nothing to judge.
That is a gap rather than a decision: the shared Go jig judges coverage per
file and this one does not yet.

Build before you gate. A stale binary answers for the tree it was built from,
and a change to the runner, the adapters or the fold will not be in it.

## Traceability fails on purpose

`traceability` is the one task that does not pass, and making it green is not
the goal. It requires every test to cite the requirement it discharges, and
every cited requirement to exist.

The uncovered rows include requirements that need tests, requirements that need
citations against existing tests, and design properties no test can observe.
The last group waits on a decision recorded in `NEXT_STEPS.md`. Marking a row
open merely to clear the gate would misreport what is settled.

Mass co-citation is refused for the reason the gate exists. A citation is
checked for naming a real row, never for the test touching it, so a wrong
citation is indistinguishable from a right one forever.

## Requirements and tests

`REQUIREMENTS.md` states what must be true. Every test names the requirement it
discharges in a comment directly above it:

```rust
// COVERS: FR-4.11a, FR-4.11b | property
```

The kinds are `positive`, `negative`, `edge`, `property` and `regression`. A
test citing nothing fails the gate, and so does one citing a row
`REQUIREMENTS.md` does not define.

A requirement can be retired or superseded, and a `## Retired` section records
where it went. An ID is never reused: reuse silently rewrites what every
existing reference to that ID means, and nothing about the new row looks wrong.
Retiring a row means fixing the `COVERS:` marks that cite it in the same change.

An ID takes at most one letter of suffix. The checker's grammar is
`(?:FR|NFR)-\d+(?:\.\d+)?[a-z]?`, so `FR-10.8a` is an ID and `FR-10.8ca` is
not: a two-letter suffix fails to match as a row at all and is silently absent
from the denominator, while the same text in a `COVERS:` mark degrades to
`FR-10` and fails loudly. Where a letter is taken, take the next number.

Count the rows against the denominator, because a row the checker cannot see is
not reported as missing:

```console
$ awk '/^## Retired/{r=1} !r && /^\| *(FR|NFR)-[0-9.a-z]+ \|/{n++} END{print n}' REQUIREMENTS.md
$ tail -1 .bolt-gate/work/traceability-1/stdout
```

Live rows must equal the denominator plus the exempt count.

## Where code goes

`src/main.rs` carries no command functionality. It delegates to `cli::main`, so
the whole interface is reachable from an external test package.

`tests/skeleton.rs` is the entire suite, in one file on purpose. A shared
`tests/common/mod.rs` compiles into every test binary separately, and its
helpers are then dead code in each binary that does not call them, which fails
the gate under `-D warnings`.

Tests are held to the same bar as the code, with no exemption from length,
duplication or complexity. `complexity` is the task that catches a change
growing a function past 15 cyclomatic complexity, 60 lines or 5 parameters, and
it reads the whole tree rather than `src`.

## When a check fails

Fix the code, not the threshold. Do not add a suppression pragma, an `allow`
attribute or a mock to quiet a gate: raise it as a question instead, with what
the check found and why silencing it would be right.

## Commits

Conventional commits. The subject says what changed. The body says what it cost,
in figures a reader of the log cannot get anywhere else, and stops there. The
reasoning belongs in the file the commit changed and the requirement belongs in
`REQUIREMENTS.md`; a message restating either has written one thing twice, and
the log is the copy nobody can correct later.

## Prose

Documentation is evergreen: it states what is true, and leaves the history to
git. That reaches prose inside the source too, including doc comments and the
text a refusal prints. No em-dashes anywhere. A comment earns its place by
saying what would cost time to rediscover, and does not restate the line below
it.
