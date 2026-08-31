# Tasks do not consume other task output

No task consumes another task's output. Work needing several steps is one
script producing one exit code and one output.

A command chain can declare several evidence files and one adapter can read all
of them. That preserves the evidence needed by multi-stage coverage work without
creating task ordering, visibility, or dependency semantics.

Several independent interpretations of one execution are a different problem.
Supporting several adapters for one task would require both the singular
adapter contract and the fixed `output.yaml` name to change. It does not require
cross-task evidence.
