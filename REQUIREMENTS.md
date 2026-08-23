# bolt, Requirements

Derived from `silo/docs/ARCHITECTURE.md` and from answers given against
`NEXT_STEPS.md`. No earlier bolt implementation, requirements document, design
note or test was read while writing this. The provenance of that earlier
material is unresolved. This document exists to establish that the requirements
reach from the architecture and from fresh answers alone, and reading it would
have destroyed that.

Requirements are stated as observable properties. Each says what must be true of
bolt or of a run, not how anything is built.

**Status markers.** `[A]` traces to a direct statement, in the architecture
document or in an answer. `[D]` is derived from one. `[A/D]` is both. `[?]` is
open, recorded so it is not lost and carrying no test yet.

No test cites any row here, because no implementation exists. Every settled row
is uncovered under toolbox's traceability gate, and marking them `[?]` to turn
that green would misreport what is settled.

## Where this departs from the architecture

The architecture document is the origin and is wrong in four places. Each
departure is deliberate.

§23 shows `passed: false` inside the merged result. The key is `success`.

§22 calls the unit of execution an "execution element". The word is *task*.

§22 spells the merged file `run_result.yaml` and the per-element file
`foo_output.yaml`. Both names change and so does where they sit. A task
execution's envelope is `output.yaml` inside that execution's own directory, and
the run's is `result.yaml` at the top of the output directory. Singular, because
a run has exactly one result, folded from the results of its tasks.

§17 lists "chains of commands" and "commands consuming earlier artifacts" among
what a run may execute, and §19 is a section whose worked example is a coverage
producer feeding a policy analyzer feeding an adapter. No task consumes another
task's output. That chain still happens inside one script, and bolt sees one
task, so the capability survives and the architecture's picture of bolt-level
composition does not.

---

## 1. Runs, and the files bolt reads and writes

| ID | Requirement | |
|---|---|---|
| FR-1.1 | A run executes the command lines its jigs declare and records what happened as files on disk. Nothing a consumer needs to know about the outcome exists only in bolt's own output streams. | [A/D] |
| FR-1.2 | Bolt holds no knowledge of any particular tool. Which commands run, and what their output means, come from the jig. Adding a language or a checker to the ecosystem changes a jig and an adapter, never bolt. | [A] |
| FR-1.3 | Task ETL is the abstraction and quality checking is its first use. A jig that runs no checker and reaches no verdict about code is a legitimate run. | [A/D] |
| FR-1.4 | A run captures each command's native results whatever form they take: stdout, stderr, exit code, and arbitrary files the command generated. Those results survive the run as evidence. | [A/D] |
| FR-1.5 | Every file bolt reads as data is parsed and validated against a schema before anything acts on it, a jig and a task's envelope and a nested run's result alike. What a schema leaves open stays open; what it requires is checked. | [A] |
| FR-1.6 | Validation is two steps: can the parser load the file at all, and does the structure it produced match the JSON Schema for that kind of file. Schemas apply to decoded maps and lists rather than to text, so one mechanism covers everything bolt reads. | [A] |
| FR-1.6a | FR-1.5, FR-1.6 and FR-3.4d are ecosystem decisions bolt honours rather than decisions bolt makes. YAML everywhere and JSON Schema over decoded structures apply to every component, and bolt does not get to differ. Filed for the architecture document as `clank/inbox/silo/yaml-and-json-schema-is-a-platform-decision/`. | [A] |
| FR-1.7 | A schema checks shape, not meaning. A command that parsed differently from how it was written is still a string of the right type, and validation passes it. | [D] |
| FR-1.8 | Validation runs before writing as well as after reading. Bolt checks a file against its schema on the way out, so it cannot emit something it would refuse to read back. | [A] |
| FR-1.9 | Every read and every write goes through one path that requires a schema. Validation is not a step a call site can omit, so a file bolt handles is covered because of how it was handled rather than because somebody remembered. | [A] |
| FR-1.10 | YAML is written in canonical form: block style, one key to a line, scalars quoted. Flow style is valid YAML and so is JSON, and neither is what bolt emits. Two results then differ by the lines that changed rather than by one long line. | [A] |
| FR-1.11 | `wrench` is the support library for reading, writing and validating the form of the ecosystem's input and output files. The schemas and a library for each language live there together, so a Go producer and a Python producer get the same definition and equivalent code rather than a schema in one repository and implementations elsewhere obliged to keep up with it. | [A] |
| FR-1.12 | Bolt reads and writes envelopes through that project's Go library, so bolt is one consumer of the contract and not its owner. | [A/D] |
| FR-1.13 | Bolt validates with nothing else installed beside it. | [A] |
| FR-1.14 | File handling is two calls, `load_formatted_file(path, schema, codec, reader)` and `save_formatted_file(data, path, schema, codec, writer)`. Validation sits in the signature: there is no way to read or write without naming what the file must conform to, which makes FR-1.9 a property of the library's surface rather than a discipline every call site has to keep. | [A] |
| FR-1.15 | The codec is the format and the reader or writer is the IO, declared separately. A format is then added without inventing a source and a source without inventing a format, rather than needing one function per combination of the two. | [A] |
| FR-1.15a | Separating them puts the IO boundary wholly outside the call. A reader is handed the path, so substituting it in a test exercises the validation paths against no filesystem at all rather than only replacing the parse. | [A/D] |
| FR-1.16 | The envelope is one schema handed to those calls and a jig is another. The shared project provides structured files with schemas attached, for anything in the ecosystem, rather than an envelope-specific facility. | [A] |
| FR-1.17 | JSON Schema validates the decoded structure, so it is indifferent to how the file was serialised on the way in. | [A/D] |
| FR-1.18 | Canonical form belongs to the save call, so a caller cannot emit something valid but written another way. | [A/D] |
| FR-1.19 | The signature compels a schema, not the right one. Passing none is impossible; passing the wrong one is not. | [D] |

## 2. Invocation

| ID | Requirement | |
|---|---|---|
| FR-2.1 | A jig is run on a directory. That is the whole of what an invocation says: which jig, and where. | [A] |
| FR-2.2 | Bolt walks that directory to find the files its tasks act on. It has no changed-since-a-ref. | [A] |
| FR-2.2a | The walk honours `.gitignore`. An ignored file is not part of the project and is not checked, which keeps `.git`, `node_modules`, a virtualenv and build output out of every run without a second list to maintain. | [A] |
| FR-2.2b | Honouring `.gitignore` means reading those files as text. Bolt does not invoke git, read anything under `.git/`, or require a repository, so it does not reach `.git/info/exclude` or a global excludes file either. A tree with no `.gitignore` in it simply excludes nothing. | [A/D] |
| FR-2.2c | A run never walks another run's output. Bolt's own run directories are excluded whatever `.gitignore` says, because a tree accumulating them would otherwise feed each run the last one's evidence. | [D] |
| FR-2.3 | The directory is the run's base, and everything a run touches comes from inside it. Containment is how the input is formed, not a check applied to it afterwards. | [A/D] |
| FR-2.4 | Paths are resolved to absolute before anything runs. | [A] |
| FR-2.5 | A run refuses to start if the directory it was given is not there. | [A/D] |
| FR-2.6 | `--output-dir` names the directory a run writes into. Given none, a run creates `.bolt-<iso8601>`. | [A] |
| FR-2.7 | Bolt reads no git. A run over a tree that is not a repository behaves exactly as one over a tree that is. | [D] |
| FR-2.8 | A config directory argument says where `bolt.<name>.yaml` files are found, so where jigs live is told to bolt rather than inferred from the directory being run on. | [A] |
| FR-2.9 | A relative path is resolved against the base directory of the invocation it is written in. One rule covers filter patterns, a jig task's subdirectory, a `config-dir` field and anything else written as a path, so nobody has to remember which kind of path follows which convention. | [A] |
| FR-2.9a | For a field on a jig task that is the parent's base, because the parent's jig is where the field is written. A path configuring the child is still read in the frame of whoever wrote it. | [A/D] |

## 3. Jigs and tasks

| ID | Requirement | |
|---|---|---|
| FR-3.1 | A jig is the unit of configuration and composition. What bolt executes for a project is read from that project's jig. | [A] |
| FR-3.2 | A task declares a name, an optional description, `matching` and `excluding` lists, its adapter, its evidence files, `short-circuit-failure`, and a command written as a shell line. There is no runmode field. | [A] |
| FR-3.3 | A task's name prefixes its work directories, so a task's evidence is identifiable on disk without opening anything. | [A/D] |
| FR-3.4 | `matching` is a condition on a task: a list of patterns or literal paths saying which files inside the run's directory that task acts on, where `**` matches zero or more directory levels. Every Python file through the formatter is one task with one pattern. A task never sees a path its condition rejects. | [A] |
| FR-3.4a | `excluding` is its counterpart, taking the same list of patterns or literal paths and removing from what `matching` selected. A task wanting everything but one shape of file says so directly instead of writing a pattern that means "not that", and a single known-bad file is named outright. | [A] |
| FR-3.4b | `matching` and `excluding` belong to a task that consumes paths. On a command naming neither path variable they are a jig error, caught in validation rather than quietly ignored. Whether a whole-project command should run at all is a question about where the jig is pointed, and FR-5.15 already answers it. | [A] |
| FR-3.4c | The jig format carries comments, and an entry's reasoning sits beside it. Somebody asking why a path is excluded finds the answer where the path is, rather than reconstructing it from git history. | [A/D] |
| FR-3.4d | A jig is YAML, as an envelope is. One serialisation everywhere: one parser, one schema mechanism, and a jig and a result readable by the same tooling. | [A] |
| FR-3.5 | Filter patterns are relative to the base directory of the run they are declared in. A jig written for reuse therefore says `**/*.go` and never names the subtree it was dropped into, which is what makes it the same jig at the repository root and at `backend/`. | [A] |
| FR-3.6 | Organisation-wide, language-specific and repository-specific behaviour compose through jigs, with none of it hard-coded into bolt. | [A] |
| FR-3.7 | A jig maintained outside the repository and made available inside it, as toolbox's `link-jigs` does, runs without being copied into the tree. | [D] |
| FR-3.8 | Bolt draws no line between a shared jig and a project-specific one. The same fields serve both, and every literal path or narrow pattern a jig carries trades reuse for fit. Where a jig sits on that scale is its author's choice and not a rule bolt enforces. | [A/D] |
| FR-3.9 | A jig file is `bolt.<name>.yaml`, so jig files are identifiable in a directory holding everything else a project keeps, and a jig is spoken of by its `<name>` rather than by a filename. | [A] |
| FR-3.10 | A jig declares `requires`, every executable it invokes: the tools its commands run, the adapters its tasks name, and any checker it calls. Nothing a jig reaches for is absent from that list, so the list is the jig's whole dependency inventory rather than a note about unusual tools. | [A] |
| FR-3.10a | An adapter named by a task therefore appears in `requires` too, which is a consistency a checker can hold: an adapter no entry covers is found before a run instead of when the task reaches it. | [A/D] |
| FR-3.11 | Those declarations are readable by things other than bolt. An anvil image is built from a jig, with its install logic driven by that jig's `requires`, so what an image carries is generated from the list rather than mirroring it. There is one source and nothing to reconcile, and nothing depends on bolt gathering anything up. | [A] |
| FR-3.12 | Bolt validates the jig it is handed and does not go looking for others. Every reachable jig being well-formed is a checker's job, run over the config directory as a task like any other, so a broken jig fails a gate instead of surfacing halfway through one. | [A] |
| FR-3.13 | That leaves no bootstrap hole. The jig bolt is given is validated as it loads, so a broken one fails at once, and the checker covers the jigs bolt was never asked to read. | [A/D] |

## 4. Substitution and execution

| ID | Requirement | |
|---|---|---|
| FR-4.1 | Three locations are separately specifiable and separately available to every task: the project root, the base this run operates from, and the execution's own work directory. The outermost run is assumed to sit at the project root and a nested one is not, so a jig based on a subtree can still reach a config file at the root without giving up its base. | [A] |
| FR-4.1a | A command runs at the base directory. A tool has to stand where the jig's frame of reference is, or `./...` and a bare relative path mean something other than what the jig meant, and FR-3.5 already puts the patterns there. FR-5.12's override is the exception, running a jig that needs the repository root at the repository root. | [A] |
| FR-4.1b | The base is where a command stands rather than the only place it can reach. The project root, the config directory and the work directory are all named to it, so needing one is not a reason to stand somewhere else. | [A/D] |
| FR-4.2 | How a task runs is read off its command, not declared beside it. `{each_path}` means one execution per matched path. `{all_paths}` means one execution with the whole selection substituted. Neither means one execution and no paths. A command naming both is a jig error. | [A] |
| FR-4.3 | Every path bolt substitutes is individually quoted, so a path carrying a space, a quote or a semicolon can neither split the command line nor inject into it. | [A] |
| FR-4.4 | A task whose command names `{each_path}` or `{all_paths}` does not execute when its filtered selection is empty, and produces no output. A task naming neither always executes. | [A] |
| FR-4.5 | Tasks execute serially. | [A] |
| FR-4.6 | No task consumes another task's output. Work needing several steps is one script producing one exit code and one output. | [A] |
| FR-4.7 | Because no task depends on another, the merged result does not vary with the order tasks ran in. | [D] |
| FR-4.8 | A failing task does not stop the run. The tasks after it still execute, because a run that stops early throws away the evidence they would have produced and leaves a reader unable to tell what else was wrong. | [A] |
| FR-4.9 | A task may set `short-circuit-failure`, defaulting to false, to stop the run when it fails. Stopping is what a jig asks for rather than what it gets. | [A] |
| FR-4.10 | A command that cannot start at all produces `success: false` with a reason naming the `requires` entry that was missing. A missing tool is a failing task, and which kind of failure it was is what the reason carries. | [A] |
| FR-4.11 | A time limit may be set for a task run and for the whole run. Both are options, so unset means a tool is allowed to finish. | [A] |
| FR-4.11a | A task's limit covers everything that task does, taken together: every command invocation and every adapter invocation. Thirty seconds over four hundred paths is thirty seconds for the task, not for every path in turn, and the adapter runs come out of the same budget. | [A] |
| FR-4.11b | Reaching it kills the execution in flight and the executions after it do not start. | [A/D] |
| FR-4.12 | A task exceeding its limit fails, with a reason saying the limit was passed. The run carries on, by FR-4.8, because a slow task is no more reason to discard the rest than a failing one. | [A] |
| FR-4.12a | A killed command keeps whatever output it managed to gather, and its adapter runs over that. A tool that reported forty problems before hanging reported forty real problems, and discarding them would throw away the only evidence the execution produced. | [A] |
| FR-4.12b | It fails regardless of what its adapter concluded, and its reasons carry at least the limit being passed. A partial run cannot report a pass, because what it did not reach is exactly what is unknown about it. | [A] |
| FR-4.12c | Where the limit catches the adapter rather than the command, bolt writes that envelope itself. Nothing else is left to write one, and the guarantee below has to hold whichever of the two was running. | [A/D] |
| FR-4.12d | A timed-out execution therefore has a valid envelope, which distinguishes it from one whose adapter died of its own accord and left none. Under FR-7.6 a timeout is an authoritative failure and a crash is no result at all. | [A/D] |
| FR-4.13 | A run exceeding its limit fails, with a reason saying the limit was passed. | [A] |
| FR-4.14 | A run that times out still writes its result, carrying what completed before the limit. Bolt is alive and in control when the limit passes, so the rule is the one FR-5.8 already sets for a refusal: only a bolt that dies leaves nothing behind. | [A/D] |

## 5. Nested jigs

| ID | Requirement | |
|---|---|---|
| FR-5.1 | A task may name a jig in place of a command. | [A] |
| FR-5.1a | A nested run is not a mode. Inside its subdirectory it is identical to the same jig run on that directory from the command line, so there is one operation and one code path, invoked from two places. | [A] |
| FR-5.1b | A parent knows a jig's name and where to run it, and nothing about what is inside it. The child follows its own process when invoked: its own `requires`, its own tasks, its own filtering. Nothing rolls up and no parent reads a child's content. | [A] |
| FR-5.2 | A nested run writes into its own subdirectory inside that task's work directory, and its `result.yaml` is linked as the task's `output.yaml` by a relative symlink, so the whole tree survives being moved or archived. | [A/D] |
| FR-5.3 | A jig task carries the same bookkeeping files as a command task, so nothing reading `work/*/` needs to know which kind it is looking at. | [A/D] |
| FR-5.4 | The merge does not know that a constituent was a nested run. Nesting is a special case at invocation and nowhere else. | [A/D] |
| FR-5.5 | A jig task carries no condition. The child walks its own subdirectory and its own tasks decide what they act on. Selecting files is the nested jig's business, never its caller's. | [A] |
| FR-5.6 | Bolt carries its nesting depth in the environment of every process it spawns, and increments it on finding the variable already set. The depth therefore survives reparenting, backgrounding and a task command that invokes bolt directly rather than through a jig task. | [A/D] |
| FR-5.7 | The ceiling defaults to 4 and is read from the environment only at the outermost invocation, so a jig cannot raise the limit it is running under. | [A/D] |
| FR-5.8 | A run refused for depth writes its own `result.yaml` with `success: false` and a reason naming the limit, then exits non-zero. Its parent's link resolves, and the merge folds an ordinary failure. | [A] |
| FR-5.9 | Paths are absolute at every depth, so a nested run's evidence folds into its parent with nothing rewritten. A path means the same thing to a child and to its parent. | [A/D] |
| FR-5.10 | A jig task names a jig and, optionally, a subdirectory of the current base to run it in. Naming one says something specific: a project of that jig's kind lives there, a Go module with its own `go.mod`, a Python package with its own `pyproject.toml`. The declaration is that applying this jig at this level is worth something, and that directory becomes the child's base. | [A] |
| FR-5.11 | The subdirectory is a written path, not a pattern. A jig states where its nested projects are, because a pattern can say which files look like Go and never that a directory is a Go module. | [A] |
| FR-5.12 | One jig may be named by many jig tasks at different bases. Eight Go subprojects is eight jig tasks, so the work directory prefix is the task's name and never the jig's. | [A] |
| FR-5.13 | Naming a subdirectory narrows the base and the containment check together while the project root stays what it was, so a jig distributed by toolbox drops in at any depth without being written to know where it was placed. | [A/D] |
| FR-5.13a | A jig task declares what it changes about the invocation it makes, as fields rather than as a command line. What it does not declare is inherited, so a nested jig runs with its parent's settings until a field says otherwise. | [A] |
| FR-5.13b | `config-dir` names where the child looks for jigs. Left out, it inherits, so a subproject carrying jigs nobody else sees is something a jig asks for rather than something it gets. | [A] |
| FR-5.13c | `output-dir` names the child's output directory rather than placing it. Whatever it is set to, the result is a subdirectory of that task's work directory, so renaming is expressible and relocating is not sayable, and FR-5.2's layout cannot be undone by a field. | [A] |
| FR-5.13d | There is no field for the directory a child runs on. It comes from FR-5.13's subdirectory and from nowhere else, which is what makes containment a property rather than a habit. | [A/D] |
| FR-5.13e | There is no field for the depth ceiling. FR-5.7 has a nested invocation read the propagated one, so a field would have nothing to act on. | [A/D] |
| FR-5.13f | Every one of these is schema-checkable, which a command line would not have been. FR-1.5 validates a jig, and the part with the most power over a nested run is not the part exempt from it. | [A/D] |
| FR-5.14 | A jig that genuinely needs the repository root says so, and that overrides a subdirectory base. | [A] |
| FR-5.15 | A jig task with no input paths under its base does not run, by the same rule that stops a path-consuming task with nothing to consume. A nested project nobody touched contributes nothing. | [A] |

## 6. Adapters

| ID | Requirement | |
|---|---|---|
| FR-6.1 | An adapter is a separate process. It turns one task execution's captured output into a result envelope, and nothing else in bolt decides whether that execution passed. The single exception is FR-4.12b: an execution bolt terminated cannot pass, whatever its adapter made of what it gathered. | [A] |
| FR-6.2 | The default adapter invocation names the captured files: `--stdout`, `--stderr`, `--evidence` and `--exitcode`. A task may write its adapter invocation explicitly in place of the default. | [A] |
| FR-6.2a | An adapter is handed the same three locations every task gets, the project root, the run's base and the execution's work directory. | [A] |
| FR-6.2c | A task declares its evidence files, and those are what `--evidence` names. Discovery would hand an adapter whatever a tool happened to leave behind, a lock file or a temporary or an intermediate, and let something irrelevant ruin a run. An artifact nobody declared still sits in the work directory as evidence on disk; it is simply not passed to the adapter. | [A] |
| FR-6.2b | An adapter writes `output.yaml` into that work directory. The path is the work directory it was given and the name never varies, so no flag says where the envelope goes and no task can put it somewhere else. | [A] |
| FR-6.3 | A child process's exit code reaches its adapter as a file. Bolt reaches no verdict of its own from it, and does not record it in the envelope either: whether that number explains anything is the adapter's judgement, not bolt's. | [A] |
| FR-6.4 | An adapter is chosen by the output format it reads. Any tool emitting a format some adapter understands reuses that adapter, whoever wrote the tool. | [A] |
| FR-6.5 | Adapters read structured formats as well as exit codes: Cobertura, pytest JSON, and other structured test and coverage reports. | [A] |
| FR-6.6 | Fixing an adapter and re-folding a finished run costs no re-execution, because every input an adapter reads is already on disk. | [D] |
| FR-6.7 | Every adapter, and the merge, carries tests asserting that what it produces validates against the envelope schema. FR-1.8's check on the way out is a backstop; the guarantee is that a producer which would emit something invalid fails its own suite first. | [A] |
| FR-6.8 | Those tests are boilerplate rather than written afresh per adapter, so conformance arrives with writing an adapter instead of being something each author has to remember. | [A] |

## 7. Result envelopes

| ID | Requirement | |
|---|---|---|
| FR-7.1 | `success`, a boolean, is the only key every envelope carries. | [A] |
| FR-7.2 | `reasons` is present when `success` is false. Its members are objects whose shape is open, so whatever detail a producer holds can travel with the failure. | [A] |
| FR-7.3 | `metadata` is optional, and carries `statistics` and `evidence` where a producer has them. | [A] |
| FR-7.3a | Nothing puts the exit status into the envelope by default. It matters when the adapter says it matters, and then it goes into a reason, because a reason is where an adapter says what a result rests on. | [A] |
| FR-7.3b | Leaving it out loses nothing. The raw value sits in the `exitcode` file either way, so a reader who wants it has it and a consumer is not handed a number nobody claimed was relevant. | [A/D] |
| FR-7.3c | Timings go in `metadata` and are not in the first version. Nothing therefore has to hand an adapter a clock it could not read for itself, and the adapter contract stays as it is. | [A] |
| FR-7.4 | Bolt's envelopes use the ecosystem's shared vocabulary. An envelope from a task, from a merge, from a task node or from azimuth is read the same way by the same consumer. | [A] |
| FR-7.5 | An envelope is written whole or not at all. A run killed partway leaves no half-written envelope for a consumer to read as authoritative. | [D] |
| FR-7.5a | Every file bolt or an adapter writes as a unit is written atomically, to a temporary and renamed into place, which is what makes FR-7.5 true rather than hoped for. A process killed mid-write leaves absence, and absence is a state FR-7.6 already knows how to read. | [A] |
| FR-7.5b | The temporary sits beside its target. A temporary somewhere else makes the move a copy across filesystems, which is not atomic and defeats the point. | [A/D] |
| FR-7.5c | Captured streams are the exception, because they are not written as a unit. FR-4.12a needs a killed command's partial output to survive, and output still arriving cannot be written atomically without discarding exactly what that row keeps. | [A/D] |
| FR-7.6 | Absent and invalid are different conditions. No `output.yaml` means no authoritative result has been reached. One that is present and fails validation is a failure. One that validates is authoritative. | [A] |
| FR-7.7 | Producing a valid envelope means a well-formed YAML file carrying `success` as a boolean, and `reasons` as a list of objects when `success` is false. Nothing further is required of any producer, inside bolt or outside it. | [A] |

## 8. The merge

| ID | Requirement | |
|---|---|---|
| FR-8.1 | A run has exactly one result. The merge reads every `work/*/output.yaml` and folds them into one `result.yaml`, mechanically, and repeatably over a finished directory. | [A/D] |
| FR-8.2 | The merge rewrites `evidence` from a list of paths into a mapping keyed by task, each entry carrying that task's args and the filepath of its own result. | [A] |
| FR-8.3 | The merged result passes only when every constituent passes. There is no constituent whose failure does not count: a check nobody wants enforced is a check not in the jig. | [A/D] |
| FR-8.4 | The merged result carries the reasons, statistics and evidence references its constituents produced, so what failed and why is readable from the merged file alone. | [A/D] |
| FR-8.5 | Constituent envelopes survive the merge. Both levels stay on disk. | [D] |
| FR-8.6 | Only the outermost invocation relativises. Preparing the final result, a bolt that finds no depth set in its environment rewrites the output and evidence references going into `result.yaml` as relative to the project directory; a nested run leaves them absolute. No root has to be propagated for this, because the only bolt needing one is the bolt doing the conversion. | [A/D] |
| FR-8.7 | Rewriting reaches the structured path references and stops there. Text a tool emitted, carried up inside a reason, stays as the tool wrote it and may still name an absolute path. | [A/D] |

## 9. The output directory

A run's whole output is one directory:

```
<output-dir>/
  result.yaml
  work/
    <task>-<ordinal>/
      manifest
      stdout
      stderr
      exitcode
      <artifacts the command wrote>
      output.yaml
```

| ID | Requirement | |
|---|---|---|
| FR-9.1 | A run's whole output is one directory, so a run can be archived, moved or handed to somebody as a single artifact. | [A/D] |
| FR-9.2 | Each task execution gets its own directory holding the command as executed, captured stdout and stderr, the exit code as a file, whatever artifacts the command wrote, and the adapter's `output.yaml`. | [A] |
| FR-9.2a | The ordinal is the execution index within the task. Each task numbers its own executions from one, independently of every other task, so a directory name says which task and which of its executions without needing the run's order. For a per-path task the index is the position in the matched list, which FR-9.5's manifest records, so an execution traces back to the path it was handed. | [A] |
| FR-9.2b | The ordinal is zero-padded to the width that task's execution count needs, so a listing sorts correctly with no arbitrary cap and no wasted digits. The count is known before the first execution, because the matched list is settled before any of it runs. | [A] |
| FR-9.3 | One execution's evidence is complete inside one directory. A reader needs nothing outside it to see what ran and what happened. | [D] |
| FR-9.4 | Serial execution makes the ordinals deterministic, so two runs over the same tree produce identical work directory names and the two trees line up file for file. | [D] |
| FR-9.4a | Whether their contents match is a separate matter. Envelopes carry absolute paths, so a run directory named after its own timestamp turns up inside them and two such runs differ wherever a path is recorded. Point both runs at a stable output directory and they do not. | [D] |
| FR-9.5 | An execution's manifest records which paths `matching` selected and which `excluding` removed, for a task that consumes paths. What that task saw, and what it was kept from seeing, sits on disk beside what it did. | [A] |
| FR-9.5a | A manifest is written before its command runs, so an execution that was killed, or that never got started, still records what was going to be attempted. The case that most needs a record is the one that would otherwise have none. | [A] |
| FR-9.5b | It therefore holds only what is known beforehand. Anything a run learns by finishing is not in the manifest, because the manifest was closed before there was anything to learn. | [A/D] |
| FR-9.5c | Every value bolt exposed as a template variable for that execution is in the manifest: the three locations, and whichever path variable applied. A reader sees what the task was given and not only what its command became, which matters where one path appears several times in a line or where a variable was available and went unused. | [A] |
| FR-9.5d | That is a rule rather than a list, so a variable added later is recorded because it is a variable, not because somebody remembered to add it. | [A/D] |
| FR-9.5e | The environment is not among what it holds. A dump of it carries whatever the shell was holding, into a file that exists to be handed around as evidence, and recording it safely means filtering it, which is not a first-version problem worth having. | [A] |
| FR-9.5f | So an execution is not fully reconstructable from its evidence. What a tool read from its environment is not written down, and behaviour that turned on `PATH`, a locale or a tool's own configuration variable cannot be explained from the run directory. | [A/D] |
| FR-9.6 | A task naming no path variable was handed no list, so its manifest claims none. Recording one would say the command saw files it never received. | [A] |
| FR-9.7 | What such a task examined is the tool's own business, and bolt does not know it. A run's evidence covers what bolt handed over, never what a tool went and found for itself. | [A/D] |

## 10. Exit status

| ID | Requirement | |
|---|---|---|
| FR-10.1 | Bolt's exit status answers one question: could bolt execute the requested task ETL? | [A] |
| FR-10.2 | A run in which every task executed and some tools reported failures exits 0 and writes `success: false`. That pairing is correct. | [A/D] |
| FR-10.3 | The authoritative quality verdict is the envelope. A caller reading bolt's exit status to learn whether the tools passed has read the wrong thing. | [A] |
| FR-10.4 | Bolt exits non-zero when it could not carry out the requested ETL. | [A] |

## 11. Where a run happens

| ID | Requirement | |
|---|---|---|
| FR-11.1 | A run needs nothing beyond the jigs it was named, the paths it was handed, and the tree those paths sit in. Control-plane state is absent from a worker sandbox, so a run depending on it could not execute there. | [D] |
| FR-11.2 | A run's whole effect is the directory it writes. It changes no graph state, no task state and no other control-plane record. | [D] |
| FR-11.3 | The same jig runs against whatever tree state it is pointed at, including a throwaway copy prepared to test a prospective merge. | [D] |

## 12. The program

| ID | Requirement | |
|---|---|---|
| NFR-12.1 | Bolt runs itself. Its own quality gate is a bolt run over its own repository. | [A] |
| NFR-12.2 | Bolt installs into a standardised development image beside a toolchain it knows nothing about. | [D] |
| NFR-12.3 | Bolt is MIT licensed. | [A] |
| NFR-12.4 | Bolt builds without a C toolchain and links statically, so an image carries one file and a cross-build needs no target compiler. Anything bolt links against inherits that constraint. | [D] |

## 13. Open

Each row states a property that must eventually hold and cannot be stated yet.
The questions that would settle them are in `NEXT_STEPS.md`.

| ID | Requirement | |
|---|---|---|
| FR-13.1 | An adapter writes its envelope to a defined place, named in the contract that invokes it. | [?] |
| FR-13.2 | A task that could not execute is distinguishable in `result.yaml` from one that executed and failed. | [?] |
| FR-13.3 | A task skipped for an empty file list is distinguishable in `result.yaml` from one that was never declared, so a green result cannot mean that nothing was checked. | [?] |
| FR-13.4 | Whether a constituent is required is declared, with a stated default. | [?] |
| FR-13.5 | A task that exceeds a time budget reaches a defined outcome. | [?] |
| FR-13.6 | Run directories are removed on a stated rule, so a dogfooding repository does not accumulate them without bound. | [?] |
| FR-13.7 | Evidence can be tied to the exact tree state it was produced from, as §65 requires, by something. Bolt reads no git, so either it acquires that dependency or the requirement belongs to the caller. | [?] |
| FR-13.8 | The number of bolt runs a user may have live at once is bounded, if that guard is wanted at all. | [?] |
| FR-13.9 | The jig is written in a defined format, validated against a schema. | [?] |
| FR-13.10 | The envelope schema is owned and published somewhere every producer and consumer in the ecosystem can validate against. | [?] |
