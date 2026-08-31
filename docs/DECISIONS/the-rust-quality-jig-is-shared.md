# The Rust quality jig is shared

The Rust quality jig belongs in the shared jig collection. Its common tasks are
traceability, suppression checking, and complexity checking. Its Rust-specific
tasks are formatting, linting, building, testing, coverage, vulnerability
checking, and licence checking.

Promotion preserves the checks rather than merely their tool names. Replacing
the complexity task with native lints requires equivalent thresholds. The
measured lizard limits were 60 lines and 5 arguments, while the native defaults
were 100 lines and 7 arguments. Removing the stricter tool without pinning its
limits would silently relax the gate.

Every executable invoked by the jig belongs in `requires`. Coverage evidence
also needs a policy and a checker; producing a coverage file alone enforces no
coverage requirement.
