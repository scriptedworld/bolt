# The installed binary gates everything, and nothing rebuilds it

`~/bin/bolt` resolves to `bolt/bin/bolt`, so every gate in the estate runs that
file. It is a copy, made by hand, and no part of the system puts a current one
there.

    cargo build --release && cp target/release/bolt bin/bolt

That is the install, and forgetting it is silent.

## A stale binary reports green

`a-second-build-answers-for-the-tree.md` records the one-session version of
this: a stale `target/release` gave a wrong answer about whether a change had
landed, caught by a peer noticing the wording differed.

At estate scale it does not give a wrong answer to notice. It runs the previous
bolt over the current source and reports success. The tools still read the
working tree, so the verdict about the *code* stays honest. What goes unreported
is anything wrong with **bolt itself**.

## Bolt cannot catch this about bolt

`bolt rust-quality .` is bolt gating bolt. Its tasks are `cargo build`, `cargo
clippy`, `cargo llvm-cov` and the rest, and those read the source, so a broken
source is caught. The runner executing them is the old binary. Break execution,
the adapters, the fold or the exit status, skip the install, and the thing that
would report the break is the thing that was replaced.

The suite is no defence either. `cargo test` builds from source and passes,
which is a true statement about code nobody is running.

## The estate is one instrument, so the usual remedy is unavailable

Every other instance in `a-check-that-answers-a-weaker-question.md` was caught by
two readings disagreeing: two binaries with different wordings, two sessions with
different counts, a row count against a denominator. The remedy that file arrives
at is *have something to disagree with*.

A stale `bin/bolt` removes that by construction. Twelve projects run the same
file, so twelve green gates are one observation repeated, not twelve. Agreement
across the estate looks identical whether the binary is current or a week old,
and **the more projects that agree, the more convincing the wrong answer gets**.

So the byte comparison below is not a nicety on top of the ordinary defences. It
is the only defence, because the ordinary one cannot exist here.

## The check, and it can fail

    cargo build --release && cmp -s target/release/bolt bin/bolt \
        && echo current || echo STALE

Seen to fail as well as to pass, which is what makes it a check: `current`
against a fresh install, `STALE` after a single byte was appended to `bin/bolt`.

Prefer this to comparing timestamps. `mtime` says which file is newer, not
whether they are the same program, and `cp` sets a fresh one whatever it copied.

## Install as part of gating

The gate line in `docs/PROJECT.md` is three commands for this reason:

    cargo build --release && cp target/release/bolt bin/bolt && bolt rust-quality .

Then the binary under test is the binary doing the testing, and the window does
not exist instead of being one somebody has to remember.

`./target/debug/bolt` and `cargo test` stay right for iterating. Only the
installed binary gates, and that distinction is the whole of this file.

## If it goes wrong

Reverting the estate to the Go build is one command and needs nobody's
permission:

    ln -sfn ../../bolt.go/bin/bolt.go ~/.projects/dotfiles/bin/bolt

`bolt.go` stays on `PATH` by name, so comparing the two does not require a
revert at all.

## Why this is bolt's lesson and not the estate's

The exposure is everybody's and the thing that can create it is one session's
habit. Every other project consumes the risk and cannot cause it, and a lesson
belongs where the cause is.
