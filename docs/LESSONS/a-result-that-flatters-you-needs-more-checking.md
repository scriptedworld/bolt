# A result that flatters you needs more checking, not less

`a-check-that-answers-a-weaker-question.md` is about instruments that cannot see
the thing. This is about having the right instrument, knowing the rule, running
it twice, and then not running it on the case that mattered.

Both instances below are from one session, 2026-08-30.

## Skipping the check on the third of three

The mutation probe has a documented hazard: a mutation that misses the branch a
row lives on looks exactly like a weak test, so a survival is a question until
the probe itself is checked.

Three rows survived. The rule got applied to two.

    FR-9.2a   probe hit a struct field; the directory name is built from a
              separate `index + 1` four lines above.        checked, false
    FR-5.7    probe raised DEFAULT_CEILING; the test sets BOLT_MAX_DEPTH
              itself and never reads the default.           checked, false
    FR-8.3    probe rewrote the verdict on its way into the file and left
              the fold alone.                               NOT CHECKED

The third was the one that confirmed a defect with estate-wide reach, on a day
this estate had been told that standing debt is a dent in the resume armour. It
was written into a commit, into `docs/PROJECT.md`, and sent to two other
sessions before the correction landed: the envelope it produced was valid and
said exactly what that build concluded. Retracted at `25fc253`.

The two that got checked were unwelcome. The one that got shipped was welcome.
That is the whole pattern.

## A count that reconciles is not evidence the edit was clean

The script merging thirteen requirement rows into their parents substituted each
row away and then tidied the leftover blank with a document-wide
`"\n\n|" -> "\n|"`. Every count came out right:

    live rows      244 -> 229      correct
    retired ids     48 ->  63      correct
    every row       3 cells        correct
    no id both live and retired    correct

Four checks, all passing, none of them looking at the file. The replace had run
once per merge over the whole document and eaten the blank line before every
table in it, fourteen of them. It surfaced because 465 minus 15 plus 15 is 465
and the file was 451.

Deleting a matched line instead of substituting it away and tidying afterwards
is the fix. Wanting the merge to have worked is why four green checks were
enough.

## What to do

Ask what result you were hoping for before you read the output. Where the answer
is the one you wanted, that is the point to run the verification step you would
have run on a result you did not want.

The two instances differ in which step went missing. One skipped a rule already
written down; the other ran four checks that all measured the same easy thing.
Both are the same failure to ask what a wrong answer would have looked like.
