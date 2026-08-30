---
name: Bug report
about: Bolt did something other than what it says it does
---

## What you ran

The command line, and the jig it named. Paste the jig if you can; if it is
sensitive, the task that misbehaved is usually enough.

## What happened, and what you expected instead

## The run directory

Bolt prints the path to `result.yaml` and keeps everything it did beside it.
The three files that answer most questions:

    result.yaml                              the verdict and its reasons
    work/<task>-<ordinal>/manifest.yaml      the command as executed, and
                                             every value substituted into it
    work/<task>-<ordinal>/stderr             what the tool said

The manifest is the important one. It records the command after substitution,
which is where a wrong path or an unexpected selection shows up.

## Your environment

    bolt      the commit you built from, since there is no released version
    rustc     rustc --version
    platform  uname -sr

## If the run refused

A refusal prints a reason with a `kind` on it and still writes a `result.yaml`.
Include the kind: it says which of sixteen refusals happened, and they have
sixteen different fixes.
