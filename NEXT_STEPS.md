# What is still open

`REQUIREMENTS.md` covers what `silo/docs/ARCHITECTURE.md` supports plus what has
been answered against this file. This is what remains, written as questions
rather than as guesses.

**Nothing here blocks a build.** Part one held 35 questions that did. Each is
now a row in `REQUIREMENTS.md`, listed under "Defaults taken" below, and every
one of those is reversible by editing the row it became.

What is left is real and none of it stops an implementation starting. Leaving a
question unanswered costs a gap in the requirements, not a wrong decision in the
code.

Where a question offers candidate answers, they are candidates and not a
recommendation.

---

# Defaults taken

Taken 2026-08-26 rather than asked, because each has a defensible answer and
asking all of them costs a session that could have been spent writing code.
Every one is a `[D]` row, so a wrong default is one edit away from being right.

Four were answered rather than defaulted: one jig per invocation, `requires`
checked both up front and at run time, underscored variables against hyphenated
flags, and the merge reading task and args off disk.

| Was | Now | What was decided |
|---|---|---|
| Multiple jigs per invocation | FR-2.1a | One jig, one directory. Several at once is a jig whose tasks are nested jigs, already specified. |
| Walk order | FR-2.2d | Sorted, which is what FR-9.4's identical directory names rest on. |
| Symlinks | FR-2.2e | Not followed. |
| Reaching a `.gitignore`d file | FR-2.2f | No escape hatch until something needs one. |
| Missing-directory refusal | FR-2.5a | Same shape as every refusal: a result, a reason, a non-zero exit. |
| `--output-dir` absent | FR-2.6a | Created, parents included. |
| `--output-dir` populated | FR-2.6b | Refused. |
| Where `.bolt-<iso8601>` sits | FR-2.6c | At the run's base. |
| Timestamp spelling | FR-2.6d | Filesystem-safe, local offset. |
| Duplicate task names | FR-3.3a | A jig error. |
| `requires` checked when | FR-3.10b, FR-3.10c | Up front, and FR-4.10 still covers a command that cannot start for another reason. |
| A project jig's extra tooling | FR-3.10d | FR-3.10b catches it at the start of the run. Installing it is not bolt's. |
| Naming the five locations | FR-4.1c, FR-4.1d | All five exposed. Variables underscored, flags hyphenated, as a rule. |
| Per-path without a path variable | FR-4.2a | Not expressible. |
| Subprocess or in-process | FR-4.15 | Subprocess. |
| A jig task's missing subdirectory | FR-5.15a | Same as an empty one: it does not run, the run carries on. |
| A jig task per path | FR-5.16 | Once against its base. |
| Adapter when a task names none | FR-6.9 | The generic exit-code adapter. |
| Resolving an adapter by name | FR-6.10 | From the config directory, where jigs already come from. |
| A broken adapter | FR-6.11 | Bolt writes the envelope, and the reason says which of the three happened. |
| Canonical form on `output.yaml` | FR-6.12, FR-6.13 | The adapter's, free from wrench. No reparse-and-compare check. |
| A declared evidence file missing | FR-6.14 | Fails its task, reason naming the path. |
| `message` on a reason | FR-7.8 | Required. |
| Telling a missing tool from a finding | FR-7.9, FR-7.10 | A reason carries `kind`. |
| `args` in the merged mapping | FR-8.8 | The argv as executed. |
| The run's base in the result | FR-8.9 | Recorded. |
| A per-path manifest's path list | FR-9.8 | The whole matched list. |
| Ordinal on a single execution | FR-9.9 | Always carried. |
| Exit statuses | FR-10.5, FR-10.6 | 0 ran, 1 could not run, 128+n on a signal. |
| Partial result on failure | FR-10.7 | Written whenever bolt is alive to write it. |

Two questions left for wrench, which now states them itself: which codecs and
readers ship, and whether the schemas are files an editor can be pointed at.
Both are `wrench/NEXT_STEPS.md`.

Four rows left section 13 settled by the body: FR-13.1 by FR-6.2b, FR-13.2 by
FR-7.9 and FR-7.10, FR-13.5 by FR-4.12, FR-13.9 by FR-3.4d, FR-1.5 and FR-3.12.

---

# Recorded and deferred

## The envelope

1. Is `statistics.source` a literal key, or a placeholder for the source's
   name used as the key? And what makes it a list rather than one object?
2. Do a task's own `evidence` paths survive into `result.yaml`, or does a
   reader follow the mapping's result filepath to the task's own envelope?
3. May `metadata` carry adapter-specific keys beyond `statistics` and
   `evidence`?
4. Is there a schema version field, given `success` is the only guarantee?
   What does a consumer do with a version it does not recognise?
5. FR-1.5 validates everything read as data, and the consequence of failing
   differs by file. A jig that fails its schema refuses the run. An envelope
   that fails is a failure under FR-7.6. Is a nested run's `result.yaml`
   failing validation the child's failure or the parent's?
6. Does `result.yaml` keep the envelope shape exactly, or carry keys a task
   envelope never has? FR-8.9 puts the run's base in it, which is one such key.

## Tasks

7. Are the jig errors FR-3.4b and FR-4.2 name caught when the jig is read, or
   when the task is reached? Reading catches every one of them before anything
   runs; reaching catches them after half a gate has executed. FR-3.10b now
   checks `requires` up front, which argues for reading.
8. Is the environment handed to a task command inherited wholesale, filtered
   to an allowlist, or declared per task? Whatever the rule, the depth
   variable has to survive it.
9. Can a task be disabled in a jig without deleting it, and does a disabled
   task appear in the result?
10. FR-1.5 validates a jig. Does an unrecognised key fail it or warn? Failing
    makes a jig written for a newer bolt unusable by an older one; warning lets
    a typo pass as an ignored field.

## Adapters

11. May an adapter read the repository tree, or only the files it was handed?

## Time

Nothing in the architecture mentions time, and what is here now comes from
answers rather than from it.

12. Where is each limit declared? A task's belongs on the task; a run's could
    sit on the jig, on the command line, or both, and if both then which wins.
13. How is a timed-out child terminated, which signal and with what grace, and
    are its descendants killed with it? A command that spawns its own children
    leaves them running when only the child is signalled, and they go on
    writing into a work directory bolt has finished with, and into the streams
    an adapter is about to read under FR-4.12a.
14. What records the executions a task never reached? A per-path task cut off
    at path fifty leaves fifty work directories and nothing saying the other
    three hundred and fifty were never attempted. The run fails, correctly, and
    a reader still cannot see how much went unchecked.
15. Does a run that times out fold in the constituents that completed, or does
    its result carry only the timeout? FR-4.14 keeps the evidence; whether the
    merge runs over what is there is the part that is not settled.

## The output directory

16. Is a task skipped for an empty selection recorded in `result.yaml`?
    FR-8.3a closes the case where every task skips, since a merge finding no
    constituent fails. Open is the partial case: four tasks green and a fifth
    skipped reads the same as four tasks and no fifth.
17. Does a manifest record what the walk found, or only what the task matched?
    FR-9.8 settles that it holds the whole matched list rather than one path.
    What it does not settle is whether the walk's own findings are in there
    too, which is what separates a task offered a hundred files and wanting
    none from one run against an empty tree.
18. `manifest` already means the read and write authorization scope in §53 to
    §55, and both kinds land in task evidence trees. Rename bolt's?
19. Who deletes run directories, and when? The lifetime is settled by FR-9.1a:
    wanted while the result is being reviewed, not afterwards. Open is whether
    bolt prunes its own, whether whoever named `--output-dir` owns removing it,
    and what a graph node's `.ephemera/` does about it.

## Input paths

20. Does a run over a large tree exceed `ARG_MAX` when `{all_paths}` expands?
    Whole-project runs make this the ordinary case rather than an edge one.
    Does bolt chunk into several executions, or is it the jig author's problem?
21. §67's pre-commit overlay wants a gate over what changed, and a
    directory-only invocation cannot express that. Does the overlay run the
    whole project on every commit, or does something have to give?

## Nested jigs

22. Is there a shorthand for naming one jig at many bases? Nine Go subprojects
    is nine jig tasks each needing its own name, and a list form would say it
    once. Against that, a written-out task per instance is what makes the
    project jig readable as an inventory of what is in the tree.
23. Does FR-5.12's whole-jig override survive separate location variables? A
    jig that "needs the repository root" usually needs it for one path, a
    shared config or a header template, and `{project_root}` covers that per
    use without surrendering the base. What it does not cover is a tool that
    must be standing at the root. It also cuts against FR-5.10: the point of a
    base is that a nested project has its own root.
24. Is a cycle detected by name, or only by depth exhaustion? A jig stack in
    the environment would name the cycle; a counter can only report a number.
25. Can a jig be referenced by version, or is it always whatever is on disk?

## Bounds and guards

26. Is the ancestry cross-check worth building? It closes `unset BOLT_DEPTH`,
    which the counter alone cannot, at the cost of Linux-specific code and
    process-spawning tests, and only matters against a jig actively trying to
    get around the guard.
27. Is the per-user cap on live runs wanted? If so, what is the default, and is
    the default off?
28. When parallel execution arrives, is the bound a per-run budget that
    descends with nesting rather than a shared counter?

## Composition and overlay

**A proposal, 2026-08-26, which may replace most of what follows.** Not a
settled row, recorded here so the questions below are read against it rather
than answered separately.

**A jig carries placeholders and a definitions file supplies the values.** A
subdirectory then runs the *same* shared jig with its own values, instead of
the jig being copied, overlaid, or merged by task id.

    bolt.common-quality.yaml       ... --requirements {requirements} ...
    <base>/bolt.definitions.yaml   requirements: ../REQUIREMENTS.md

**What it is instead of.** The retired bolt merged definitions by task id and
took the last writer, which is what `bolt -c common -c go` meant and what
toolbox's shared jigs are still written for. Questions 30 to 33 are all that
mechanism's edges: what a parent may override, at what granularity, whether it
may disable a task, and what wins when two layers set the same key. **A
parameterised jig has none of those edges**, because nothing merges: there is
one jig and a set of values, and a value either has a definition or it does not.

**What it answers immediately.** `common-quality` runs traceability against
`REQUIREMENTS.md` relative to the run root. wrench keeps one requirements
document at its root and wants that jig run at `go/` and at `python/`, where no
such file sits. With a definitions file per base, each says where its
requirements are, and one document serves both without the checker changing.
`clank/tasks/wrench/gate/10-a-composite-jig.planning` is the case in full.

**What it leaves open.** Where the file sits and what it is called. Whether it
is found by walking up from the base or named on the jig task. What happens to
a placeholder no definition supplies, which is either a jig error before
anything runs, like FR-3.10b's `requires`, or an empty substitution, which is
the reading that fails silently. Whether a definitions file may itself carry
substitutions, which is where this stops being simple.

**It is not free.** FR-4.2 reads how a task runs off its command line, and
FR-9.5c records every value bolt exposed as a template variable. Both would have
to account for values that come from a file rather than from bolt, and the
manifest is the place a reader finds out what a placeholder stood for.


29. When a jig invokes another, what does the child inherit: environment,
    timeouts, the required default?
30. Can a parent override a field of a child's task, and at what granularity:
    the whole task, or one field?
31. Can a parent disable a task a shared jig declares, and is the omission
    visible in the result?
32. Is there a user-level or machine-level layer above the repository's jig?
    §67 describes exactly that for pre-commit, a repository policy plus an
    independent personal one.
33. What is the precedence when the same key is set at more than one layer, and
    are collections merged or replaced?

## Boundaries with the rest of the ecosystem

34. Does the per-file coverage policy of §20 live in an adapter, in a toolbox
    analyzer an adapter then reads, or in bolt? Which repository owns it
    decides whether it is a bolt requirement at all.
35. Is `result.yaml` what a ratchet node depends on directly, or does a node
    wrap it in something of its own?
36. Does bolt stamp the tree state §65 wants, or does that move to the caller?
    Bolt reads no git for anything else.
37. Does bolt have an interface other than a command line? An importable
    library used by another Go component would change what the requirements
    cover.

---

# Files here that belong to another repository

Not a question. Recorded here because bolt has no `docs/` tree yet, and a
session that does not know this will edit a file it cannot commit.

`bin/test-traceability.py` is a **symlink** to
`toolbox/bin/test-traceability.py`. That is how a project adopts a shared
checker: the file lands at the same relative path in the adopter, and
`{config_dir}` resolves it back through the link.

    bin/test-traceability.py -> ../../toolbox/bin/test-traceability.py

**Editing it from here edits toolbox**, and the change belongs to a repository
this one cannot commit to. It then sits in toolbox's working tree looking like
toolbox's own uncommitted work. Fix the checker in toolbox and it arrives here
through the link.

The same applies to anything `link-jigs` places later, which is the shared
jigs and every adapter they name. `toolbox/jigs.yaml` says which files each set
carries.

---

# Rejected from the inbox

Two entries described the retired implementation and were resolved by rejection
rather than by action, because the tree each names no longer exists and neither
can be verified against anything. Both concepts survive independently, reached
without reading either entry.

- **`gate-runs-a-stale-fork-of-the-checkers`**, filed from toolbox, on a jig
  running a forked copy of shared checkers. The drift it describes is covered
  by FR-3.11, where an anvil image is generated from a jig's `requires` rather
  than mirroring it, and by the jig checker filed as
  `clank/inbox/toolbox/a-checker-that-validates-every-jig-in-the-config-dir/`.
- **`plan-does-not-evaluate-requires`**, filed from agent-support, on a `plan`
  subcommand that did not check `requires`. There is no `plan` in this
  specification. Whether `requires` is checked before anything runs is
  settled by FR-3.10b, reached from the architecture rather than from the entry.
