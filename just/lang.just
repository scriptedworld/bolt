# The Rust layer for bolt, which is a binary every gate in the estate runs.
#
# Two things differ from an ordinary Rust project, and both come from bolt
# gating itself.
#
# The gate runs the bolt built from this tree, never the installed one.
# `bin/bolt` is what the rest of the estate runs, and gating with it would run
# the previous bolt over the current source: a break in execution, the adapters
# or the fold would go unreported by the program that was broken.
# docs/LESSONS/the-installed-binary-gates-everything.md has the measurement. So
# `_verdict` below replaces base's, building first and running
# target/release/bolt. The secrets child that common-quality composes still
# resolves `bolt` through PATH, so `install` is what brings that half current.
#
# The Rust jig is toolbox's and is not linked into this tree, so every call to
# it carries `--config-dir ../toolbox`. common-quality and secrets are linked at
# the root and need nothing.

# The same verdict as base's, from the tree's own build.
_verdict jig *flags:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release --quiet
    out=$("{{ justfile_directory() }}/target/release/bolt" {{ flags }} "{{ jig }}" .)
    result=$(printf '%s\n' "$out" | tail -1)
    if [ ! -f "$result" ]; then
        echo "REFUSED: bolt wrote no result for {{ jig }}" >&2
        printf '%s\n' "$out" >&2
        exit 1
    fi
    if grep -q '"success": *true' "$result"; then
        echo "pass  {{ jig }}  $result"
    else
        echo "FAIL  {{ jig }}  $result" >&2
        exit 1
    fi

# Both jigs, each run whether or not the other failed. The Rust jig's `tests`
# task is the suite with coverage judged per file, so `test` and `coverage` are
# inside it and not repeated here.

# everything: quality, tests, coverage, secrets
checks:
    #!/usr/bin/env bash
    set -uo pipefail
    status=0
    just _verdict common-quality || status=1
    just _verdict rust-std-quality --config-dir ../toolbox || status=1
    exit "$status"

# the suite
test:
    cargo test

# Coverage is judged per file by toolbox's lcov adapter inside the Rust jig, and
# nothing lighter applies the same floor, so this runs that jig whole.

# the suite with coverage, and the minimum enforced
coverage:
    @just _verdict rust-std-quality --config-dir ../toolbox

# formatting verified, nothing written
format-check:
    cargo fmt --check

# formatting written
format:
    cargo fmt

# every target, so the tests compile too
build:
    cargo build --all-targets

# `cargo package` cannot work yet: Cargo.toml takes wrench from ../wrench/rust,
# so the crate is not self-contained. The release binary is what bolt ships.

# whatever this language distributes
dist:
    cargo build --release
    @echo "dist: target/release/bolt"

# Every gate in the estate runs bin/bolt, so this changes all of them at once.
# Bytes are compared, not timestamps: `cp` sets a fresh mtime whatever it
# copied, and a check that cannot fail is not one.

# put the built artefact where it is used from
install:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release
    if cmp -s target/release/bolt bin/bolt; then
        echo "current  bin/bolt"
        exit 0
    fi
    cp target/release/bolt bin/bolt
    if cmp -s target/release/bolt bin/bolt; then
        echo "installed  bin/bolt"
    else
        echo "STALE: bin/bolt does not match target/release/bolt" >&2
        exit 1
    fi

# bin/bolt is executed by every gate in the estate and is left alone. target/
# is this tree's build output and nothing else runs it.

# remove run directories and build output
clean:
    rm -rf .bolt-*
    cargo clean
