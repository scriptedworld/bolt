# Composition is a command line

A jig composes another run by invoking `bolt` in an ordinary task command.
There is no separate jig task kind.

Every capability of the retired task kind already had a command-line spelling.
Treating bolt like any other tool keeps one general execution rule instead of a
special case with separate fields, inheritance, emptiness, and containment
semantics. The cost is that a parent's grant is visible in its command line but
is not separately schema-checked as a nesting relationship.

The fold remains intact. The child prints its result path, the task adapter
reads the child result and writes a task envelope, and the parent folds that
envelope as an ordinary constituent.

Shell composition and jig composition use different verdict paths.
`--result-to-exitcode` lets `&&` observe a failed result. An adapter lets a
child's verdict enter a parent result while the default exit status continues to
describe whether bolt carried out the run.
