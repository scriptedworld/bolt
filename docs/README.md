# bolt documentation

`README.md` at the top of the repository is the front door.
`CONTRIBUTING.md` is what a change is held to, and `SECURITY.md` states the
trust boundary and how to report a vulnerability.

    runbook.md         getting bolt, wrench and toolbox onto a box so the
                       gate runs, and what the two setup failures look like
    jig-reference.md   every jig field, every placeholder, the run directory,
                       the reason kinds, and the exit status rule
    PROJECT.md         what bolt is for, how it is gated, what is decided,
                       and what is not done

## Patterns

    the-adapter-contract.md    what bolt hands an adapter and what it expects
                               back, which is what you write one against
    measuring-an-entry-point.md    how the command line is held to the same
                               coverage bar as everything else

## Lessons

Each one is a mistake worth not repeating, written once.

    the-installed-binary-gates-everything.md   a self-hosted gate run with a
                               stale binary reports on the tree it was built
                               from
    a-second-build-answers-for-the-tree.md     the same hazard at one
                               worktree's scale
    chained-substitution-is-a-command-injection.md   why substitution is one
                               left-to-right pass and not one pass per variable
    a-check-that-answers-a-weaker-question.md  a check that runs, reports
                               success, and never looked at the thing
    a-result-that-flatters-you-needs-more-checking.md   a mutation probe that
                               misses its branch survives meaninglessly

`REQUIREMENTS.md` and `NEXT_STEPS.md` sit at the top of the repository.
Requirements state what must be true, including the retired rows and where each
one went. `NEXT_STEPS.md` holds the open questions and the defaults taken
against them.
