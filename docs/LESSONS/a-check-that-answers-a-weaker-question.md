# A check that answers a weaker question than the one it is named for

The check runs. It reports success. It did not look at the thing.

That is different from a check that fails wrongly, and worse, because a failure
gets investigated and this does not. From inside the session running it, a check
that could not see the thing and a check that saw nothing wrong are the same
observation.

## Eleven instances, 2026-08-28 and 2026-08-29, across four repositories

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
| `ls -la bolt.go/bin/` | is there a file, not is it tracked |
| `find … -name 'bolt.*.yaml' \| wc -l` | how many paths, not how many files |
| a status line's staleness flag | is the flag set, on a line nobody printed |

Six are this tree's, four are wrench's, one is a coordinator's. Nobody in the
estate is better at this than anybody else, which is the argument for the
remedies rather than for care.

**Five of eleven are not bolt's, so this file is not a bolt lesson**, and where
it should live is filed at
`clank/inbox/silo/a-cross-repository-lesson-collection-lives-in-one-project/`.
A copy in wrench drifted to six instances before either of us noticed, in
`docs/LESSONS/`, which is the last directory anyone would search for a stale
copy. **If you are reading a version of this that says fewer than eleven, it is
not the one being maintained.**

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

## A tenth, three commits after this file was written

`ls -la` showed a 6MB executable in `bolt.go/bin/`, and I reported it to our
user as **committed**, contrasting it with the Rust tree's build artefact and
making that asymmetry a cutover blocker. It is gitignored:

    git -C ~/.projects/bolt.go ls-files bin/       bin/test-traceability.py only
    git -C ~/.projects/bolt.go check-ignore -v bin/bolt   .gitignore:17:/bin/bolt

`ls` answers "is there a file", and I read it as "is it tracked". One command
apart, and the second is the one the claim needed.

**The aggravating detail is the one to keep.** `NEXT_STEPS.md` already said
`gitignored at .gitignore:17`, in the section about that exact symlink. I had
the fact written down in my own tree and inferred a contradicting one from a
directory listing instead. So the remedies above are necessary and not
sufficient, and this is the one they were missing: **check whether you already
wrote it down.** A file you own is the cheapest second source there is, and the
one you are least likely to consult.

## An eleventh, and our user caught this one

"35 jigs in the estate" was quoted three times, into two repositories, as the
size of the cutover's compatibility surface. Our user asked whether it was
thirty-five jigs or the same four or five symlinked repeatedly.

    35 paths     22 symlinks, 13 real files, 6 of those are .definitions.yaml
     7 jigs      four of them toolbox's, symlinked 26 times between them

`find | wc -l` answers **how many places a jig is reachable from**. I wanted how
many jigs there are. `link-jigs` symlinking shared jigs into every adopter is
documented in this very tree, in the section of `PROJECT.md` about anvil, so
this is the second time in a day that a fact I had written down lost to an
inference from a listing.

**The remedy is one flag.** `find … -exec readlink -f {} \; | sort -u`. Resolve
before counting, whenever a tree is known to contain links.

**The conclusion survived and the number did not**, which is the tell for this
whole class: the key set over seven jigs is the same as over thirty-five,
because the duplicates were duplicates. A wrong number that supports a right
conclusion is the hardest kind to notice, and the only defence is that somebody
asks.

## The one that generalises furthest

**Running the wrong binary is re-running the claim.** The check does not fail,
it does not fire, and from the inside that is indistinguishable from it passing.
Anything that re-derives a fact from an artefact has this property, so the
question to keep asking is not "did the check pass" but **"could this check have
failed?"**
