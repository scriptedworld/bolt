# bolt, the project

## What it is FOR

Bolt executes a declared set of command lines over a directory and records what
happened, as evidence on disk and as one envelope.

**It knows nothing about any tool it runs.** That is the whole design and every
other decision here is downstream of it. A jig names commands and which files
they act on; bolt runs them, keeps every stream and exit status, hands each
execution's output to an adapter that turns it into a verdict, and folds those
verdicts into a result. Replacing a linter is an edit to a jig, not to bolt.

A gate that knows about its tools has to change whenever they do, and gates
written that way decay into shell scripts nobody will touch. Bolt is the part
that does not have to change.

### Where it sits against its siblings

    toolbox   the jigs bolt runs, and the checkers and adapters they name
    wrench    reads and writes every structured file, and owns the schemas
    bolt      runs the jig, records what happened, and folds one result
    anvil     the portable execution environments a jig runs inside

### Anvil is where bolt runs, and `requires` is its manifest

Anvil is Docker-based, and the images carry the toolchain, bolt itself, the
common quality tooling and its configuration. **They are execution environments
rather than places a person works**: the point is that the tooling a jig needs is
present and the same everywhere.

**The coupling to bolt is `requires:`, and it is tighter than it looks.** From
`silo/docs/ARCHITECTURE.md` §25:

> The package list is not maintained beside the jig; it derives from it. An
> image installs exactly what the `requires:` fields in toolbox's jigs declare,
> so the image manifest is the jig, not a second list to drift from it.

So FR-3.10's rule that a jig declares **every** executable it invokes is not
only an up-front check for a missing tool. **It is what an image is built
from.** An under-declared jig runs locally, where the tool happens to be on
`PATH`, and produces an image without it.

That raises the stakes on FR-3.10b and FR-3.10d, and it is why FR-3.10's
inventory is a whole-jig obligation rather than a note about unusual tools.

The same tooling arrives two ways on purpose: locally `link-jigs` symlinks it
out of toolbox, and in an image it is already present from the anvil layer
beneath.

**The fold is bolt's, by name, in the architecture.** `silo/docs/ARCHITECTURE.md`
line 79 has `BOLT fold -> result envelope`, and §22 states it as a property of a
run rather than of nesting: one `result.yaml` at the top of the output
directory, "singular, because a run has exactly one result, folded from the
results of its tasks".

That is why a loop of separate invocations is not the whole answer to running
one jig over several subprojects: N runs produce N results and nothing that says
whether the repository passed. It does **not** follow that bolt has to nest.
FR-5.19 keeps the fold here and puts the composition on a command line, because
bolt prints where its result is and an adapter can read it, so a child run
reaches its parent as an ordinary constituent.

Bolt reads and writes nothing structured itself. Every jig, manifest, envelope
and definitions file goes through wrench, which validates it against the shipped
schema on the way in and on the way out. **A jig bolt cannot read is refused by
wrench's schema before bolt sees it**, which is why several of bolt's rules are
discharged by a test asserting the refusal arrives rather than by code here.

## The rebuild, and the rule that protects it

**This tree is Rust and it is a rebuild. Do not read `~/.archive/bolt.archived`,
and do not go looking for an older bolt requirements document, design note or
test.**

Everything here reaches from `silo/docs/ARCHITECTURE.md` and from answers given
against `NEXT_STEPS.md`, with nothing from the retired implementation in it. The
provenance of that earlier material is unresolved, and this repository exists to
establish that the requirements derive from the architecture alone. **Reading it
voids that and cannot be undone.** The rule is unchanged by the language and it
does not expire.

`~/.projects/bolt.go` is a different thing and is not sealed: it is the Go build,
still running every gate in the estate, kept until this replaces it. Reading it
is fine and its task discharges are worth reading before rebuilding a piece.
What is sealed is the *archived* tree, which is where the provenance question
lives.

## Layout

    src/run.rs          the runner: selection, execution, adapters, limits
    src/jig.rs          a jig and its tasks, derived off what wrench parsed
    src/merge.rs        folding every envelope into the run's one result
    src/definitions.rs  the three-layer substitution mapping
    src/error.rs        every refusal, and what each one says
    src/selection.rs    matching, excluding, and shell quoting
    src/adapter.rs      the adapter contract as bolt invokes it
    src/limit.rs        time limits, and how long is left of one
    src/depth.rs        how deep a run is nested, and how deep it may go
    src/walk.rs         the walk, honouring .gitignore
    src/cli.rs          the command line, which is the only interface today
    src/main.rs         eleven lines, delegating to cli::main

    tests/skeleton.rs   the whole suite, one external test package
    REQUIREMENTS.md     what must be true, and the only source for a COVERS mark
    NEXT_STEPS.md       what is not done, the open questions, defaults taken
    bolt.rust-quality.yaml   bolt's own gate, run by bolt

`src/main.rs` carries no command functionality, so the interface is reachable
from an external test package. `tests/` is one file on purpose: it is the
project's whole observable surface in the order the requirements state it.

## The gate

    cargo build && ./target/debug/bolt rust-quality .

**`cargo build` first, and `target/debug` always.** The gate rebuilds the binary
it then runs, so the debug build cannot lag what is committed. A hand-built
`target/release` can and did: 2026-08-28 one sat five hours behind `HEAD` and
answered that a landed change was unbuilt.
`docs/LESSONS/a-second-build-answers-for-the-tree.md`.

Eight tasks: format, lint, build, tests with coverage, vulnerabilities,
licences, complexity, traceability.

**Seven pass. `traceability` fails, deliberately, and should not be made green.**
It requires every test to cite a requirement and every cited requirement to
exist. It reports 131 of 234 covered as of 2026-08-28. The uncovered rows are
specified and unbuilt, and marking them `[?]` to turn the gate green would
misreport what is settled. **The number going up is the progress signal**; it was
79 four tasks ago.

Re-measure rather than believing that figure:

    ./target/debug/bolt rust-quality . --output-dir .ephemera/qa
    tail -1 .ephemera/qa/work/traceability-1/stdout

### Adopter status

Bolt runs its own gate through its own binary, which is NFR-12.1 and is what
`runner/60` finishes. It does **not** yet run through `~/bin/bolt`: that symlink
resolves to the Go build, and every other project in the estate gates through it.

**Moving the symlink is blocked on one file.** Of 35 jig files in the estate,
exactly one was written against the retired nesting mechanism:
`wrench/bolt.wrench-quality.yaml`, with two `jig:` tasks. This bolt refuses those
by name and says what replaced them, by FR-5.22. Converting them is two command
lines and an adapter, filed against wrench and toolbox.

`bin/test-traceability.py` is a symlink into toolbox, so that task's verdict
moves when toolbox moves.

## Conventions particular to this repository

**Substitution is a single left-to-right pass, and that is a security property.**
Chained `str::replace` re-expands a template token appearing inside an
already-substituted filename, which breaks the quoting and puts the rest of the
name on the command line. Measured: a file named ``p{all_paths};id #`` executed
`id`. Quoting alone is not enough; never reading substituted bytes again is the
other half. The test is
`a_filename_containing_a_template_token_is_not_re_expanded`.

**A requirement id takes at most one letter of suffix.** `FR-10.8a` is an id and
`FR-10.8ca` is not: the checker's grammar is `(?:FR|NFR)-\d+(?:\.\d+)?[a-z]?`, so
a two-letter suffix **fails to match as a row at all** and, in a `COVERS:` mark,
degrades to whatever prefix does match. Measured 2026-08-29: two such rows were
silently absent from the denominator, and the citation resolved to `FR-10`,
reported as an id `REQUIREMENTS.md` does not define. Only the second half is
loud. Where a letter is taken, take the next number.

**Every test cites the requirement it discharges**, directly above it:

    // COVERS: FR-4.11a, FR-4.11b | property

Kinds are `positive`, `negative`, `edge`, `property`, `regression`. A test citing
nothing fails the gate, and so does one citing a row `REQUIREMENTS.md` does not
define.

**Citations added minus coverage gained should be zero.** A gate checks that a
cited row *exists*, not that the test *touches* it, so a wrong citation is
indistinguishable from a right one forever. The subtraction is the only
instrument that has caught one here, and it has caught two.

**Mutation-test anything whose test was written after the code.** A test written
afterwards tends to assert the outcome the code already produces.
`.ephemera/mutate-time-limits.py` breaks the code twenty ways and checks the test
that should catch each one does. It has found four tests that could not fail.

**Expect the gate to catch the change you are making**, and fix the code rather
than the threshold. `complexity` has failed on every task since
`definitions/10`.

## What is decided

- One jig, one directory, per invocation. Several at once is a jig whose tasks
  invoke bolt.
- **Composition is a command line and there is no second mechanism.** A task
  running another jig names `bolt` in its command, like any other tool, and an
  adapter reads the result path bolt printed. Nesting as a task kind, with its
  own fields and inheritance, is retired: 26 rows, 2026-08-28. What it bought
  was a schema-checkable grant, which FR-5.21 records as given up, leaving
  FR-5.7's depth ceiling as the guard.
- The exit status says whether bolt could carry out the run, never whether the
  tools passed. The verdict is in the envelope. **`--result-to-exitcode` opts
  out**, making the exit code `0 if success else 1`, because a Justfile recipe
  chaining bolt calls cannot short-circuit otherwise. Off unless named, so
  nothing already written changes meaning.
- **Two exit outcomes under that flag, not three, and no engine codes.** A
  refusal is 1, like any other failure, and `success` is what wrench's envelope
  schema calls the authoritative verdict, so reading `kind` to promote a refusal
  to "no verdict" would overrule an authoritative field with its neighbour. The
  deeper reason is that a task set always resolves: an optional task matching
  nothing is satisfied, a required one that never ran has failed, and neither is
  an absent verdict. Built two other ways first, a no-verdict code and then a
  code per remedy, and corrected both times by our user.
- **Discrimination between refusals lives in the envelope's `kind`, not in the
  exit status.** Every refusal names its own, so a base that is not there is
  `base-missing` where a task carrying a retired field is `jig-task-retired`.
  They all said `bolt-refused` until 2026-08-29, which is one name across
  sixteen situations with sixteen different fixes. The exit status has the
  verdict to carry, and a consumer reads the envelope anyway.
- A failing task does not stop the run; a jig asks for the opposite with
  `short-circuit-failure`.
- A run refuses rather than writing into a directory that already holds one.
- Tasks execute serially, because one at a time is the simplest thing that works
  and nothing requires otherwise.
- No task consumes another task's output. Work needing several steps is one
  script producing one exit code.
- Apache-2.0, with a `NOTICE` naming the holder, and every manifest agreeing.

## What is not done

**`runner/60`, bolt running itself under the estate's own jig**, and the
`~/bin/bolt` cutover behind it. Composition landed 2026-08-28 as a command line
rather than as nesting, so what remains before the symlink moves is wrench's two
`jig:` tasks becoming command tasks and toolbox shipping the adapter that reads a
child's result.

`runner/50d`, standing a jig's commands at the repository root, waits on
`NEXT_STEPS.md` question 39: whether a child inherits the project root. Under
composition its five rows are vacuous unless it does.

Beyond that, `NEXT_STEPS.md` holds the open questions and the defaults taken.
Every default is a `[D]` row and reversible by editing the row it became.

This `docs/` tree is new and thin. The estate's standard is one file per
decision, pattern and lesson under `docs/DECISIONS/`, `docs/PATTERNS/` and
`docs/LESSONS/`. Bolt has two patterns and one lesson, and **no `DECISIONS/`
directory yet**: its decisions are still the `[D]` rows in `REQUIREMENTS.md` and
the "Defaults taken" table in `NEXT_STEPS.md`, which is a defensible place for
them and not the estate's shape. The rest of its durable reasoning is in the task
records under `clank/tasks/bolt/`, where the test plans carry more about why the
tests are what they are than this tree does.

**`REQUIREMENTS.md` is a single file and the estate's standard is now a
directory**, one file per requirement under `docs/REQUIREMENTS/<category>/`. Bolt
has not migrated and **cannot yet**: it has 17 retired rows and where a retired
id lives under the directory layout is an open estate question. Recorded as
`clank/tasks/bolt/commission/10-requirements-becomes-a-directory.blocked`, which
carries the row-diff proof the move would need.

**`Cargo.lock` is gitignored**, which is the convention for a library and the
opposite of the convention for a binary. Bolt ships a binary. Nobody has said
whether that was deliberate, so it is recorded rather than changed.

There is no `docs/SPEC.md`. Bolt arguably needs one, since it has interfaces
other projects build against: the jig format is wrench's schema, but the adapter
contract and the evidence layout are bolt's own and are currently described in
`REQUIREMENTS.md` as properties rather than anywhere as a shape.
