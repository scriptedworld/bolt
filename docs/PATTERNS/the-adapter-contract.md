# The adapter contract

What bolt hands an adapter, and what it expects back. Written down here because
**it was written down nowhere**: a port had to derive it by reading the Go
build's `internal/adapter/adapter.go`, and three of toolbox's four adapters had
drifted onto a retired version of it with nothing detecting the drift.

Verified against `src/adapter.rs` and `src/run.rs` at bolt `1a77e4e`.

## What an adapter is

A separate program that reads one execution's captured output and writes an
envelope saying whether it passed. It is chosen by **the format it reads, not by
the tool that produced it**: an adapter that reads a count off stdout serves any
tool emitting one.

Where an adapter reaches a result, **that result is the verdict** and bolt does
not second-guess it. A task naming no adapter gets the generic exit-code one,
which is the single adapter that needs to know nothing about what it is reading.

## The invocation

Bolt builds this line, substitutes it like a command, and runs it through `sh`:

    <adapter> --stdout <work>/stdout --stderr <work>/stderr
              --exitcode <work>/exitcode
              --project-root <root> --base-dir <base> --work-dir <work>
              [--evidence <work>/<file>]...

**Flags, always, and all of them, every time.** Nothing is positional and nothing
is omitted when empty. The three locations are the same three every task gets.

`--evidence` appears once per file the task **declared**, and never for anything
else the tool happened to leave in the work directory. Declared, never
discovered: discovery would hand an adapter whatever was lying around and let
something irrelevant decide a run.

**Variables are underscored and flags are hyphenated**, as a rule rather than an
accident: `{work_dir}` in a jig, `--work-dir` on a command line.

## What it gets

**Nothing on stdin.** It is `/dev/null`. An adapter reading stdin is on the
retired contract and will block or read nothing.

**The exit code as a file**, not as an argument and not as a verdict. Whether
that number means anything is the adapter's judgement, which is the whole reason
the exit-code adapter is one adapter among several rather than a special case in
bolt.

**Its own stdout and stderr are discarded.** The envelope is the channel. Chatter
is not collected anywhere, so an adapter that prints instead of writing the
envelope reports nothing.

## What it must produce

An envelope at **`<work_dir>/output.yaml`**. The path is the work directory it
was handed and the name never varies; no flag says where it goes.

    success: false
    reasons:
      - kind: findings
        message: "3 problems"

`success` is the only key every envelope carries. `reasons` is required when
success is false, and each reason needs both `kind` and `message`: the kind so a
consumer can tell one sort of failure from another without reading English, the
message so any consumer can render it.

**Bolt validates it against wrench's envelope schema on the way in** and does not
reparse to compare formatting. An adapter is free to write whatever canonical
form it likes.

## The three ways to fail, which bolt tells apart

    adapter-failed          it ran and exited non-zero
    adapter-wrote-nothing   it exited 0 and left no output.yaml
    adapter-wrote-invalid   it left one that will not parse or validate

Kept apart because they have different causes and different fixes. Bolt writes
the envelope itself in each case, because none of them left a result to take.

**The envelope is removed before the adapter runs.** One left by an earlier fold
would otherwise satisfy "the adapter wrote one" and hand a silent adapter the
previous run's verdict. The Go build found that; it is mutation-tested here.

## Two things that are not the adapter's problem

**A killed command.** When a time limit fires, the adapter still runs, over
whatever the tool managed to gather: forty problems reported before hanging are
forty real problems. The execution fails regardless of what the adapter
concluded, and bolt adds that reason itself.

**Being told which task it served.** An adapter never learns the task name. The
merge takes that from the work directory, which keeps this contract as narrow as
it is.

## An adapter reading a composed child's result takes the last line

FR-10.3a says bolt prints where the result is on stdout and prints nothing else
there. That describes this bolt. `bolt.go` is still installed and reachable by
name, and it prints a transcript first:

    bolt.go                               bolt
    1  always-passes-0                    1  /…/result.yaml
    2
    3  passed: 1 execution(s)
    4  /…/result.yaml

Reading the first line gets a task name. An adapter that takes the last
non-empty line is correct against both. toolbox's `bolt-result` does this.

The contract is not weakened to match. "Prints nothing else there" is the
property worth having, and relaxing it to "the last line" would license bolt to
print other things on stdout, which is the summary line FR-10.3's note exists to
keep out. The strict rule is what bolt promises; last-line is how a consumer
stays robust while a second implementation is reachable.

Flag order differs in the safe direction. `bolt.go` refuses flags written after
the positionals and this build accepts them anywhere, so anything authored
against `bolt.go` keeps working here. Put flags first if it has to run against
both.

## Writing one

An adapter is a filter with a fixed argument list. In practice:

    parse the flags you care about, ignore the rest
    read what you were pointed at
    write output.yaml
    exit 0

Exiting non-zero says *you* failed, not that the tool did. A tool with findings
is `success: false` in the envelope and exit 0 from the adapter.
