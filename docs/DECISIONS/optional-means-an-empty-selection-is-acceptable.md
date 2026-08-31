# Optional means an empty selection is acceptable

The task field is named `optional`. It says that an empty selection is an
acceptable result for the task. `allow-empty` describes the mechanism instead
of the meaning.

An optional task with an empty selection does not execute and produces no
constituent. A required task with an empty selection fails. The distinction is
also why an empty task set does not create a third, absent verdict.
