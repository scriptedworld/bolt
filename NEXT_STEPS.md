# What the architecture does not settle

`REQUIREMENTS.md` covers what `silo/docs/ARCHITECTURE.md` actually supports.
This is everything else a requirements document needs and that the architecture
leaves open, written as questions rather than as guesses.

The order does not matter. Completeness does: each row in section 9 of
`REQUIREMENTS.md` stays `[?]` until the questions behind it are closed.

Where a question offers candidate answers, they are candidates and not a
recommendation.

---

## The envelope's field schema

The architecture calls the shared envelope one of the primary contracts of the
system and never states a field of it. `passed: false` is the only key it shows.

1. What are the envelope's fields, and which are mandatory?
2. Is `passed` a boolean, or is there a status value with more than two states
   (pass, fail, error, skipped, timed out)?
3. Is there a distinct value for "the adapter could not produce a result",
   separate from "the tool reported failure"? FR-4.5 assumes there is.
4. Does the envelope carry a schema version, and what must a consumer do when it
   meets a version it does not know: fail, warn, or read what it recognises?
5. Is YAML the only serialisation, or is JSON also produced or accepted?
6. `reasons`, `failures`, `statistics`, `metadata` and `evidence references` are
   named in §22 as things the merge preserves. What is the shape of each: a free
   text list, a map, or a defined structure?
7. Is a finding a first-class structure with file, line, rule identifier,
   severity and message, or does each adapter shape its own?
8. Where does a per-file coverage failure land: as a finding per file, as a
   statistic, or as both?
9. How does an envelope point at evidence: a path relative to the run directory,
   a path relative to the repository root, an absolute path, or a content hash?
10. Does the envelope record the command line that was actually executed?
11. Does it record the tool's version, and if so how is that obtained?
12. Does it record start time, end time and duration, for the element and for
    the run?
13. Does it record the environment the command ran under, and if so, filtered
    how? A raw environment dump can carry credentials.
14. Does bolt record the repository or tree state the run validated, as §65's
    `validated tree SHA == current tree SHA` comparison needs, or is that the
    caller's job? FR-7.4 assumes it is bolt's.
15. Are envelopes required to be stable byte for byte across two runs over an
    unchanged tree, so a diff of two runs is meaningful? Timestamps and
    durations make that impossible unless they are separated out.
16. Does bolt validate its own envelope against the schema before writing it?
17. Who owns the schema, given §3.1 makes it ecosystem-wide: bolt, toolbox, or a
    separate shared definition every component depends on?
18. Is an envelope produced by something other than bolt, an azimuth run or a
    task node, held to exactly the same schema, or to a common subset?

## Jig and element fields

19. What format is a jig written in: YAML, TOML, or something else?
20. How is a jig located: a fixed filename at the repository root, a search
    upward from the working directory, or an explicit argument?
21. What fields define one execution element? Candidates: name, command,
    adapter, working directory, environment, timeout, required, inputs,
    outputs, condition.
22. Is an element's name constrained, given FR-4.7 makes it part of a filename?
23. How is the adapter named on an element: a registry name bolt resolves, a
    path to an executable, or inferred from a declared output format?
24. Can an element omit an adapter and get the generic exit-code one by default?
25. How does an element declare the artifacts it produces, so §17's "commands
    consuming earlier artifacts" can find them?
26. How does an element declare what it consumes, and is that declaration the
    ordering mechanism or is ordering separate?
27. Can an element be conditional, running only when a path exists or only when
    a named earlier element passed?
28. Is `required` a per-element field, given §22 says "all required constituent
    results"? What is its default?
29. What does a failing non-required element do to the merge: nothing, a warning
    in the merged result, or a distinct non-fatal status?
30. Are environment variables inherited wholesale, filtered to an allowlist, or
    declared explicitly per element?
31. What is an element's working directory by default, and can it be set per
    element?
32. Can a jig declare an element that is skipped without being deleted, and does
    a skipped element appear in the merge?
33. Is a jig validated against a schema, and does an unrecognised key fail the
    run or warn?

## Adapter invocation

34. Is an adapter a separate process bolt executes, a plugin compiled into bolt,
    or may it be either?
35. If it is a process, what does it receive: argv, stdin, environment
    variables, or a directory it is pointed at?
36. Which of the element's results does an adapter get: exit code, captured
    stdout, captured stderr, generated files, the command line, the working
    directory?
37. How does an adapter return its result: the envelope on stdout, or a file
    written to a path bolt supplies?
38. What does bolt do when an adapter exits non-zero, writes nothing, or writes
    something that fails schema validation? All three are FR-4.5's "could not
    produce a result", and they may want different handling.
39. Does an adapter get a timeout of its own, distinct from the element's?
40. Can one element's output be fed to more than one adapter, producing more
    than one constituent result?
41. How is an adapter resolved by name: a search path, a fixed directory inside
    toolbox, a symlink placed by `link-jigs`?
42. Is there a declared interface version between bolt and an adapter, and what
    happens on a mismatch?
43. May an adapter read files from the repository tree, or only the element
    outputs it was handed?

## Timeout and failure semantics

The architecture does not mention time at all. Everything here is unstated.

44. Does an element have a timeout? Where is it declared, and what is the
    default when it is not?
45. When an element times out, is the outcome a failing result, an error result,
    or a bolt-level failure that exits non-zero?
46. How is a timed-out child terminated: which signal, and is there a grace
    period before a harder one?
47. Are the child's descendants killed with it, or can a run leave orphans
    behind?
48. Is there a whole-run timeout distinct from the per-element ones?
49. When an element's command cannot start at all, because the binary is missing
    or is not executable, is that a failing element or a bolt failure? The
    distinction matters: a missing tool means the gate did not run, and
    reporting it as a normal failure conceals that.
50. Does a failing element stop the run, or does a run always execute every
    element and decide at the merge?
51. Is there a fail-fast mode, and if so how does the merged result represent
    the elements that never ran?
52. If an element produced no envelope at all, does the run still write
    `run_result.yaml`? §3.4 distinguishes missing evidence from malformed
    evidence, and the merge has to represent that distinction somehow.
53. If bolt itself fails partway, does it write a partial `run_result.yaml` or
    none? FR-4.4 says an envelope is whole or absent, which argues for none, but
    a caller then cannot tell a crashed run from a run that never started.
54. What exit statuses does bolt use, and what does each mean? Candidates: a jig
    that will not parse, an unknown adapter, an unwritable output directory, an
    element that could not start, receiving a signal.
55. What happens when a tool modifies the repository tree during a run that is
    validating that exact state? Is that detected, prevented, or ignored?
56. Is a run resumable after an interruption, or is a partial run always
    discarded and redone?

## Artifact paths and naming

57. Where does a run write? A directory bolt chooses, one the caller names, or a
    fixed location inside the repository?
58. Is the run directory unique per run so runs accumulate, or is one location
    overwritten each time?
59. §22 names `foo_output.yaml` for an element and `run_result.yaml` for the
    merge. Is `_output` the element's envelope, or is `_output` the captured
    native output with a separate `_result` envelope beside it? Two files per
    element and one file per element are different contracts for a consumer.
60. What are the file names for an element's captured stdout and stderr?
61. Where do a tool's own generated artifacts go: left where the tool wrote
    them, or collected into the run directory?
62. Are paths inside an envelope relative to the run directory, relative to the
    repository root, or absolute? An absolute path from a worker sandbox is
    meaningless to a control-plane reader.
63. Does a nested jig get a nested output directory, and how is an element named
    when two composed jigs declare the same element name?
64. Is there an ordinal prefix on element output files, and does it follow
    declaration order or execution order? Under concurrency those differ.
65. Who removes a run directory, and when? An accumulating directory in a
    dogfooded repository is a growing pile of untracked state.
66. Does §16's `.ephemera` mean anything to bolt, or is the output location
    simply whatever the caller supplies?

## Which files a run covers

67. Does bolt select the file set an element runs over, or does the declared
    command line select its own?
68. If bolt selects, by what: glob patterns in the jig, gitignore awareness, a
    tracked-files listing, or files changed since a named ref?
69. Can a run be restricted to changed files? §67's planned pre-commit overlay
    is the obvious consumer of that.
70. What happens when a selection matches nothing: the element passes, is
    skipped, or fails?
71. How does a subtree jig from §21 get its file set scoped to that subtree, and
    are paths in its envelope relative to the subtree or to the repository root?
72. Can a caller run a subset of a jig, and how is the subset named: by element
    name, by tag, by subtree?

## Ordering and concurrency

73. Are elements ordered by declaration order, by declared dependencies, or by
    both?
74. Do independent elements run concurrently by default, or serially unless
    asked?
75. What bounds concurrency, and is the bound set in the jig, on the command
    line, or from the machine?
76. Do nested jigs run concurrently with each other, and with elements of their
    parent?
77. If two elements would write the same artifact path, is that detected before
    the run or discovered as corruption during it?
78. Is the merged result's ordering deterministic regardless of execution order?
    FR-9.7 assumes so.
79. Can two bolt runs operate on the same tree at once, and is there a lock? The
    dogfooding case makes this real.

## Composition, overlay and inheritance

80. When a jig invokes another, what does the child inherit: environment,
    working directory, timeouts, the `required` default, file selection?
81. Can a parent override a value inside a child's element, and at what
    granularity: the whole element, or one field of it?
82. Can a parent disable an element that a shared jig declares, and is that
    recorded in the merged result so the omission is visible?
83. Is there a user-level or machine-level layer above the repository's jig?
    §67 describes exactly that arrangement for pre-commit, with a repository
    policy and an independent personal policy.
84. What is the precedence order when the same key is set at more than one
    layer?
85. When both sides set a collection, are they merged or does one replace the
    other?
86. Can a jig reference another jig by version, or is it always whatever is on
    disk at the referenced path?
87. Is recursion between jigs detected, and does it fail the run?
88. Is nesting a jig the same mechanism as bolt invoking itself as a subprocess,
    or is it in-process composition? A subprocess produces its own
    `run_result.yaml`, which the parent merge then has to absorb.
89. Does a nested jig's merged result become a single constituent of the
    parent's merge, or are its elements flattened into the parent?

## Boundaries with the rest of the ecosystem

90. Does the per-file coverage policy of §20 live in a bolt adapter, in a
    toolbox analyzer that an adapter then reads, or in bolt itself? Which
    repository owns it decides whether it is a bolt requirement at all.
91. Is `run_result.yaml` the file a ratchet node depends on directly, or does a
    node wrap bolt's output in something of its own?
92. Does bolt do anything about §3.3's viable-producer logic, or is that
    entirely ratchet's and invisible to bolt?
93. §65 wants evidence tied to an exact tree state. Is bolt responsible for
    computing and recording that, or does the caller stamp it afterwards?
    Question 14 is the same question from the envelope's side, and one answer
    settles both.
94. Does bolt need to run outside a git repository at all, given §59's sandbox
    is always one? If it does, what fills the tree-state field?
95. What is the minimum a component outside bolt must do to be a valid envelope
    producer, since §3.1 makes the vocabulary ecosystem-wide?
96. Does bolt have any interface other than a command line? An importable
    library used by another Go component in the ecosystem would change what the
    requirements have to cover.
