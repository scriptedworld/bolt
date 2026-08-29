# A second build answers for the tree, and it answers as of when it was built

Found 2026-08-28 by the wrench session, measuring bolt's behaviour against this
repository rather than asking about it. That is the right way round, and it is
why they hit it and I did not.

## What happened

Two binaries existed here and they disagreed about whether a change had landed.

    target/debug/bolt      19:53   task child carries the retired jig field;
                                   run the jig as a command instead,
                                   bolt <jig> <directory>
    target/release/bolt    14:15   task child names a jig; nested jigs are
                                   specified and not built yet

Reproduce the shape with a jig carrying one task and one `jig:` field:

    ./target/<build>/bolt jigtask <dir> --config-dir <cfg> --output-dir <out>

The release binary predated `f3304d8`, where nesting retired. It gave the
message from before the change, so it reported that composition was unbuilt.
**A reader measuring against it concludes the retirement has not landed**, which
is a wrong answer arriving with all the authority of a measurement.

wrench hit the release binary first and would have quoted the old message back
as a finding. They checked which binary they were holding, which is what turned
it from a wrong report into this note.

## Why it is worse than a stale document

A stale document is read as prose and weighed. **A binary is run, and its output
is evidence.** The whole discipline here is to measure rather than believe, so an
artifact that answers wrongly defeats the check that was supposed to catch it.

It is the same shape as the Go build issuing false greens from a `+dirty` tree:
the thing that looks like the tool is not the tool the tree describes.

## What to do

**One built binary, and the gate builds it.** `cargo build &&
./target/debug/bolt rust-quality .` is the gate line, so the debug binary is
rebuilt by the act of running the gate and cannot lag what is committed. The
release tree was built once by hand, is referenced by nothing in this repository
or the estate, and was deleted rather than kept in step.

**Deleting it is not the fix, and saying so is the point.** `cargo build
--release` re-creates it, and the next hand-built copy will go stale the same
way. What survives is the habit: **before quoting what bolt does, check which
bolt you ran.**

    ls -la --time-style=+%H:%M target/*/bolt
    git log -1 --format=%h%x20%cd --date=format:%H:%M

A binary older than `HEAD` describes an earlier tree. That comparison is one
command and it is the whole check.

## For a sibling measuring against this repository

Use `target/debug/bolt`, and run `cargo build` first. It costs under a second on
a warm tree and removes the question entirely.
