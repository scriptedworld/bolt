# Requirements are a directory

Requirements belong in one file per category under
`docs/REQUIREMENTS/<category>/`. The single-file format is retired rather than
kept beside the directory format.

The traceability checker reads either shape, so nothing in the tooling holds the
migration up. What is unresolved is where a retired id is recorded once the
single file is gone. A retired row cannot share a file with live ones, because a
checker reading the tree parses every file it finds, and an id that is both live
and retired is the collision the never-reuse rule exists to catch.

Bolt carries 63 retired rows, so that has to be answered before the split rather
than after. The checker already supports one answer, a document whose filename
ends `.retired`, which is a candidate and not a ruling. `NEXT_STEPS.md` tracks
the question.
