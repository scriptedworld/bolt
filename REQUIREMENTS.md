# bolt, Requirements

Derived from `silo/docs/ARCHITECTURE.md` and from nothing else. Sections 17 to
23 describe bolt directly; sections 2, 3, 24 and 25 carry the evidence model and
the ecosystem contracts it sits inside. No earlier bolt implementation,
requirements document, design note or test was read while writing this. The
provenance of that earlier material is unresolved. This document exists to
establish that the requirements reach from the architecture alone, and reading
it would have destroyed that.

Requirements are stated as observable properties. Each says what must be true of
bolt or of a run, not how anything is built.

**Status markers.** `[A]` traces to a statement in the architecture document.
`[D]` is derived from one. `[A/D]` is both. `[?]` is open, recorded so it is not
lost and carrying no test yet.

The architecture document describes an ecosystem, not a program. Much of what a
requirements document needs is therefore absent from it: the envelope's fields,
the jig's fields, how an adapter is called, what a timeout does, where artifacts
land. `NEXT_STEPS.md` lists every one of those gaps as a question. Section 9
below records the properties that must eventually hold and cannot yet be stated.

No test cites any row here, because no implementation exists. Every settled row
is uncovered under toolbox's traceability gate, and marking them `[?]` to turn
that green would misreport what is settled.

---

## 1. What a run is

*Derives from:* §17 on bolt as task ETL, §2 and §3.1 on evidence being files on
disk.

| ID | Requirement | |
|---|---|---|
| FR-1.1 | A run executes the command lines its jig declares and records what happened as files on disk. Nothing a consumer needs to know about the outcome exists only in bolt's own output streams. | [A/D] |
| FR-1.2 | A run may execute one command or many: a single tool, several tools, scripts, analyzers, a chain in which a later command consumes an artifact an earlier one produced, and other jigs nested inside it. | [A] |
| FR-1.3 | Bolt holds no knowledge of any particular tool. Which commands run, and what their output means, come from the jig. Adding a language or a checker to the ecosystem changes a jig and an adapter, never bolt. | [A] |
| FR-1.4 | A run captures each command's native results whatever form they take: stdout, stderr, exit code, JSON, XML, a coverage report, or arbitrary files the command generated. Those results survive the run as evidence. | [A/D] |
| FR-1.5 | Task ETL is the abstraction and quality checking is its first use. A jig that runs no checker and reaches no verdict about code is a legitimate run. | [A/D] |

## 2. Jigs

*Derives from:* §21 on the jig as configuration and composition unit, §19 on
composition, §20 on per-file coverage, §24 on toolbox distributing jigs.

| ID | Requirement | |
|---|---|---|
| FR-2.1 | A jig is the unit of configuration and composition. What bolt executes for a project is read from that project's jig. | [A] |
| FR-2.2 | A jig may invoke another jig, and an invoked jig may invoke others in turn. | [A] |
| FR-2.3 | A jig may be scoped to a subtree, so one repository composes a shared quality jig, a secret-scanning jig, and a separate language jig for each of `backend/`, `tooling/` and `frontend/`. | [A] |
| FR-2.4 | Organisation-wide, language-specific and repository-specific behaviour compose through jigs, with none of it hard-coded into bolt. | [A] |
| FR-2.5 | A jig maintained outside the repository and made available inside it, as toolbox's `link-jigs` does, runs without being copied into the tree. | [D] |
| FR-2.6 | A run may apply a policy its producing tool does not implement, by running the producer, feeding that output to an analyzer carrying the policy, and adapting the analyzer's output. | [A] |
| FR-2.7 | FR-2.6 is sufficient to express a threshold held per file, `each file: branch coverage >= 80%`, wherever the toolchain reports meaningful branch coverage. A project average is a different policy and does not satisfy it, because an average lets a well-tested large file conceal a poorly tested small one. | [A] |

## 3. Adapters

*Derives from:* §17 and §18 on adapters, §23 on child exit codes being data.

| ID | Requirement | |
|---|---|---|
| FR-3.1 | An adapter turns one execution element's native output into a canonical result envelope. Nothing else in bolt decides whether an element passed. | [A/D] |
| FR-3.2 | A generic adapter treats exit 0 as pass and any other exit code as fail, and retains stdout and stderr as supporting evidence. A command with those semantics needs nothing written for it. | [A] |
| FR-3.3 | An adapter is chosen by the output format it reads. Any tool emitting a format some adapter understands reuses that adapter, whoever wrote the tool. | [A] |
| FR-3.4 | Adapters read structured formats as well as exit codes: Cobertura, pytest JSON, and other structured test and coverage reports. | [A] |
| FR-3.5 | A child process's exit code is data an adapter consumes. Bolt reaches no verdict of its own from it. | [A] |

## 4. Result envelopes

*Derives from:* §3.1 on the shared envelope model, §3.4 on malformed evidence,
§22 on the files a run produces.

| ID | Requirement | |
|---|---|---|
| FR-4.1 | Every execution element produces a result envelope, and so does the merge. | [A] |
| FR-4.2 | Bolt's envelopes use the ecosystem's shared envelope vocabulary. An element envelope, a merge envelope, a task-node envelope and an azimuth envelope are read the same way by the same consumer. | [A] |
| FR-4.3 | An envelope bolt writes conforms to the envelope schema. A malformed envelope is a failure of whatever produced it, so bolt emitting one is a defect and never a reportable outcome. | [A/D] |
| FR-4.4 | An envelope is written whole or not at all. A run killed partway through leaves no half-written envelope for a consumer to read as authoritative. | [D] |
| FR-4.5 | An element whose adapter could not produce a result is distinguishable from an element whose adapter produced a failing result. A crashed producer has reached no authoritative outcome; a failing one has. | [D] |
| FR-4.6 | Envelopes are files and survive the process that wrote them. | [A/D] |
| FR-4.7 | Each execution element writes its own file, named after the element, and the merge writes `run_result.yaml`. | [A] |

## 5. The merge and the gate

*Derives from:* §22.

| ID | Requirement | |
|---|---|---|
| FR-5.1 | The merge passes only when every required constituent result passes. | [A] |
| FR-5.2 | The merged result carries the reasons, failures, statistics, metadata and evidence references its constituents produced, so what failed and why is readable from the merged file alone. | [A/D] |
| FR-5.3 | The merged result references its constituent envelopes without replacing them. Both levels stay on disk. | [D] |

## 6. Exit status

*Derives from:* §23, and §3.1 on not inferring success from a child's exit code.

| ID | Requirement | |
|---|---|---|
| FR-6.1 | Bolt's exit status answers one question: could bolt execute the requested task ETL? | [A] |
| FR-6.2 | A run in which every element executed and some tools reported failures exits 0 and writes `passed: false`. That pairing is correct. | [A] |
| FR-6.3 | The authoritative quality verdict is the envelope. A caller reading bolt's exit status to learn whether the tools passed has read the wrong thing. | [A] |
| FR-6.4 | Bolt exits non-zero when it could not carry out the requested ETL, and says why on its own error stream, because in that case there may be no envelope to read. | [D] |

## 7. Where a run happens

*Derives from:* §4 on the control plane sitting outside worker sandboxes, §63
and §65 on validating an exact tree state.

| ID | Requirement | |
|---|---|---|
| FR-7.1 | A run needs nothing from outside the tree it is given. Control-plane state is absent from a worker sandbox, so a run depending on it could not execute there. | [D] |
| FR-7.2 | A run's whole effect is the evidence it writes. It changes no graph state, no task state and no other control-plane record. | [D] |
| FR-7.3 | The same jig runs against whatever tree state it is pointed at, including a throwaway copy prepared to test a prospective merge. | [D] |
| FR-7.4 | A result identifies the exact repository state it was produced from, so a later reader can tell whether the evidence still describes the tree in front of them. Trust attaches to a tree state and never to a branch name. | [D] |

## 8. The program

*Derives from:* §5 on licensing, §25 on standardised images, §70 on dogfooding.

| ID | Requirement | |
|---|---|---|
| NFR-8.1 | Bolt runs itself. Its own quality gate is a bolt run over its own repository. | [A] |
| NFR-8.2 | Bolt installs into a standardised development image beside a toolchain it knows nothing about. | [D] |
| NFR-8.3 | Bolt is MIT licensed. | [A] |

## 9. Open

Each row states a property that must eventually hold and cannot be stated yet.
The questions that would settle them are in `NEXT_STEPS.md`.

| ID | Requirement | |
|---|---|---|
| FR-9.1 | The envelope has one defined field schema, published where every producer and consumer validates against it. The architecture names the envelope as a primary contract and does not state its fields. | [?] |
| FR-9.2 | An execution element declares what it runs, what adapts it, and what it depends on, through a defined set of fields. | [?] |
| FR-9.3 | An adapter is invoked through a defined contract fixing what it receives and what it must return. | [?] |
| FR-9.4 | An element that exceeds a time budget reaches a defined outcome, and the run stops or continues by a stated rule. | [?] |
| FR-9.5 | A run's artifacts are written to defined paths under defined names, so a consumer finds an element's evidence without guessing. | [?] |
| FR-9.6 | Which files an element runs over is decided by a stated rule. | [?] |
| FR-9.7 | Element ordering and concurrency follow stated rules, and the merged result does not vary with execution order. | [?] |
| FR-9.8 | A jig invoking another combines their settings by a stated precedence, and a parent can override or disable what a child declares. | [?] |
| FR-9.9 | Whether a constituent is required is declared, with a stated default. | [?] |
| FR-9.10 | The boundary between bolt and the rest of the ecosystem is fixed: what bolt produces, and what ratchet, toolbox and the caller produce around it. | [?] |
