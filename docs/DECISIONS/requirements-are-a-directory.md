# Requirements are a directory

Requirements belong in one file per category under
`docs/REQUIREMENTS/<category>/`. The single-file format is retired rather than
kept beside the directory format.

The migration follows support in the traceability checker. Splitting first
would turn a checker that reports incomplete traceability into one that cannot
read its input.
