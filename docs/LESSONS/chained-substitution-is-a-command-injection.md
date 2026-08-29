# Chained substitution is a command injection, and quoting does not fix it

Found 2026-08-28 by a cold-read reviewer, against the built binary. Fixed at
`7e3198f`. This is the most expensive thing this tree has found and the shape of
it generalises well past bolt.

## What happened

Substitution replaced each template variable in turn:

    line = line.replace("{each_path}", &quoted_paths);
    line = line.replace("{all_paths}", &quoted_paths);
    line = line.replace("{work_dir}", &quoted_work_dir);
    …

Every path was quoted correctly on the way in. `quote()` was not the defect and
never was.

A file named ``p{all_paths};id #`` was selected by an `{each_path}` task. The
first replacement put it into the line **correctly quoted**, as `'p{all_paths};id
#'`. The second replacement then found the literal `{all_paths}` **inside that
filename**, which was now just text in the line, and expanded it. That spliced a
fresh `'…'` string into the middle of the already-quoted region, ended the
quoting early, and left the rest of the filename on the command line as shell
syntax.

`id` executed. A second fixture escaped the base directory and created a file
beside it while the run reported success.

## Why quoting was never going to be enough

**The guarantee is not a property of the quoting. It is a property of the
quoting AND of never reading substituted bytes again.**

Each individual replacement was correct. The bug lives in the relationship
between them: pass two treated pass one's output as input, and pass one's output
contains attacker-controlled text by construction, because that is what
substitution *is*.

Chaining cannot be fixed by quoting harder. Any amount of escaping still leaves
the substituted region as text that the next pass will read.

## The fix

**One left-to-right pass.** Walk the template once, emit literal text as it
comes, and when a `{name}` is met, emit its substituted value and continue past
it. Output is never re-scanned.

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        …
        out.push_str(&substituted);
        rest = &after[close + 1..];
    }

That is the whole fix and it is shorter than what it replaced.

## What to take from it

**An unmatched brace is literal text**, and the validator and the substituter
have to agree about that or a command passes validation and then fails to
substitute.

**A refusal for an unknown placeholder belongs in the same pass.** Chained
replace left `{requirements}` in the string and handed it to the shell.
Substituting empty is the reading that fails silently: a command short an
argument, and a tool that reports something else.

**The test to keep** is `a_filename_containing_a_template_token_is_not_re_expanded`.
If substitution is ever refactored, that is the one that matters, and it fails
against every chained implementation.

## Where else this shape appears

Anywhere a template is expanded in more than one pass over the same buffer:
shell command builders, SQL fragment assembly, path templating, log-format
expanders. The question to ask is never "is the value escaped" but **"can the
output of one substitution be read as input by another"**.

A single pass is not an optimisation. It is the property.
