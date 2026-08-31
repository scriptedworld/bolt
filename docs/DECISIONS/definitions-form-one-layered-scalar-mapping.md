# Definitions form one layered scalar mapping

Substitution resolves against one mapping. Bolt supplies reserved locations and
path values, the jig supplies defaults, and at most one definitions file named
by the invocation replaces jig defaults by key.

The mapping is one level of scalar values. Values are literal and are not
rescanned for substitutions. Nothing deep-merges, appends, or combines, and no
definition reaches into a task or disables one.

A task set stays fixed because a run-time condition would make two runs of one
jig incomparable without either result recording that the configured task set
changed. A task needed only in some directories belongs in a separate jig whose
use is visible in configuration.

The complete mapping and the source layer for each key belong in the manifest,
because the command line alone cannot show which layer supplied a value.
