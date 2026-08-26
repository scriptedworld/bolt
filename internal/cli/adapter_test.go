package cli_test

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/scriptedworld/bolt/internal/cli"
)

// adapter writes an executable script into the config directory, which is
// where an adapter is resolved from.
func adapter(t *testing.T, root, name, script string) {
	t.Helper()
	path := filepath.Join(root, name)
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("mkdir for %s: %v", name, err)
	}
	if err := os.WriteFile(path, []byte("#!/bin/sh\n"+script), 0o755); err != nil {
		t.Fatalf("writing %s: %v", name, err)
	}
}

// writesEnvelope is an adapter that reads the flags bolt hands it and writes a
// verdict of its own, ignoring the exit status entirely.
const writesEnvelope = `
while [ $# -gt 0 ]; do
  case "$1" in
    --stdout) out="$2"; shift 2 ;;
    --work-dir) work="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [ -s "$out" ]; then
  printf '"reasons":\n  - "kind": "found-something"\n    "message": "the tool printed on stdout"\n"success": false\n' > "$work/output.yaml"
else
  printf '"success": true\n' > "$work/output.yaml"
fi
`

// COVERS: FR-6.1, FR-6.4, FR-6.10 | positive
func TestTheAdapterReachesTheVerdictAndNotTheExitStatus(t *testing.T) {
	// The whole point: a tool that lists problems on stdout and exits 0. Its
	// exit status answers "did it run", never "is this clean".
	root := project(t, `
tasks:
  - name: listy
    command: "echo a.go is not formatted"
    adapter: adapters/listy.sh
`, nil)
	adapter(t, root, "adapters/listy.sh", writesEnvelope)

	got := runBolt(t, root)

	if got.status != cli.Ran {
		t.Fatalf("status %d: %s", got.status, got.stderr)
	}
	if got.result["success"] != false {
		t.Fatalf("the run passed, so the exit status was believed over the adapter: %v", got.result)
	}
	reasons := got.result["reasons"].([]any)
	if reasons[0].(map[string]any)["kind"] != "found-something" {
		t.Errorf("the reason is not the adapter's: %v", reasons[0])
	}
}

// COVERS: FR-6.1, FR-6.3 | positive
func TestAnAdapterMayPassACommandThatExitedNonZero(t *testing.T) {
	// The other direction, and the reason bolt reaches no verdict from the
	// number itself: whether it explains anything is the adapter's judgement.
	root := project(t, `
tasks:
  - name: noisy
    command: "exit 7"
    adapter: adapters/always-ok.sh
`, nil)
	adapter(t, root, "adapters/always-ok.sh", `
while [ $# -gt 0 ]; do
  case "$1" in --work-dir) work="$2"; shift 2 ;; *) shift ;; esac
done
printf '"success": true\n' > "$work/output.yaml"
`)

	got := runBolt(t, root)

	if got.result["success"] != true {
		t.Errorf("a non-zero exit overrode the adapter: %v", got.result)
	}
}

// COVERS: FR-6.2, FR-6.2a | positive
func TestTheDefaultInvocationNamesTheCapturesAndTheLocations(t *testing.T) {
	root := project(t, `
tasks:
  - name: records
    command: "true"
    adapter: adapters/record.sh
`, nil)
	// Writes the argv it was given beside the envelope, so the test asserts on
	// what bolt actually handed it.
	adapter(t, root, "adapters/record.sh", `
work=""
for a in "$@"; do
  if [ "$prev" = "--work-dir" ]; then work="$a"; fi
  prev="$a"
done
printf '%s\n' "$@" > "$work/argv"
printf '"success": true\n' > "$work/output.yaml"
`)

	got := runBolt(t, root)
	if got.status != cli.Ran {
		t.Fatalf("status %d: %s", got.status, got.stderr)
	}

	argv, err := os.ReadFile(filepath.Join(got.output, "work", "records-0", "argv"))
	if err != nil {
		t.Fatalf("the adapter did not record its argv: %v", err)
	}
	for _, flag := range []string{"--stdout", "--stderr", "--exitcode", "--project-root", "--base-dir", "--work-dir"} {
		if !strings.Contains(string(argv), flag) {
			t.Errorf("%s was not passed:\n%s", flag, argv)
		}
	}
}

// COVERS: FR-6.2b | positive
func TestTheEnvelopeGoesInTheWorkDirectoryAndNowhereElse(t *testing.T) {
	root := project(t, `
tasks:
  - name: writes
    command: "true"
    adapter: adapters/ok.sh
`, nil)
	adapter(t, root, "adapters/ok.sh", `
while [ $# -gt 0 ]; do
  case "$1" in --work-dir) work="$2"; shift 2 ;; *) shift ;; esac
done
printf '"success": true\n' > "$work/output.yaml"
`)

	got := runBolt(t, root)

	if _, err := os.Stat(filepath.Join(got.output, "work", "writes-0", "output.yaml")); err != nil {
		t.Errorf("no envelope in the work directory: %v", err)
	}
	if got.result["success"] != true {
		t.Errorf("the merge did not read it: %v", got.result)
	}
}

// COVERS: FR-6.2c | positive
func TestDeclaredEvidenceIsWhatTheAdapterIsPointedAt(t *testing.T) {
	root := project(t, `
tasks:
  - name: produces
    command: "echo data > {work_dir}/report.txt && echo junk > {work_dir}/scratch.tmp"
    adapter: adapters/count.sh
    evidence:
      - report.txt
`, nil)
	adapter(t, root, "adapters/count.sh", `
n=0
while [ $# -gt 0 ]; do
  case "$1" in
    --evidence) n=$((n+1)); shift 2 ;;
    --work-dir) work="$2"; shift 2 ;;
    *) shift ;;
  esac
done
printf '"success": true\n' > "$work/output.yaml"
printf '%s\n' "$n" > "$work/evidence-count"
`)

	got := runBolt(t, root)

	count, err := os.ReadFile(filepath.Join(got.output, "work", "produces-0", "evidence-count"))
	if err != nil {
		t.Fatalf("reading the count: %v", err)
	}
	if strings.TrimSpace(string(count)) != "1" {
		t.Errorf("the adapter saw %s evidence flags, want 1: the undeclared temporary was passed", count)
	}
}

// COVERS: FR-6.14 | negative
func TestADeclaredEvidenceFileThatWasNotProducedFailsTheTask(t *testing.T) {
	root := project(t, `
tasks:
  - name: promises
    command: "true"
    adapter: adapters/ok.sh
    evidence:
      - coverage.out
`, nil)
	adapter(t, root, "adapters/ok.sh", `
while [ $# -gt 0 ]; do
  case "$1" in --work-dir) work="$2"; shift 2 ;; *) shift ;; esac
done
printf '"success": true\n' > "$work/output.yaml"
`)

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Fatalf("a task that did not write its declared evidence passed: %v", got.result)
	}
	reasons := got.result["reasons"].([]any)
	first := reasons[0].(map[string]any)
	if first["kind"] != "evidence-missing" {
		t.Errorf("kind is %v", first["kind"])
	}
	if !strings.Contains(first["message"].(string), "coverage.out") {
		t.Errorf("the reason does not name the path: %v", first["message"])
	}
}

// COVERS: FR-6.11, FR-7.6 | negative
func TestAnAdapterThatReachesNoResultIsDistinguishedByCause(t *testing.T) {
	cases := map[string]struct {
		script string
		kind   string
	}{
		"it exited non-zero": {
			script: "echo broke >&2\nexit 4\n",
			kind:   "adapter-failed",
		},
		"it wrote nothing": {
			script: "exit 0\n",
			kind:   "adapter-wrote-nothing",
		},
		"it wrote something that will not validate": {
			script: `
while [ $# -gt 0 ]; do
  case "$1" in --work-dir) work="$2"; shift 2 ;; *) shift ;; esac
done
printf '"success": "yes"\n' > "$work/output.yaml"
`,
			kind: "adapter-wrote-invalid",
		},
	}

	for name, want := range cases {
		t.Run(name, func(t *testing.T) {
			root := project(t, `
tasks:
  - name: broken
    command: "true"
    adapter: adapters/broken.sh
`, nil)
			adapter(t, root, "adapters/broken.sh", want.script)

			got := runBolt(t, root)

			if got.status != cli.Ran {
				t.Fatalf("a broken adapter refused the run: %s", got.stderr)
			}
			if got.result["success"] != false {
				t.Fatalf("no authoritative result, yet the run passed: %v", got.result)
			}
			reasons := got.result["reasons"].([]any)
			if kind := reasons[0].(map[string]any)["kind"]; kind != want.kind {
				t.Errorf("kind is %v, want %s", kind, want.kind)
			}
		})
	}
}

// COVERS: FR-6.11 | edge
func TestABrokenAdaptersOwnOutputIsKept(t *testing.T) {
	// Otherwise the reason says the adapter failed and nothing says why.
	root := project(t, `
tasks:
  - name: broken
    command: "true"
    adapter: adapters/broken.sh
`, nil)
	adapter(t, root, "adapters/broken.sh", "echo could-not-parse-the-report >&2\nexit 2\n")

	got := runBolt(t, root)

	output, err := os.ReadFile(filepath.Join(got.output, "work", "broken-0", "adapter-output"))
	if err != nil {
		t.Fatalf("the adapter's own output was not kept: %v", err)
	}
	if !strings.Contains(string(output), "could-not-parse-the-report") {
		t.Errorf("what the adapter said is not in it: %s", output)
	}
}

// COVERS: FR-6.11 | regression
func TestALeftoverEnvelopeCannotStandInForOne(t *testing.T) {
	// The adapter is run over a work directory that already holds an envelope
	// from a previous fold. An adapter that now writes nothing must not be
	// credited with the old one.
	root := project(t, `
tasks:
  - name: broken
    command: "echo x > {work_dir}/output.yaml"
    adapter: adapters/silent.sh
`, nil)
	adapter(t, root, "adapters/silent.sh", "exit 0\n")

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Fatalf("a stale file was read as this run's verdict: %v", got.result)
	}
	reasons := got.result["reasons"].([]any)
	if kind := reasons[0].(map[string]any)["kind"]; kind != "adapter-wrote-nothing" {
		t.Errorf("kind is %v, want adapter-wrote-nothing", kind)
	}
}

// COVERS: FR-6.10, FR-10.5 | negative
func TestAnUnknownAdapterRefusesTheRunBeforeAnythingExecutes(t *testing.T) {
	root := project(t, `
tasks:
  - name: first
    command: "echo ran > {work_dir}/marker"
  - name: second
    command: "true"
    adapter: adapters/absent.sh
`, nil)

	var stdout, stderr bytes.Buffer
	output := filepath.Join(t.TempDir(), "evidence")
	status := cli.Main([]string{"--output-dir", output, "check", root}, &stdout, &stderr)

	if status != cli.Refused {
		t.Errorf("status %d, want a refusal", status)
	}
	if !strings.Contains(stderr.String(), "absent.sh") {
		t.Errorf("the refusal does not name the adapter: %s", stderr.String())
	}
	if _, err := os.Stat(filepath.Join(output, "work", "first-0")); err == nil {
		t.Error("the first task executed, so the refusal came after half a gate had run")
	}
}

// COVERS: FR-6.10 | negative
func TestAnAdapterThatIsNotExecutableIsRefused(t *testing.T) {
	root := project(t, `
tasks:
  - name: task
    command: "true"
    adapter: adapters/plain.txt
`, map[string]string{"adapters/plain.txt": "not a program\n"})

	var stdout, stderr bytes.Buffer
	status := cli.Main([]string{"--output-dir", filepath.Join(t.TempDir(), "e"), "check", root}, &stdout, &stderr)

	if status != cli.Refused {
		t.Errorf("status %d, want a refusal", status)
	}
	if !strings.Contains(stderr.String(), "executable") {
		t.Errorf("the refusal does not say why: %s", stderr.String())
	}
}

// COVERS: FR-6.9 | positive
func TestATaskNamingNoAdapterStillGetsOne(t *testing.T) {
	root := project(t, `
tasks:
  - name: plain
    command: "exit 1"
`, nil)

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Fatal("the exit-code adapter did not run")
	}
	reasons := got.result["reasons"].([]any)
	if kind := reasons[0].(map[string]any)["kind"]; kind != "nonzero-exit" {
		t.Errorf("kind is %v, want nonzero-exit", kind)
	}
}
