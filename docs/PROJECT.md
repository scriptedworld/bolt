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
which ran every gate in the estate until 2026-08-29 and now runs none. It stays
reachable as `bolt.go` on `PATH` so a comparison is one command. Reading it is
fine and its task discharges are worth reading before rebuilding a piece. What is
sealed is the *archived* tree, which is where the provenance question lives.

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

    cargo build --release && cp target/release/bolt bin/bolt && bolt rust-quality .

**Install before gating, and that is not optional now that bolt is self-hosted.**
Since the cutover `bolt` is this tree's installed binary, so `bolt rust-quality .`
runs **the last binary installed** over the **current** source.

The tasks it runs are `cargo build`, `cargo clippy`, `cargo llvm-cov` and the
rest, which all read the working tree, so the verdict is about the source and is
honest. **What it cannot catch is a regression in the runner itself**: break
execution, adapters or the fold, do not reinstall, and the gate runs on the old
binary and passes. The three-command line above closes it by making the binary
under test the binary doing the testing.

`./target/debug/bolt` is still right while iterating, and `cargo test` is the
faster loop. The distinction to hold is that **only the installed binary gates**.

Check it rather than remembering it, and note this is a byte comparison because
`mtime` says which file is newer and not whether they are the same program:

    cargo build --release && cmp -s target/release/bolt bin/bolt \
        && echo current || echo STALE

`docs/LESSONS/the-installed-binary-gates-everything.md` carries it in full, and
`a-second-build-answers-for-the-tree.md` is the same hazard at one session's
scale before the estate was downstream of it.

Eight tasks: format, lint, build, tests with coverage, vulnerabilities,
licences, complexity, traceability.

**Seven pass. `traceability` fails, deliberately, and should not be made green.**
It requires every test to cite a requirement and every cited requirement to
exist. It reports 141 of 245 covered as of 2026-08-29. The uncovered rows are
specified and unbuilt, and marking them `[?]` to turn the gate green would
misreport what is settled. **The number going up is the progress signal**; it was
79 six tasks ago.

Re-measure rather than believing that figure:

    bolt rust-quality . --output-dir .ephemera/qa
    tail -1 .ephemera/qa/work/traceability-1/stdout

### Adopter status

**Bolt gates itself through `~/bin/bolt`, and so does the estate**, since
2026-08-29. That is NFR-12.1 discharged at full strength rather than through a
path in `target/`: `bolt rust-quality .` runs the installed binary, so the tool
under test and the tool doing the testing are the same file every other project
uses.

The trap that comes with it is in `docs/LESSONS/a-check-that-answers-a-weaker-question.md`
under trap 1: this suite is now reachable from a gate that exports `BOLT_DEPTH`
into it, so a test creating a nested run is a level deeper here than under
`cargo test`.

### The estate has seven jigs, not thirty-five

`find` over the estate returns 35 paths matching `bolt.*.yaml`. **Twenty-two are
symlinks** placed by `link-jigs`, and six of the rest are `.definitions.yaml`
files, which are not jigs. Distinct jigs:

    10x  toolbox/bolt.common-quality.yaml        shared, symlinked
     9x  toolbox/bolt.secrets.yaml               shared, symlinked
     4x  toolbox/bolt.go-std-quality.yaml        shared, symlinked
     3x  toolbox/bolt.python-std-quality.yaml    shared, symlinked
     1x  wrench/bolt.wrench-quality.yaml
     1x  bolt.go/bolt.go-quality.yaml
     1x  bolt/bolt.rust-quality.yaml

**Resolve before counting.** `find … -exec readlink -f {} \; | sort -u`. Counting
paths answers how many places a jig is reachable from, which is a different
question and four times the number.

### Compatibility, measured over the seven

Task keys in use: `command`, `description`, `evidence`, `matching`, `adapter`,
`excluding`. **Nothing else.** No jig uses `short-circuit-failure`, `time-limit`,
`optional` or `adapter-command`, so this bolt is a superset of what is in
service. The Go build also refuses flags written after the positionals where this
one accepts them anywhere, so the accepted-argument set only widens.

### Both code blockers are gone

`toolbox/adapters/common/bolt-result.py` landed at toolbox `e89e7d0`, and wrench
converted their two `jig:` tasks to composed command tasks at wrench `8a1b9dd`.
**Verified end to end**: this bolt runs `wrench-quality` through to 19 work
directories with no refusal, and the composed tasks carry an adapter verdict
rather than bolt's exit status.

**A composed task resolves `bolt` through `PATH`**, so until the symlink moves
the parent can be this build and the child the Go one. That happened in the
verification above, and it is why a composed child's refusal message may not be
this tree's: `the directory X is not there` is ours, `X is not a directory to run
over` is the Go build's.

### The cutover is done, 2026-08-29

    ~/bin/bolt     -> ../../bolt/bin/bolt        this tree, release build
    ~/bin/bolt.go  -> ../../bolt.go/bin/bolt.go  still reachable by name

**Every distinct jig in the estate was run through it first**, against an empty
directory so that each one's validation and `requires` were exercised without
running anybody's suite. All seven accepted and produced work directories:
`common-quality` 3 tasks, `secrets` 2, `go-std-quality` 7, `python-std-quality`
10, `wrench-quality` 19, `go-quality` 7, `rust-quality` 8. No refusals.

**The binary is installed, not committed**, which is what bolt.go did before it:

    cargo build --release && cp target/release/bolt bin/bolt

**Nothing rebuilds it for you**, and a stale one reports green rather than
failing, over every project rather than one session.
`docs/LESSONS/the-installed-binary-gates-everything.md`.

Reverting is one command, and worth knowing before it is needed:

    ln -sfn ../../bolt.go/bin/bolt.go ~/.projects/dotfiles/bin/bolt

`bin/test-traceability.py` is a symlink into toolbox, so that task's verdict
moves when toolbox moves.

## Telling a refusal from a verdict, by the reason's `kind`

Three sessions asked this on 2026-08-29 and none could answer it without reading
the source, so it belongs here. **The vocabularies are disjoint**, and which one
a `kind` comes from is the whole discrimination.

**Bolt could not run.** Sixteen, a closed set, all in `src/error.rs`, and each is
`Error::kind()`:

    base-missing            duplicate-task-name    no-constituents
    both-path-forms         io-failed              output-directory-in-use
    definitions-unreadable  jig-task-retired       requires-missing
    depth-exceeded          jig-unreadable         reserved-definition
                            malformed-time-limit   task-without-command
                            unknown-placeholder    unsafe-task-name

**Bolt ran and judged.** Four, and bolt writes these itself rather than taking
them from a tool:

    empty-selection     FR-4.4b, the task matched nothing and was not optional
    evidence-missing    FR-6.14, a declared file was not produced
    nonzero-exit        FR-6.9, the generic exit-code adapter's verdict
    constituent-failed  the fold, in `src/merge.rs`

**An adapter said so.** Open set, and not bolt's to enumerate: an adapter writes
whatever `kind` its format warrants, `findings` and `child-failed` among them.
FR-6.1 makes the adapter's result the verdict and bolt does not second-guess it.

**So "is the kind in `error.rs`" answers it**, and the practical form for a
reader without the source is the sixteen above. Anything else is a run that
happened.

### `evidence-missing` supersedes `nonzero-exit`, and drops the status

Measured by the skid session across a Go and Rust baseline, generalised by
dispatch from the source, confirmed here at `src/run.rs`:

    Go     nonzero-exit      tests exited 4
    Rust   evidence-missing  tests declared coverage.xml and did not write it

The evidence check returns before the exit-code path, so **whenever both apply
the status is lost.** That is systematic and not incidental.

**It is the more actionable reason and it drops the more diagnostic fact.** A
`pytest` exit of 4 is a usage error and means something different from 1, and it
is usually *why* the coverage file is absent: the tool never ran. Reporting only
the symptom sends a reader to their coverage configuration when the command line
is what is wrong. Recorded as a question in `NEXT_STEPS.md` rather than changed,
because it alters what every gate in the estate prints.

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

**So count the rows against the denominator, because a row the checker cannot
see is not reported as missing.** That is the general form and the suffix is one
instance of it: any row the grammar rejects is absent from a run that says
nothing is wrong. Two numbers, and they must reconcile:

    awk '/^## Retired/{r=1} !r && /^\| *(FR|NFR)-[0-9.a-z]+ \|/{n++} END{print n}' REQUIREMENTS.md
    tail -1 .ephemera/qa/work/traceability-1/stdout

**Live rows must equal the denominator plus the exempt count.** Measured
2026-08-29: 247 live, `140 of 244 … 3 open and exempt`, and 244 + 3 = 247. When
it was wrong it was 245 against 240 + 3, and nothing in the gate's output said
so.

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
