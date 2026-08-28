# What is still open

`REQUIREMENTS.md` covers what `silo/docs/ARCHITECTURE.md` supports plus what has
been answered against this file. This is what remains, written as questions
rather than as guesses.

**Nothing here blocks a build.** Part one held 35 questions that did. Each is
now a row in `REQUIREMENTS.md`, listed under "Defaults taken" below, and every
one of those is reversible by editing the row it became.

What is left is real and none of it stops an implementation starting. Leaving a
question unanswered costs a gap in the requirements, not a wrong decision in the
code.

Where a question offers candidate answers, they are candidates and not a
recommendation.

---

# Defaults taken

Taken 2026-08-26 rather than asked, because each has a defensible answer and
asking all of them costs a session that could have been spent writing code.
Every one is a `[D]` row, so a wrong default is one edit away from being right.

Four were answered rather than defaulted: one jig per invocation, `requires`
checked both up front and at run time, underscored variables against hyphenated
flags, and the merge reading task and args off disk.

| Was | Now | What was decided |
|---|---|---|
| Multiple jigs per invocation | FR-2.1a | One jig, one directory. Several at once is a jig whose tasks are nested jigs, already specified. |
| Walk order | FR-2.2d | Sorted, which is what FR-9.4's identical directory names rest on. |
| Symlinks | FR-2.2e | Not followed. |
| Reaching a `.gitignore`d file | FR-2.2f | No escape hatch until something needs one. |
| Missing-directory refusal | FR-2.5a | Same shape as every refusal: a result, a reason, a non-zero exit. |
| `--output-dir` absent | FR-2.6a | Created, parents included. |
| `--output-dir` populated | FR-2.6b | Refused. |
| Where `.bolt-<iso8601>` sits | FR-2.6c | At the run's base. |
| Timestamp spelling | FR-2.6d | Filesystem-safe, local offset. |
| Duplicate task names | FR-3.3a | A jig error. |
| `requires` checked when | FR-3.10b, FR-3.10c | Up front, and FR-4.10 still covers a command that cannot start for another reason. |
| A project jig's extra tooling | FR-3.10d | FR-3.10b catches it at the start of the run. Installing it is not bolt's. |
| Naming the five locations | FR-4.1c, FR-4.1d | All five exposed. Variables underscored, flags hyphenated, as a rule. |
| Per-path without a path variable | FR-4.2a | Not expressible. |
| Subprocess or in-process | FR-4.15 | Subprocess. |
| A jig task's missing subdirectory | FR-5.15a | Same as an empty one: it does not run, the run carries on. |
| A jig task per path | FR-5.16 | Once against its base. |
| Adapter when a task names none | FR-6.9 | The generic exit-code adapter. |
| Resolving an adapter by name | FR-6.10 | From the config directory, where jigs already come from. |
| A broken adapter | FR-6.11 | Bolt writes the envelope, and the reason says which of the three happened. |
| Canonical form on `output.yaml` | FR-6.12, FR-6.13 | The adapter's, free from wrench. No reparse-and-compare check. |
| A declared evidence file missing | FR-6.14 | Fails its task, reason naming the path. |
| `message` on a reason | FR-7.8 | Required. |
| Telling a missing tool from a finding | FR-7.9, FR-7.10 | A reason carries `kind`. |
| `args` in the merged mapping | FR-8.8 | The argv as executed. |
| The run's base in the result | FR-8.9 | Recorded. |
| A per-path manifest's path list | FR-9.8 | The whole matched list. |
| Ordinal on a single execution | FR-9.9 | Always carried. |
| Exit statuses | FR-10.5, FR-10.6 | 0 ran, 1 could not run, 128+n on a signal. |
| Partial result on failure | FR-10.7 | Written whenever bolt is alive to write it. |
| How a definitions file is named | FR-4.16a | `--definitions <name>`, read from the config directory as `bolt.<name>.definitions.yaml`. |
| How many to an invocation | FR-4.16b | One. Several per project, each scoped by its name. |
| A definitions file's shape | FR-4.16c | One level of names, scalar values. |
| A placeholder nothing defines | FR-4.18, FR-4.18a | Refuses the run up front, checked when `requires` is. |
| A value carrying substitutions | FR-4.17a | Literal. Reading the file settles every value in it. |
| A definition shadowing a location | FR-4.19 | Reserved. Refuses the run. |
| What a child inherits | FR-5.13j, FR-5.17 | Its parent's definitions file, until a field says otherwise. |

Two questions left for wrench, which now states them itself: which codecs and
readers ship, and whether the schemas are files an editor can be pointed at.
Both are `wrench/NEXT_STEPS.md`.

Four rows left section 13 settled by the body: FR-13.1 by FR-6.2b, FR-13.2 by
FR-7.9 and FR-7.10, FR-13.5 by FR-4.12, FR-13.9 by FR-3.4d, FR-1.5 and FR-3.12.

---

# The language

**Bolt is being rewritten in Rust.** Answered 2026-08-27. qwark and grim stay
Go, to show I can work in Go too.

**The Go implementation is a repository of its own, `bolt.go`.** It carries the
whole history to that point, so the derivation record travels with the code that
was derived. It is the runner in use until the Rust tree reaches parity, and
nothing in this tree builds a binary any more.

## What a rewrite has to solve that a port does not

Measured or read 2026-08-27, before anything was moved.

**wrench has no Rust pack**, and this is the largest item. A Rust bolt needs a
third implementation of the canonical-form emitter, byte-identical to the Go and
Python ones and held to the same fixture set, which `wrench/START_HERE.md` calls
the only thing keeping the existing two level. That work is wrench's.

**NFR-12.4 gets harder.** `CGO_ENABLED=0` gives Go a static binary for nothing.
Rust links glibc dynamically by default and a genuinely static build wants the
musl target, which wants a musl toolchain: the exact "no C toolchain" constraint
the row states. Either the row bends or the image grows a toolchain, and that is
a decision rather than a detail.

**Timeouts and process groups need crates.** Go has `exec.CommandContext` and
`SysProcAttr.Setpgid` in its standard library; Rust's has no wait-with-timeout.
`runner/40` is the task that meets this, and its tests are written and its
implementation deliberately is not.

Everything else is ordinary translation: errors to `Result`, `filepath` to
`std::path`, and logic that is the same logic.

## REQUIREMENTS.md becomes a directory, and not yet

**Ruled 2026-08-27**, every project, one file per requirement under
`docs/REQUIREMENTS/<category>/`. Single-file is retired with the format rather
than kept alongside. `silo/docs/DECISIONS/requirements-are-a-directory.md` at
`055f0c2`.

**Bolt is the measurement the ruling rests on**, and the measurement has moved
twice since it was taken.

FACT 2026-08-27, re-derived: 264 row-shaped lines in 441, counting the retired
table; the traceability checker declares 244 live. The earlier figure here, 245
rows in 397 lines, is now **`bolt.go`'s document**, because the repository split
forked this file and froze the copy at that point. So the number did not drift,
it migrated, and a reader finding 245 today finds it in the other tree.

**The ruling still rests on it and one half of the old sentence no longer
holds.** Bolt is the largest of eight. "More than twice the next" was true and
now depends on whether `bolt.go` counts, since the split created a near-copy at
245 that is bolt's own former self. Against the largest document that is not
bolt, qwark at 127, it holds.

    for f in ~/.projects/*/REQUIREMENTS.md; do
      printf '%-14s %s\n' "$(basename $(dirname $f))" "$(grep -cE '^\| (FR|NFR)-[0-9]+\.[0-9]+[a-z]* \|' $f)"
    done | sort -k2 -rn

**That command reads the whole estate, so this claim is not checkable from a
clone of bolt.** It is recorded here because the ruling it supports is
estate-wide, and the scope is stated so nobody restates it as a local fact.

**Do not split the file yet.** The checker cannot read a directory: it guards
with `.exists()`, which a directory satisfies, then `read_text()` raises
`IsADirectoryError`. bolt's gate runs `traceability` against this file, so
splitting first turns a task that fails honestly into one that crashes. Filed
for toolbox at `clank/inbox/toolbox/traceability-must-read-a-directory/`.

The order is checker, then migrate. Nothing here is owed until it lands.

## bolt's own `lint` task was passing without denying warnings

**Corrected 2026-08-28**, and it retracts a figure this session reported twice.

`bolt.rust-quality.yaml` runs `cargo clippy --all-targets -- -D {deny}`, where
`{deny}` comes from the jig's `definitions` block. Bolt does not read that block
yet, so before FR-4.18 was implemented the placeholder went to the shell
verbatim. Measured, from that run's own evidence:

    command   cargo clippy --all-targets -- -D {deny}
    exitcode  0
    stderr    warning: `bolt` (bin "bolt") generated 1 warning
    envelope  success: true

clippy accepted `{deny}` as an unrecognised lint name, so **`-D warnings` never
applied** and the task passed while enforcing nothing. Every "seven of eight
tasks pass" in this session's record was six and a vacuous one.

**The envelope was not wrong.** The exit was 0 and the exit-code adapter says so
correctly. Nothing in the evidence layer can know a tool was handed a flag it
did not understand, which is the argument for an adapter that reads what the
tool said rather than only what it returned, and it is FR-6.9's own limit stated
in its own row.

Bolt now refuses the jig outright: `task lint names {deny}, which nothing
defines`. So **NFR-12.1 waits on `definitions/10`**, and the Rust bolt cannot
gate itself until that lands. That is a smaller claim than the one it replaces
and it is the true one.

## The gate's own replacements, owed rather than open

Not questions. Each is a task in the jig that runs one tool now and is expected
to run another later, listed so the swap is tracked rather than living in a
comment nobody greps.

**The metrics are the requirement. The tool and the task's home are both
open.** Answered 2026-08-27, and it is a weaker commitment than the current jig
implies.

What is wanted is complexity, function length and parameter count. A single
cross-language analyser is the **nice-to-have** shape: helga if it exists, per
`silo/docs/DECISIONS/components-are-named-from-the-great-clock.md` at `633cb31`,
and what helga becomes is silo's rather than recorded here. lizard stands in
meanwhile, and FACT 2026-08-27: it parses Rust, measured against a probe file.

**Getting the same metrics from language-native tools is an accepted outcome,
not a fallback to apologise for.** If one analyser turns out to have too many
tricky bits across every language, `complexity` leaves the common jig entirely
and each language jig carries its own.

For Rust that is already available and needs no new tool. FACT 2026-08-27, every
lint present in the installed clippy:

    clippy::cognitive_complexity   the measure complexipy gives Python
    clippy::too_many_arguments     the parameter count
    clippy::too_many_lines         the function length
    clippy::excessive_nesting      what cognitive complexity is mostly about

So the Rust gate could drop `complexity` and `lizard` from `requires` tomorrow
by enabling four lints in `Cargo.toml`, where the `lint` task already refuses
warnings. It has not, because the common-jig shape is still the one being tried
and moving early would prejudge it.

**That swap would loosen the gate, and nothing in it says so.** Two of those
lints are already enabled through `pedantic` and both are dead, because lizard's
thresholds are stricter and bind first:

    lizard      --length 60      --arguments 5
    clippy      too_many_lines   100    too_many_arguments  7

There is no `clippy.toml`, so those are the defaults. FACT 2026-08-28, measured
while implementing the skeleton: `run_task` at 73 lines and `write_manifest` at
6 parameters both **failed lizard and passed clippy** under
`--all-targets -- -D warnings` with `pedantic` on. So the four lints are not
equivalent to the task they would replace, and dropping lizard without setting
`clippy.toml` moves the limits from 60 and 5 to 100 and 7 with no line of the
diff mentioning a threshold.

Two thresholds where the looser cannot bind is the shape the wrench session
found in the shared Python jig, filed at
`clank/inbox/toolbox/two-docstring-thresholds-and-the-lower-one-is-dead/`:
interrogate at 80% under pylint's implicit 100%. Same mechanism, opposite end,
and here the dead one becomes live the moment the strict one is removed.

**So the swap owes a `clippy.toml` pinning 60 and 5**, written in the same
change, or it is a relaxation disguised as a tooling simplification.

**Cyclomatic and cognitive complexity are different measures and neither
substitutes for the other.** toolbox's Python jig says so and pairs lizard with
complexipy. It is the reason a single number may not be what this task wants,
whichever tool produces it.

**`suppressions` is not in the jig at all.** toolbox's common jig has it and
nothing here runs it, so hard rule 4's register is unenforced in this tree. It
arrives with the shared common jig or before.

**Unused dependencies have no task.** `cargo-udeps` would be it, and FACT
2026-08-27: it is not installed. `requires` refuses a run naming a tool that is
not there, so naming it would break the gate rather than extend it.

**Docstring coverage is a lint, not a number.** `missing_docs` in `Cargo.toml`
fails the build; interrogate reports a percentage. If the number is wanted, that
is a tool this jig does not have.

**The Go-as-provenance argument is not a reason to keep it.** It reached this
session second-hand: that nothing here was ever written in Go, so a Go bolt
cannot plausibly derive from the archived one, making the language itself a
clean-room proof. It is not what the choice rests on, and nothing should be
built on it.

**The provenance is carried by the derivation, not by the language.** The first
paragraph of `REQUIREMENTS.md` records that these requirements were reached from
`silo/docs/ARCHITECTURE.md` and from answers, with no earlier bolt
implementation, requirements document, design note or test read. That is what
makes the chain clean, and a Rust bolt translated from this one inherits it,
because this one's own provenance is documented rather than inferred from what
it is written in.

**So what a rewrite carries over is this document, `REQUIREMENTS.md`, and the
test suite**, all of which are language-neutral. The Go code is the reference
implementation those were proved against, and continuing it is what makes the
suite worth translating.

The archived tree stays sealed either way. That is the constraint the derivation
rests on, and it does not change with the language.

---

# Recorded and deferred

## The envelope

1. Is `statistics.source` a literal key, or a placeholder for the source's
   name used as the key? And what makes it a list rather than one object?
2. Do a task's own `evidence` paths survive into `result.yaml`, or does a
   reader follow the mapping's result filepath to the task's own envelope?
3. May `metadata` carry adapter-specific keys beyond `statistics` and
   `evidence`?
4. Is there a schema version field, given `success` is the only guarantee?
   What does a consumer do with a version it does not recognise?
5. FR-1.5 validates everything read as data, and the consequence of failing
   differs by file. A jig that fails its schema refuses the run. An envelope
   that fails is a failure under FR-7.6. Is a nested run's `result.yaml`
   failing validation the child's failure or the parent's?
6. Does `result.yaml` keep the envelope shape exactly, or carry keys a task
   envelope never has? FR-8.9 puts the run's base in it, which is one such key.

## Tasks

7. Are the jig errors FR-3.4b and FR-4.2 name caught when the jig is read, or
   when the task is reached? Reading catches every one of them before anything
   runs; reaching catches them after half a gate has executed. FR-3.10b now
   checks `requires` up front, which argues for reading.
8. Is the environment handed to a task command inherited wholesale, filtered
   to an allowlist, or declared per task? Whatever the rule, the depth
   variable has to survive it.
9. Can a task be disabled in a jig without deleting it, and does a disabled
   task appear in the result?
10. FR-1.5 validates a jig. Does an unrecognised key fail it or warn? Failing
    makes a jig written for a newer bolt unusable by an older one; warning lets
    a typo pass as an ignored field.
38. Do tasks execute in the order the jig declares them? **No row says so.**
    FR-4.5 says only that they execute serially, FR-4.5a says that is the
    simplest thing rather than something required, and FR-4.7 says the merged
    result does not vary with the order tasks ran in.

    It is load-bearing elsewhere. FR-4.9's `short-circuit-failure` says the
    tasks *after* the failing one do not execute, and "after" needs a defined
    order. A jig author putting `format` before `build` expects that order too,
    and the console summary prints it.

    Found at stage 4 of `walking-skeleton/10`, 2026-08-27, because the test plan
    had mapped declaration order onto FR-4.5. The test now asserts what FR-4.5
    says, which is that no two executions overlap. If the answer is yes, this
    wants its own row and `runner/20` should cite it rather than FR-4.5.
39. Is a dotfile a project file? **No row says.** FACT 2026-08-27: at `ignore`'s
    defaults a tree holding `.editorconfig` and `.github/workflows/ci.yml` walks
    as `["plain.txt"]`, because `hidden` defaults to true. So a jig linting CI
    workflow files reports a clean run over zero files, and FR-8.3a does not
    catch it, because the other tasks produced constituents.

    Both answers are defensible and neither is written. `hidden(false)` walks
    `.git/` too unless something else excludes it, and FR-2.2b says bolt reads
    nothing under `.git/` rather than that the walk skips it.

    The measured outputs above are the evidence and they are quoted here
    deliberately, because the probe that produced them is in `.ephemera/` and
    will not survive a clone or a cleanup. To re-derive: build a tree holding
    `plain.txt`, `.editorconfig` and `.github/workflows/ci.yml`, walk it with
    `ignore::WalkBuilder` at defaults, then again with `hidden(false)`.
40. Does FR-2.2e mean bolt does not *traverse* a symlink, or that it does not
    *return* one? FACT 2026-08-27: with `follow_links` false, a base holding a
    file symlink pointing outside walks as
    `["dirlink", "filelink.txt", "inside.txt"]`. Nothing is traversed, so the
    row is satisfied, and a task handed `filelink.txt` reads through it to
    outside the base, which FR-2.3's containment is not.

    The row's own rationale is about `link-jigs` leaving tracked symlinks
    pointing into toolbox, and those are file symlinks, so the case the row was
    written for is the case it does not settle.
41. May a task name another task's output directory as an input? **FR-4.6 says
    no today**: no task consumes another task's output, and work needing several
    steps is one script. Promoted from `clank/inbox/bolt/`, filed by silo from
    our user 2026-08-27, arguing the refusal costs too much in one real case.

    Four tasks where the third needs the first's artifact *while it runs*. The
    aggregator sees every envelope but only after all have finished. Scripting
    the chain works and collapses four tasks into one, losing three sets of
    evidence: an intermediate step inside a script produces no envelope, no
    manifest and no work directory.

    It is explicitly **not** the task-to-task chaining removed from the
    architecture the same day. Naming a path is weaker than a dependency graph.

    Two things need settling before any spelling is chosen. **Where the name
    comes from**: a task cannot know another's output directory until the run
    assigns one, so a jig either predicts `work/<name>-<n>/`, coupling every jig
    to bolt's layout, or is handed it by a substitution, which is a small
    language and a decision about how much of one bolt wants. And **what
    `depends` would mean**, which covers visibility, ordering and verdict
    separately, so one word for all three decides two of them by accident.

    **This is where question 38 stops being theoretical.** If task three reads
    task one's output, something must guarantee task one ran first, and no row
    states an execution order at all.
42. What does the console summary say, and does any row govern it? **None does.**
    Two sessions found the same defect independently: `failed: 9 execution(s)`
    over a run with two failing tasks reads as nine failures and is the count of
    executions in a failed run. Measured by skid against its own gate, and here
    against bolt's.

    The Rust rebuild will write that line from nothing, so the wording is a
    decision waiting to be taken rather than a bug to port. What a reader needs
    is which tasks failed, not how many executions there were.

    The defect itself is `bolt.go`'s, at `internal/cli/cli.go` in `report`.
43. What happens when bolt's embedded schema is older than wrench's? **Nothing
    tells anyone.** FACT 2026-08-27: wrench embeds its schemas at build time,
    `//go:embed schemas/*.schema.json` in the Go pack and a `build.rs`
    generating `include_str!` in the Rust one, so bolt carries a copy fixed at
    the moment it was compiled. The Rust rebuild inherits this.

    So **the schema this estate enforces is whatever its consumers last built
    with, not what wrench ships.** Measured by the wrench session: `allow-empty`
    was committed at 21:45 and reached enforcement at 22:08 only because bolt
    happened to be rebuilt after. Swapped half an hour earlier, the field would
    have been in wrench's contract and unenforceable everywhere, with nothing
    saying so from either end.

    This is not a defect. A static binary wants its schemas embedded, and
    FR-1.12 makes bolt a consumer of wrench's contract rather than its owner.
    What is missing is any way to notice the gap: **a new constraint is
    unenforced everywhere the moment it lands**, and the window between wrench
    committing one and a consumer rebuilding is invisible from both sides.

    Candidates, none chosen. Bolt reports the schema version it carries so a
    reader can compare. Wrench stamps a version its consumers can assert
    against. Or it stays a known property and the swap discipline covers it,
    which is what happened today by luck rather than by design.

    The estate ran one shared binary today, so there was one stale schema rather
    than eight. That is the current mitigation and nothing states it. It holds
    only while `~/bin/bolt` is a single file: once a repository builds its own,
    or the link moves while somebody keeps an old binary, there are N embedded
    schemas and nothing says which answered a given run.

    **Two staleness regimes, not one.** FACT 2026-08-27, measured in wrench's
    source and confirmed by the wrench session against a live schema edit:

        Go      `//go:embed schemas/*.schema.json`              build time
        Rust    `build.rs` generating `include_str!`            build time
        Python  `Path(__file__).resolve().parents[2]`           RUN TIME

    Bolt takes the Rust pack, so it embeds. toolbox's adapters take the Python
    pack, so they read wrench's working tree. **Two consumers of one contract
    can enforce different versions of it at the same moment, and both conform.**

    **Which way it hurts depends on the direction of the change, and the two
    are opposites.**

    A *restrictive* change: the live reader refuses at once, the embedded one
    keeps accepting until rebuilt. That is exactly this morning's false green,
    where a binary carrying an older schema passed a jig with `version: 1`.

    An *additive* change, such as `allow-empty`: the live reader accepts at
    once, the embedded one does not until rebuilt. What it does instead is
    **question 10's**, which is open: an unrecognised key either fails the jig or
    warns. If it fails, every additive schema change breaks every consumer that
    has not rebuilt, and the two questions are coupled more tightly than either
    records.

    So answering question 10 with "fail" makes this sharp, and answering it with
    "warn" makes an additive change silently do nothing on an old binary. Both
    are defensible and neither is free, which is worth knowing before question 10
    is settled on its own merits.

## Promoting the Rust jig to toolbox, and what it owes first

Chosen by our user 2026-08-27, heard first-hand by the wrench session and routed
here by silo. wrench's Rust pack is the waiting consumer: three packs, and the
Rust one gated by `cargo test` and nothing else.

**The split is already written in the jig's own header**, so promotion is closer
to deleting the common tasks and handing the rest over than to a redesign:

    common-quality     traceability, suppressions, complexity
    rust-std-quality   format, lint, build, tests, coverage, vuln, licences

**It has been run against one repository, so its assumptions are bolt-shaped
until somebody checks.** Audited 2026-08-27 against that question. Two were
defects here and are fixed; two are decisions a shared jig has to take and are
not mine alone.

**Fixed: the complexity task graded 16% of the tree.** It read `src`, so it
never saw the test suite. Measured: `lizard src` is 148 nloc in 12 functions,
`lizard .` is 912 in 52. Tests carry no exemption from complexity, and this
surfaced only once bolt had tests, which is the same root cause as toolbox's
bandit excluding `tests` in a repository with no pytest suite. **An exclusion
written where the excluded thing does not exist is invisible until an adopter
has one**, and that is the failure mode most likely to repeat on promotion.

**Fixed: `requires` under-declared three tools.** It named `cargo`, which says
nothing about `cargo-llvm-cov`, `cargo-audit` or `cargo-deny`, each a separate
binary. Three tasks could fail halfway through a run on a machine missing them,
which is what FR-3.10's up-front check exists to prevent.

**Open: `cargo deny check` needs a `deny.toml` and nothing declares it.**
`requires` names tools, not files, so an adopter without one meets a confusing
failure from a task that looks like it should work anywhere. A shared jig either
ships a default policy, or the adopter owes a file that nothing tells them
about. bolt's own policy took two failures to get right and is not obviously the
one another project wants.

**Open: coverage is measured and enforced nowhere.** The `tests` task writes
`coverage.lcov` and declares it as evidence, and nothing reads it. No threshold
exists anywhere in the jig, so the task passes exactly when the suite passes and
the profile is decoration. The standard says coverage is judged per file, and
hard rule 5 says a coverage failure is never settled by excluding the file, so
what is missing is the number and the thing that reads it. A shared Rust jig has
to answer that, and the answer belongs with whoever owns the standard rather
than with the first adopter.

## A caller that finds its output directory by timestamp is wrong

Not a question. Recorded from skid's withdrawn finding, because the same mistake
will be available to whoever writes the Rust CLI's consumers.

Bolt refusing with usage and writing **no** output directory is correct, and it
is also what misleads. A caller that redirects both streams away and then reads
`ls -dt .bolt-*/ | head -1` gets the *previous* run's directory and grades it as
this run's. Nothing about that looks wrong.

`--output-dir` is the answer: a caller that names where evidence goes never has
to discover it. FR-10.7b already says a caller wanting a parseable refusal in
every case names one outside the tree, and this is the same advice reached from
the other direction.

## FR-10.3 is why bolt prints no summary line, and the Go build shows the cost

Not a question. Recorded because the pressure to add one will recur, and the
argument against it is easy to lose.

The Go build ends a run with a line pairing the overall verdict with the total
execution count, so three tasks with one failing print `failed: 3 execution(s)`.
FACT 2026-08-28, reproduced here. A reader takes it as a failure count. The
passing case reads correctly by accident, because with nothing failing the two
numbers coincide, which is how it survived.

**The wording is the symptom.** The cause is a command line stating a verdict at
all. `cli.rs` prints the result path and nothing else, citing FR-10.3: the
verdict is in the envelope, so what a caller is told is *where to read it* rather
than what it says. That is already the stronger position and it is the one that
makes this defect unreachable rather than merely unlikely.

So the note is not "word it carefully" but **do not grow one**. A summary that
states a verdict duplicates the envelope, and a duplicate is free to disagree
with it, which is exactly what the Go build did to itself. If a run summary is
ever wanted for a person at a terminal, it belongs behind a flag that says so,
and it counts what it says it counts.

## FR-10.7 has a destructive shape, and the Go bolt is standing on it

Not a question. Recorded because implementing FR-10.7 without this in hand
reproduces a defect that already exists next door.

FR-10.7 wants every refusal to write a `result.yaml`. The Rust tree writes none,
which is the first of the four unfixed findings in `review-stage-5.md`. The
obvious fix, writing the refusal into the run directory bolt had resolved, is
the one that breaks.

FACT 2026-08-28, reproduced first attempt against `~/bin/bolt`, the Go build:
two runs starting inside one second resolve to the same `.bolt-<iso8601>`. The
second refuses, correctly, with `already holds a run`. It then writes that
refusal into the colliding directory's `result.yaml`, replacing a completed
verdict. The `work/*/output.yaml` files survive carrying `kind: nonzero-exit`
while the top-level document says `kind: bolt-refused`, so the run that ran is
readable only from the evidence that the merge was supposed to summarise.

Reported by the resume session against palette-print; reproduced here on a
fixture. `clank/inbox/bolt.go/a-refusal-overwrites-the-run-it-refused/` carries
the evidence and a `repro.sh`.

**The constraint this puts on the Rust build.** Refusing to use a directory and
writing a refusal into it have to be separable. The Rust guard at `run.rs:104`
returns before `create_dir`, so today the ordering is already right and the
`result.yaml` is simply missing. Keep that ordering when FR-10.7 lands: a
refusal that names a directory as unusable must not then write to it. The
collision refusal is the case that proves the general rule, and FR-10.7b's
advice, that a caller wanting a parseable refusal names `--output-dir` outside
the tree, is the same answer reached from the other direction again.

There is no test for the collision guard here, for the reason already recorded:
the fixture needs two runs inside one second. The Go reproduction shows that is
reachable on the first attempt rather than being a rare race, which weakens the
argument for leaving it untested.

## Adapters

11. May an adapter read the repository tree, or only the files it was handed?

### The contract is undocumented, and that has already cost a port

Not a question. Reported by the resume session 2026-08-28 after porting
toolbox's coverage adapter: there is no written adapter contract anywhere, so it
was derived by reading the Go build's `internal/adapter/adapter.go` and
`internal/run/envelope.go`.

What an adapter author needs and had to reverse-engineer: no stdin, locations
arrive as flags, `--evidence` once per declared file, the adapter writes
`{work_dir}/output.yaml` itself, that file conforms to wrench's envelope schema,
and cwd is the base directory. CLAIM, since it is the Go build's contract read
by someone else and not measured here; the Rust build has only the built-in
exit-code adapter so far, and `runner/30` is where a second one arrives.

The cost is drift rather than inconvenience: three of toolbox's four adapters
are on a retired contract and would fail `adapter-wrote-invalid`. A contract
nobody wrote down cannot be versioned, so nothing told them it had moved.

**Write it when `runner/30` lands**, because that is the task that has to settle
it anyway, and an undocumented contract with two implementations is what
produced the drift the first time. It is also the strongest argument yet for
`/commission`, since there is nowhere in this tree for such a document to go.

The design question underneath it is answered already, and I said otherwise here
before checking. **FR-4.6 is settled and `[A]`: "No task consumes another task's
output. Work needing several steps is one script producing one exit code and one
output."** The next section is what the resume session ran into and what it
would take to reopen it.

### FR-4.6 was challenged and holds. Nothing forces cross-task evidence

Not a question. Recorded so it is not reopened on an example that was withdrawn.

The resume session hit what looked like a forcing case for cross-task evidence
while porting toolbox's coverage adapter, argued it to me, then **measured it and
retracted within the hour**. The retraction is the useful part and it is theirs,
not mine.

**Entrypoint coverage does not need cross-task access.** FR-4.6 permits it as
written. Measured by them on palette-print, end to end:

    go test -coverprofile={work_dir}/cover-test.out ./...
    go build -cover -o {work_dir}/app ./cmd/palette-print
    GOCOVERDIR={work_dir}/covdata {work_dir}/app --palettes
    go tool covdata textfmt -i={work_dir}/covdata -o={work_dir}/cover-entry.out

One command chain, one exit code, one work directory, **two declared evidence
files, one adapter**. The adapter keys blocks by `(file, span)` and takes
`max(count)`, so the entry point reads uncovered in the first profile and covered
in the second, and palette-print goes green with no exclusion. Hard rule 5 is
satisfied rather than dodged. That is exactly FR-4.6's "one script producing one
exit code and one output".

So **(b), declared cross-task evidence, has zero examples, not one.** It should
be costed as speculative rather than as answering a known need.

**What survives, and it is (a).** Several readings of one execution should be
separately named: test result, coverage, timing. Today a task has one adapter, so
they fuse into one adapter that has to know about all of them, and their coverage
adapter is now that shape, reading an exit status it should not care about. This
crosses no task boundary and needs no dependency graph.

**(a) is still a requirements change, on two rows.** FR-3.2 declares "its
adapter", singular. **FR-6.2b fixes the envelope at `output.yaml` with the name
never varying**, the row FR-13.1 was retired into, so several adapters per task
collide on that filename by construction. Both would have to move.

Their instinct to forbid an adapter discovering files in the output tree is
**already FR-6.2c**, with the same reason.

**I overstated the preamble and they were right to push back.** I wrote that it
predicted their case by name. The preamble's cost is that an intermediate step
produces no envelope, manifest or work directory, so a middle stage's **own
verdict** is lost. The entrypoint chain does not pay that: every artifact is
declared evidence of the one task and all of it reaches the adapter. §19's worked
example resembles the entrypoint case superficially and is narrower than the
preamble's phrasing suggests. **Do not reopen FR-4.6 on the strength of that
paragraph**, which is what I was heading for.

### Entry points need the two-profile pattern here too, and bolt is one

FACT 2026-08-28: `src/main.rs` is 11 lines and delegates to `bolt::cli::main`,
and **four tests already invoke the built binary** through a `bolt()` helper
using `Command::new(env!("CARGO_BIN_EXE_bolt"))`, at `tests/skeleton.rs:55`.

So bolt is in the same position as palette-print: the entry point runs in a
subprocess during the suite, and its coverage lands in that subprocess rather
than in the harness's profile. Hard rule 5 says measure it, not exclude it. The
shape is the Go one: instrument, run the binary, write a second profile, merge by
taking the covered maximum. Nothing here needs cross-task evidence either.

The resume session reported that this recurs across the estate, five Go repos,
one Rust, six Python with a milder form. CLAIM, their measurement not mine, and
**two of its figures are wrong where they touch this tree**: they recorded bolt's
binary as never invoked, having grepped for `assert_cmd`, `Command::cargo_bin`
and `escargot` and missed Cargo's built-in `CARGO_BIN_EXE_*`; and bolt's one
Python entry point is `bin/test-traceability.py`, a **symlink into toolbox**,
already counted under toolbox. The estate total is inflated by at least that one.

The conclusion is unaffected and is theirs: recurrence argues for **writing the
entrypoint pattern down once per language**, not for a dependency graph.

## Time

Nothing in the architecture mentions time, and what is here now comes from
answers rather than from it.

12. Where is each limit declared? A task's belongs on the task; a run's could
    sit on the jig, on the command line, or both, and if both then which wins.
13. How is a timed-out child terminated, which signal and with what grace, and
    are its descendants killed with it? A command that spawns its own children
    leaves them running when only the child is signalled, and they go on
    writing into a work directory bolt has finished with, and into the streams
    an adapter is about to read under FR-4.12a.
14. What records the executions a task never reached? A per-path task cut off
    at path fifty leaves fifty work directories and nothing saying the other
    three hundred and fifty were never attempted. The run fails, correctly, and
    a reader still cannot see how much went unchecked.
15. Does a run that times out fold in the constituents that completed, or does
    its result carry only the timeout? FR-4.14 keeps the evidence; whether the
    merge runs over what is there is the part that is not settled.

## The output directory

16. Is a task skipped for an empty selection recorded in `result.yaml`?
    FR-8.3a closes the case where every task skips, since a merge finding no
    constituent fails. Open is the partial case: four tasks green and a fifth
    skipped reads the same as four tasks and no fifth.
17. Does a manifest record what the walk found, or only what the task matched?
    FR-9.8 settles that it holds the whole matched list rather than one path.
    What it does not settle is whether the walk's own findings are in there
    too, which is what separates a task offered a hundred files and wanting
    none from one run against an empty tree.
18. `manifest` already means the read and write authorization scope in §53 to
    §55, and both kinds land in task evidence trees. Rename bolt's?
19. Who deletes run directories, and when? The lifetime is settled by FR-9.1a:
    wanted while the result is being reviewed, not afterwards. Open is whether
    bolt prunes its own, whether whoever named `--output-dir` owns removing it,
    and what a graph node's `.ephemera/` does about it.

## Input paths

20. Does a run over a large tree exceed `ARG_MAX` when `{all_paths}` expands?
    Whole-project runs make this the ordinary case rather than an edge one.
    Does bolt chunk into several executions, or is it the jig author's problem?
21. §67's pre-commit overlay wants a gate over what changed, and a
    directory-only invocation cannot express that. Does the overlay run the
    whole project on every commit, or does something have to give?

## Nested jigs

22. Is there a shorthand for naming one jig at many bases? Nine Go subprojects
    is nine jig tasks each needing its own name, and a list form would say it
    once. Against that, a written-out task per instance is what makes the
    project jig readable as an inventory of what is in the tree.
23. ~~Does the whole-jig override survive separate location variables?~~ (It
    cited FR-5.12, which is one jig at many bases. The override is FR-5.14.)
    **Answered by this question's own reasoning, as FR-5.14 to FR-5.14d.** It
    was right that `{project_root}` covers the per-path case and that only a
    tool which must *stand* at the root is left over, so that is all
    `needs-repository-root` does: the working directory moves and the base does
    not. Nothing surrenders its base, so the cut against FR-5.10 does not
    arise.

    **Built the other way first, which is why FR-5.14c carries a measurement.**
    Overriding the base let a child read outside the grant its caller wrote,
    and this question was sitting beside the row saying so.
24. Is a cycle detected by name, or only by depth exhaustion? A jig stack in
    the environment would name the cycle; a counter can only report a number.
    **Only depth today**, by FR-5.7. `bolt.recursive.yaml` naming itself is
    stopped at 4 with a reason naming the limit and not the cycle.
25. Can a jig be referenced by version, or is it always whatever is on disk?

## Bounds and guards

26. Is the ancestry cross-check worth building? It closes `unset BOLT_DEPTH`,
    which the counter alone cannot, at the cost of Linux-specific code and
    process-spawning tests, and only matters against a jig actively trying to
    get around the guard.
27. Is the per-user cap on live runs wanted? If so, what is the default, and is
    the default off?
28. When parallel execution arrives, is the bound a per-run budget that
    descends with nesting rather than a shared counter?

## Composition and overlay

**Settled 2026-08-26 as FR-3.14 to FR-3.15, FR-4.16 to FR-4.20, FR-5.13j,
FR-5.17 and FR-9.5g.** What follows is why, and questions 30, 31 and 33 below
are struck by it.

**Substitution resolves against one mapping, built in three layers.** Bolt seeds
it with the locations and path variables, a jig's own `definitions` block gives
each placeholder a default, and one definitions file named on the invocation
merges over both. Every key in the result is a template variable.

    bolt.common-quality.yaml            definitions:
                                          requirements: REQUIREMENTS.md
                                        ... --requirements {requirements} ...

    bolt.python-override.definitions.yaml
                                        requirements: ../REQUIREMENTS.md

**What it is instead of.** The retired bolt merged definitions by task id and
took the last writer, which is what `bolt -c common -c go` meant and what
toolbox's shared jigs are still written for. Questions 30 to 33 are that
mechanism's edges: what a parent may override, at what granularity, whether it
may disable a task, and what wins when two layers set the same key.

**What merges here is a mapping of scalars, not a jig.** So 30 and 31 have
nothing to answer: no task is reached into, and none can be switched off. 33 is
answered rather than dissolved, by FR-4.17 and FR-4.16b: the file wins over the
jig's defaults, and there is one file, so there is no ordering. **32 survives**,
and is the only one of the four that does.

**What it answers immediately.** Six Python subprojects share one set of
adjustments instead of carrying six copies. The project names a definitions
file, the six jig tasks name none and inherit it by FR-5.13a, and a seventh
that genuinely differs names its own. A project keeps several such files, one
per toolchain, scoped by their names: `bolt.python-override.definitions.yaml`
beside `bolt.go-override.definitions.yaml`.

It also settles wrench's case. `common-quality` runs traceability against
`{requirements}`, and wrench's one document at the root serves runs based at
`go/` and at `python/` because FR-4.17b resolves a relative value against the
run's base. `clank/tasks/wrench/gate/10-a-composite-jig.planning` is that case
in full.

**A conditional task is refused, and the reason is reproducibility.** FR-3.14:
state cannot be relied on to be the same between runs, so a task set that varied
with it would make two results incomparable without either of them saying so.
FR-3.14a puts a task wanted in some directories and not others into a jig of its
own, listed by the jigs that want it, where the selection is readable rather
than decided mid-run.

**What it cost.** FR-4.2 reads how a task runs off its command line and FR-9.5c
records every value bolt exposed. FR-4.17c settles the first: a literal value
cannot introduce a path variable, so FR-4.2 still reads the command as written.
FR-9.5g settles the second, putting the whole mapping in the manifest with the
layer each key came from, because the same key means different things depending
on which file won and the command line alone does not say.

**Reading FR-9.5c to write FR-9.5g found it stale.** It enumerated "the three
locations" where FR-4.1c exposes five. FR-9.5d already made it a rule rather
than a list, so nothing rested on the count, and it now says five.

**Building it corrected FR-4.17b.** The row had bolt resolving a relative value
against the base. Bolt cannot: a definition is a scalar and nothing tells
`../REQUIREMENTS.md` from `100`, so there is no set of values to apply FR-2.4
to. The outcome the row wanted holds anyway, because FR-4.1a stands the command
at the base and the value is substituted as written. The row now says which of
the two is doing the work.

**What is still open.** A schema for the file, which is wrench's: it ships
`jig`, `envelope` and `manifest` today and would need a fourth for FR-4.20 to be
checkable. The jig schema also grows a `definitions` block, by FR-3.15.

**And one hazard, recorded rather than decided.** Under FR-4.17's update
semantics a key matching no placeholder does nothing, so `line_lenght: 100`
leaves the jig's default standing and the gate runs on a value nobody chose.
It is detectable, because a jig's placeholders are readable from its commands,
but refusing it is a check on top of plain update rather than something the
semantics give. FR-4.18 covers the opposite case, a placeholder no layer fills.
Not numbered with 1 to 37, because `wrench/gate/10` cites that range and
renumbering it would move what those citations point at.


29. When a jig invokes another, what does the child inherit: environment,
    timeouts, the required default? **Two of the three are settled.** The
    environment carries the depth and the ceiling and nothing else bolt puts
    there, by FR-5.6 and FR-5.7; the rest of it is question 8 and still open.
    Config directory, output directory and definitions inherit unless a field
    says otherwise, by FR-5.13a to FR-5.13c and FR-5.13j. **Timeouts are
    unbuilt**, so nothing has been decided about them, and the required default
    is FR-13.4.
30. ~~Can a parent override a field of a child's task, and at what granularity:
    the whole task, or one field?~~ **Nothing reaches into a task.** What merges
    is a mapping of scalars, and a jig task changes a child by declaring fields
    under FR-5.13a.
31. ~~Can a parent disable a task a shared jig declares, and is the omission
    visible in the result?~~ **No, by FR-3.14, and reproducibility is the
    reason**: a task set varying with anything read at run time makes two runs
    incomparable without either saying so. FR-3.14a is the answer instead, a
    separate jig listed by the directories that want it.
32. Is there a user-level or machine-level layer above the repository's jig?
    §67 describes exactly that for pre-commit, a repository policy plus an
    independent personal one. **Open, and the only one of 30 to 33 that
    survives.** Parameterising one jig says nothing about a second, personal
    one, and FR-4.16b's single definitions file is deliberately not the place to
    put one.
33. ~~What is the precedence when the same key is set at more than one layer,
    and are collections merged or replaced?~~ **The file wins over the jig's
    defaults, by FR-4.17, and there is no ordering to settle because FR-4.16b
    allows one file.** Nothing is collected: FR-4.16c makes every value a
    scalar.

## Boundaries with the rest of the ecosystem

34. Does the per-file coverage policy of §20 live in an adapter, in a toolbox
    analyzer an adapter then reads, or in bolt? Which repository owns it
    decides whether it is a bolt requirement at all.
35. Is `result.yaml` what a ratchet node depends on directly, or does a node
    wrap it in something of its own?
36. Does bolt stamp the tree state §65 wants, or does that move to the caller?
    Bolt reads no git for anything else.
37. Does bolt have an interface other than a command line? An importable
    library used by another Go component would change what the requirements
    cover.

---

# The binary on PATH is the Go bolt, deliberately

Not a question. **This is a bridge and it is meant to be there.** Recorded here
rather than in `START_HERE.md`, which is gitignored and rewritten every session,
because at least three other sessions gate on this binary and a fact they depend
on cannot live in a file designed to disappear. The wrench session asked whether
it was deliberate or drift on 2026-08-27, which is what showed the record was
ephemeral.

`~/bin/bolt` resolves to `bolt/bin/bolt`, gitignored at `.gitignore:17`. It is
the **Go** implementation. Nothing in this tree builds it: `cmd/` and `go.mod`
left with the split, and the Rust bolt has no working CLI yet.

**Every consumer's gate runs through it.** wrench, toolbox and bolt itself. It
keeps working while the Rust rebuild happens, and the alternative was every gate
in the ecosystem going dark for the length of the rebuild.

## It was not reproducible, and it gave false verdicts. Swapped 2026-08-27

**Done, on our user's decision.** `bin/bolt` is now built from `bolt.go` at
`7604557`, reporting `v0.0.0-20260827201109-7604557974a5` with no `+dirty`.

    md5 4d319301bfda9673fc19a01f6a36dfa5   the old one, built 11:51, +dirty
    md5 7a2d3bb04e12738e71fad84c38047495   the current one, from a clean tree

    cd ~/.projects/bolt.go && CGO_ENABLED=0 go build -o ../bolt/bin/bolt ./cmd/bolt

**The old one is unreproducible and a copy is kept at
`.ephemera/bolt-stale-4d319301`.** That path is gitignored and will not survive a
cleanup, which is why the two checksums are recorded here instead: the fact
outlives the artifact.

**Why it had to go, and it was not tidiness.** The two builds disagreed about
whether a jig is valid, because the old one bundles an older schema. Filed by
toolbox and reproduced here on a jig whose only fault is `version: 1`, a YAML
number where the schema wants a semver string:

    old binary   passed: 1 execution(s)          exit 0
    new binary   at '/version': got number,      exit 1
                 want string

So a repository with that fault gated green on whatever PATH resolved and was
refused by the newer build, with no output naming which ran. That is the
false-green class the estate exists to refuse, arriving through tooling rather
than through a check.

FACT 2026-08-27, after the swap: `bolt badversion .` exits 1 and refuses, and
`bolt rust-quality .` still runs eight tasks with six passing.

FACT 2026-08-27: `bolt.go` at `7604557` is clean and rebuilds it without the
suffix, at `v0.0.0-20260827201109-7604557974a5`, with a **byte-identical help
surface** including `--definitions`, and it runs the gate.

    cd ~/.projects/bolt.go && CGO_ENABLED=0 go build -o bin/bolt ./cmd/bolt
    diff <(~/bin/bolt --help) <(~/.projects/bolt.go/bin/bolt --help)

So the reproducibility gap closes whenever somebody rebuilds from `bolt.go`.

## The durable fix belongs to dotfiles

Repointing `~/bin/bolt` at `bolt.go/bin/bolt`. Filed at
`clank/inbox/dotfiles/three-components-are-in-no-manifest/` along with `bolt`'s
manifest summary still ending in "Go". silo is chasing it.

**Leave the orphan in place until that lands.** Deleting it breaks `bolt` for
everyone and gains nothing while the link still points here.

## The cutover has one ordering constraint, and it is bolt's to hold

**`~/bin/bolt` must not move to the Rust binary before
`clank/tasks/bolt/runner/50-nested-jigs` has landed.** Raised by the wrench
session 2026-08-28 and recorded here because sequencing it is bolt's.

`bolt.wrench-quality.yaml` runs `common-quality` and `python-std-quality` at
`python/` as two jig tasks. The Rust bolt does not implement nested jigs, so
the day the symlink moves, wrench's gate stops working.

FACT 2026-08-28, measured against wrench's real jig:

    bolt wrench-quality ~/.projects/wrench
    bolt: task python-common names a jig; nested jigs are specified and
          not built yet                                          exit 1

**It fails safely, and that is deliberate.** A jig task has no `command`, so
serde refused it with `missing field command` until this was given its own
refusal. That message named the symptom, read as a malformed jig, and invited
somebody to add a command to a task that must not have one. It now names the
feature and the task.

So the cutover is gated on `runner/50`, which is unstarted and sits behind
`walking-skeleton/10`. Nothing about that is urgent while the link points at the
Go binary, and the whole hazard is that moving the link is a one-line change
somebody could make without knowing this.

## What a consumer should expect

The Rust bolt reaches the Go one's surface task by task, not at once.
`--definitions` is `clank/tasks/bolt/definitions/10-the-three-layer-mapping`,
`.ready` and behind `walking-skeleton/10`, which is at stage 4 with nothing
implemented. **A consumer designing against `--definitions` today is designing
against the Go binary**, and that is fine as long as it is deliberate.

Nothing about the Rust rebuild removes the Go binary from PATH. The swap is
dotfiles repointing the link, and it moves between two Go binaries, not from Go
to Rust.

# Files here that belong to another repository

Not a question. Recorded here because bolt has no `docs/` tree yet, and a
session that does not know this will edit a file it cannot commit.

`bin/test-traceability.py` is a **symlink** to
`toolbox/bin/test-traceability.py`. That is how a project adopts a shared
checker: the file lands at the same relative path in the adopter, and
`{config_dir}` resolves it back through the link.

    bin/test-traceability.py -> ../../toolbox/bin/test-traceability.py

**Editing it from here edits toolbox**, and the change belongs to a repository
this one cannot commit to. It then sits in toolbox's working tree looking like
toolbox's own uncommitted work. Fix the checker in toolbox and it arrives here
through the link.

The same applies to anything `link-jigs` places later, which is the shared
jigs and every adapter they name. `toolbox/jigs.yaml` says which files each set
carries.

---

# Rejected from the inbox

Two entries described the retired implementation and were resolved by rejection
rather than by action, because the tree each names no longer exists and neither
can be verified against anything. Both concepts survive independently, reached
without reading either entry.

- **`gate-runs-a-stale-fork-of-the-checkers`**, filed from toolbox, on a jig
  running a forked copy of shared checkers. The drift it describes is covered
  by FR-3.11, where an anvil image is generated from a jig's `requires` rather
  than mirroring it, and by the jig checker filed as
  `clank/inbox/toolbox/a-checker-that-validates-every-jig-in-the-config-dir/`.
- **`plan-does-not-evaluate-requires`**, filed from agent-support, on a `plan`
  subcommand that did not check `requires`. There is no `plan` in this
  specification. Whether `requires` is checked before anything runs is
  settled by FR-3.10b, reached from the architecture rather than from the entry.
