# Measuring an entry point instead of excluding it

Hard rule 5 in the standing rules: never settle a coverage failure by excluding
the file. Coverage is judged per file so a well-tested file cannot carry an
untested one, and an exclusion drops that guarantee quietly.

An entry point is where that rule bites, because the test process usually cannot
reach it. This is what to do instead.

## The shape of the problem

`src/main.rs` is eleven lines and delegates to `bolt::cli::main`. Four tests
drive the real binary:

    Command::new(env!("CARGO_BIN_EXE_bolt"))

That is a subprocess. Its coverage is not in the harness profile, so the entry
point reads as uncovered however thoroughly it is exercised, and the tempting
fix is an exclusion that also hides everything else in the file forever.

## What to do instead

Keep the entry point empty enough that there is little to measure. That is the
first line of defence and is already the rule here: command functionality never
lives in `main.rs`, so the interface is reachable from an external test package.
What is left is argument handoff.

Then measure the subprocess instead of excluding it. The Go equivalent has a
worked example, and the shape is the same in Rust:

    Go     go build -cover, run the binary, go tool covdata textfmt,
           declare both profiles as evidence of one task, and have the
           adapter take the covered maximum per line.

    Rust   cargo llvm-cov instruments the binary too. A subprocess launched
           from a test writes its own profraw into the same directory when
           LLVM_PROFILE_FILE carries a pattern, and the report merges them.

Where two profiles are involved, declare both as evidence of one task and let
the adapter take the maximum. Two profiles disagreeing about a line is not a
conflict: one process reached it and the other did not, and covered-by-either is
the honest answer.

## The Rust route needs no second profile

`cargo llvm-cov` already merges the subprocess runs, and the entry point comes
out fully covered. Read out of the gate's own profile:

    src/cli.rs LF:84
    src/cli.rs LH:78
    src/main.rs LF:3
    src/main.rs LH:3

Three of three, and that number can only come from the subprocess. The in-process
tests call `bolt::cli::main` directly and never `main()` itself, so the only
thing that executes those three lines is a test launching the real binary.
Unmerged profiles would read 0 of 3.

Re-derive it instead of believing this. `end_of_record` has to reset the current
file, because `SF:` otherwise carries forward and the counts you read belong to
whatever record came next:

    bolt rust-quality . --output-dir .ephemera/qa
    awk '/^SF:/{f=$0} /^end_of_record/{f=""} /^L[FH]:/ && f ~ /\/(main|cli)\.rs$/ \
        {sub(/^SF:.*\/src\//,"src/",f); print f, $0}' \
        .ephemera/qa/work/tests-1/coverage.lcov

So `cargo llvm-cov` instruments the binary that `CARGO_BIN_EXE_bolt` points at,
and a subprocess started from a test writes into the same profile directory.
Nothing here needs a `go build -cover` equivalent.

For another Rust project in the estate, check the two numbers before assuming,
because it is one command.
