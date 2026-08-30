# Security

## What bolt is trusted to do

Bolt executes the commands a jig declares. Each one runs through `sh -c` with
the privileges of whoever started bolt, and adapters are separate programs bolt
resolves by name from the config directory and runs the same way.

**A jig is executable input.** Running one is running the commands in it, so
treat a jig from somewhere else the way you would treat a shell script from
somewhere else. The same goes for the adapters a jig names, which travel with it
and are resolved relative to it.

That is the design and not an oversight. Bolt knows nothing about any tool it
runs, which is what lets a project change its gate by editing a jig.

## What bolt does defend

**A filename cannot become a command.** Every substituted path is quoted, and
substitution is a single left-to-right pass over the command line: bytes that
have been substituted are never read again, so a filename containing what looks
like a placeholder stays a filename.

Both halves are load-bearing. Chained replacement, one pass per variable,
re-expands a token appearing inside an already-substituted filename and breaks
the quoting. A file named ``p{all_paths};id #`` executed `id`. The property is
covered by `a_filename_containing_a_template_token_is_not_re_expanded` in
`tests/skeleton.rs`, and
`docs/LESSONS/chained-substitution-is-a-command-injection.md` has the
measurement.

**A timed-out command does not outlive its run.** A time limit kills the process
group, so a command that spawned children does not leave them writing into a
directory bolt has finished with.

**A run refuses a directory that already holds a run**, so two runs cannot
interleave their evidence.

## What is not a vulnerability here

A jig that runs a destructive command, an adapter that does something
unexpected, or a definitions file that redirects a command at another path. All
three are the jig author's to answer for, in the same way a Makefile's contents
are. Bolt carrying them out as written is what it is for.

A tool bolt ran reporting a vulnerability in your project is that tool's finding,
not bolt's.

## Reporting a vulnerability

Report it privately through GitHub private vulnerability reporting, under this
repository's Security tab. Do not open a public issue.

Include the jig, the command line, and the run directory if you have one. The
manifest under `work/<task>-<ordinal>/manifest.yaml` records the command exactly
as executed and every value substituted into it, which is usually the whole
reproduction.

## Supported versions

There is no released version and no published crate. The only supported code is
the current state of the default branch, and there is nothing pinned in the
wild to patch.
