# A check that answers a weaker question than the one it is named for

The check runs. It reports success. It did not look at the thing.

That is different from a check that fails wrongly, and worse, because a failure
gets investigated and this does not. From inside the session running it, a check
that could not see the thing and a check that saw nothing wrong are the same
observation.

## Nine instances, 2026-08-28 and 2026-08-29, across four repositories

They are collected because the shape is only obvious with several beside each
other. Any one of them reads as an ordinary bug.

| what it was named for | what it actually answered |
|---|---|
| `failed: 3 execution(s)` | how many executions ran |
| `cargo test` passing | do the tests pass at *this* depth |
| running `target/release/bolt` | what did bolt do *when that binary was built* |
| a `COVERS:` row count | how many rows the grammar could parse |
| `cmd \| tail -3` then `$?` | did `tail` succeed |
| two mtimes, one second apart | did the second **that the clock can see** change |
| two runs into one directory | do two *refusals* differ |
| `grep -rn … src/` | how many in `src/` |
| a status line's staleness flag | is the flag set, on a line nobody printed |

Four are this tree's, four are wrench's, one is a coordinator's. Nobody in the
estate is better at this than anybody else, which is the argument for the
remedies rather than for care.

## Why the name is the trap

Every one of these is a **correct answer to a real question**. `tail` did
succeed. The grammar did parse every row it could. The release binary did behave
that way when it was built. Nothing is malfunctioning, so nothing reports a
malfunction.

The gap is between the question asked and the question the instrument can reach,
and it closes silently because the instrument reports on its own terms and the
name is written in yours.

## What to do, since care is not a remedy

**Reconcile two numbers that must agree.** One instrument cannot detect its own
blind spot; two disagreeing can. The requirement rows were caught this way:
`live rows == denominator + exempt`, and 245 against 240 + 3 was the whole
signal. `docs/PROJECT.md` carries it as a standing check.

**Have something to disagree with.** The stale binary surfaced only because two
sessions quoted the exact output and the wordings differed. A measurement with
no second source returns whatever the artefact says and reports it as fact.
`a-second-build-answers-for-the-tree.md` carries that one, including the
correction that measuring was *not* what caught it.

**Quote the bytes, not the summary.** "The refusal names the field" and "task
child carries the retired jig field" are the same claim, and only the second can
be seen to differ from what somebody else is holding.

**Pick an instrument finer than the thing measured.** Two writes inside one
second are indistinguishable by mtime and distinct by checksum. If the
resolution is not obviously finer, it is probably not.

**Construct the case so the thing is actually present.** A reuse test needs run
one to have *passed*; against a jig that refuses for another reason it compares
two refusals and finds no overwrite because there was no verdict to overwrite.
Ask what a positive result would look like before running it.

**Measure at the widest scope, then narrow.** A scan of `src/` quoted as a
repository total was a quarter of the real number. Run it against everything,
then explain any exclusion.

**Make the scope visible in the output.** `141 of 245 … 3 open and exempt` can be
reconciled by a reader; `failed: 3` cannot. A check that prints only its verdict
cannot be audited by the person reading it.

## The one that generalises furthest

**Running the wrong binary is re-running the claim.** The check does not fail,
it does not fire, and from the inside that is indistinguishable from it passing.
Anything that re-derives a fact from an artefact has this property, so the
question to keep asking is not "did the check pass" but **"could this check have
failed?"**
