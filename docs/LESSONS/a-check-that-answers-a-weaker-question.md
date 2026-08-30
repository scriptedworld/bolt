# A check that answers a weaker question than the one it is named for

The check runs. It reports success. It did not look at the thing.

That differs from a check that fails wrongly, and is worse, because a failure
gets investigated and this does not. From inside the session running it, a check
that could not see the thing and a check that saw nothing wrong are the same
observation.

## Twelve instances, across four repositories

Collected because the shape is only obvious with several beside each other. Any
one of them reads as an ordinary bug.

| what it was named for | what it actually answered |
|---|---|
| `failed: 3 execution(s)` | how many executions ran |
| `cargo test` passing | do the tests pass at *this* depth |
| running `target/release/bolt` | what bolt did when that binary was built |
| a `COVERS:` row count | how many rows the grammar could parse |
| `cmd \| tail -3` then `$?` | did `tail` succeed |
| two mtimes, one second apart | did the second the clock can see change |
| two runs into one directory | do two *refusals* differ |
| `grep -rn … src/` | how many in `src/` |
| `ls -la bolt.go/bin/` | is there a file, not is it tracked |
| `find … -name 'bolt.*.yaml' \| wc -l` | how many paths, not how many files |
| a status line's staleness flag | is the flag set, on a line nobody printed |
| `git status --short && echo clean` | did `git status` run |

Seven are this tree's, four are wrench's, one is a coordinator's. Nobody in the
estate is better at this than anybody else, which is the argument for the
remedies rather than for care.

Five of twelve are not bolt's, so where this file should live is filed at
`clank/inbox/silo/a-cross-repository-lesson-collection-lives-in-one-project/`.
A copy in wrench drifted, in `docs/LESSONS/`, which is the last directory
anybody would search for a stale copy. **If you are reading a version of this
that says fewer than twelve, it is not the one being maintained.**

## Why the name is the trap

Every one of these is a correct answer to a real question. `tail` did succeed.
The grammar did parse every row it could. The release binary did behave that way
when it was built. Nothing is malfunctioning, so nothing reports a malfunction.

The gap is between the question asked and the question the instrument can reach,
and it closes silently because the instrument reports on its own terms while the
name is written in yours.

## What to do, since care is not a remedy

**Reconcile two numbers that must agree.** One instrument cannot detect its own
blind spot; two disagreeing can. The requirement rows were caught this way:
`live rows == denominator + exempt`, and 245 against 240 + 3 was the whole
signal. `docs/PROJECT.md` carries it as a standing check.

**Have something to disagree with.** The stale binary surfaced only because two
sessions quoted the exact output and the wordings differed. A measurement with
no second source returns whatever the artefact says and reports it as fact.

**Quote the bytes, not the summary.** "The refusal names the field" and "task
child carries the retired jig field" are the same claim, and only the second can
be seen to differ from what somebody else is holding.

**Pick an instrument finer than the thing measured.** Two writes inside one
second are indistinguishable by mtime and distinct by checksum. If the
resolution is not obviously finer, it is not.

**Construct the case so the thing is actually present.** A reuse test needs run
one to have passed; against a jig that refuses for another reason it compares
two refusals and finds no overwrite, because there was no verdict to overwrite.
Ask what a positive result would look like before running it.

**Measure at the widest scope, then narrow.** A scan of `src/` quoted as a
repository total was a quarter of the real number. Run it against everything,
then explain any exclusion.

**Check whether you already wrote it down.** `ls -la` showed a 6MB executable in
`bolt.go/bin/` and it went into a report as *committed*; `NEXT_STEPS.md` said
`gitignored at .gitignore:17` three lines from where the claim landed. A file
you own is the cheapest second source there is and the one you are least likely
to consult.

**Make the scope visible in the output.** `141 of 245 … 3 open and exempt` can be
reconciled by a reader; `failed: 3` cannot. A check that prints only its verdict
cannot be audited by the person reading it.

## Which commands the remedies get skipped on

The remedies above are known and do get used. Where they are not is the
throwaway check, which is where they cost most.

Wrench found both of its pipe-status instances in ad-hoc verification one-liners
and none in its gate commands, which already redirect to a file and test for the
artefact afterwards. The coordinator's was a `$?` read after a pipe while
checking whether a peer's report was true. This tree's newest row is the same
shape: `git status --short && echo clean` prints `clean` against a dirty tree,
because `git status` exits 0 either way, and it was written to tell somebody the
tree was clean.

The ceremony goes where the output is kept and is skipped where the output only
decides what somebody believes. So the targeting rule is not which commands
matter; it is which produce nothing but an answer, because a gate command leaves
an artefact somebody reads again and a one-liner leaves a conclusion.

## A sibling shape: the answer was right and expired

Kept out of the table deliberately. Every row above is a check that never looked
at the thing. This one looked, correctly, and the world moved.

    wrench 519cfd4 committed        08:17:00
    the find ran                    between then and 08:19:35
    "bolt and wrench, two projects" 08:19:35
    silo's result.yaml written      08:19:58

178 seconds. The instrument was an epoch comparison against the symlink's mtime,
which is the right one, and the claim was true when written and false when read.
"Who has run since X" is a question about a moving target.

An instrument error is fixed by a better instrument. A perishable answer is
fixed by publishing the time of the reading and not only the threshold it was
measured against, since a threshold with no reading time lets a correct
measurement become a wrong claim with nothing having gone wrong.

So when the question is "since when", record when you looked, and prefer handing
on the command over its output: a command re-runs and a number does not.

## The one that generalises furthest

Running the wrong binary is re-running the claim. The check does not fail, it
does not fire, and from the inside that is indistinguishable from it passing.
Anything that re-derives a fact from an artefact has this property, so the
question to keep asking is not "did the check pass" but **"could this check have
failed?"**
