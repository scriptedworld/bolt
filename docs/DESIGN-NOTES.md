# Design notes

This file records durable rationale that is useful to maintainers but is not
future work. Normative behaviour remains in `REQUIREMENTS.md`.

## Composition is command composition

An invocation accepts one jig and one directory. A jig composes another run by
putting `bolt` in an ordinary task command. Bolt has no jig task kind and does
not distinguish a command that invokes bolt from any other command.

The child prints the path to its result. The task's adapter reads that result
and writes the task envelope, which the parent folds as an ordinary
constituent. This preserves one top-level result without adding a second
composition mechanism.

Depth travels through `BOLT_DEPTH` and `BOLT_MAX_DEPTH`, which bolt sets for
every process it starts. The ceiling protects against accidental recursion. It
is not a security boundary because a command can alter its environment.

## Results and exit status

The default exit status reports whether bolt carried out the run. The result
envelope reports whether the work passed. `--result-to-exitcode` maps the
envelope to exit status 0 for success and 1 otherwise, allowing shell command
chains to stop on a failed result.

A refusal also produces a failed envelope. There is no third "no verdict"
state because every resolved task set is either satisfied or failed.

Bolt prints only the result path. Counts and summaries belong in the result,
where a caller reads the same document for successful runs and refusals.

## Output directories

The default output directory contains a timestamp and process ID so concurrent
runs started in the same second do not collide. A caller that needs a stable
location supplies `--output-dir` and uses the path bolt prints rather than
searching for the newest timestamped directory.

A timestamp search is especially unsafe after a refusal that creates no output
directory. It returns the preceding run and lets a caller grade old evidence as
the current result. A caller that needs a known location supplies
`--output-dir`; every caller reads the path bolt prints.

Bolt refuses to reuse a directory that already contains a run. Reuse could
interleave evidence from different runs or destroy the earlier record.

## Definitions

Substitution uses one scalar mapping built from three layers: bolt's reserved
locations, the jig's defaults, and one definitions file named by the invocation.
The definitions file overrides jig defaults. Values are substituted once and
are never scanned again for template tokens.

Definitions parameterize commands. They do not merge jigs, replace task fields,
or disable tasks. A conditional set of tasks is represented by a separate jig,
which keeps the executed task set visible in configuration.

## Evidence and adapters

Every execution receives a work directory and manifest before its command
starts. Output gathered before a timeout remains evidence, and the adapter still
runs over it.

An adapter owns the interpretation of tool output and the canonical form of its
envelope. Bolt validates and folds the envelope without second-guessing the
adapter's verdict. A task without an explicit adapter uses the generic exit-code
adapter.

No task has a dependency on another task's output. A multi-stage operation that
requires private intermediate data is one script and one task. Separate tasks
are appropriate when each stage needs its own manifest, evidence, and verdict.

One task can declare several evidence files. This supports work such as merging
test and entry-point coverage profiles inside one command chain without adding
cross-task dependencies. Several readings of the same execution are a separate
adapter-contract question.

An optional task that matches nothing produces no constituent. The evidence
tree therefore cannot distinguish a skipped empty selection from a task that
was not reached or was not present in the jig. A success `kind` cannot repair
that absence: `kind` belongs to a reason, reasons describe failure, and no
envelope exists for the optional empty selection. Any future representation
should address all forms of a task not running together.

## Rewrite-specific constraints

The rewrite carries behaviour through the requirements and tests rather than
through source translation. Rust also changes implementation constraints that a
port within one language would not encounter.

Structured output requires the Rust implementation of the shared canonical
writer. Time limits and descendant termination require explicit timeout and
process-group support rather than a standard-library equivalent of the legacy
implementation. The build remains free of a C toolchain but dynamically links
the system libraries; producing one static image file would require a musl
target and is not a current requirement.

## Shared structured-file contract

Wrench is the separate structured-file library used by bolt. It parses,
validates, and writes jigs, manifests, definitions, and envelopes against shared
schemas. Bolt consumes that contract rather than defining a competing format.

Toolbox is the separate collection of shared jigs, adapters, and quality
checkers. Repositories can link those files into a checkout while keeping the
single relative layout expected by a jig.

Schema consumers can observe different revisions of that contract at the same
time. Compiled consumers embed schemas at build time while interpreted consumers
can read them at run time. A restrictive schema change can therefore be enforced
by a live reader while an older binary still accepts the old shape. An additive
change can be accepted by the live reader while the older binary still rejects
or ignores it. Rebuilding consumers is part of deploying a schema change; a
schema version remains an open envelope question.
