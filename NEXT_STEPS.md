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

1. Are the schemas shipped as files a jig author can point an editor at? A YAML
   language server given a JSON Schema gives completion and inline errors while
   the jig is being written, which is where a `matching` typo is cheapest to
   find. It also decides whether the schemas are an artifact or an
   implementation detail.
2. Which codecs does the shared project ship? YAML is what the ecosystem
   writes, so a YAML reader and writer are needed and nothing else is, until an
   adapter wants to read a tool's own output through the same validated path.
   Cobertura is XML and pytest emits JSON, and those are exactly the files an
   adapter parses today by hand.
3. Where does bolt look for `bolt.<name>.yaml`? The run's directory is the
   obvious place, and `link-jigs` puts shared jigs somewhere a bare name can
   resolve. Whether a nested jig resolves against its own base or against the
   outermost directory decides whether a subproject can carry a jig of its own.
4. May an invocation name more than one jig? An earlier answer said bolt is
   given a list of them; a jig now runs on a directory, which reads as one jig
   and one place. If several, they share a directory and their results fold
   into one, which is the merge already specified.
5. Must task names be unique within a jig? FR-3.3 makes the name the work
   directory prefix and FR-5.12 has nine subprojects as nine jig tasks, so a
   duplicate would put two tasks' executions in one place.
6. Is there any way to express one execution per path where the command does
   not take the path as an argument? Nothing needs it yet, and inference makes
   it inexpressible, which is worth knowing before it turns up rather than
   after.
7. Can a task be marked required or not, and what is the default? §22's gate is
   "all required constituent results must pass", and FR-8.3 repeats it.

## Locations

8. What is a command's working directory: the project root, the run's base, or
   its work directory? Separate location variables do not answer this, because
   `go vet ./...` cares where it is standing and not which path it was handed.
9. What are the three locations called? `{project_root}` currently means the
   run's base, so splitting them needs a name for each; `{project_root}`,
   `{base_dir}` and `{work_dir}` is one spelling.
10. What is the project-directory argument called?

## Input paths

11. Is the walk order deterministic, sorted rather than whatever the filesystem
    returns? FR-9.4 claims two runs over the same tree produce identical work
    directory names, and that claim rests on this.
12. Can a task reach a file `.gitignore` excludes? `matching` narrows what the
    walk found, and the walk has already dropped ignored files, so there is
    currently no way back. Generated code somebody does want checked is the
    case, and it may not be worth an escape hatch until it turns up.
13. Does the walk follow symlinks? Following one out of the directory breaks
    containment, and `link-jigs` leaves tracked symlinks pointing into toolbox,
    so a project using shared jigs has them sitting in the tree being walked.
14. Does a missing-directory refusal take the same shape as a depth refusal,
    writing a `result.yaml` with `success: false` and a reason, then exiting
    non-zero?
15. What does a run report when the directory holds nothing any task matches?
    Every task is skipped and the run reports `success: true` having checked
    nothing.

## Adapters

16. Where does an adapter write `output.yaml`? Stdout captured by bolt, its cwd
    set to the work directory, or an `--output` flag the default contract does
    not currently carry.
17. What does `--evidence` list: everything the command left in the work
    directory that bolt did not put there, or a set the task declares?
18. Which adapter runs when a task names none?
19. How is an adapter resolved by name: a search path, a directory inside
    toolbox, a symlink placed by `link-jigs`?
20. What happens when an adapter exits non-zero, writes nothing, or writes
    something that will not parse? All three are FR-7.6's no-authoritative-
    result and may deserve different reasons.

## The envelope

21. Is `message` a required key on a reason object, or merely conventional?
    Something guaranteed makes every reason renderable by one consumer while
    leaving the rest of the object open.
22. In the merged mapping, is `args` the argv as executed, or the jig-declared
    arguments before substitution?
23. Does an envelope name its own task, or does the merge take the name from
    the work directory?

## The output directory

24. What does `####` count: execution index within the task, or position in the
    run's order?
25. Does a task that executes exactly once still carry an ordinal?
26. What is in `manifest` besides the command line: cwd, environment, timings,
    the input path?
27. What does bolt do when `--output-dir` already holds a previous run?
28. Is `--output-dir` created if absent? A graph node's `.ephemera/` may not
    exist yet.
29. Where is `.bolt-<iso8601>` rooted when no output directory is named? Bolt
    reads no git, which argues for the working directory.
30. What spelling of ISO 8601? The strict form carries colons, which are legal
    here and hostile to a Windows checkout. Local time with an offset, or UTC?

## Failure

31. When a task's command cannot start at all, because the binary is missing or
    is not executable, is that a failed task or a bolt failure? A missing tool
    means the gate did not run, and reporting it as a normal failure conceals
    that.
32. Does a failing task stop the run, or does a run always execute every task
    and decide at the merge?
33. If bolt fails partway, does it write a partial `result.yaml` or none?
    FR-7.5 argues for none, but a caller then cannot tell a crashed run from
    one that never started.
34. What exit statuses does bolt use, and what does each mean? Candidates: a
    jig that will not parse, an unknown adapter, an unwritable output
    directory, depth exceeded, a task that could not start, receiving a signal.
35. Is a task that could not execute recorded distinguishably from one that
    executed and failed?

## Nested jigs

36. Must a jig task's subdirectory exist? A written path in a shared jig can
    point at a subproject a repository does not have, and FR-5.15 already says
    a base with no input paths under it does not run. Is a missing directory
    the same as an empty one, or a jig error that refuses the run the way a
    missing input path does?
37. A jig task has no command to read a mode off, so it runs once against its
    base. Worth confirming there is no case wanting a jig run per path.
38. Subprocess or in-process? Subprocess gives the bookkeeping files for free,
    and serial execution removes the argument for in-process.

---

# Part two: recorded and deferred

## The envelope

39. Is `statistics.source` a literal key, or a placeholder for the source's
    name used as the key? And what makes it a list rather than one object?
40. Do a task's own `evidence` paths survive into `result.yaml`, or does a
    reader follow the mapping's result filepath to the task's own envelope?
41. May `metadata` carry adapter-specific keys beyond `statistics` and
    `evidence`?
42. Is there a schema version field, given `success` is the only guarantee?
    What does a consumer do with a version it does not recognise?
43. FR-1.5 validates everything read as data, and the consequence of failing
    differs by file. A jig that fails its schema presumably refuses the run. An
    envelope that fails is a failure under FR-7.6. Is a nested run's
    `result.yaml` failing validation the child's failure or the parent's?
44. Does `result.yaml` keep the envelope shape exactly, or carry keys a task
    envelope never has?

## Tasks

45. Are the jig errors FR-3.4b and FR-4.2 name caught when the jig is read, or
    when the task is reached? Reading catches every one of them before anything
    runs; reaching catches them after half a gate has executed.
46. Is the environment handed to a task command inherited wholesale, filtered
    to an allowlist, or declared per task? Whatever the rule, the depth
    variable has to survive it.
47. Can a task be disabled in a jig without deleting it, and does a disabled
    task appear in the result?
48. FR-1.5 validates a jig. Does an unrecognised key fail it or warn? Failing
    makes a jig written for a newer bolt unusable by an older one; warning lets
    a typo pass as an ignored field.

## Adapters

49. With an explicit adapter invocation, are the same substitutions available,
    and is the envelope still expected in the same place?
50. Does an adapter get its own timeout, separate from the task's?
51. May an adapter read the repository tree, or only the files it was handed?

## Time

Nothing in the architecture mentions time. A first implementation can run
without any of it and let tools finish.

52. Does a task have a timeout? Where is it declared, and what is the default?
53. When a task times out, is the outcome a failed task or a task that could
    not execute?
54. How is a timed-out child terminated, which signal and with what grace, and
    are its descendants killed with it?
55. Is there a whole-run timeout distinct from the per-task ones?

## The output directory

56. Is a task skipped for an empty selection recorded in `result.yaml`? Absence
    currently reads the same as never declared, so a green result can mean that
    nothing was checked.
57. Does a manifest record what the walk found, or only what the task matched?
    The difference shows when nothing matched: a task that was offered a
    hundred files and wanted none looks the same as one run against an empty
    tree.
58. `manifest` already means the read and write authorization scope in §53 to
    §55, and both kinds land in task evidence trees. Rename bolt's?
59. Who deletes run directories, and on what rule?

## Input paths

60. Does a run over a large tree exceed `ARG_MAX` when `{all_paths}` expands?
    Whole-project runs make this the ordinary case rather than an edge one.
    Does bolt chunk into several executions, or is it the jig author's problem?
61. §67's pre-commit overlay wants a gate over what changed, and a
    directory-only invocation cannot express that. Does the overlay run the
    whole project on every commit, or does something have to give?

## Nested jigs

62. Is there a shorthand for naming one jig at many bases? Nine Go subprojects
    is nine jig tasks each needing its own name, and a list form would say it
    once. Against that, a written-out task per instance is what makes the
    project jig readable as an inventory of what is in the tree.
63. Does FR-5.12's whole-jig override survive separate location variables? A
    jig that "needs the repository root" usually needs it for one path, a
    shared config or a header template, and a variable naming the root covers
    that per use without surrendering the base. What it does not cover is a
    tool that must be standing at the root, which is question 7. It also cuts
    against FR-5.10: the point of a base is that a nested project has its own
    root.
64. Is a cycle detected by name, or only by depth exhaustion? A jig stack in
    the environment would name the cycle; a counter can only report a number.
65. Can a jig be referenced by version, or is it always whatever is on disk?

## Bounds and guards

66. Is the ancestry cross-check worth building? It closes `unset BOLT_DEPTH`,
    which the counter alone cannot, at the cost of Linux-specific code and
    process-spawning tests, and only matters against a jig actively trying to
    get around the guard.
67. Is the per-user cap on live runs wanted? If so, what is the default, and is
    the default off?
68. When parallel execution arrives, is the bound a per-run budget that
    descends with nesting rather than a shared counter?

## Composition and overlay

69. When a jig invokes another, what does the child inherit: environment,
    timeouts, the required default?
70. Can a parent override a field of a child's task, and at what granularity:
    the whole task, or one field?
71. Can a parent disable a task a shared jig declares, and is the omission
    visible in the result?
72. Is there a user-level or machine-level layer above the repository's jig?
    §67 describes exactly that for pre-commit, a repository policy plus an
    independent personal one.
73. What is the precedence when the same key is set at more than one layer, and
    are collections merged or replaced?

## Boundaries with the rest of the ecosystem

74. Does the per-file coverage policy of §20 live in an adapter, in a toolbox
    analyzer an adapter then reads, or in bolt? Which repository owns it
    decides whether it is a bolt requirement at all.
75. Is `result.yaml` what a ratchet node depends on directly, or does a node
    wrap it in something of its own?
76. Does bolt stamp the tree state §65 wants, or does that move to the caller?
    Bolt reads no git for anything else.
77. What is the project called, and does the name stay `wrench`? Filed as a
    candidate in `clank/inbox/silo/yaml-and-json-schema-is-a-platform-decision/`
    along with the argument against it, that a wrench turns a bolt while here
    bolt depends on wrench.
78. Inside that project: one C core with bindings, or a library per language
    wrapping a stock JSON Schema validator? NFR-12.4 argues against C, because
    a Go binding means cgo, and cgo costs static linking and cross-compilation
    for the one thing bolt most wants to stay: a single file an image carries.
    JSON Schema also exists so independent implementations agree, so a C core
    solves by construction what the specification already solves. What per
    language costs is the wrapper around validation, canonical emission and
    error shape, which can drift between bindings, and a shared fixture suite
    every binding runs would hold them level.
79. Does bolt have an interface other than a command line? An importable
    library used by another Go component would change what the requirements
    cover.
