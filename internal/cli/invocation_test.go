package cli_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/scriptedworld/bolt/internal/cli"
)

// COVERS: FR-6.2, FR-6.2d | positive
func TestATaskMayWriteItsOwnAdapterInvocation(t *testing.T) {
	// The default names the captures as flags. A task that needs an adapter
	// invoked another way says so, and bolt runs what it wrote.
	root := project(t, `
tasks:
  - name: bespoke
    command: "echo findings"
    adapter: adapters/reads-stdin.sh
    adapter-command: "{config_dir}/adapters/reads-stdin.sh --into {work_dir} < {work_dir}/stdout"
`, nil)
	adapter(t, root, "adapters/reads-stdin.sh", `
while [ $# -gt 0 ]; do
  case "$1" in --into) work="$2"; shift 2 ;; *) shift ;; esac
done
read -r line
printf '"reasons":\n  - "kind": "read-from-stdin"\n    "message": "%s"\n"success": false\n' "$line" > "$work/output.yaml"
`)

	got := runBolt(t, root)

	if got.status != cli.Ran {
		t.Fatalf("status %d: %s", got.status, got.stderr)
	}
	if got.result["success"] != false {
		t.Fatalf("the explicit invocation did not reach the verdict: %v", got.result)
	}
	first := got.result["reasons"].([]any)[0].(map[string]any)
	if first["kind"] != "read-from-stdin" {
		t.Errorf("kind is %v, so the default invocation ran instead", first["kind"])
	}
	if first["message"] != "findings" {
		t.Errorf("message is %v, so the redirection did not reach it", first["message"])
	}
}

// COVERS: FR-6.2b, FR-6.2e | edge
func TestAnExplicitInvocationStillLeavesTheEnvelopeWhereTheDefaultWould(t *testing.T) {
	root := project(t, `
tasks:
  - name: bespoke
    command: "true"
    adapter: adapters/elsewhere.sh
    adapter-command: "{config_dir}/adapters/elsewhere.sh {work_dir}"
`, nil)
	// Writes its envelope somewhere else entirely, and nothing at output.yaml.
	adapter(t, root, "adapters/elsewhere.sh", `
printf '"success": true\n' > "$1/somewhere-else.yaml"
`)

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Fatal("an envelope written under another name was accepted")
	}
	first := got.result["reasons"].([]any)[0].(map[string]any)
	if first["kind"] != "adapter-wrote-nothing" {
		t.Errorf("kind is %v, want adapter-wrote-nothing", first["kind"])
	}
}

// COVERS: FR-6.2 | negative
func TestAnAdapterCommandWithNoAdapterIsAJigError(t *testing.T) {
	// The invocation names what to run, but `requires` and the resolution
	// check both work off `adapter`. Writing one without the other means
	// nothing is resolved and nothing is declared.
	root := project(t, `
tasks:
  - name: dangling
    command: "true"
    adapter-command: "./whatever --into {work_dir}"
`, nil)

	got := runBolt(t, root)

	if got.status != cli.Refused {
		t.Errorf("status %d, want a refusal", got.status)
	}
	if !strings.Contains(got.stderr, "adapter") {
		t.Errorf("the refusal does not say what was wrong: %s", got.stderr)
	}
}

// COVERS: FR-6.6 | property
func TestRefoldingAFinishedRunNeedsNoReExecution(t *testing.T) {
	// Every input an adapter reads is already on disk, so a fold is repeatable
	// over a finished directory. Here the check is that the run directory holds
	// all of them: the captures, the exit status and the declared evidence.
	root := project(t, `
tasks:
  - name: produces
    command: "echo out; echo err >&2; echo data > {work_dir}/report.txt"
    adapter: adapters/ok.sh
    evidence:
      - report.txt
`, nil)
	adapter(t, root, "adapters/ok.sh", `
while [ $# -gt 0 ]; do
  case "$1" in --work-dir) work="$2"; shift 2 ;; *) shift ;; esac
done
printf '"success": true\n' > "$work/output.yaml"
`)

	got := runBolt(t, root)
	dir := filepath.Join(got.output, "work", "produces-0")

	for _, name := range []string{"stdout", "stderr", "exitcode", "report.txt", "manifest.yaml"} {
		if _, err := os.Stat(filepath.Join(dir, name)); err != nil {
			t.Errorf("%s is not on disk, so a re-fold would have to re-execute: %v", name, err)
		}
	}
}
