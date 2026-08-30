# A second build answers for the tree, and it answers as of when it was built

Two binaries in this tree disagreed about whether a change had landed.

    target/debug/bolt      19:53   task child carries the retired jig field;
                                   run the jig as a command instead,
                                   bolt <jig> <directory>
    target/release/bolt    14:15   task child names a jig; nested jigs are
                                   specified and not built yet

Reproduce the shape with a jig carrying one task and one `jig:` field:

    ./target/<build>/bolt jigtask <dir> --config-dir <cfg> --output-dir <out>

The release binary predated the commit where nesting retired, so it gave the
message from before the change and reported that composition was unbuilt. A
reader measuring against it concludes the retirement has not landed, which is a
wrong answer arriving with the authority of a measurement.

## Why it is worse than a stale document

A stale document is read as prose and weighed. A binary is run, and its output is
evidence. The whole discipline here is to measure rather than believe, so an
artefact that answers wrongly defeats the check meant to catch it.

Running the wrong binary is re-running the claim. The check does not fail; it
does not fire, because from the inside it is indistinguishable from the check
passing. That is what makes it worse than stale prose, which at least still
looks like something to be verified.

One instance of a class collected in `a-check-that-answers-a-weaker-question.md`,
and the sharpest of them, because the artefact answering is the same kind of
thing as the artefact that should have answered.

Same shape as the Go build issuing false greens from a `+dirty` tree: the thing
that looks like the tool is not the tool the tree describes.

## What catches it is disagreement, not measurement

The wrench session hit the release binary first, because it was the obvious one
to reach for, and only tried `target/debug` because the wording did not match
what had been quoted at them.

So the mechanism was measuring **and having something to disagree with**. Either
alone fails here: a message with no measurement is one unverified claim, and a
measurement with no second source returns whatever the artefact says and reports
it as fact.

That is the converse of the estate's rule that neither your own search nor an
agreeing peer is a check. A disagreeing peer was the whole mechanism, and the
disagreement was visible only because both sides quoted the exact output instead
of summarising it.

So quote the bytes. "The refusal names the field" and "task child carries the
retired jig field" are the same claim, and only the second can be noticed to
differ from what the other side is holding.

## What to do

One built binary, and the gate builds it. The gate line rebuilds and installs
before running, so what gates cannot lag what is committed. The release tree here
was built once by hand, referenced by nothing, and deleted rather than kept in
step.

Deleting it is not the fix. `cargo build --release` re-creates it and the next
hand-built copy goes stale the same way. What survives is the habit: before
quoting what bolt does, check which bolt you ran.

    ls -la --time-style=+%H:%M target/*/bolt
    git log -1 --format=%h%x20%cd --date=format:%H:%M

A binary older than `HEAD` describes an earlier tree, and that comparison is the
whole check.

`the-installed-binary-gates-everything.md` is this hazard once the estate is
downstream of one copy, where no second reading exists to disagree.

## For a sibling measuring against this repository

Use `target/debug/bolt` and run `cargo build` first. It costs under a second on a
warm tree and removes the question.
