# Result to exit code has two outcomes

`--result-to-exitcode` returns 0 when the envelope says `success: true` and 1
otherwise. There is no third outcome for a missing verdict and no separate
engine code.

The envelope defines `success` as the authoritative verdict. Reading a
neighbouring `kind` to promote a refusal into a third state would overrule that
field.

A task set always resolves. An optional task that matches nothing is satisfied.
A required task that never runs has failed. A path reaching this flag without
an envelope is an implementation defect, not another outcome.
