# The CLI prints only the result path

Standard output contains the result path and no summary line. The authoritative
verdict and counts live in the result envelope.

A summary duplicates the envelope and can disagree with it. Pairing an overall
failed verdict with the total execution count, for example, reads as though the
count were the number of failures. A caller instead reads the one authoritative
document whose path bolt prints.

A human-oriented summary, if one is added, belongs behind an explicit flag and
must count what it says it counts.
