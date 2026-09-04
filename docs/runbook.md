# Runbook

Bolt is one of three repositories that work together. `wrench` is a build
dependency. `toolbox` holds the quality gate, and ten repositories adopt it from
that one source, so a change to the standard reaches all of them instead of
being re-argued in each. The cost is that a fresh clone has no gate until you
link one, which is the step below and the reason it exists.

## What has to be on the box

Rust 1.97 or newer, with `rustfmt` and `clippy`.

| Tool | The task that needs it |
|---|---|
| `cargo-llvm-cov` | `tests` |
| `cargo-audit` | `vuln` |
| `cargo-deny` | `licences` |
| `python3` with PyYAML | `traceability`, and every checker and adapter toolbox ships |
| `gitleaks`, `detect-secrets` | the secret scan, which is a separate run |

Installed here with the spellings `dotfiles/bin/setup` uses:

```sh
rustup component add rustfmt clippy
cargo install --locked cargo-llvm-cov cargo-audit cargo-deny
uv tool install detect-secrets
mise use --global gitleaks@latest
```

**PyYAML has to be importable by the `python3` that runs the adapters**, and
nothing declares it. `adapters/common/bolt-result.py` and
`adapters/rust/coverage.py` both import `yaml`, so a machine without it fails
the `secrets` composition and the coverage judgement with an ImportError rather
than with a verdict.

```sh
python3 -c "import yaml"   # must succeed for the gate to run
```

## Clone the three as siblings

```sh
git clone https://github.com/scriptedworld/bolt.git
git clone https://github.com/scriptedworld/wrench.git
git clone https://github.com/scriptedworld/toolbox.git
```

The parent directory's name does not matter and the sibling relationship does.
`Cargo.toml` reaches wrench at `../wrench/rust`, and the links made next are
relative to `../toolbox`.

## Link the shared gate

From the bolt checkout:

```console
$ python3 ../toolbox/bin/link-toolbox.py . rust --yes
linked 8 file(s)

all 8 link(s) present and correct
```

**Name `rust`, not `common`.** The sets nest — `rust` includes `common`, which
includes `secrets` — so naming `rust` alone gets all eight. Naming `common`
gets five, and the five are the wrong five: `bolt.rust-std-quality.yaml`,
`adapters/rust/coverage.py` and `config/rust/clippy.toml` are not among them, so
the second of the two gate runs below has no jig to run and fails as an
unreadable file. This document said `common` until 2026-09-04, which is the
whole of that mistake.

    common   bolt.secrets.yaml, bolt.common-quality.yaml,
             bin/test-traceability.py, bin/suppression-register.py,
             adapters/common/bolt-result.py
    rust     the five above, plus bolt.rust-std-quality.yaml,
             adapters/rust/coverage.py, config/rust/clippy.toml

`--plan` shows the list and stops; `--check` re-verifies at any time and exits 1
on drift.
Nothing is ever overwritten, so a real file where a link belongs is reported and
left alone. The links are gitignored, and `git status` being clean afterwards is
how you know the set is complete rather than partly ignored.

## Build, then gate

```console
$ cargo build --release
$ ./target/release/bolt common-quality . --output-dir .bolt-gate
$ ./target/release/bolt rust-std-quality . --output-dir .bolt-gate-rust
/home/you/bolt/.bolt-gate/result.yaml
```

Two runs against toolbox's jigs; this repository carries none of its own since
2026-09-03. Three tasks in `common-quality`, six in `rust-std-quality`. Bolt
exits 0 whenever it could carry the run out, so `success` in `result.yaml` is
the answer and the exit status is not. `traceability` fails on purpose;
`CONTRIBUTING.md` says why and how to read its count.

The secret scan is its own run, over the working tree and the history:

```console
$ ./target/release/bolt secrets .
```

## When it goes wrong

**A jig `is unreadable`, naming a path that is not on disk.**

    bolt: the jig /home/you/bolt/bolt.secrets.yaml is unreadable: wrench:
    reading /home/you/bolt/bolt.secrets.yaml: No such file or directory

The links are absent. Run the linking step. It reads like a corrupt checkout
rather than like a step that has not been done yet.

**`traceability exited 2`**, which looks like a coverage failure and is not. The
task's own `stderr` under `.bolt-gate/work/traceability-1/` carries the cause:

    python3: can't open file '/home/you/bolt/bin/test-traceability.py'

Same cause. Once linked, that task exits 1 and reports a coverage figure.

**`failed to load source for dependency wrench`** at build time: wrench is not
beside the checkout.

**A git worktree cannot run the gate.** A worktree gets tracked content only and
the links are untracked, so link into the worktree as well or gate from the
checkout.

## If they cannot be siblings

`--absolute` points the links at wherever toolbox actually is, at the cost of
links that do not travel with the project:

```sh
python3 /path/to/toolbox/bin/link-toolbox.py . common --absolute --yes
```

wrench has no equivalent. The path dependency needs the sibling.
