# What is still open

`REQUIREMENTS.md` covers what `silo/docs/ARCHITECTURE.md` supports plus what has
been answered against this file. This is what remains, written as questions
rather than as guesses.

Part one is what blocks a build: a question here has no defensible default, and
guessing it produces something that has to be unpicked later. Part two is
recorded and deferred. Every question in it is real, and none of them stops an
implementation starting; leaving one unanswered costs a gap in the requirements,
not a wrong decision in the code.

Each row in section 13 of `REQUIREMENTS.md` stays `[?]` until the questions
behind it are closed, wherever they sit here.

Where a question offers candidate answers, they are candidates and not a
recommendation.

---

# Part one: blocks a build

## The jig, and reaching it

1. What format is a jig written in? FR-3.4c narrows it: the format has to carry
   comments, since that is where an entry's reasoning lives. JSON is out.
2. How is a jig named on the command line: a path, or a bare name resolved
   against a search path? `link-jigs` gives a bare name somewhere to resolve.
3. May an invocation name more than one jig? An earlier answer said bolt is
   given a list of them; a jig now runs on a directory, which reads as one jig
   and one place. If several, they share a directory and their results fold
   into one, which is the merge already specified.
4. `perPath` is settled. Are the other two `once` and `batch`?
5. What is a task's runmode when it declares none? `once` is the safe default
   because it needs no path variables, and inferring it from which variable the
   command names would make the field redundant rather than optional.
6. Can a task be marked required or not, and what is the default? §22's gate is
   "all required constituent results must pass", and FR-8.3 repeats it.

## Locations

7. What is a command's working directory: the project root, the run's base, or
   its work directory? Separate location variables do not answer this, because
   `go vet ./...` cares where it is standing and not which path it was handed.
8. What are the three locations called? `{project_root}` currently means the
   run's base, so splitting them needs a name for each; `{project_root}`,
   `{base_dir}` and `{work_dir}` is one spelling.
9. What is the project-directory argument called?

## Input paths

10. Is the walk order deterministic, sorted rather than whatever the filesystem
    returns? FR-9.4 claims two runs over the same tree produce identical work
    directory names, and that claim rests on this.
11. Can a task reach a file `.gitignore` excludes? `matching:` narrows what the
    walk found, and the walk has already dropped ignored files, so there is
    currently no way back. Generated code somebody does want checked is the
    case, and it may not be worth an escape hatch until it turns up.
12. Does the walk follow symlinks? Following one out of the directory breaks
    containment, and `link-jigs` leaves tracked symlinks pointing into toolbox,
    so a project using shared jigs has them sitting in the tree being walked.
13. Does a missing-directory refusal take the same shape as a depth refusal,
    writing a `result.yaml` with `success: false` and a reason, then exiting
    non-zero?
14. What does a run report when the directory holds nothing any task matches?
    Every task is skipped and the run reports `success: true` having checked
    nothing.

## Adapters

15. Where does an adapter write `output.yaml`? Stdout captured by bolt, its cwd
    set to the work directory, or an `--output` flag the default contract does
    not currently carry.
16. What does `--evidence` list: everything the command left in the work
    directory that bolt did not put there, or a set the task declares?
17. Which adapter runs when a task names none?
18. How is an adapter resolved by name: a search path, a directory inside
    toolbox, a symlink placed by `link-jigs`?
19. What happens when an adapter exits non-zero, writes nothing, or writes
    something that will not parse? All three are FR-7.6's no-authoritative-
    result and may deserve different reasons.

## The envelope

20. Is `message` a required key on a reason object, or merely conventional?
    Something guaranteed makes every reason renderable by one consumer while
    leaving the rest of the object open.
21. In the merged mapping, is `args` the argv as executed, or the jig-declared
    arguments before substitution?
22. Does an envelope name its own task, or does the merge take the name from
    the work directory?

## The output directory

23. What does `####` count: execution index within the task, or position in the
    run's order?
24. Does a task that executes exactly once still carry an ordinal?
25. What is in `manifest` besides the command line: cwd, environment, timings,
    the input path?
26. What does bolt do when `--output-dir` already holds a previous run?
27. Is `--output-dir` created if absent? A graph node's `.ephemera/` may not
    exist yet.
28. Where is `.bolt-<iso8601>` rooted when no output directory is named? Bolt
    reads no git, which argues for the working directory.
29. What spelling of ISO 8601? The strict form carries colons, which are legal
    here and hostile to a Windows checkout. Local time with an offset, or UTC?

## Failure

30. When a task's command cannot start at all, because the binary is missing or
    is not executable, is that a failed task or a bolt failure? A missing tool
    means the gate did not run, and reporting it as a normal failure conceals
    that.
31. Does a failing task stop the run, or does a run always execute every task
    and decide at the merge?
32. If bolt fails partway, does it write a partial `result.yaml` or none?
    FR-7.5 argues for none, but a caller then cannot tell a crashed run from
    one that never started.
33. What exit statuses does bolt use, and what does each mean? Candidates: a
    jig that will not parse, an unknown adapter, an unwritable output
    directory, depth exceeded, a task that could not start, receiving a signal.
34. Is a task that could not execute recorded distinguishably from one that
    executed and failed?

## Nested jigs

35. Must a jig task's subdirectory exist? A written path in a shared jig can
    point at a subproject a repository does not have, and FR-5.15 already says
    a base with no input paths under it does not run. Is a missing directory
    the same as an empty one, or a jig error that refuses the run the way a
    missing input path does?
36. Does a jig task carry a runmode, or is it always handed its base once?
37. Subprocess or in-process? Subprocess gives the bookkeeping files for free,
    and serial execution removes the argument for in-process.

---

# Part two: recorded and deferred

## The envelope

38. Is `statistics.source` a literal key, or a placeholder for the source's
    name used as the key? And what makes it a list rather than one object?
39. Do a task's own `evidence` paths survive into `result.yaml`, or does a
    reader follow the mapping's result filepath to the task's own envelope?
40. May `metadata` carry adapter-specific keys beyond `statistics` and
    `evidence`?
41. Is there a schema version field, given `success` is the only guarantee?
    What does a consumer do with a version it does not recognise?
42. FR-1.5 validates everything read as data, and the consequence of failing
    differs by file. A jig that fails its schema presumably refuses the run. An
    envelope that fails is a failure under FR-7.6. Is a nested run's
    `result.yaml` failing validation the child's failure or the parent's?
43. Does `result.yaml` keep the envelope shape exactly, or carry keys a task
    envelope never has?

## Tasks

44. Is naming `{input_paths}` in a `perPath` task a jig error caught at parse,
    or a runtime failure?
45. Is the environment handed to a task command inherited wholesale, filtered
    to an allowlist, or declared per task? Whatever the rule, the depth
    variable has to survive it.
46. Can a task be disabled in a jig without deleting it, and does a disabled
    task appear in the result?
47. FR-1.5 validates a jig. Does an unrecognised key fail it or warn? Failing
    makes a jig written for a newer bolt unusable by an older one; warning lets
    a typo pass as an ignored field.

## Adapters

48. With an explicit adapter invocation, are the same substitutions available,
    and is the envelope still expected in the same place?
49. Does an adapter get its own timeout, separate from the task's?
50. May an adapter read the repository tree, or only the files it was handed?

## Time

Nothing in the architecture mentions time. A first implementation can run
without any of it and let tools finish.

51. Does a task have a timeout? Where is it declared, and what is the default?
52. When a task times out, is the outcome a failed task or a task that could
    not execute?
53. How is a timed-out child terminated, which signal and with what grace, and
    are its descendants killed with it?
54. Is there a whole-run timeout distinct from the per-task ones?

## The output directory

55. Is a task skipped for an empty selection recorded in `result.yaml`? Absence
    currently reads the same as never declared, so a green result can mean that
    nothing was checked.
56. Does a manifest record what the walk found, or only what the task matched?
    The difference shows when nothing matched: a task that was offered a
    hundred files and wanted none looks the same as one run against an empty
    tree.
57. `manifest` already means the read and write authorization scope in §53 to
    §55, and both kinds land in task evidence trees. Rename bolt's?
58. Who deletes run directories, and on what rule?

## Input paths

59. Does a run over a large tree exceed `ARG_MAX` when `{input_paths}` expands?
    Whole-project runs make this the ordinary case rather than an edge one.
    Does bolt chunk into several executions, or is it the jig author's problem?
60. §67's pre-commit overlay wants a gate over what changed, and a
    directory-only invocation cannot express that. Does the overlay run the
    whole project on every commit, or does something have to give?

## Nested jigs

61. Is there a shorthand for naming one jig at many bases? Nine Go subprojects
    is nine jig tasks each needing its own name, and a list form would say it
    once. Against that, a written-out task per instance is what makes the
    project jig readable as an inventory of what is in the tree.
62. Does FR-5.12's whole-jig override survive separate location variables? A
    jig that "needs the repository root" usually needs it for one path, a
    shared config or a header template, and a variable naming the root covers
    that per use without surrendering the base. What it does not cover is a
    tool that must be standing at the root, which is question 7. It also cuts
    against FR-5.10: the point of a base is that a nested project has its own
    root.
63. Is a cycle detected by name, or only by depth exhaustion? A jig stack in
    the environment would name the cycle; a counter can only report a number.
64. Can a jig be referenced by version, or is it always whatever is on disk?

## Bounds and guards

65. Is the ancestry cross-check worth building? It closes `unset BOLT_DEPTH`,
    which the counter alone cannot, at the cost of Linux-specific code and
    process-spawning tests, and only matters against a jig actively trying to
    get around the guard.
66. Is the per-user cap on live runs wanted? If so, what is the default, and is
    the default off?
67. When parallel execution arrives, is the bound a per-run budget that
    descends with nesting rather than a shared counter?

## Composition and overlay

68. When a jig invokes another, what does the child inherit: environment,
    timeouts, the required default?
69. Can a parent override a field of a child's task, and at what granularity:
    the whole task, or one field?
70. Can a parent disable a task a shared jig declares, and is the omission
    visible in the result?
71. Is there a user-level or machine-level layer above the repository's jig?
    §67 describes exactly that for pre-commit, a repository policy plus an
    independent personal one.
72. What is the precedence when the same key is set at more than one layer, and
    are collections merged or replaced?

## Boundaries with the rest of the ecosystem

73. Does the per-file coverage policy of §20 live in an adapter, in a toolbox
    analyzer an adapter then reads, or in bolt? Which repository owns it
    decides whether it is a bolt requirement at all.
74. Is `result.yaml` what a ratchet node depends on directly, or does a node
    wrap it in something of its own?
75. Does bolt stamp the tree state §65 wants, or does that move to the caller?
    Bolt reads no git for anything else.
76. Who owns the envelope schema, given §3.1 makes it ecosystem-wide: bolt,
    toolbox, or a standalone definition none of them owns? This one has moved
    into blocking territory: FR-1.5 validates everything read as data, and
    there is nothing to validate against until it is answered.
77. Is there a published validator for FR-7.7's bar that another producer can
    run against its own output? Toolbox is the obvious home, and without one
    every producer discovers its mistakes through somebody else's merge.
78. Does bolt have an interface other than a command line? An importable
    library used by another Go component would change what the requirements
    cover.
