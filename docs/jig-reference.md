# Jig reference

Every field bolt reads, every placeholder it substitutes, and the shape of what
a run writes. For what bolt is and how to run it once, see `README.md`. For
writing an adapter, see `PATTERNS/the-adapter-contract.md`.

## The command line

    bolt <jig> <directory> [--definitions <name>] [--output-dir <path>]
                           [--config-dir <path>] [--result-to-exitcode]

`<jig>` is a name and never a path. Bolt reads `bolt.<jig>.yaml` from the
config directory, which defaults to `<directory>`. A shared jig can therefore be
distributed and adopted without a project knowing where the file came from.

    --definitions <name>    also read bolt.<name>.definitions.yaml from the
                            config directory, as the outermost layer
    --output-dir <path>     write the run here instead of the default
    --config-dir <path>     look for the jig, its definitions and its adapters
                            here instead of in <directory>
    --result-to-exitcode    exit 0 when the run succeeded and 1 when it did not

Flags may be written before or after the positionals.

## Jig fields

| Field | Meaning |
|---|---|
| `tasks` | The tasks, in a list. The only required field. |
| `requires` | Every executable the jig's commands invoke. A run refuses before anything executes if one is not on `PATH`. |
| `time-limit` | How long the whole run may take. |
| `definitions` | A map of values the commands name as `{placeholder}`. |
| `version` | The jig format version. |

`requires` is an inventory of the whole jig rather than a note about the unusual
tools in it. Half a gate run on a machine missing a tool costs more than the
check that would have caught it, and an under-declared jig passes locally where
the tool happens to be installed.

## Task fields

| Field | Meaning |
|---|---|
| `name` | Required. Prefixes the task's work directories, so it has to be safe as a path component. |
| `command` | Required. The command line, carrying at most one of the two path forms. |
| `matching` | Patterns or literal paths selecting the files this task acts on, relative to the directory being run over. `**` matches zero or more directory levels. |
| `excluding` | Patterns or literal paths removed from what `matching` selected. |
| `optional` | Whether an empty selection is an acceptable result. False by default: a pattern matching nothing is usually a typo or a moved directory, and a silent skip stays green forever. |
| `short-circuit-failure` | Stop the run when this task fails. False by default. |
| `adapter` | The adapter that turns this task's output into a verdict, resolved by name from the config directory. Left out, the generic exit-code adapter runs. |
| `adapter-command` | An explicit adapter invocation in place of the default one. It gets the same substitutions a command gets, and it is still expected to leave the envelope where the default would. |
| `time-limit` | How long this task may take, covering all of its executions together. |
| `evidence` | Files this task produces that its adapter should read, relative to the work directory. Declared, never discovered. |

A task carrying the retired `jig` field is refused by name, with
`jig-task-retired`. Composition is a command line: a task that runs another jig
writes `bolt` in its command like any other tool.

The jig format is validated against wrench's `jig.schema.json`, which is the
authority on what a valid jig is. A field the schema accepts and this table does
not name is read by nothing here.

## Path forms

A command names at most one of two placeholders, and which one it names decides
how many times the task executes.

    {each_path}    one execution per matched file
    {all_paths}    one execution, with the whole selection substituted

Naming both is a jig error, `both-path-forms`. Naming neither runs the command
once with no selection substituted into it.

## Locations

Every command and every adapter invocation can name these.

| Placeholder | Value |
|---|---|
| `{work_dir}` | This execution's directory under `work/`. |
| `{base_dir}` | The directory bolt was pointed at. |
| `{project_root}` | The same directory as `{base_dir}`. |
| `{config_dir}` | Where the jig was found. |
| `{output_dir}` | The run directory. |

All five are absolute, and they are reserved: a jig or a definitions file naming
one is refused with `reserved-definition`, so nothing can shadow them.

`{project_root}` and `{base_dir}` always name the same directory. Composition is
a command line rather than a nesting, so every invocation is pointed at its own
base and none inherits another's.

A path that travels with the jig resolves against `{config_dir}`. A path
belonging to the project being checked stays relative to the base, where the
command already stands.

## Substitution

Every substituted path is quoted, and substitution is a single left-to-right
pass over the command. Bytes that have been substituted are never read again, so
a filename containing what looks like a placeholder is a filename.

Both halves are load-bearing. Chained replacement, one pass per variable,
re-expands a token appearing inside an already-substituted filename and breaks
the quoting: a file named ``p{all_paths};id #`` executed `id`.
`LESSONS/chained-substitution-is-a-command-injection.md` has the measurement.

A placeholder naming nothing is refused with `unknown-placeholder`. An unmatched
brace is literal text.

## Definitions

A definition is a value a command names as `{placeholder}`. Three layers
resolve, each overriding the one before it:

1. Bolt's own locations, which cannot be overridden.
2. The jig's `definitions` block.
3. `bolt.<name>.definitions.yaml`, named with `--definitions`.

A shared jig ships defaults in its own block and an adopter overrides one line
in a definitions file without forking the jig.

Values are scalars. A value carrying a space arrives as one argument, because it
is quoted the same way a location is.

## Time limits

A limit is a decimal and a unit: `30s`, `5m`, `1.5h`. The grammar is
`^[0-9]*\.?[0-9]+[smh]$`, so `.5s` is a limit and `5` and `5.s` are not. The
schema refuses anything else before any task executes.

A task's limit covers all of that task's executions taken together, measured as
wall clock from the moment the task starts. The jig's own limit covers the run.

Firing a limit kills the process group, so a command that spawned children does
not leave them writing into a directory bolt has finished with. The killed
command keeps whatever output it gathered and its adapter still runs over it,
and the execution fails regardless of what the adapter concluded.

## Depth

A run that composes another run passes its depth down in the environment.

    BOLT_DEPTH        this run's level
    BOLT_MAX_DEPTH    the ceiling, 4 unless it is set

A run past the ceiling is refused with `depth-exceeded`, checked before the jig
is read, so it opens no files and still writes a result for its parent to fold.

## What a run writes

```
.bolt-2026-08-30T03-34-55Z-1921009/
├── result.yaml
└── work/
    └── <task>-<ordinal>/
        ├── manifest.yaml
        ├── stdout
        ├── stderr
        ├── exitcode
        └── output.yaml
```

The default run directory carries the time and the process id. One invocation is
one process, so two runs starting in the same second get two directories.
`--output-dir` names one instead, and a run refuses a directory that already
holds a run.

Ordinals are the execution index within the task, numbered from one and
independently of every other task, zero-padded to the width that task's
execution count needs.

Every execution gets a directory whether it passed, failed, was killed at a time
limit, or never started. `manifest.yaml` is written before the command runs, so
an execution that was killed still records what it was going to attempt:

```yaml
"command": "lizard --CCN 15 '/home/you/project/src/run.rs'"
"ordinal": 1
"selection":
  "excluded": []
  "matched":
    - "/home/you/project/src/run.rs"
"task": "complexity"
"variables":
  "base_dir":
    "from": "bolt"
    "value": "/home/you/project"
```

`variables` records every value substituted into the command and which layer
supplied it, so a wrong value can be traced to the file that set it.

`output.yaml` is one execution's verdict, written by its adapter.
`result.yaml` is the run's one verdict, folded from all of them, and carries the
evidence index:

```yaml
"metadata":
  "base": "/home/you/project"
  "evidence":
    "complexity-1":
      "args": "lizard --CCN 15 '/home/you/project/src/run.rs'"
      "result": "/home/you/project/.bolt-.../work/complexity-1/output.yaml"
"reasons":
  - "kind": "nonzero-exit"
    "message": "complexity exited 1"
"success": false
```

Both files are envelopes in the same shape: `success`, and `reasons` carrying a
`kind` and a `message` when it is false. A consumer reads one format whatever
produced it.

## Reason kinds

Two vocabularies, and they are disjoint. Which one a `kind` comes from says
whether bolt could not run at all or ran and reached a verdict.

Bolt refused. A closed set of sixteen, and the run did no work:

    base-missing            duplicate-task-name    no-constituents
    both-path-forms         io-failed              output-directory-in-use
    definitions-unreadable  jig-task-retired       requires-missing
    depth-exceeded          jig-unreadable         reserved-definition
                            malformed-time-limit   task-without-command
                            unknown-placeholder    unsafe-task-name

Bolt ran and judged. Eight, which bolt writes itself:

    empty-selection         the task matched nothing and was not optional
    evidence-missing        a declared file was not produced
    nonzero-exit            the generic exit-code adapter's verdict
    time-limit              the task or the run ran out
    adapter-failed          the adapter ran and exited non-zero
    adapter-wrote-nothing   it left no output.yaml
    adapter-wrote-invalid   it left one that will not parse or validate
    constituent-failed      the fold, reporting a task that failed

`adapter-failed` reads like a verdict an adapter reached and is not one. It is
bolt reporting that it could not get a verdict out of the adapter at all.

Anything else is an adapter's own, and that set is open. An adapter writes
whatever kind its format warrants, `findings` among them, and bolt does not
second-guess it.

`evidence-missing` supersedes `nonzero-exit`. The evidence check returns first,
so where a task declared a file, did not produce it, and also exited non-zero,
the exit status is not reported. A tool that never ran is the usual cause of
both.

## Exit status

The exit status says whether bolt could carry out the run, never whether the
tools passed.

    0      the run was carried out. Read result.yaml for the verdict.
    non-0  bolt refused. result.yaml is still written, and names the refusal.

`--result-to-exitcode` replaces that with `0 if success else 1`. It has no
cases: a refusal is 1 like any other failure, because a refusal is a verdict
bolt reached.
