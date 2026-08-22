# What is still open

`REQUIREMENTS.md` covers what `silo/docs/ARCHITECTURE.md` supports plus what has
been answered against this file. This is what remains, written as questions
rather than as guesses.

The order does not matter. Completeness does: each row in section 13 of
`REQUIREMENTS.md` stays `[?]` until the questions behind it are closed.

Where a question offers candidate answers, they are candidates and not a
recommendation.

---

## The envelope

1. Is `message` a required key on a reason object, or merely conventional?
   Something guaranteed makes every reason renderable by one consumer while
   leaving the rest of the object open.
2. Is `statistics.source` a literal key, or a placeholder for the source's name
   used as the key? And what makes it a list rather than one object?
3. In the merged mapping, is `args` the argv as executed, or the jig-declared
   arguments before substitution?
4. Do a task's own `evidence` paths survive into `result.yaml`, or does a reader
   follow the mapping's result filepath to the task's own envelope?
5. Does an envelope name its own task, or does the merge take the name from the
   work directory?
6. May `metadata` carry adapter-specific keys beyond `statistics` and
   `evidence`?
7. Is there a schema version field, given `success` is the only guarantee? What
   does a consumer do with a version it does not recognise?
8. Is YAML the only serialisation, or is JSON accepted or emitted?
9. Does `result.yaml` keep the envelope shape exactly, or carry keys a task
   envelope never has?

## Tasks and the jig

10. `perPath` is settled. Are the other two `once` and `batch`?
11. Is `matching:` one glob or a list, and is there an exclude counterpart?
12. Is naming `{input_paths}` in a `perPath` task a jig error caught at parse, or
    a runtime failure?
13. What is the command's working directory: the project root, the run's base,
    or the work directory? This is the one thing separate location variables do
    not solve, because a tool like `go vet ./...` cares where it is standing
    and not which path it was handed.
    And what are the three called? `{project_root}` currently means the run's
    base, so splitting them needs a name for each; `{project_root}`,
    `{base_dir}` and `{work_dir}` is one spelling.
14. Is the environment handed to a task command inherited wholesale, filtered to
    an allowlist, or declared per task? Whatever the rule, the depth variable
    has to survive it.
15. Can a task be marked required or not, and what is the default? §22's gate is
    "all required constituent results must pass".
16. Can a task be disabled in a jig without deleting it, and does a disabled
    task appear in the result?
17. Is a jig validated against a schema, and does an unrecognised key fail the
    run or warn?
18. What format is a jig written in?
19. How is a jig named on the command line: a path, or a bare name resolved
    against a search path? `link-jigs` gives a bare name somewhere to resolve.
20. How does bolt know where the jig list ends and the file list begins?

## Adapters

21. Where does an adapter write `output.yaml`? Stdout captured by bolt, its cwd
    set to the work directory, or an `--output` flag the default contract does
    not currently carry.
22. What does `--evidence` list: everything the command left in the work
    directory that bolt did not put there, or a set the task declares?
23. With an explicit adapter invocation, are the same substitutions available,
    and is the envelope still expected in the same place?
24. Which adapter runs when a task names none?
25. What happens when an adapter exits non-zero, writes nothing, or writes
    something that will not parse? All three are FR-7.6's no-authoritative-
    result and may deserve different reasons.
26. Does an adapter get its own timeout, separate from the task's?
27. How is an adapter resolved by name: a search path, a directory inside
    toolbox, a symlink placed by `link-jigs`?
28. May an adapter read the repository tree, or only the files it was handed?

## Failure and time

Nothing in the architecture mentions time. All of this is unstated.

29. Does a task have a timeout? Where is it declared, and what is the default?
30. When a task times out, is the outcome a failed task or a task that could not
    execute?
31. How is a timed-out child terminated, which signal and with what grace, and
    are its descendants killed with it?
32. Is there a whole-run timeout distinct from the per-task ones?
33. When a task's command cannot start at all, because the binary is missing or
    is not executable, is that a failed task or a bolt failure? A missing tool
    means the gate did not run, and reporting it as a normal failure conceals
    that.
34. Does a failing task stop the run, or does a run always execute every task
    and decide at the merge?
35. If bolt fails partway, does it write a partial `result.yaml` or none? FR-7.5
    argues for none, but a caller then cannot tell a crashed run from one that
    never started.
36. What exit statuses does bolt use, and what does each mean? Candidates: a jig
    that will not parse, an unknown adapter, an unwritable output directory,
    depth exceeded, a task that could not start, receiving a signal.
37. Is a task skipped for an empty file list recorded in `result.yaml`? Absence
    currently reads the same as never declared, so a green result can mean that
    nothing was checked.
38. Is a task that could not execute recorded distinguishably from one that
    executed and failed?

## The output directory

39. What does `####` count: execution index within the task, or position in the
    run's order?
40. Does a task that executes exactly once still carry an ordinal?
41. Does the envelope or the manifest record which input file that execution was
    handed? Without it, `shellcheck-0007` traces back to a file only by
    recomputing the filter and the ordering.
42. What is in `manifest` besides the command line: cwd, environment, timings,
    the input file?
43. `manifest` already means the read and write authorization scope in §53 to
    §55, and both kinds land in task evidence trees. Rename bolt's?
44. What does bolt do when `--output-dir` already holds a previous run?
45. Is `--output-dir` created if absent? A graph node's `.ephemera/` may not
    exist yet.
46. Where is `.bolt-<iso8601>` rooted when no output directory is named? Bolt
    reads no git, which argues for the working directory.
47. What spelling of ISO 8601? The strict form carries colons, which are legal
    here and hostile to a Windows checkout. Local time with an offset, or UTC?
48. Who deletes run directories, and on what rule?

## The input file list

49. Does resolving to absolute follow symlinks? Lexical resolution leaves a
    symlink inside the project pointing at `/etc/shadow` passing the
    containment check. Following links closes that and breaks a legitimate
    call: `link-jigs` leaves tracked symlinks whose targets are in toolbox, so
    `bolt go-quality $(git ls-files)` in a project using shared jigs would
    refuse. Follow links and let callers filter, judge containment by the
    link's own path, or follow links and treat an outward-pointing symlink as
    excluded rather than fatal.
50. Globbing through a folder walks it, and a walk finds `.git`, `node_modules`,
    `.venv` and build output. Bolt has no gitignore awareness, so `bolt py-jig
    .` currently means checking every `.py` under `.venv`. Does bolt carry a
    built-in exclusion set, read the repository's ignore rules after all, take
    exclusions from the jig, or leave it to callers never to name a folder that
    contains any of that, which is most folders?
    Separately: when a folder globs through to several files, does a `perPath`
    task execute once for the folder or once per matched file? And does
    `{input_paths}` in a batch task carry the folder or the files under it?
51. Does bolt dedupe the list and preserve the caller's order? Order decides the
    per-path ordinal, so a caller reordering its `git diff` output renumbers
    every work directory.
52. Does a refusal take the same shape as a depth refusal, writing a
    `result.yaml` with `success: false` and a reason naming the offending
    paths, then exiting non-zero? That is the rule already set once, and only a
    bolt that dies leaves nothing behind.
53. `{input_paths}` over a large list will exceed `ARG_MAX`. Does bolt chunk
    into several executions, or is that the jig author's problem?
54. What does an entirely empty file list mean for a jig whose tasks all consume
    files? Every task is skipped and the run reports `success: true` having
    checked nothing.

## Nested jigs

55. Is a jig task's subdirectory required to be a direct child of the current
    base, or may it name a deeper path like `services/api/`? "A subdir of your
    dir" reads as either.
56. Does FR-5.12's whole-jig override survive separate location variables? A jig
    that "needs the repository root" usually needs it for one path, a shared
    config or a header template, and a variable naming the root covers that
    per use without surrendering the base. What it does not cover is a tool
    that must be standing at the root, which is question 13. If the override is
    only for that, it is about the working directory and not about the base.
    It also cuts against FR-5.10: the point of a base is that a nested project
    has its own root, and a jig reaching past it is working against that.
57. May the same jig appear as two jig tasks with different bases, `go-quality`
    on `backend` and again on `tools`? Two nested Go modules in one repository
    is the ordinary case, so presumably yes. FR-3.3 makes the task name the
    work directory prefix, so each needs a distinct name, and it is worth
    knowing whether that name is the jig's or the task's.
58. Does a jig task carry a runmode, or is it always handed its filtered list
    once?
59. Subprocess or in-process? Subprocess gives the five bookkeeping files for
    free, and serial execution removes the argument for in-process.
60. Is a cycle detected by name, or only by depth exhaustion? A jig stack in the
    environment would name the cycle; a counter can only report a number.
61. Can a jig be referenced by version, or is it always whatever is on disk?

## Bounds and guards

62. Is the ancestry cross-check worth building? It closes `unset BOLT_DEPTH`,
    which the counter alone cannot, at the cost of Linux-specific code and
    process-spawning tests, and only matters against a jig actively trying to
    get around the guard.
63. Is the per-user cap on live runs wanted? If so, what is the default, and is
    the default off?
64. When parallel execution arrives, is the bound a per-run budget that descends
    with nesting rather than a shared counter?

## Composition and overlay

65. When a jig invokes another, what does the child inherit: environment,
    timeouts, the required default?
66. Can a parent override a field of a child's task, and at what granularity:
    the whole task, or one field?
67. Can a parent disable a task a shared jig declares, and is the omission
    visible in the result?
68. Is there a user-level or machine-level layer above the repository's jig?
    §67 describes exactly that for pre-commit, a repository policy plus an
    independent personal one.
69. What is the precedence when the same key is set at more than one layer, and
    are collections merged or replaced?

## Boundaries with the rest of the ecosystem

70. Does the per-file coverage policy of §20 live in an adapter, in a toolbox
    analyzer an adapter then reads, or in bolt? Which repository owns it decides
    whether it is a bolt requirement at all.
71. Is `result.yaml` what a ratchet node depends on directly, or does a node
    wrap it in something of its own?
72. Does bolt stamp the tree state §65 wants, or does that move to the caller?
    Bolt reads no git for anything else.
73. Who owns the envelope schema, given §3.1 makes it ecosystem-wide: bolt,
    toolbox, or a standalone definition none of them owns?
74. What is the minimum a component outside bolt must do to be a valid envelope
    producer?
75. Does bolt have an interface other than a command line? An importable library
    used by another Go component would change what the requirements cover.
