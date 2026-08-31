# Default output directories include the process id

The default run directory is `.bolt-<iso8601>-<pid>`.

The timestamp is only precise to one second, so two invocations can otherwise
resolve to the same directory and the second is refused by the collision guard.
The process id is short, separates invocations, and identifies the process that
left the directory behind.

This does not weaken the rule for an explicit `--output-dir`. Reusing an
explicit directory still refuses the run.
