# bolt, the project

## What it is FOR

Bolt executes a declared set of command lines over a directory and records what
happened, as evidence on disk and as one envelope.

It knows nothing about any tool it runs. That is the whole design and every
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
common quality tooling and its configuration. They are execution environments
and not places a person works: the tooling a jig needs is present and the same
everywhere.

The coupling to bolt is `requires:`, and it is tighter than it looks. From the
ecosystem architecture bolt is derived from:

> The package list is not maintained beside the jig; it derives from it. An
> image installs exactly what the `requires:` fields in toolbox's jigs declare,
> so the image manifest is the jig, not a second list to drift from it.

So FR-3.10's rule that a jig declares every executable it invokes is not
only an up-front check for a missing tool. It is what an image is built
from. An under-declared jig runs locally, where the tool happens to be on
`PATH`, and produces an image without it.

That raises the stakes on FR-3.10b and FR-3.10d, and it is why FR-3.10's
inventory covers the whole jig instead of flagging the unusual tools in it.

The same tooling arrives two ways on purpose: locally `link-jigs` symlinks it
out of toolbox, and in an image it is already present from the anvil layer
beneath.

The fold is bolt's, by name, in that architecture: `BOLT fold -> result
envelope`, stated as a property of a run and not of nesting. One `result.yaml`
at the top of the output directory, "singular, because a run has exactly one
result, folded from the results of its tasks".

That is why a loop of separate invocations is not the whole answer to running
one jig over several subprojects: N runs produce N results and nothing that says
whether the repository passed. It does not follow that bolt has to nest.
FR-5.19 keeps the fold here and puts the composition on a command line, because
bolt prints where its result is and an adapter can read it, so a child run
reaches its parent as an ordinary constituent.

Bolt reads and writes nothing structured itself. Every jig, manifest, envelope
and definitions file goes through wrench, which validates it against the shipped
schema on the way in and on the way out. A jig bolt cannot read is refused by
wrench's schema before bolt sees it, which is why several of bolt's rules are
discharged by a test asserting the refusal arrives, with no code here.

## The rebuild, and the rule that protects it

**This tree is Rust and it is a rebuild. The archived first implementation is
sealed: do not read it, and do not go looking for an older bolt requirements
document, design note or test.**

Everything here reaches from the ecosystem architecture and from answers given
against `NEXT_STEPS.md`, with nothing from the retired implementation in it. The
provenance of that earlier material is unresolved, and this repository exists to
establish that the requirements derive from the architecture alone. Reading the
sealed tree voids that and cannot be undone. The rule is unchanged by the
language and it does not expire.

The later Go build is a different thing and is not sealed. It gates nothing and
stays reachable for comparison, and its task discharges are useful before
rebuilding a piece. What is sealed is the first tree, which is where the
provenance question lives.

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

Bolt gates itself, so `bolt rust-quality .` is a bolt run over this repository.
`CONTRIBUTING.md` has the commands and the conventions a change is held to.

The gate cannot catch a regression in the runner itself. Break execution, the
adapters or the fold, gate with a binary built before the change, and it passes.
Whatever binary gates has to be the one built from the tree under test, checked
by byte comparison, because `mtime` says which file is newer and not whether
they are the same program.
`LESSONS/the-installed-binary-gates-everything.md` and
`LESSONS/a-second-build-answers-for-the-tree.md` carry that in full.

Seven of the eight tasks pass. `traceability` fails deliberately and should not
be made green: it reports 147 of 226 requirements covered. Marking the rest `[?]`
to clear the gate would misreport what is settled, and the number going up is the
progress signal.

The 79 uncovered rows fall into three groups, and the third is why the gate
cannot simply be worked down to green:

    57  a new test discharges it
    19  asserts a design property no test can observe
     3  an existing test exercises it and needs the citation

The 19 are negative universals, ecosystem decisions bolt honours rather than
owns, and claims resting on tools outside a clone. Each has to be rewritten as
something checkable, retired in favour of the project that owns it, or accepted
as permanently exempt, and that is a ruling rather than work.

`bin/test-traceability.py` is a symlink to the shared checker in toolbox, so
that task's verdict moves when toolbox does.

No jig in service uses `short-circuit-failure`, `time-limit`, `optional` or
`adapter-command`, so this build is a superset of what is exercised in practice.

## Where this build differs from the Go implementation

Work directories number from one. FR-9.2a specifies that, so the Go build was the
divergence, but it breaks anything naming a directory and the failure presents as
`No such file or directory`, which does not look like a rename.

    Go build   out/work/composed-0/stdout
    this one   out/work/composed-1/stdout

`metadata.evidence` moved with it, in three ways at once. A task's key carries
the ordinal, `complexity-1` rather than `complexity`; the value is one mapping
rather than a list of them; and `result` is absolute rather than relative to the
base. A consumer looking a task up by bare name and indexing `[0]` breaks on all
three, and breaks by raising rather than by reading a wrong value. Nothing in the
estate reads that block: audited across `~/.projects` on 2026-08-30, the only
readers are bolt's own tests, and `adapters/common/bolt-result.py` folds a
child's `reasons` and `success` without touching it.

Flags may be written after the positionals here and not there, so the accepted
argument set only widens.

A composed task resolves `bolt` through `PATH`, so a refusal message says which
build ran: `the directory X is not there` is this one, `X is not a directory to
run over` is the Go build's.

## Telling a refusal from a verdict

The `kind` on a reason says which of two disjoint vocabularies it came from:
sixteen refusals, where bolt could not run, and eight verdicts bolt writes
itself once it has. Anything else is an adapter's own and that set is open.
`jig-reference.md` lists both closed sets, and which file a name is defined in
is the discrimination. Three sessions have asked this and none could answer it
from the source.

The one that misleads is `adapter-failed`. It reads like a verdict an adapter
reached and is bolt reporting that it could not get one out of the adapter at
all.

### `evidence-missing` supersedes `nonzero-exit`, and drops the status

The evidence check returns before the exit-code path, so whenever both apply the
status is lost. A task declaring `coverage.xml` and not writing it reports
`evidence-missing` where the same task with no declared evidence would report
`nonzero-exit` and the number.

It is the more actionable reason and it drops the more diagnostic fact. A
`pytest` exit of 4 is a usage error and means something different from 1, and it
is usually *why* the coverage file is absent: the tool never ran. Reporting only
the symptom sends a reader to their coverage configuration when the command line
is what is wrong. Recorded as a question in `NEXT_STEPS.md` and left unchanged,
because it alters what every gate downstream of bolt prints.

## Conventions particular to this repository

Substitution is a single left-to-right pass, and that is a security property.
Chained `str::replace` re-expands a template token appearing inside an
already-substituted filename, which breaks the quoting and puts the rest of the
name on the command line. Measured: a file named ``p{all_paths};id #`` executed
`id`. Quoting alone is not enough; never reading substituted bytes again is the
other half. The test is
`a_filename_containing_a_template_token_is_not_re_expanded`.

The requirement and citation conventions are in `CONTRIBUTING.md`: the grammar
an id has to satisfy, the shape of a `COVERS:` mark, and the two counts that
have to reconcile. Live rows must equal the traceability denominator plus the
exempt count, because a row the checker's grammar rejects is absent from a run
that says nothing is wrong.

A gate checks that a cited row *exists*, not that the test *touches* it, so a
wrong citation is indistinguishable from a right one forever. Two instruments
find them. Citations added minus coverage gained should be zero, which has
caught two, and mutation probes, which have caught four.

Mutation-test anything whose test was written after the code, since such a test
tends to assert the outcome the code already produces. A probe breaks the code a
row governs and runs the tests citing it, resolved from the `COVERS:` marks so
it cannot run the wrong test. The probes are local to a worktree that built
them, and a fresh clone carries none.

Read a survival as a question until the probe itself is checked. A mutation that
misses the branch the row lives on looks exactly like a weak test, and three of
eight did. `LESSONS/a-result-that-flatters-you-needs-more-checking.md` has all
three and what each one actually hit.

Expect the gate to catch the change you are making, and fix the code rather than
the threshold. `complexity` is the task that catches it, and each time the answer
has been to split the function.

## What is decided

- One jig, one directory, per invocation. Several at once is a jig whose tasks
  invoke bolt.
- Composition is a command line and there is no second mechanism. A task
  running another jig names `bolt` in its command, like any other tool, and an
  adapter reads the result path bolt printed. Nesting as a task kind, with its
  own fields and inheritance, is retired, 26 rows. What it bought
  was a schema-checkable grant, which FR-5.21 records as given up, leaving
  FR-5.7's depth ceiling as the guard.
- The exit status says whether bolt could carry out the run, never whether the
  tools passed. The verdict is in the envelope. `--result-to-exitcode` opts
  out, making the exit code `0 if success else 1`, because a Justfile recipe
  chaining bolt calls cannot short-circuit otherwise. Off unless named, so
  nothing already written changes meaning.
- Two exit outcomes under that flag, not three, and no engine codes. A
  refusal is 1, like any other failure, and `success` is what wrench's envelope
  schema calls the authoritative verdict, so reading `kind` to promote a refusal
  to "no verdict" would overrule an authoritative field with its neighbour. The
  deeper reason is that a task set always resolves: an optional task matching
  nothing is satisfied, a required one that never ran has failed, and neither is
  an absent verdict. Built two other ways first, a no-verdict code and then a
  code per remedy, and corrected both times.
- Discrimination between refusals lives in the envelope's `kind`, not in the
  exit status. Every refusal names its own, so a base that is not there is
  `base-missing` where a task carrying a retired field is `jig-task-retired`.
  One kind, `bolt-refused`, covered all of them: one name across
  sixteen situations with sixteen different fixes. The exit status has the
  verdict to carry, and a consumer reads the envelope anyway.
- A failing task does not stop the run; a jig asks for the opposite with
  `short-circuit-failure`.
- A run refuses a directory that already holds a run.
- Tasks execute serially, because one at a time is the simplest thing that works
  and nothing requires otherwise.
- No task consumes another task's output. Work needing several steps is one
  script producing one exit code.
- Apache-2.0, with a `NOTICE` naming the holder, and every manifest agreeing.

## What is not done

Standing a jig's commands at the repository root waits on `NEXT_STEPS.md`
question 48: how an invocation pointed at a subdirectory learns the root above
it. Under composition those rows are vacuous until it is answered, since every
invocation is outermost and inherits nothing.

79 requirement rows are uncovered. Sampling them shows most are built and
untested instead of unbuilt: FR-9.4's deterministic ordinals, FR-7.9's reason
kinds and FR-9.8's manifests all work. So the bulk of the remaining work is
writing tests.

`NEXT_STEPS.md` holds the open questions and the defaults taken. Every default
is a `[D]` row, reversible by editing the row it became.

This `docs/` tree is thin. There is no `DECISIONS/` directory: the decisions are
still the `[D]` rows in `REQUIREMENTS.md` and the defaults table in
`NEXT_STEPS.md`, which is a defensible place for them.

`REQUIREMENTS.md` is one file where the shape it is heading for is a directory,
one file per requirement. The 48 retired rows are what blocks the move, because
where a retired id lives under that layout is unsettled.

`Cargo.lock` is gitignored, which is the convention for a library and the
opposite of the convention for a binary. Bolt ships a binary, and a gitignored
lockfile means a clone does not build against the versions this tree gates
against. Nobody has said whether that was deliberate, so it is recorded and left
alone.

`Cargo.toml` takes wrench as a path dependency at `../wrench/rust`, so a clone
of bolt alone does not build. That has to resolve before bolt can be published.

There is no `docs/SPEC.md`. `jig-reference.md` documents the jig format, the
placeholders and the evidence layout as observable behaviour, and
`PATTERNS/the-adapter-contract.md` documents the interface an adapter is written
against, but neither is traced against the requirements the way a spec would be.
