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

## 1. What a run is

| ID | Requirement | |
|---|---|---|
| FR-1.1 | A run executes the command lines its jigs declare and records what happened as files on disk. Nothing a consumer needs to know about the outcome exists only in bolt's own output streams. | [A/D] |
| FR-1.2 | Bolt holds no knowledge of any particular tool. Which commands run, and what their output means, come from the jig. Adding a language or a checker to the ecosystem changes a jig and an adapter, never bolt. | [A] |
| FR-1.3 | Task ETL is the abstraction and quality checking is its first use. A jig that runs no checker and reaches no verdict about code is a legitimate run. | [A/D] |
| FR-1.4 | A run captures each command's native results whatever form they take: stdout, stderr, exit code, and arbitrary files the command generated. Those results survive the run as evidence. | [A/D] |

## 2. Invocation

| ID | Requirement | |
|---|---|---|
| FR-2.1 | Bolt is given the jigs to run, and everything after them is the input file list. | [A] |
| FR-2.2 | Bolt does not choose the input set. It has no gitignore awareness and no changed-since-a-ref: a caller wanting either computes the list and passes it. What bolt does walk is a folder a caller named explicitly, to apply filters through it. | [A] |
| FR-2.3 | `--output-dir` names the directory a run writes into. Given none, a run creates `.bolt-<iso8601>`. | [A] |
| FR-2.4 | Bolt reads no git. A run over a tree that is not a repository behaves exactly as one over a tree that is. | [D] |
| FR-2.5 | An argument names the project directory a run is confined to. | [A] |
| FR-2.6 | Every input path is resolved to an absolute path before anything runs. | [A] |
| FR-2.7 | A run refuses to start if any input path lies outside the project directory or does not exist. Naming a path that is not there, or that has no business in the run, is a caller error and is not worked around silently. | [A] |
| FR-2.8 | An input path names a file or a folder. A filter globs through it either way: against the one file, or against the files a folder holds. | [A] |

## 3. Jigs and tasks

| ID | Requirement | |
|---|---|---|
| FR-3.1 | A jig is the unit of configuration and composition. What bolt executes for a project is read from that project's jig. | [A] |
| FR-3.2 | A task declares a name, an optional description, a runmode, a `matching:` glob, and a command written as a shell line. | [A] |
| FR-3.3 | A task's name prefixes its work directories, so a task's evidence is identifiable on disk without opening anything. | [A/D] |
| FR-3.4 | `matching:` names the subset of the run's input paths a task accepts, where `**` matches zero or more directory levels. A task never sees a path its filter rejects. | [A] |
| FR-3.5 | Filter patterns are relative to the base directory of the run they are declared in. A jig written for reuse therefore says `**/*.go` and never names the subtree it was dropped into, which is what makes it the same jig at the repository root and at `backend/`. | [A] |
| FR-3.6 | Organisation-wide, language-specific and repository-specific behaviour compose through jigs, with none of it hard-coded into bolt. | [A] |
| FR-3.7 | A jig maintained outside the repository and made available inside it, as toolbox's `link-jigs` does, runs without being copied into the tree. | [D] |

## 4. Substitution and execution

| ID | Requirement | |
|---|---|---|
| FR-4.1 | Three locations are separately specifiable and separately available to every task: the project root, the base this run operates from, and the execution's own work directory. The outermost run is assumed to sit at the project root and a nested one is not, so a jig based on a subtree can still reach a config file at the root without giving up its base. | [A] |
| FR-4.2 | `{input_paths}` is available in batch mode alone, and `{input_path}` in `perPath` mode alone. | [A] |
| FR-4.3 | Every path bolt substitutes is individually quoted, so a path carrying a space, a quote or a semicolon can neither split the command line nor inject into it. | [A] |
| FR-4.4 | A task whose command names `{input_path}` or `{input_paths}` does not execute when its filtered selection is empty, and produces no output. A task naming neither always executes. | [A] |
| FR-4.5 | Tasks execute serially. | [A] |
| FR-4.6 | No task consumes another task's output. Work needing several steps is one script producing one exit code and one output. | [A] |
| FR-4.7 | Because no task depends on another, the merged result does not vary with the order tasks ran in. | [D] |

## 5. Nested jigs

| ID | Requirement | |
|---|---|---|
| FR-5.1 | A task may name a jig in place of a command. | [A] |
| FR-5.2 | A nested run writes into its own subdirectory inside that task's work directory, and its `result.yaml` is linked as the task's `output.yaml` by a relative symlink, so the whole tree survives being moved or archived. | [A/D] |
| FR-5.3 | A jig task carries the same bookkeeping files as a command task, so nothing reading `work/*/` needs to know which kind it is looking at. | [A/D] |
| FR-5.4 | The merge does not know that a constituent was a nested run. Nesting is a special case at invocation and nowhere else. | [A/D] |
| FR-5.5 | A jig task carries no filter. The child receives the input paths lying under its base, and the child jig's own tasks filter from there against that base. Selecting files is the nested jig's business, not its caller's. | [A] |
| FR-5.6 | Bolt carries its nesting depth in the environment of every process it spawns, and increments it on finding the variable already set. The depth therefore survives reparenting, backgrounding and a task command that invokes bolt directly rather than through a jig task. | [A/D] |
| FR-5.7 | The ceiling defaults to 4 and is read from the environment only at the outermost invocation, so a jig cannot raise the limit it is running under. | [A/D] |
| FR-5.8 | A run refused for depth writes its own `result.yaml` with `success: false` and a reason naming the limit, then exits non-zero. Its parent's link resolves, and the merge folds an ordinary failure. | [A] |
| FR-5.9 | Paths are absolute at every depth, so a nested run's evidence folds into its parent with nothing rewritten. A path means the same thing to a child and to its parent. | [A/D] |
| FR-5.10 | A jig task names a jig and, optionally, a subdirectory of the current base to run it in. Naming one says something specific: a project of that jig's kind lives there, a Go module with its own `go.mod`, a Python package with its own `pyproject.toml`. The declaration is that applying this jig at this level is worth something, and that directory becomes the child's base. | [A] |
| FR-5.11 | Naming a subdirectory narrows the base and the containment check together while the project root stays what it was, so a jig distributed by toolbox drops in at any depth without being written to know where it was placed. | [A/D] |
| FR-5.12 | A jig that genuinely needs the repository root says so, and that overrides a subdirectory base. | [A] |
| FR-5.13 | A jig task with no input paths under its base does not run, by the same rule that stops a `perPath` task. A nested project nobody touched contributes nothing. | [A] |

## 6. Adapters

| ID | Requirement | |
|---|---|---|
| FR-6.1 | An adapter is a separate process. It turns one task execution's captured output into a result envelope, and nothing else in bolt decides whether that execution passed. | [A] |
| FR-6.2 | The default adapter invocation names the captured files: `--stdout`, `--stderr`, `--evidence` and `--exitcode`. A task may write its adapter invocation explicitly in place of the default. | [A] |
| FR-6.3 | A child process's exit code reaches its adapter as a file. Bolt reaches no verdict of its own from it. | [A] |
| FR-6.4 | An adapter is chosen by the output format it reads. Any tool emitting a format some adapter understands reuses that adapter, whoever wrote the tool. | [A] |
| FR-6.5 | Adapters read structured formats as well as exit codes: Cobertura, pytest JSON, and other structured test and coverage reports. | [A] |
| FR-6.6 | Fixing an adapter and re-folding a finished run costs no re-execution, because every input an adapter reads is already on disk. | [D] |

## 7. Result envelopes

| ID | Requirement | |
|---|---|---|
| FR-7.1 | `success`, a boolean, is the only key every envelope carries. | [A] |
| FR-7.2 | `reasons` is present when `success` is false. Its members are objects whose shape is open, so whatever detail a producer holds can travel with the failure. | [A] |
| FR-7.3 | `metadata` is optional, and carries `statistics` and `evidence` where a producer has them. | [A] |
| FR-7.4 | Bolt's envelopes use the ecosystem's shared vocabulary. An envelope from a task, from a merge, from a task node or from azimuth is read the same way by the same consumer. | [A] |
| FR-7.5 | An envelope is written whole or not at all. A run killed partway leaves no half-written envelope for a consumer to read as authoritative. | [D] |
| FR-7.6 | A task with no readable `output.yaml` has reached no authoritative result, which is a different condition from having failed. | [A/D] |

## 8. The merge

| ID | Requirement | |
|---|---|---|
| FR-8.1 | A run has exactly one result. The merge reads every `work/*/output.yaml` and folds them into one `result.yaml`, mechanically, and repeatably over a finished directory. | [A/D] |
| FR-8.2 | The merge rewrites `evidence` from a list of paths into a mapping keyed by task, each entry carrying that task's args and the filepath of its own result. | [A] |
| FR-8.3 | The merged result passes only when every required constituent passes. | [A] |
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
    <task>-####/
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
| FR-9.3 | One execution's evidence is complete inside one directory. A reader needs nothing outside it to see what ran and what happened. | [D] |
| FR-9.4 | Serial execution makes the ordinals deterministic, so two runs over the same input file list produce identical work directory names and two run directories diff cleanly. | [D] |

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

## 13. Open

Each row states a property that must eventually hold and cannot be stated yet.
The questions that would settle them are in `NEXT_STEPS.md`.

| ID | Requirement | |
|---|---|---|
| FR-13.1 | Walking a named folder does not drag in what nobody meant: `.git`, `node_modules`, a virtualenv, build output. Bolt has no gitignore awareness, so nothing currently keeps them out. | [?] |
| FR-13.2 | An adapter writes its envelope to a defined place, named in the contract that invokes it. | [?] |
| FR-13.3 | A task that could not execute is distinguishable in `result.yaml` from one that executed and failed. | [?] |
| FR-13.4 | A task skipped for an empty file list is distinguishable in `result.yaml` from one that was never declared, so a green result cannot mean that nothing was checked. | [?] |
| FR-13.5 | Whether a constituent is required is declared, with a stated default. | [?] |
| FR-13.6 | A task that exceeds a time budget reaches a defined outcome. | [?] |
| FR-13.7 | Run directories are removed on a stated rule, so a dogfooding repository does not accumulate them without bound. | [?] |
| FR-13.8 | Evidence can be tied to the exact tree state it was produced from, as §65 requires, by something. Bolt reads no git, so either it acquires that dependency or the requirement belongs to the caller. | [?] |
| FR-13.9 | The number of bolt runs a user may have live at once is bounded, if that guard is wanted at all. | [?] |
| FR-13.10 | The jig is written in a defined format, validated against a schema. | [?] |
| FR-13.11 | The envelope schema is owned and published somewhere every producer and consumer in the ecosystem can validate against. | [?] |
