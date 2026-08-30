# Contributing to bolt

## What you need

Rust 1.97 or newer, and a checkout of wrench at `../wrench/rust`, which
`Cargo.toml` takes as a path dependency.

The gate additionally runs `rustfmt`, `lizard`, `python3`, `cargo-llvm-cov`,
`cargo-audit` and `cargo-deny`. A jig names every executable it invokes in
`requires`, so a missing one refuses the run up front instead of failing a task
halfway through.

## The loops

`cargo test` is the fast one. The suite is 99 tests and runs in under a second.

The gate is a bolt run over bolt's own repository, so the binary under test is
the binary doing the testing:

```console
$ cargo build --release
$ ./target/release/bolt rust-quality . --output-dir .bolt-gate
/home/you/bolt/.bolt-gate/result.yaml
```

Eight tasks: format, lint, build, tests with coverage, vulnerabilities,
licences, complexity, traceability. Read the verdict in `result.yaml` and each
task's own output under `.bolt-gate/work/`. The run exits 0 whenever it could be
carried out, so the exit status is not the answer; `success` in the result is.

Build before you gate. A stale binary answers for the tree it was built from,
and a change to the runner, the adapters or the fold will not be in it.

## Traceability fails on purpose

`traceability` is the one task that does not pass, and making it green is not
the goal. It requires every test to cite the requirement it discharges, and
every cited requirement to exist:

```console
$ tail -1 .bolt-gate/work/traceability-1/stdout
147 of 226 requirements covered; 3 open and exempt
```

The uncovered rows are specified and unbuilt or built and untested. Marking them
open to clear the gate would misreport what is settled. The number going up is
the progress signal, and it moves in both directions as rows retire.

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
