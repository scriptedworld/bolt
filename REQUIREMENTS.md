# bolt, Requirements

Derived from the ecosystem architecture and the decisions recorded in this
repository. The archived first implementation was not used as a source because
its provenance is unresolved.

Two separate repositories supply shared contracts. Wrench is the structured-file
library and schema owner. Toolbox contains shared jigs, adapters, and quality
checkers.

Requirements are stated as observable properties. Each says what must be true of
bolt or of a run, not how anything is built.

**Status markers.** `[A]` traces to a direct statement, in the architecture
document or in an answer. `[D]` is derived from one. `[A/D]` is both. `[?]` is
open, recorded so it is not lost and carrying no test yet.

Every `[D]` row is a default taken instead of a question asked, and all of them
are listed in `NEXT_STEPS.md` under "Defaults taken", so a wrong one is found by
reading that table instead of by meeting it in the code.

A settled row that no test cites reads as uncovered under the traceability gate,
and marking those `[?]` to turn it green would misreport what is settled.

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

That simplification has a cost worth stating, because a reader will otherwise
rediscover it. An intermediate step inside a script produces no envelope, no
manifest and no work directory, so §19's worked example loses its evidence
exactly where it was interesting: the analyzer's own input and verdict are no
longer on disk. A pipeline wanting every stage evidenced writes each stage as
its own task and passes files through the tree, which FR-4.6 permits and does
not help with.

---

## 1. Runs, and the files bolt reads and writes

| ID | Requirement | |
|---|---|---|
| FR-1.1 | A run executes the command lines its jig declares and records what happened as files on disk. Nothing a consumer needs to know about the outcome exists only in bolt's own output streams. | [A/D] |
| FR-1.2 | Bolt holds no knowledge of any particular tool. Which commands run, and what their output means, come from the jig. Adding a language or a checker to the ecosystem changes a jig and an adapter, never bolt. | [A] |
| FR-1.3 | Task ETL is the abstraction and quality checking is its first use. A jig that runs no checker and reaches no verdict about code is a legitimate run. | [A/D] |
| FR-1.4 | A run captures each command's native results whatever form they take: stdout, stderr, exit code, and arbitrary files the command generated. Those results survive the run as evidence. | [A/D] |
| FR-1.5 | Every file bolt reads as data is parsed and validated against a schema before anything acts on it, a jig, a definitions file and a task's envelope alike. What a schema leaves open stays open; what it requires is checked. | [A] |
| FR-1.6 | Validation is two steps: can the parser load the file at all, and does the structure it produced match the JSON Schema for that kind of file. Schemas apply to decoded maps and lists rather than to text, so one mechanism covers everything bolt reads. | [A] |
| FR-1.6a | FR-1.5, FR-1.6 and FR-3.4d are ecosystem decisions bolt honours rather than decisions bolt makes. YAML everywhere and JSON Schema over decoded structures apply to every component, and bolt does not get to differ. Raised against the architecture document rather than settled here. | [A] |
| FR-1.7 | A schema checks shape, not meaning. A command that parsed differently from how it was written is still a string of the right type, and validation passes it. | [D] |
| FR-1.8 | Validation runs before writing as well as after reading. Bolt checks a file against its schema on the way out, so it cannot emit something it would refuse to read back. | [A] |
| FR-1.9 | Every read and every write of a file bolt treats as data goes through one path that requires a schema. Validation is not a step a call site can omit, so such a file is covered because of how it was handled rather than because somebody remembered. | [A] |
| FR-1.9a | FR-1.5's "as data" is the scope, and most of what a run touches is outside it. Captured stdout and stderr, the `exitcode` file, `.gitignore`, and every artifact a command wrote are read or written without a schema, because none of them has a structure bolt reasons about. A rule claiming every read would be false the moment a command produced anything. | [D] |
| FR-1.10 | YAML is written in canonical form: block style, one key to a line, and a scalar quoted exactly when it is meant to be a string, so its type is never in question. Booleans and numbers stay bare. `no`, `1.20` and `null` therefore survive a round trip as the strings they were, and `success` stays a boolean. Flow style is valid YAML and so is JSON, and neither is what bolt emits. Two results then differ by the lines that changed rather than by one long line. | [A] |
| FR-1.12 | Bolt reads and writes every structured file through wrench, so bolt is one consumer of that contract and not its owner. `Cargo.toml` names the Rust crate at `../wrench/rust`. What the contract is belongs to `wrench/REQUIREMENTS.md`, and bolt does not restate it. | [A/D] |
| FR-1.13 | Bolt validates with nothing else installed beside it. | [A] |

## 2. Invocation

| ID | Requirement | |
|---|---|---|
| FR-2.1 | A jig is run on a directory. That is the whole of what an invocation says: which jig, and where. | [A] |
| FR-2.1a | One jig, and one directory. Running several over one tree is a jig whose tasks invoke bolt, which FR-5.18 makes an ordinary command line and which gives each its own work directory and depth accounting. A second composition mechanism beside that one would buy nothing. | [D] |
| FR-2.2 | Bolt walks that directory to find the files its tasks act on. It has no changed-since-a-ref. | [A] |
| FR-2.2a | The walk honours `.gitignore`. An ignored file is not part of the project and is not checked, which keeps `.git`, `node_modules`, a virtualenv and build output out of every run without a second list to maintain. | [A] |
| FR-2.2b | Honouring `.gitignore` means reading those files as text. Bolt does not invoke git, read anything under `.git/`, or require a repository, so it does not reach `.git/info/exclude` or a global excludes file either. A tree with no `.gitignore` in it simply excludes nothing. | [A/D] |
| FR-2.2c | A run never walks its own output directory, whatever it was named and whatever `.gitignore` says, which is knowable because the run created it. Another run's output directory is not recognisable by name, so FR-2.6b refuses one that already holds a run rather than pretending the walk can spot it. | [D] |
| FR-2.2d | The walk returns paths in sorted order, so the matched list is the same list on every run over the same tree. FR-9.4's identical work directory names rest on this and on nothing else. | [D] |
| FR-2.2e | The walk does not follow symlinks. Following one leaves the base and breaks the containment FR-2.3 states, and `link-toolbox` leaves symlinks pointing into toolbox, so a project using shared jigs has them sitting in the tree being walked. | [D] |
| FR-2.2f | There is no way to reach a file `.gitignore` excludes. `matching` narrows what the walk found and the walk has already dropped ignored files. Generated code somebody wants checked is the case this refuses, and it stays refused until something needs it. | [D] |
| FR-2.3 | The directory is the run's base, and every input path a run acts on comes from inside it. Containment is how the input is formed, not a check applied to it afterwards. It is a claim about the walked input and about nothing else: the config directory is named separately by FR-2.8 and is routinely outside the base, `link-toolbox` places shared jigs that resolve into another repository, `{project_root}` reaches above the base by FR-4.1, and the output directory may be anywhere by FR-2.6. A row claiming everything a run touches is inside the base would make four settled rows violations of it. | [A/D] |
| FR-2.4 | Paths are resolved to absolute before anything runs. | [A] |
| FR-2.5 | A run refuses to start if the directory it was given is not there. | [A/D] |
| FR-2.5a | Refusing a missing directory takes the shape every refusal takes: a `result.yaml` carrying `success: false` and a reason naming the directory, then a non-zero exit. A caller parses one thing whatever went wrong. | [D] |
| FR-2.6 | `--output-dir` names the directory a run writes into. Given none, a run creates `.bolt-<iso8601>`. | [A] |
| FR-2.6a | `--output-dir` is created if it is not there, parents included. A graph node's `.ephemera/` may not exist yet, and making the caller create it first buys nothing. | [D] |
| FR-2.6b | A run refuses an output directory that already holds a run. Writing into one interleaves two runs' evidence, and FR-2.2c's exclusion cannot recognise a directory it did not name. Removing it is the caller's decision. | [D] |
| FR-2.6c | `.bolt-<iso8601>` is created at the run's base. Bolt reads no git, so there is no repository root to prefer, and the base is the one directory every invocation names. | [D] |
| FR-2.6d | The timestamp is filesystem-safe: hyphens where the strict form has colons, and the local offset spelled the same way. A directory name is a path on every platform, and the strict form's colons are legal here and hostile to a Windows checkout. | [D] |
| FR-2.6e | The default directory ends in the process id, after the timestamp. The stamp is second-granular, so without it two invocations that start in the same second resolve to one directory and FR-2.6b refuses the second, which fails or does not according to where the wall clock falls; an intermittent gate gets diagnosed several times before anyone reaches the stamp. One invocation is one process, so the id separates them and also says which run left a directory behind. | [D] |
| FR-2.7 | Bolt reads no git. A run over a tree that is not a repository behaves exactly as one over a tree that is. | [D] |
| FR-2.8 | A config directory argument says where `bolt.<name>.yaml` files are found, so where jigs live is told to bolt rather than inferred from the directory being run on. | [A] |
| FR-2.9 | A relative path is resolved against the base directory of the invocation it is written in. One rule covers filter patterns, a definitions value, an argument on a command line and anything else written as a path, so nobody has to remember which kind of path follows which convention. | [A] |

## 3. Jigs and tasks

| ID | Requirement | |
|---|---|---|
| FR-3.1 | A jig is the unit of configuration and composition. What bolt executes for a project is read from that project's jig. | [A] |
| FR-3.2 | A task declares a name, an optional description, `matching` and `excluding` lists, its adapter, its evidence files, `short-circuit-failure`, and a command written as a shell line. There is no runmode field. | [A] |
| FR-3.3 | A task's name prefixes its work directories, so a task's evidence is identifiable on disk without opening anything. | [A/D] |
| FR-3.3a | Task names are unique within a jig and a duplicate is a jig error. FR-3.3 makes the name the work directory prefix, so two tasks sharing one would put their executions in the same place. | [D] |
| FR-3.4 | `matching` is a condition on a task: a list of patterns or literal paths saying which files inside the run's directory that task acts on, where `**` matches zero or more directory levels. Every Python file through the formatter is one task with one pattern. A task never sees a path its condition rejects. | [A] |
| FR-3.4a | `excluding` is its counterpart, taking the same list of patterns or literal paths and removing from what `matching` selected. A task wanting everything but one shape of file says so directly instead of writing a pattern that means "not that", and a single known-bad file is named outright. | [A] |
| FR-3.4b | `matching` and `excluding` belong to a task that consumes paths. On a command naming neither path variable they are a jig error, caught in validation rather than quietly ignored. Whether a whole-project command should run at all is a question about where the jig is pointed, and FR-4.4 already answers it: a command naming neither variable always executes. | [A] |
| FR-3.4e | FR-4.4b's guarantee reaches only the tasks bolt selects for. A command handed a directory, whose tool finds its own files, is opaque: bolt cannot know whether it read a thousand files or none, so a tool that silently matched nothing reports a pass and bolt has nothing to notice. Where that matters, the task takes `matching` and a path variable so the selection is bolt's and FR-4.4b applies. | [D] |
| FR-3.4f | Tasks that pass a directory directly to their tool cannot record the selected paths promised by FR-9.5. This is a reason to move selection into the jig, not a reason against the rule. | [D] |
| FR-3.4c | The jig format carries comments, and an entry's reasoning sits beside it. Somebody asking why a path is excluded finds the answer where the path is, rather than reconstructing it from git history. | [A/D] |
| FR-3.4d | A jig is YAML, as an envelope is. One serialisation everywhere: one parser, one schema mechanism, and a jig and a result readable by the same tooling. | [A] |
| FR-3.5 | Filter patterns are relative to the base directory of the run they are declared in. A jig written for reuse therefore says `**/*.go` and never names the subtree it was dropped into, which is what makes it the same jig at the repository root and at `backend/`. | [A] |
| FR-3.6 | Organisation-wide, language-specific and repository-specific behaviour compose through jigs, with none of it hard-coded into bolt. | [A] |
| FR-3.7 | A jig maintained outside the repository and made available inside it, as toolbox's `link-toolbox` does, runs without being copied into the tree. | [D] |
| FR-3.8 | Bolt draws no line between a shared jig and a project-specific one. The same fields serve both, and every literal path or narrow pattern a jig carries trades reuse for fit. Where a jig sits on that scale is its author's choice and not a rule bolt enforces. | [A/D] |
| FR-3.9 | A jig file is `bolt.<name>.yaml`, so jig files are identifiable in a directory holding everything else a project keeps, and a jig is spoken of by its `<name>` rather than by a filename. | [A] |
| FR-3.10 | A jig declares `requires`, every executable it invokes: the tools its commands run, the adapters its tasks name, and any checker it calls. Nothing that jig reaches for directly is absent from the list, so it is that jig's whole inventory rather than a note about unusual tools. | [A] |
| FR-3.10e | `requires` is the jig's own inventory and not its children's. FR-5.1b keeps a parent from reading a child's content, so a jig composing another says `bolt` in its `requires` and stops, and what the child needs is the child's own list checked when the child runs. A parent that gathered them up would be reading inside a jig it is only supposed to invoke. | [D] |
| FR-3.10a | An adapter named by a task therefore appears in `requires` too, which is a consistency a checker can hold: an adapter no entry covers is found before a run instead of when the task reaches it. | [A/D] |
| FR-3.10b | `requires` is checked before any task executes and the run refuses, naming what is missing. An incomplete toolchain is known before half a gate has run rather than partway through it. | [D] |
| FR-3.10c | FR-4.10 still stands for a command that cannot start for any other reason. Checking up front is a guarantee about `requires`, not about every way a process fails to launch. | [D] |
| FR-3.10d | A project's own jig has no image built from it, so `requires` naming a tool the base image lacks is caught by FR-3.10b at the start of the run. Installing it is not bolt's to do. | [D] |
| FR-3.11 | A jig's `requires` is readable by things other than bolt, and nothing depends on bolt gathering anything up. What a consumer builds from that list is the consumer's business. | [A] |
| FR-3.12 | Bolt validates the jig it is handed and does not go looking for others. Every reachable jig being well-formed is a checker's job, run over the config directory as a task like any other, so a broken jig fails a gate instead of surfacing halfway through one, and nothing is left unvalidated: the jig bolt is given fails at once and the checker covers the ones bolt was never asked to read. | [A/D] |
| FR-3.14 | A jig's task set is fixed by the jig. No task is conditional on anything read at run time, so two runs of one jig show the same tasks and differ only where the tree differed. State cannot be relied on to be the same between runs, and a task set that varied with it would make two results incomparable without either of them saying so. FR-4.4's empty selection is not an exception: what a task matched is a property of the tree being run over, which is the run's input and is recorded in its manifest, where a condition read at run time is neither. | [A/D] |
| FR-3.14a | A task wanted in some directories and not others is a separate jig, listed by the jigs that want it and left out of the ones that do not. FR-5.18 already has one jig invoked by many tasks at different directories, so selecting per directory is what a project jig is for, and the selection is readable in the jig rather than decided during the run. | [A] |
| FR-3.15 | A jig may declare `definitions`, giving defaults to the placeholders it uses. The block is optional and so is any entry in it, so a jig leaving a value to its adopter names the placeholder in a command and defines nothing. | [D] |

## 4. Substitution and execution

| ID | Requirement | |
|---|---|---|
| FR-4.1 | Three locations are separately specifiable and separately available to every task: the project root, the base this run operates from, and the execution's own work directory. The outermost run is assumed to sit at the project root and a nested one is not, so a jig based on a subtree can still reach a config file at the root without giving up its base. | [A] |
| FR-4.1a | A command runs at the base directory. A tool has to stand where the jig's frame of reference is, or `./...` and a bare relative path mean something other than what the jig meant, and FR-3.5 already puts the patterns there. FR-5.14's declaration is the exception, standing a jig that needs the repository root at the repository root while its base stays what it was. The base is where a command stands and not the only place it can reach: the project root, the config directory and the work directory are all named to it, so needing one is not a reason to stand somewhere else. | [A/D] |
| FR-4.1c | Five locations are exposed as template variables: `{project_root}`, `{base_dir}`, `{work_dir}`, `{config_dir}` and `{output_dir}`. All five rather than the three a task acts within, so FR-9.5d's rule holds with no carve-out to remember. | [D] |
| FR-4.1d | Template variables are underscored and command-line flags are hyphenated, as a rule rather than as an accident. `{config_dir}` and `--config-dir` name one thing in the two shapes their contexts use. | [D] |
| FR-4.2 | How a task runs is read off its command, not declared beside it. `{each_path}` means one execution per matched path. `{all_paths}` means one execution with the whole selection substituted. Neither means one execution and no paths. A command naming both is a jig error. | [A] |
| FR-4.2a | There is no way to ask for one execution per path where the command does not name a path variable. FR-4.2 reads how a task runs off its command, so a command naming neither variable has said it runs once. Nothing needs the other thing yet. | [D] |
| FR-4.3 | Every path bolt substitutes is individually quoted, so a path carrying a space, a quote or a semicolon can neither split the command line nor inject into it. | [A] |
| FR-4.4 | A command task whose command names `{each_path}` or `{all_paths}` does not execute when its filtered selection is empty. A command task naming neither always executes. | [A] |
| FR-4.4b | An empty selection is a failure. The task does not execute, and bolt writes it a work directory, a manifest recording the patterns that matched nothing, and an envelope carrying `success: false` and a reason naming the task and its patterns. It is a constituent like any other, so FR-8.3 folds it into the run's verdict. | [D] |
| FR-4.4c | `optional` on a task says an empty selection is an acceptable result for it. The task does not execute and produces no constituent, which is what FR-4.4 alone used to mean for every task. A shared jig spanning languages declares it on the tasks that legitimately find nothing in a given project. | [D] |
| FR-4.4d | `optional` on a command task naming neither path variable is a jig error, caught in validation, for the reason FR-3.4b gives about `matching` and `excluding`: no selection exists, so the field says nothing. | [D] |
| FR-4.4h | Both refusals are enforced by the jig schema rather than by the runner, so a jig carrying either is rejected at validation before bolt reads a task. FR-1.5 already validates every jig on the way in, and a rule the schema can state is one the runner does not have to restate. | [D] |
| FR-4.4e | The default is failure because the alternative hides the common defect. A pattern that matches nothing is usually a typo or a moved directory, and under a silent skip it stays green forever. Declaring the exception costs one line in the jig that spans languages, written by whoever knew it spanned them, and it buys a check on every jig written by somebody who did not expect to match nothing. | [D] |
| FR-4.4f | The exit status is unaffected. Bolt carried the run out, so FR-10.2 applies and it exits 0 with `success: false` in the result. An empty selection is a finding about the jig or the project, not bolt failing to execute. | [D] |
| FR-4.4a | The empty-selection rule holds for every task, because every task is a command task by FR-5.18. A task composing bolt over a subdirectory that is not there matches nothing and fails by FR-4.4b, and says `optional` when a missing subproject is expected. There is no second kind of task with a second rule to remember. | [A/D] |
| FR-4.5 | Tasks execute serially, because one execution at a time is the simplest thing that works and not because anything requires it. FR-4.6 and FR-4.7 already give the independence parallelism would need, and FR-9.2a takes the ordinals from the matched list rather than from execution order, so nothing in the evidence layout depends on it either. | [A/D] |
| FR-4.6 | No task consumes another task's output. Work needing several steps is one script producing one exit code and one output. | [A] |
| FR-4.7 | Because no task depends on another, the merged result does not vary with the order tasks ran in. | [D] |
| FR-4.8 | A failing task does not stop the run. The tasks after it still execute, because a run that stops early throws away the evidence they would have produced and leaves a reader unable to tell what else was wrong. | [A] |
| FR-4.9 | A task may set `short-circuit-failure`, defaulting to false, to stop the run when it fails. Stopping is what a jig asks for rather than what it gets. | [A] |
| FR-4.10 | A command that cannot start at all fails its task with a reason, and the run carries on. A missing tool is a failing task rather than a refusal, and which kind of failure it was is what the reason carries. | [A] |
| FR-4.10a | The reason names the task and what the shell reported, not a `requires` entry. FR-3.10b resolves every declared entry before anything executes, so a declared tool cannot be the one that failed to start: the reachable case is a command invoking something the jig never declared, and there is no entry to name. | [D] |
| FR-4.10b | So an under-declared jig is visible only as a task that failed, which FR-3.10's inventory rule is what closes. Bolt does not read a command to work out what it invokes, and a checker holding a jig to its own `requires` is toolbox's rather than bolt's. | [D] |
| FR-4.11 | A time limit may be set for a task run and for the whole run. Both are options, so unset means a tool is allowed to finish. | [A] |
| FR-4.11c | The limit governs commands. An adapter runs outside it, because the adapter is what records that the limit fired, and a budget exhausted by the command it was killing would leave nothing to write the envelope FR-4.12d requires. | [D] |
| FR-4.11a | A task's limit covers all of that task's command invocations, taken together. Thirty seconds over four hundred paths is thirty seconds for the task, not for every path in turn. | [A] |
| FR-4.11b | Reaching it kills the execution in flight and the executions after it do not start. | [A/D] |
| FR-4.11d | A task's limit is `time-limit` on the task; the run's is `time-limit` on the jig. Neither is settable on the command line, because two places setting one value is a precedence question, and the only one bolt has agreed to answer is FR-4.16's layering of definitions. One place each, so there is no ordering to settle. | [D] |
| FR-4.11e | A limit is a decimal followed by `s`, `m` or `h`. One that is not refuses the run before any task executes, in FR-2.5a's shape, with a reason naming the task and what was written. Reading an unparseable limit as no limit is the alternative that fails silently: the run goes unbounded exactly where somebody asked for a ceiling, and the jig still looks like it has one. | [D] |
| FR-4.11f | A task's limit is wall clock from the moment the task starts. So the adapters between its executions spend it, even though FR-4.11c keeps the limit from killing one. A budget counting only the commands would let a task run for any length of wall clock behind slow adapters, which is the thing a ceiling exists to prevent. | [D] |
| FR-4.12 | A task exceeding its limit fails, with a reason saying the limit was passed. The run carries on, by FR-4.8, because a slow task is no more reason to discard the rest than a failing one. | [A] |
| FR-4.12a | A killed command keeps whatever output it managed to gather, and its adapter runs over that. A tool that reported forty problems before hanging reported forty real problems, and discarding them would throw away the only evidence the execution produced. | [A] |
| FR-4.12b | A task a limit killed fails regardless of what its adapter concluded, and its reasons carry at least the limit being passed. A partial run cannot report a pass, because what it did not reach is exactly what is unknown about it. | [A] |
| FR-4.12c | Where the limit catches the adapter rather than the command, bolt writes that envelope itself. Nothing else is left to write one, and the guarantee below has to hold whichever of the two was running. | [A/D] |
| FR-4.12d | A timed-out execution therefore has a valid envelope, which distinguishes it from one whose adapter died of its own accord and left none. Under FR-7.6 a timeout is an authoritative failure and a crash is no result at all. | [A/D] |
| FR-4.12e | A timed-out command is killed with `SIGKILL` to its process group, so the children it spawned go with it. Signalling the child alone leaves them running, writing into a work directory bolt has finished with and into the streams an adapter is about to read under FR-4.12a. There is no grace period and no `SIGTERM` first: the limit was the grace period, the command has had all of it, and a second countdown would make the declared number mean something other than what it says. | [D] |
| FR-4.12f | The reason on a timed-out execution says how many of its task's executions were not attempted. A per-path task cut off at path fifty leaves fifty work directories, and nothing else on disk says the other three hundred and fifty were never tried, so a reader can see the run failed and not how much of it went unchecked. The count is known before the first execution, because FR-9.4's matched list is settled then. | [D] |
| FR-4.13 | A run exceeding its limit fails, with a reason saying the limit was passed. | [A] |
| FR-4.14 | A run that times out still writes its result, carrying what completed before the limit. Bolt is alive and in control when the limit passes, so the rule is the one FR-5.8 already sets for a refusal: only a bolt that dies leaves nothing behind. | [A/D] |
| FR-4.14a | The merge runs over the constituents that completed, and the run's own reason sits with theirs in the result. So a timed-out run reports both what it managed to check and why the rest is missing. A result carrying only the timeout would discard evidence that was already written and paid for. FR-8.3a's refusal does not apply to a run that timed out before anything finished: that row exists to stop a green result over zero checks, and this result is not green. | [D] |
| FR-4.15 | A task command runs as a subprocess. The captured streams and the exit status FR-9.2 records come from the process boundary rather than from bookkeeping bolt would otherwise keep, and FR-4.5's serial execution removes what would have argued for running anything in process. | [D] |
| FR-4.16 | Substitution resolves against one mapping, built in three layers, each winning over the one before it: bolt's own values, then the jig's `definitions` block, then the definitions file named on the invocation. Every key in the result is a template variable, so a value a jig defined and a location bolt exposed are written and read the same way. | [D] |
| FR-4.16d | Bolt's layer is the exception to that ordering. The locations and path variables are reserved by FR-4.19 rather than overridable, so nothing above them can win and the precedence rule only ever settles a key two files both set. | [D] |
| FR-4.16a | `--definitions <name>` names one, read from the config directory as `bolt.<name>.definitions.yaml`. A jig is `bolt.<name>.yaml` in the same place by FR-3.9, so a definitions file is adopted, linked and spoken of exactly as a jig is, and `link-toolbox` can distribute a shared one. | [D] |
| FR-4.16b | At most one definitions file to an invocation, and none is ordinary: a jig whose defaults cover its placeholders runs without one. A project keeps several by naming them for what they scope, `bolt.python-override.definitions.yaml` beside `bolt.go-override.definitions.yaml`, and each run names the one it wants. There is no ordering to settle, because there is never more than one file over the jig's defaults. | [D] |
| FR-4.16c | A definitions mapping is `{key: value}`: one level, scalar values, the same shape whether it is a jig's block or a file. A filename carries the scope nesting would have expressed, and a list has no defined spelling on a command line. | [D] |
| FR-4.17 | The merge is a successive replacement by key. Each layer adds the keys the layers below did not have, replaces the values of those they did, and leaves every key it does not name standing. Nothing is deep-merged, appended to or combined, so a project overriding one detail writes that one line and inherits everything else the jig shipped. | [D] |
| FR-4.17a | A value is a literal. A definitions file carries no substitutions of its own, so reading one settles every value it holds and nothing resolves in terms of anything else. | [D] |
| FR-4.17b | A value written as a relative path resolves against the base, and it does so because FR-4.1a stands the command there rather than because bolt rewrote it. Nothing distinguishes `../REQUIREMENTS.md` from `100`, so FR-2.4's resolution to absolute cannot reach a definition and does not try. `requirements: ../REQUIREMENTS.md` therefore reaches the repository root from a run based at `go/`, and one contract serves both packs from one file. | [D] |
| FR-4.17c | A definition cannot introduce `{each_path}` or `{all_paths}`, because a value is a literal. FR-4.2 therefore still reads how a task runs off the command as written: substitution changes what a command says, never how many times it runs. | [D] |
| FR-4.18 | A placeholder no layer supplies refuses the run before anything executes, in the shape FR-2.5a states, with a reason naming the placeholder. Substituting empty is the reading that fails silently, leaving a command line short an argument and a tool reporting something else. | [D] |
| FR-4.18a | A placeholder is checked when `requires` is, under FR-3.10b, so a jig run where nothing defines what it needs refuses in the first second rather than partway through a gate. | [D] |
| FR-4.18b | A definition holding an empty value is defined. FR-4.18 refuses a placeholder no layer holds at all, which is a different state from a layer holding the empty string, and a jig wanting a flag to carry nothing says so by defining it rather than by leaving it out. | [D] |
| FR-4.19 | The location and path variables are reserved. A jig's `definitions` block or a definitions file naming one refuses the run, because `{base_dir}` redefined would substitute something other than where FR-4.1a stands the command, and the jig would say one thing while the process did another. | [D] |
| FR-4.20 | A definitions file is schema-validated under FR-1.5 like everything else bolt reads as data. One that will not parse or will not validate refuses the run, and is not taken for an absent file. | [D] |

## 5. Composition, and the depth it runs at

| ID | Requirement | |
|---|---|---|
| FR-5.18 | Bolt composes with itself as a command, and in no other way. A jig wanting another jig run over a subdirectory writes `bolt` on a command line, as it writes any other tool. There is no task kind for composition, no field set configuring it, and nothing in the runner that knows one command is bolt and another is not. | [D] |
| FR-5.19 | A child's verdict reaches its parent through the adapter contract. Bolt prints where its result is, by FR-10.3, so an adapter reading that path off stdout is the ordinary case of an adapter reading an execution's output, and the envelope it writes folds into the parent by FR-8.3 like any other constituent. | [D] |
| FR-5.20 | A child's evidence goes where its command puts it. `--output-dir {work_dir}/<name>` places the child's tree inside the parent's work directory, and that is a line in a jig rather than a rule in the runner. | [D] |
| FR-5.21 | Containment under composition is the command's own. A parent grants nothing and narrows nothing, so there is no grant for a child to widen past, and a jig composing bolt is exactly as trusted as a jig running any other command. This is what was traded away for FR-5.13's schema-checkable fields, and it is why FR-5.7's ceiling is the guard that remains. | [D] |
| FR-5.22 | A task carrying a `jig` field is a jig error, refused by name. The field is retired, and the refusal says what replaced it, because the alternative message is serde's `missing field command`, which reads as a malformed task and invites somebody to add a command to one that meant to name a jig. | [D] |
| FR-5.1a | A child run is not a mode. It is the same binary invoked the same way, so a jig run over a subdirectory by a parent's command line and that jig run over the same directory by a person are one operation. There is one code path because there was never a second one. | [A] |
| FR-5.1b | A parent knows the command line it wrote and nothing about what is inside the jig that command names. The child follows its own process when invoked: its own `requires`, its own tasks, its own filtering. Nothing rolls up and no parent reads a child's content. | [A] |
| FR-5.6 | Bolt carries its depth in the environment of every process it spawns, and increments it on finding the variable already set. The depth therefore survives reparenting, backgrounding and any number of shells between one bolt and the next, which is what FR-5.18 rests on: composition is a command line, so the environment is the only thing carrying the count. | [A/D] |
| FR-5.6a | The variables are `BOLT_DEPTH` for the current depth and `BOLT_MAX_DEPTH` for the ceiling. The names allow independently built bolt invocations to agree on the current depth. | [D] |
| FR-5.7 | The ceiling defaults to 4 and is read from the environment only at the outermost invocation, so a jig cannot raise the limit it is running under. | [A/D] |
| FR-5.7b | Bolt overwrites both variables on every process it spawns, which is the mechanism FR-5.7 describes rather than a separate check: a nested bolt reading them gets what the bolt above it set and never what a jig wrote. There is no branch on being outermost, and one would only matter for a command deliberately rewriting the variable, which FR-5.7a puts out of scope. | [D] |
| FR-5.7c | A value that will not parse is treated as absent, so the run reads as outermost rather than being refused. A caller's environment is not a document bolt was asked to validate, and refusing on stray shell state would fail runs while stopping nobody who meant it. | [D] |
| FR-5.7a | The ceiling is a guard against accident and runaway, not against a jig trying to defeat it. FR-5.18 makes every composition a task command invoking bolt, and such a command can unset the variable and be believed outermost. Closing that needs the ancestry cross-check, which is a question rather than a row. **It is now the only guard on composition**, by FR-5.21, so what it does not cover is worth knowing rather than reassuring. | [A/D] |
| FR-5.8 | A run refused for depth writes its own `result.yaml` with `success: false` and a reason naming the limit, then exits non-zero. Its parent's adapter reads that result like any other, and the merge folds an ordinary failure. | [A] |
| FR-5.9 | Paths are absolute at every depth, so a child's evidence folds into its parent with nothing rewritten. A path means the same thing to a child and to its parent. | [A/D] |

## 6. Adapters

| ID | Requirement | |
|---|---|---|
| FR-6.1 | An adapter is a separate process. It turns one task execution's captured output into a result envelope, and where an adapter reached an authoritative result that result is the verdict. Bolt does not second-guess one. | [A/D] |
| FR-6.1a | Bolt reaches a verdict itself only where no adapter's result is available to take, and each case says so where it arises: FR-6.9's generic exit-code adapter when a task names none, FR-6.11's adapter that produced nothing authoritative, FR-6.14's task that did not produce the evidence it declared, FR-4.12b's execution bolt terminated, and FR-5.8's run refused for depth. **This is the rule and not a count.** | [D] |
| FR-6.2 | The default adapter invocation names the captured files: `--stdout`, `--stderr`, `--evidence` and `--exitcode`. A task may write its adapter invocation explicitly in place of the default. | [A] |
| FR-6.2a | An adapter is handed the same three locations every task gets, the project root, the run's base and the execution's work directory. | [A] |
| FR-6.2c | A task declares its evidence files, and those are what `--evidence` names. Discovery would hand an adapter whatever a tool happened to leave behind, a lock file or a temporary or an intermediate, and let something irrelevant ruin a run. An artifact nobody declared still sits in the work directory as evidence on disk; it is simply not passed to the adapter. | [A] |
| FR-6.2b | An adapter writes `output.yaml` into that work directory. The path is the work directory it was given and the name never varies, so no flag says where the envelope goes and no task can put it somewhere else. | [A] |
| FR-6.2d | An explicit invocation gets the same substitutions a command gets, so it names the locations and the captures the same way. Two spellings of a substitution would make the jig format teach itself twice. | [D] |
| FR-6.2e | An explicit invocation is still expected to leave the envelope where the default would, because FR-6.2b's name never varies and no flag says where it goes. An invocation that writes elsewhere has written nothing, and is the same condition as an adapter that wrote nothing. | [D] |
| FR-6.3 | A child process's exit code reaches its adapter as a file. Bolt reaches no verdict of its own from it, and does not record it in the envelope either: whether that number explains anything is the adapter's judgement, not bolt's. | [A] |
| FR-6.4 | An adapter is chosen by the output format it reads. Any tool emitting a format some adapter understands reuses that adapter, whoever wrote the tool. | [A] |
| FR-6.6 | Fixing an adapter and re-folding a finished run costs no re-execution, because every input an adapter reads is already on disk. | [D] |
| FR-6.7 | The merge carries tests asserting that what it produces validates against the envelope schema. The guarantee is that a producer which would emit something invalid fails its own suite first. | [A] |
| FR-6.7a | FR-1.8 is not the backstop for an adapter's output, and reading it as one leaves the real gap unguarded. It checks what bolt writes, and an adapter is a separate process bolt does not write through. What catches a bad envelope from an adapter is validation on read, at the merge and at FR-6.11's check, which is why that check exists as a distinct case rather than as a consequence of FR-1.8. | [D] |
| FR-6.9 | A task naming no adapter gets the generic exit-code adapter, reporting success on a zero exit and failure otherwise. Every command has an exit status, so it is the one adapter that needs to know nothing about the tool it is reading. | [D] |
| FR-6.9a | The generic exit-code adapter does not run on a command a limit killed. The status of a killed command is bolt's own signal rather than an answer the tool gave, so there is nothing there to read, and FR-4.12b's reason is the verdict instead. Synthesising one anyway reports the kill twice, the second time as `exited -1`, which sends a reader looking for a decision the tool never made. A named adapter still runs, by FR-4.12a: it reads the output the tool did produce, which is a different thing from reading a status it did not choose. | [D] |
| FR-6.10 | An adapter is resolved by name from the config directory, where FR-2.8 already finds jigs. A jig and the adapters it names then travel together, so `link-toolbox` places both or neither. | [D] |
| FR-6.11 | An adapter that exits non-zero, writes no `output.yaml`, or writes one that will not parse has produced no authoritative result under FR-7.6. Bolt writes the envelope itself carrying `success: false`, and the reason says which of the three happened, because they have different causes. | [D] |
| FR-6.12 | Canonical form on `output.yaml` is the adapter's responsibility. An adapter using wrench gets it from `save_formatted_file` without doing anything, and bolt validates the envelope against its schema on the way in, at the merge. | [D] |
| FR-6.13 | Bolt does not check canonical form by reparsing and comparing. Comments do not survive a round trip, so that check fails every jig documenting itself under FR-3.4c, and byte comparison belongs in wrench's fixture suite where the emitter is what is under test. | [D] |
| FR-6.14 | A declared evidence file that was not produced fails its task, with a reason naming the path. A task declaring evidence it did not write did not do what it said, and FR-6.2c's refusal to discover means nothing else notices. | [D] |

## 7. Result envelopes

| ID | Requirement | |
|---|---|---|
| FR-7.1 | `success`, a boolean, is the only key every envelope carries. | [A] |
| FR-7.2 | `reasons` is present when `success` is false. Its members are objects whose shape is open, so whatever detail a producer holds can travel with the failure. | [A] |
| FR-7.3 | `metadata` is optional, and carries `statistics` and `evidence` where a producer has them. | [A] |
| FR-7.3a | Nothing puts the exit status into the envelope by default. It matters when the adapter says it matters, and then it goes into a reason, because a reason is where an adapter says what a result rests on. Leaving it out loses nothing: the raw value sits in the `exitcode` file either way, so a reader who wants it has it and a consumer is not handed a number nobody claimed was relevant. | [A/D] |
| FR-7.3c | Timings go in `metadata` and are not in the first version. Nothing therefore has to hand an adapter a clock it could not read for itself, and the adapter contract stays as it is. | [A] |
| FR-7.4 | Bolt's envelopes use the ecosystem's shared vocabulary. An envelope from a task, from a merge, from a task node or from azimuth is read the same way by the same consumer. | [A] |
| FR-7.5 | An envelope is written whole or not at all. A run killed partway leaves no half-written envelope for a consumer to read as authoritative. | [D] |
| FR-7.5a | Every file bolt or an adapter writes as a unit is written atomically, to a temporary and renamed into place, which is what makes FR-7.5 true rather than hoped for. A process killed mid-write leaves absence, and absence is a state FR-7.6 already knows how to read. | [A] |
| FR-7.5b | The temporary sits beside its target. A temporary somewhere else makes the move a copy across filesystems, which is not atomic and defeats the point. | [A/D] |
| FR-7.5c | Captured streams are the exception, because they are not written as a unit. FR-4.12a needs a killed command's partial output to survive, and output still arriving cannot be written atomically without discarding exactly what that row keeps. | [A/D] |
| FR-7.6 | Absent and invalid are different conditions. No `output.yaml` means no authoritative result has been reached. One that is present and fails validation is a failure. One that validates is authoritative. | [A] |
| FR-7.7 | Producing a valid envelope means a well-formed YAML file carrying `success` as a boolean, and when `success` is false, `reasons` as a list of objects each carrying `message` and `kind`. Nothing further is required of any producer, inside bolt or outside it. | [A/D] |
| FR-7.8 | A reason object carries `message`, a string, always. One consumer can then render every reason it meets, while the rest of the object stays open to whatever a producer wants to add. | [D] |
| FR-7.9 | A reason object also carries `kind`, naming what sort of thing it is. A consumer tells a missing tool from a tool that found problems without reading English, which is what FR-4.10 needs when it carries a missing binary in a reason rather than in a status. | [D] |
| FR-7.10 | A task that could not execute is therefore distinguishable in the merged result from one that executed and failed. The kind says which, and FR-8.4 carries reasons up. | [D] |

## 8. The merge

| ID | Requirement | |
|---|---|---|
| FR-8.1 | A run has exactly one result. The merge reads every `work/*/output.yaml` and folds them into one `result.yaml`, mechanically, and repeatably over a finished directory. | [A/D] |
| FR-8.2 | The merge rewrites `evidence` from a list of paths into a mapping keyed by task, each entry carrying that task's args and the filepath of its own result. | [A] |
| FR-8.2a | The merge takes each key from the work directory name, which FR-3.3 prefixes with the task, and the args from that execution's manifest, which FR-9.5c already records. Neither is read from the envelope, so an adapter never has to know what task it was run for and FR-6.2's contract stays as narrow as it is. | [A/D] |
| FR-8.3 | The merged result passes only when every constituent passes. There is no constituent whose failure does not count: a check nobody wants enforced is a check not in the jig. | [A/D] |
| FR-8.3a | A merge finding no constituent fails, with a reason saying no task produced a result. FR-8.3 on its own passes that run, because every constituent passing holds when there are none, and a green result is read as checked and fine, which over zero checks it is not. | [A] |
| FR-8.4 | The merged result carries the reasons, statistics and evidence references its constituents produced, so what failed and why is readable from the merged file alone. | [A/D] |
| FR-8.5 | Constituent envelopes survive the merge. Both levels stay on disk. | [D] |
| FR-8.6 | Only the outermost invocation relativises. Preparing the final result, a bolt that finds no depth set in its environment rewrites the output and evidence references going into `result.yaml` as relative to the project directory; a nested run leaves them absolute. No root has to be propagated for this, because the only bolt needing one is the bolt doing the conversion. | [A/D] |
| FR-8.7 | Rewriting reaches the structured path references and stops there. Text a tool emitted, carried up inside a reason, stays as the tool wrote it and may still name an absolute path. | [A/D] |
| FR-8.8 | `args` in the merged mapping is the argv as executed, after substitution. FR-9.5c records exactly that and FR-8.2a reads it from there, so the merged file says what ran rather than what was written. | [D] |
| FR-8.9 | `result.yaml` records the base the run was pointed at. It is the first thing a reader asks of a result, and FR-9.5c's per-execution manifests answer it only for somebody already inside the run directory. | [D] |

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
| FR-9.1a | A run directory lives while its result is being reviewed and is not wanted afterwards. Nothing outside it may depend on it surviving, and `result.yaml` carries whatever has to. So a reason or a metadata entry references an artifact by its path inside the run directory rather than copying it out: a reader still holding the directory can open the file the tool wrote, and a reader holding only the result has what the result carries, which is what it is for. | [A/D] |
| FR-9.2 | Each task execution gets its own directory holding the command as executed, captured stdout and stderr, the exit code as a file, whatever artifacts the command wrote there, and the adapter's `output.yaml`. | [A] |
| FR-9.2a | The ordinal is the execution index within the task. Each task numbers its own executions from one, independently of every other task, so a directory name says which task and which of its executions without needing the run's order. For a per-path task the index is the position in the matched list, which FR-9.5's manifest records, so an execution traces back to the path it was handed. | [A] |
| FR-9.2b | The ordinal is zero-padded to the width that task's execution count needs, so a listing sorts correctly with no arbitrary cap and no wasted digits. The count is known before the first execution, because the matched list is settled before any of it runs. | [A] |
| FR-9.2c | Bolt puts no artifact there. A command stands at the base under FR-4.1a and writes into the work directory because FR-4.1 named it one, so an artifact arrives by being addressed. One written elsewhere is not in the run's evidence, and going to look for it is the discovery FR-6.2c refuses. | [D] |
| FR-9.2d | A tool with no output-path flag therefore writes into the tree being checked, not into the work directory, and nothing in the run removes it. Addressing the work directory is the jig author's job, and a tool that cannot be told where to write is one a jig wraps in a script that can. | [D] |
| FR-9.3 | One execution's evidence is complete inside one directory. A reader needs nothing outside it to see what ran and what happened. | [D] |
| FR-9.4 | Serial execution makes the ordinals deterministic, so two runs over the same tree produce identical work directory names and the two trees line up file for file. Whether their contents match is a separate matter: a task envelope and a nested run's result carry absolute paths, so a run directory named after its own timestamp turns up inside them and two such runs differ wherever a path is recorded. Point both runs at a stable output directory and they do not. | [D] |
| FR-9.4b | The outermost `result.yaml` is the exception, because FR-8.6 relativises its structured path references against the base. Two runs of one jig over one tree therefore agree on the result even when their evidence does not, which is the file a consumer reads and the reason the exception is worth having. | [D] |
| FR-9.5 | An execution's manifest records which paths `matching` selected and which `excluding` removed, for a task that consumes paths. What that task saw, and what it was kept from seeing, sits on disk beside what it did. | [A] |
| FR-9.5a | A manifest is written before its command runs, so an execution that was killed, or that never got started, still records what was going to be attempted. The case that most needs a record is the one that would otherwise have none. | [A] |
| FR-9.5b | A manifest holds only what is known beforehand. Anything a run learns by finishing is not in the manifest, because the manifest was closed before there was anything to learn. | [A/D] |
| FR-9.5c | Every value bolt exposed as a template variable for that execution is in the manifest: the five locations FR-4.1c names, and whichever path variable applied. A reader sees what the task was given and not only what its command became, which matters where one path appears several times in a line or where a variable was available and went unused. It is a rule and not a list, so a variable added later is recorded because it is a variable and not because somebody remembered to add it. | [A/D] |
| FR-9.5e | The environment is not among what it holds. A dump of it carries whatever the shell was holding, into a file that exists to be handed around as evidence, and recording it safely means filtering it, which is not a first-version problem worth having. So an execution is not fully reconstructable from its evidence: what a tool read from its environment is not written down, and behaviour that turned on `PATH`, a locale or a tool's own configuration variable cannot be explained from the run directory. | [A/D] |
| FR-9.5g | The manifest records every key the three layers hold and which layer each resolved value came from. FR-9.5c already puts every value bolt exposed there; a run whose jig carries overrides also needs which file won, because the same key means different things depending on that and the command line alone does not say. Whether the layers are collapsed eagerly or consulted in turn is an implementation's to choose, and either has to be able to enumerate them for this. | [D] |
| FR-9.6 | A task naming no path variable was handed no list, so its manifest claims none. Recording one would say the command saw files it never received. | [A] |
| FR-9.7 | What such a task examined is the tool's own business, and bolt does not know it. A run's evidence covers what bolt handed over, never what a tool went and found for itself. | [A/D] |
| FR-9.8 | A per-path execution's manifest records the whole matched list, not only the path that execution was handed. FR-9.5 exists to preserve what the task was offered and what it was kept from seeing, and one path alone loses that. Repetition is the cost of every execution's evidence standing alone. | [D] |
| FR-9.9 | Every execution carries an ordinal, including a task that executes exactly once. One naming rule, so a work directory name parses without knowing how many executions there were. | [D] |

## 10. Exit status

| ID | Requirement | |
|---|---|---|
| FR-10.1 | Bolt's exit status answers one question: could bolt execute the requested task ETL? | [A] |
| FR-10.2 | A run in which every task executed and some tools reported failures exits 0 and writes `success: false`. That pairing is correct. | [A/D] |
| FR-10.3 | The authoritative quality verdict is the envelope. A caller reading bolt's exit status to learn whether the tools passed has read the wrong thing. | [A] |
| FR-10.3a | Bolt prints where the result is on stdout, and prints nothing else there. It does so whenever it wrote one, a refusal included, so stdout is unconditionally the answer to "where do I read the verdict" rather than a channel that goes quiet on the cases a caller most wants to parse. FR-5.19's adapter reads that line, and a refusal printing nothing would reach it as an empty stdout, indistinguishable from a bolt that died. | [D] |
| FR-10.4 | Bolt exits non-zero when it could not carry out the requested ETL. | [A] |
| FR-10.5 | Bolt exits 0 when the run completed, whatever the tools concluded, and 1 when it could not carry the run out: a jig that will not parse, an unknown adapter, an unwritable output directory, a depth ceiling passed, a directory that is not there. FR-10.3 keeps the quality verdict in the envelope. | [D] |
| FR-10.6 | A bolt killed by a signal exits 128 plus the signal number, which is the shell's convention and the one case where bolt does not choose its own status. | [D] |
| FR-10.7a | Two refusals cannot take that shape, and both are about the directory the result would go in. FR-2.5's: where the output directory sits inside a base that is not there, writing the result would create the base, which is what is being refused. FR-2.6b's: where the output directory already holds a run, writing the result would replace a completed run's verdict with a refusal that did not execute its tasks. Bolt says on stderr that it wrote none and why. Naming `--output-dir` outside the base gets a result as every other refusal does. | [D] |
| FR-10.7c | **The exemption is the rule and not a gap to close.** A bolt declining to start returns 1 and puts its reason on stderr, because it has nothing to report about a tree it did not read and the file it would write to is somebody else's. Settled 2026-08-29. **A later change making these two write a result would reintroduce the overwrite FR-10.7's note records**, so this row exists to be met by whoever tries. | [A/D] |
| FR-10.7b | So a caller that wants a parseable refusal for every case names an output directory outside the tree being checked. A graph node already does, by FR-2.6a's `.ephemera/`, which is why this is an edge rather than the ordinary case. | [D] |
| FR-10.7 | Bolt writes a `result.yaml` whenever it is alive and in control when it stops, a refusal and a timeout included. Only a bolt that died leaves none, so a caller finding no result knows the process was killed rather than that the run never started. | [D] |
| FR-10.8 | `--result-to-exitcode` makes bolt exit with what its envelope says rather than with whether it carried the run out. **Off unless named**, so FR-10.1's answer is what every caller that does not ask still gets, and nothing already written changes meaning. It is what makes a shell compose bolt: a Justfile recipe chaining two invocations cannot short-circuit on a failing gate while bolt exits 0 for a run it carried out, so `&&` reports the second call's ability to start rather than either call's verdict. The alternative is a wrapper reading the result path off stdout, which couples something outside bolt to bolt's output shape for a fact bolt already has. | [D] |
| FR-10.8b | **Two outcomes, and there is no third**: `0` where the envelope says success, `1` otherwise. `success` is a boolean, and wrench's `envelope.schema.json` calls it "the authoritative verdict", so nothing reads a neighbouring field to overrule it. The rule is `0 if success else 1` and it has no cases. | [D] |
| FR-10.8c | **A refusal is 1, because a refusal is a verdict bolt reached.** It writes `bolt-refused` with `success: false`, and that `false` is the answer rather than a placeholder for one. This is a row because the opposite is the tempting implementation: reading `kind` to promote a refusal to "no verdict" second-guesses an authoritative field by its neighbour, which is the drift wrench exists to prevent. | [D] |
| FR-10.8d | **There is no "could not determine" to represent.** A task set always resolves: a task that matched nothing and was declared optional is satisfied, and a required one that never ran has failed. Neither is an absent verdict, so a path reaching this flag with no envelope in hand is a defect to close rather than an outcome to allocate a code for. FR-4.4c's field is what decides which of the two an empty task is, which makes its naming and this flag one question asked twice. | [D] |
| FR-10.8e | The flag changes one number and nothing else. Not how `success` is decided, not the envelope, not what is written, and not FR-10.6's signal status. A caller wanting both readings gets them from the same run: the exit code and the `result.yaml` the run still writes and still names on stdout. | [D] |
| FR-10.9 | **A refusal names what sort of thing it is in its reason's `kind`, and each sort has its own.** A base that is not there, a jig that will not parse, a task carrying a retired field and a tool missing from `PATH` are four situations with four different fixes, so one kind across all of them cannot say which applies. This is where the discrimination lives, rather than in the exit status, because the exit status has the verdict to carry and a consumer reads the envelope anyway. | [D] |
| FR-10.9a | **The vocabulary is bolt's and not the schema's.** wrench's envelope schema takes any non-empty string and says why it does not enumerate them: a closed list would make a schema change the price of a new kind of failure. So a refusal added here adds a name here and nothing anywhere else. | [A/D] |
| FR-10.9b | Every refusal names its own kind and none defaults, so one added without deciding does not compile. A new refusal inheriting a neighbour's kind is how a consumer's match silently starts being wrong, which is the same failure FR-10.9 is fixing and would reintroduce it one variant at a time. | [D] |
| FR-10.9c | **A reused output directory is the one refusal with no kind to read**, because FR-2.6b returns before anything is written and FR-10.7a keeps it that way: the directory holds a completed run, and a refusal written there replaces a verdict. **A caller testing whether a `result.yaml` exists therefore gets a true answer about the wrong run.** For that one case the exit status and stderr are the whole of what bolt says, which is the cost FR-10.7b's advice exists to let a caller avoid. | [D] |

## 11. Where a run happens

| ID | Requirement | |
|---|---|---|
| FR-11.1 | A run needs nothing beyond the jig it was named, the directory it was pointed at, and what the walk finds inside it. Control-plane state is absent from a worker sandbox, so a run depending on it could not execute there. Jig singular and directory rather than paths, because FR-2.1a settled one jig and one directory and FR-2.2 has bolt walk that directory rather than be handed a list. A row written in the plural would describe a second interface nobody built. | [D] |
| FR-11.2 | A run changes no graph state, no task state and no other control-plane record. That is what §4 was about, and it is what makes a run safe to repeat and safe to throw away. It is not a claim that the tree is untouched: FR-9.2d already admits the opposite, since a tool with no output-path flag writes into the tree being checked and nothing in the run removes it, so a formatter run without `--check` rewrites the source. What bolt writes deliberately is the output directory; what a tool writes is the tool's. | [D] |
| FR-11.3 | The same jig runs against whatever tree state it is pointed at, including a throwaway copy prepared to test a prospective merge. | [D] |

## 12. The program

| ID | Requirement | |
|---|---|---|
| NFR-12.1 | Bolt runs itself. Its own quality gate is a bolt run over its own repository. | [A] |
| NFR-12.2 | Bolt installs into a standardised development image beside a toolchain it knows nothing about. | [D] |
| NFR-12.3 | Bolt is Apache-2.0 licensed, with a `NOTICE` naming the copyright holder. Every manifest that declares a licence declares the same one, so the machine-readable answer and the file agree. Was MIT when this row was written; the licence changed at `ada063b` and `Cargo.toml` went on saying MIT until 2026-08-28, which is the case the row now covers rather than the one it started as. | [A] |
| NFR-12.4 | Bolt builds without a C toolchain, so a cross-build needs no target compiler. Nothing in the dependency tree compiles C and `libc` is declarations only. Anything bolt links against inherits that constraint. The binary is dynamically linked against the system `libc`, `libm` and `libgcc_s`; a single-file image would need a musl target, and nothing requires one. | [D] |

## 13. Open

Rows that could not be stated until somebody decided them. A row keeps its id
once answered rather than moving to the section its subject belongs to, because
an id is never reused and never renumbered, so the section records where a
requirement came from rather than what it is about.

An answered row is settled and carries no test yet, so it reads as uncovered
until one is written. That is the honest reading and is not a reason to leave it
`[?]`.

The questions that would settle any remaining row are in `NEXT_STEPS.md`.

| ID | Requirement | |
|---|---|---|
| FR-13.6 | A run directory older than **seven days** is removed, and the cleanup is automatic, so a dogfooding repository does not accumulate them without bound. | [A] |
| FR-13.7 | An execution's manifest records the **commit SHA its source materials came from**, so evidence can be tied to the tree state that produced it, as §65 requires. Bolt reads no git and does not acquire that dependency: the SHA reaches it from the caller, through the definitions layer that already records a `from:` provenance for every key. | [A] |

## Retired

**A new row does not go here. Add it to its numbered section above.** Everything
after this heading is read as retired until the next `##`, and this is the last
section, so a row appended to the end of the file is silently retired: the id
stops being live, every reference to it starts meaning something else, and the
row looks perfectly normal. Appending is what a person does when adding a
requirement, which is what makes this worth a warning rather than a note.

The checker fails an id that is both live and retired, so the collision is
caught. **A row that is only retired is not a collision**, and nothing objects
until something cites it. A row
appended here and cited by no test passes the gate with the id absent from the
output entirely; the moment a test cites it, the gate fails naming the test and
prints the row's own text back, which for an accidental retirement is the
superseded-by cell reading as nonsense.

**So the window is exactly the gap between writing a requirement and writing
its test.** Written together there is no window, which is the argument the chain
already makes for writing them together.

A requirement can be retired or superseded. **Its ID is never reused**, because
reuse silently rewrites what every existing reference to that ID meant and
nothing about the new row looks wrong. A reader meeting one of these in an old
commit, a note or another project's document finds where it went here.

Numbering therefore has gaps, and a gap is the record working rather than an
oversight.

| ID | Retired | Superseded by |
|---|---|---|
| FR-1.11 | 2026-08-26 | `wrench/REQUIREMENTS.md` FR-1.1. It stated what wrench is, which bolt cannot discharge. |
| FR-1.14 | 2026-08-26 | `wrench` FR-2.1. The two calls are wrench's contract; FR-1.12 keeps bolt's use of it. |
| FR-1.15 | 2026-08-26 | `wrench` FR-2.5, the codec and IO split. |
| FR-1.15a | 2026-08-26 | `wrench` FR-2.5a, the IO boundary outside the call. |
| FR-1.16 | 2026-08-26 | `wrench` FR-3.1, one schema per kind of file. |
| FR-1.17 | 2026-08-26 | `wrench` FR-3.3, JSON Schema over the decoded structure. |
| FR-1.18 | 2026-08-26 | `wrench` FR-4.3, canonical form belonging to the save call. |
| FR-1.19 | 2026-08-26 | `wrench` FR-2.3, a compelled schema and not the right one. |
| FR-6.5 | 2026-08-26 | Raised with toolbox, which owns it. Adapters reading structured formats is toolbox's, not bolt's. |
| FR-6.8 | 2026-08-26 | The same entry. FR-6.7 kept the merge's half, which is bolt's own producer. |
| FR-13.1 | 2026-08-26 | FR-6.2b. An adapter writes `output.yaml` into the work directory it was given and the name never varies. |
| FR-13.2 | 2026-08-26 | FR-7.9 and FR-7.10. A reason carries `kind`, so a task that could not execute is distinguishable from one that executed and failed. |
| FR-13.3 | 2026-08-24 | FR-8.3a. A merge finding no constituent fails, so a green result cannot mean nothing was checked. |
| FR-13.8 | 2026-08-31 | Nothing replaces it. The bound was asked for conditionally, "if that guard is wanted at all", and the answer is that it is not wanted yet: no crazy nesting depth has been seen, and a runtime that is not parallel surfaces excess depth without needing a large bound. A cap on concurrent bolt processes per user may be wanted later, and takes a new id when it is. |
| FR-13.5 | 2026-08-26 | FR-4.12 and its sub-rows, which give a timed-out task a defined outcome at length. |
| FR-13.9 | 2026-08-26 | FR-3.4d, FR-1.5 and FR-3.12: a jig is YAML, every file is schema-validated, and bolt validates the jig it is handed. |
| FR-13.4 | 2026-08-27 | FR-8.3. It asked for a stated default for whether a constituent is required, and FR-8.3 answers by removing the question: every constituent counts, and a check nobody wants enforced is a check not in the jig. |
| FR-13.10 | 2026-08-27 | FR-1.12 and wrench's own requirements. The envelope schema is owned and shipped by wrench, embedded so a consumer names it rather than carrying a copy. It was open only while wrench did not exist. |
| FR-5.1 | 2026-08-28 | FR-5.18. A task never names a jig; it names `bolt` on a command line, like any other tool. |
| FR-5.2 | 2026-08-28 | FR-5.20. The child's tree lands inside the parent's work directory because the command says `--output-dir {work_dir}/…`, not because the runner places it, and FR-5.19's adapter writes the `output.yaml` a symlink used to be. |
| FR-5.3 | 2026-08-28 | FR-5.18. A composing task *is* a command task, so carrying the same bookkeeping is not a property to state. |
| FR-5.4 | 2026-08-28 | FR-5.18, for the same reason: there is no constituent kind for the merge to be ignorant of. |
| FR-5.5 | 2026-08-28 | FR-5.18. A composing task takes `matching` and `excluding` exactly as any command task may, and whether it wants them is its author's business. |
| FR-5.10 | 2026-08-28 | FR-5.18. The subdirectory is an argument on bolt's command line, where FR-2.1's directory argument already defines it. |
| FR-5.10a | 2026-08-28 | FR-5.18. There is no `in` field to name. |
| FR-5.11 | 2026-08-28 | FR-5.18. A command line carries a written path already, and a pattern was never expressible there. |
| FR-5.12 | 2026-08-28 | FR-5.18. Nine subprojects are nine command tasks, and FR-3.3a already makes the work directory prefix the task's name. |
| FR-5.13 | 2026-08-28 | FR-5.21. There is no parent grant, so narrowing is not an act bolt performs and cannot be undone by half. |
| FR-5.13a | 2026-08-28 | FR-5.18. Nothing is declared as fields and nothing is inherited; a command line says what it says. |
| FR-5.13b | 2026-08-28 | FR-5.18. `--config-dir` on the command line, which FR-2.8 already defines. |
| FR-5.13c | 2026-08-28 | FR-5.20. `--output-dir`, which FR-2.6 already defines. |
| FR-5.13d | 2026-08-28 | FR-5.21. The directory a child runs on is bolt's own second argument. |
| FR-5.13e | 2026-08-28 | FR-5.7 and FR-5.7b, unchanged. The ceiling is propagated and never declared, which is now true of a command task by the same mechanism. |
| FR-5.13f | 2026-08-28 | FR-4.3. Substitution in a command line is where it always was, and a composing task's command is a command. |
| FR-5.13g | 2026-08-28 | FR-2.9 and FR-4.3, which carry the rule without a field set to except from it. |
| FR-5.13h | 2026-08-28 | FR-4.2 and FR-3.4b. A composing task has a command, so the path variables mean what they mean everywhere else. |
| FR-5.13i | 2026-08-28 | FR-5.21, which states the loss rather than replacing it. Composition is a command line, so it is not schema-checkable, and the ceiling is the guard that remains. |
| FR-5.13j | 2026-08-28 | FR-5.18. `--definitions` on the command line, which FR-4.16 already defines. |
| FR-5.15 | 2026-08-28 | FR-4.4 and FR-4.4b. A composing task with no selection is a command task with no selection, and `optional` is how one says it may run anyway. |
| FR-5.15a | 2026-08-28 | FR-4.4b by the same route. A subdirectory that is not there matches nothing, and FR-4.4c's `optional` is the declaration that this is expected. |
| FR-5.16 | 2026-08-28 | FR-4.2, unchanged. A command naming neither path variable executes once, so nothing special is claimed for composition. |
| FR-5.17 | 2026-08-28 | FR-5.18. Six subprojects are six command lines each naming `--definitions`, which is more typing and says which file it means rather than deciding it by omission. |
| FR-2.9a | 2026-08-28 | FR-2.9, which keeps the rule. There are no jig-task fields for the exception to apply to. |
| FR-4.4g | 2026-08-28 | FR-4.4c. A composing task has a selection like any command task, so `optional` means what it means everywhere and is not an error. |
| FR-5.14 | 2026-08-29 | Nothing. Composition is a command line, so every invocation determines `{project_root}` from the directory it was given. |
| FR-5.14a | 2026-08-29 | Nothing. `needs-repository-root` has nothing left to declare, by the row above. |
| FR-5.14b | 2026-08-29 | Nothing. It said the base is not overridden by that declaration, and there is no declaration. FR-2.3's containment is unchanged and was never reached through this row. |
| FR-5.14c | 2026-08-29 | FR-5.21, which carries the measurement. The containment escape it recorded cannot be expressed once a parent grants nothing, which is why FR-5.13 retired for the same reason. |
| FR-5.14d | 2026-08-29 | Nothing. A command stands at the base by FR-4.1a, and there is no exception left for this row to describe. |
| FR-3.13 | 2026-08-30 | FR-3.12, which absorbed it. |
| FR-9.5d | 2026-08-30 | FR-9.5c, which absorbed it. |
| FR-7.3b | 2026-08-30 | FR-7.3a, which absorbed it. |
| FR-4.1b | 2026-08-30 | FR-4.1a, which absorbed it. |
| FR-11.2a | 2026-08-30 | FR-11.2, which absorbed it. |
| FR-4.5a | 2026-08-30 | FR-4.5, which absorbed it. |
| FR-9.4a | 2026-08-30 | FR-9.4, which absorbed it. |
| FR-2.3a | 2026-08-30 | FR-2.3, which absorbed it. |
| FR-9.5f | 2026-08-30 | FR-9.5e, which absorbed it. |
| FR-9.1b | 2026-08-30 | FR-9.1a, which absorbed it. |
| FR-11.1a | 2026-08-30 | FR-11.1, which absorbed it. |
| FR-3.14b | 2026-08-30 | FR-3.14, which absorbed it. |
| FR-10.8a | 2026-08-30 | FR-10.8, which absorbed it. |
| FR-6.1b | 2026-08-30 | Nothing. It recorded that FR-6.1a's count had been corrected twice, which is history git holds. |
| FR-11.2b | 2026-08-30 | Nothing. It narrated a superseded row and a test derived from it; the shape is in `docs/LESSONS/a-check-that-answers-a-weaker-question.md`. |
