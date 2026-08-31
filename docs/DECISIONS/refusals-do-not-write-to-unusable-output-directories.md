# Refusals do not write to unusable output directories

Bolt normally writes a failed result when it refuses a run. Two refusals are
exempt because the refusal concerns the directory where that result would be
written.

When the base directory is missing, writing the result inside it would create
the thing whose absence bolt is refusing. When the output directory already
holds a run, writing the refusal there would replace a completed verdict with a
refusal that executed nothing.

These refusals report the reason on standard error and exit with status 1. A
caller that requires a parseable result in either case names an output directory
outside the base.
