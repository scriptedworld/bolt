# The installed binary gates everything, and nothing rebuilds it

Since 2026-08-29 `~/bin/bolt` resolves to `bolt/bin/bolt`, so every gate in the
estate runs this file. **It is a copy, made by hand, and no part of the system
puts a current one there.**

    cargo build --release && cp target/release/bolt bin/bolt

That is the install, and forgetting it is silent.

## What changed, which is scale and not kind

`a-second-build-answers-for-the-tree.md` records a stale `target/release` giving
one session a wrong answer about whether a change had landed. Same mechanism,
one reader, caught by a peer noticing the wording differed.

The cutover moved that hazard onto every project. A stale `bin/bolt` does not
fail: it runs the previous bolt over the current source and **reports green**.
Nobody sees a wrong message to notice, because the tools still read the working
tree and the verdict about the *code* stays honest. What goes unreported is
anything wrong with **bolt itself**.

## Bolt cannot catch this about bolt

`bolt rust-quality .` is bolt gating bolt. Its tasks are `cargo build`, `cargo
clippy`, `cargo llvm-cov` and the rest, and those read the source, so a broken
source is caught. **The runner executing them is the old binary.** Break
execution, the adapters, the fold or the exit status, skip the install, and the
thing that would report the break is the thing that was replaced.

The suite is not a defence either. `cargo test` builds from source and passes,
which is a true statement about code nobody is running.

## The check, and it can fail

Byte comparison against what the current source builds:

    cargo build --release && cmp -s target/release/bolt bin/bolt \
        && echo current || echo STALE

Verified 2026-08-29 in both directions, because a check nobody has seen fail is
not yet a check: it reported `current` against a fresh install, and `STALE`
after a single byte was appended to `bin/bolt`.

Prefer this to comparing timestamps. `mtime` says which file is newer, not
whether they are the same program, and `cp` sets a fresh mtime whatever it
copied.

## What to do

**Install as part of gating, not before committing.** The gate line in
`docs/PROJECT.md` is three commands for this reason:

    cargo build --release && cp target/release/bolt bin/bolt && bolt rust-quality .

Then the binary under test is the binary doing the testing, and the window does
not exist rather than being one somebody has to remember.

`./target/debug/bolt` and `cargo test` stay right for iterating. **Only the
installed binary gates**, and that distinction is the whole of this file.

## If it goes wrong

Reverting the estate to the Go build is one command and needs nobody's
permission:

    ln -sfn ../../bolt.go/bin/bolt.go ~/.projects/dotfiles/bin/bolt

`bolt.go` stays on `PATH` by name, so comparing the two is one command and does
not require a revert at all.

## Why this is bolt's lesson and not the estate's

Filed here on the wrench session's argument, 2026-08-29: the exposure is now
everybody's, and the thing that can create it is one session's habit. Every
other project consumes the risk and cannot cause it. A lesson belongs where the
cause is.
