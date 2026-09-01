## What changed

One or two lines. The reasoning belongs in the files the change touches.

## How it was checked

`cargo test`, and the gate:

    cargo build --release
    ./target/release/bolt rust-quality . --output-dir .bolt-gate

Paste the traceability line, which is the one that moves:

    tail -1 .bolt-gate/work/traceability-1/stdout

`traceability` fails by design until every settled requirement has a test, so a failing gate
is expected there and nowhere else. `CONTRIBUTING.md` has the rest.

## Requirements

New behaviour needs a row in `REQUIREMENTS.md` and a test citing it:

    // COVERS: FR-4.11a | property

Retiring a row means moving it to `## Retired` and fixing every `COVERS:` mark
that cites it, in this change rather than a later one.
