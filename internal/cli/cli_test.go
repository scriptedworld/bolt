package cli_test

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/scriptedworld/bolt/internal/cli"
	"github.com/scriptedworld/wrench"
)

// project writes a tree with a jig in it and returns the base directory.
func project(t *testing.T, jig string, files map[string]string) string {
	t.Helper()
	root := t.TempDir()

	if err := os.WriteFile(filepath.Join(root, "bolt.check.yaml"), []byte(jig), 0o644); err != nil {
		t.Fatalf("writing the jig: %v", err)
	}
	for name, contents := range files {
		full := filepath.Join(root, name)
		if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
			t.Fatalf("mkdir for %s: %v", name, err)
		}
		if err := os.WriteFile(full, []byte(contents), 0o644); err != nil {
			t.Fatalf("write %s: %v", name, err)
		}
	}
	return root
}

type outcome struct {
	status int
	stdout string
	stderr string
	output string
	result map[string]any
}

// runBolt runs a jig over root, writing evidence to a named directory so the
// test does not have to find a timestamped one.
func runBolt(t *testing.T, root string, extra ...string) outcome {
	t.Helper()
	output := filepath.Join(t.TempDir(), "evidence")

	var stdout, stderr bytes.Buffer
	args := append([]string{"--output-dir", output}, extra...)
	args = append(args, "check", root)
	status := cli.Main(args, &stdout, &stderr)

	got := outcome{status: status, stdout: stdout.String(), stderr: stderr.String(), output: output}
	if value, err := wrench.LoadFormattedFile(
		filepath.Join(output, "result.yaml"), wrench.EnvelopeSchema, wrench.YAML, wrench.LocalFile,
	); err == nil {
		got.result, _ = value.(map[string]any)
	}
	return got
}

func metadata(t *testing.T, result map[string]any) map[string]any {
	t.Helper()
	meta, ok := result["metadata"].(map[string]any)
	if !ok {
		t.Fatalf("result carries no metadata: %v", result)
	}
	return meta
}

const passingJig = `
requires: [sh]
tasks:
  - name: always
    description: exits zero
    command: "true"
`

// COVERS: FR-2.1, FR-2.3, FR-3.9 | positive
func TestARunIsOneJigAndOneDirectory(t *testing.T) {
	root := project(t, passingJig, nil)

	got := runBolt(t, root)

	if got.status != cli.Ran {
		t.Fatalf("status %d, want %d. stderr: %s", got.status, cli.Ran, got.stderr)
	}
	if got.result == nil {
		t.Fatal("no result.yaml was written")
	}
	if got.result["success"] != true {
		t.Errorf("success is %v, want true", got.result["success"])
	}
}

// COVERS: FR-8.9 | positive
func TestTheResultRecordsTheBaseTheRunWasPointedAt(t *testing.T) {
	root := project(t, passingJig, nil)

	got := runBolt(t, root)

	if base := metadata(t, got.result)["base"]; base != root {
		t.Errorf("base is %v, want %s", base, root)
	}
}

// COVERS: FR-9.2, FR-9.2c | positive
func TestAnExecutionCarriesItsBookkeepingFiles(t *testing.T) {
	root := project(t, passingJig, nil)

	got := runBolt(t, root)

	dir := filepath.Join(got.output, "work", "always-0")
	for _, name := range []string{"manifest.yaml", "stdout", "stderr", "exitcode", "output.yaml"} {
		if _, err := os.Stat(filepath.Join(dir, name)); err != nil {
			t.Errorf("%s is missing from the work directory: %v", name, err)
		}
	}
}

// COVERS: FR-9.5, FR-9.5a, FR-9.5c | positive
func TestTheManifestRecordsWhatTheExecutionWasGiven(t *testing.T) {
	root := project(t, passingJig, nil)
	got := runBolt(t, root)

	value, err := wrench.LoadFormattedFile(
		filepath.Join(got.output, "work", "always-0", "manifest.yaml"),
		wrench.ManifestSchema, wrench.YAML, wrench.LocalFile,
	)
	if err != nil {
		t.Fatalf("reading the manifest: %v", err)
	}
	manifest := value.(map[string]any)

	if manifest["task"] != "always" {
		t.Errorf("task is %v", manifest["task"])
	}
	variables := manifest["variables"].(map[string]any)
	for _, name := range []string{"project_root", "base_dir", "work_dir", "config_dir", "output_dir"} {
		if variables[name] == nil {
			t.Errorf("%s is not recorded, so a reader cannot see what the task was given", name)
		}
	}
}

// COVERS: FR-9.5e | negative
func TestTheManifestDoesNotCarryTheEnvironment(t *testing.T) {
	// A dump of it carries whatever the shell was holding, into a file that
	// exists to be handed around as evidence.
	root := project(t, passingJig, nil)
	got := runBolt(t, root)

	raw, err := os.ReadFile(filepath.Join(got.output, "work", "always-0", "manifest.yaml"))
	if err != nil {
		t.Fatalf("reading the manifest: %v", err)
	}
	if strings.Contains(string(raw), "environment") || strings.Contains(string(raw), "PATH") {
		t.Errorf("the manifest carries the environment:\n%s", raw)
	}
}

// COVERS: FR-6.9, FR-7.1, FR-7.7 | positive
func TestTheExitCodeAdapterReportsSuccessOnAZeroExit(t *testing.T) {
	root := project(t, passingJig, nil)
	got := runBolt(t, root)

	value, err := wrench.LoadFormattedFile(
		filepath.Join(got.output, "work", "always-0", "output.yaml"),
		wrench.EnvelopeSchema, wrench.YAML, wrench.LocalFile,
	)
	if err != nil {
		t.Fatalf("reading the envelope: %v", err)
	}
	if value.(map[string]any)["success"] != true {
		t.Error("a command that exited 0 did not produce a passing envelope")
	}
}

// COVERS: FR-6.9, FR-7.8, FR-7.9 | negative
func TestAFailingCommandProducesAReasonWithAKind(t *testing.T) {
	root := project(t, `
tasks:
  - name: fails
    command: "exit 3"
`, nil)

	got := runBolt(t, root)

	if got.status != cli.Ran {
		t.Errorf("status %d: a run whose tools failed still completed, so it exits 0", got.status)
	}
	if got.result["success"] != false {
		t.Fatalf("success is %v, want false", got.result["success"])
	}

	reasons := got.result["reasons"].([]any)
	first := reasons[0].(map[string]any)
	if first["kind"] == nil || first["message"] == nil {
		t.Errorf("a reason is missing kind or message: %v", first)
	}
}

// COVERS: FR-10.1, FR-10.2, FR-10.3, FR-10.5 | property
func TestTheExitStatusSaysWhetherBoltRanAndNotWhetherToolsPassed(t *testing.T) {
	root := project(t, `
tasks:
  - name: fails
    command: "exit 1"
`, nil)

	got := runBolt(t, root)

	if got.status != cli.Ran {
		t.Errorf("status is %d, want 0: the run completed and the tools failed", got.status)
	}
	if got.result["success"] != false {
		t.Error("the envelope does not carry the failure, which is where the verdict lives")
	}
}

// COVERS: FR-4.8, FR-4.5 | positive
func TestAFailingTaskDoesNotStopTheRun(t *testing.T) {
	root := project(t, `
tasks:
  - name: first
    command: "exit 1"
  - name: second
    command: "true"
  - name: third
    command: "true"
`, nil)

	got := runBolt(t, root)

	evidence := metadata(t, got.result)["evidence"].(map[string]any)
	for _, name := range []string{"first", "second", "third"} {
		if evidence[name] == nil {
			t.Errorf("%s produced no evidence, so the run stopped early", name)
		}
	}
}

// COVERS: FR-8.1, FR-8.2, FR-8.2a, FR-8.8 | positive
func TestTheMergeKeysEvidenceByTaskAndTakesArgsFromTheManifest(t *testing.T) {
	root := project(t, passingJig, nil)
	got := runBolt(t, root)

	evidence := metadata(t, got.result)["evidence"].(map[string]any)
	entries, ok := evidence["always"].([]any)
	if !ok || len(entries) != 1 {
		t.Fatalf("evidence for always is %v", evidence["always"])
	}

	entry := entries[0].(map[string]any)
	if entry["args"] != "true" {
		t.Errorf("args is %v, want the command as executed", entry["args"])
	}
	if !strings.HasSuffix(entry["result"].(string), "output.yaml") {
		t.Errorf("result is %v, want the filepath of its own envelope", entry["result"])
	}
}

// COVERS: FR-8.3 | negative
func TestTheMergePassesOnlyWhenEveryConstituentPasses(t *testing.T) {
	root := project(t, `
tasks:
  - name: good
    command: "true"
  - name: bad
    command: "false"
`, nil)

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Error("one failing constituent did not fail the merge")
	}
}

// COVERS: FR-4.4, FR-8.3a | negative
func TestAJigWhoseFiltersAllMissFailsRatherThanPassingOverNothing(t *testing.T) {
	// The task is skipped for an empty selection and produces no output, so
	// the merge finds no constituent. A green result over zero checks is the
	// outcome this refuses.
	root := project(t, `
tasks:
  - name: lint
    command: "true {each_path}"
    matching: ["**/*.nothing-matches-this"]
`, map[string]string{"a.go": ""})

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Fatal("a run that checked nothing reported success")
	}
	if !strings.Contains(got.stdout, "skipped") {
		t.Errorf("the skip was not reported: %s", got.stdout)
	}
}

// COVERS: FR-4.2, FR-9.2a, FR-9.2b, FR-9.9 | positive
func TestEachPathRunsOncePerPathWithAPaddedOrdinal(t *testing.T) {
	root := project(t, `
tasks:
  - name: each
    command: "cat {each_path}"
    matching: ["**/*.txt"]
`, map[string]string{
		"a.txt": "", "b.txt": "", "c.txt": "", "d.txt": "",
		"e.txt": "", "f.txt": "", "g.txt": "", "h.txt": "",
		"i.txt": "", "j.txt": "", "k.txt": "",
	})

	got := runBolt(t, root)

	// Eleven executions need two digits, so the listing sorts into execution
	// order rather than into 1, 10, 2.
	for _, name := range []string{"each-00", "each-10"} {
		if _, err := os.Stat(filepath.Join(got.output, "work", name)); err != nil {
			t.Errorf("%s is missing: %v", name, err)
		}
	}
	entries := metadata(t, got.result)["evidence"].(map[string]any)["each"].([]any)
	if len(entries) != 11 {
		t.Errorf("got %d executions, want one per matched path", len(entries))
	}
}

// COVERS: FR-9.8 | positive
func TestAPerPathManifestRecordsTheWholeMatchedList(t *testing.T) {
	root := project(t, `
tasks:
  - name: each
    command: "cat {each_path}"
    matching: ["**/*.txt"]
`, map[string]string{"a.txt": "", "b.txt": ""})

	got := runBolt(t, root)

	value, err := wrench.LoadFormattedFile(
		filepath.Join(got.output, "work", "each-0", "manifest.yaml"),
		wrench.ManifestSchema, wrench.YAML, wrench.LocalFile,
	)
	if err != nil {
		t.Fatalf("reading the manifest: %v", err)
	}

	selection := value.(map[string]any)["selection"].(map[string]any)
	matched := selection["matched"].([]any)
	if len(matched) != 2 {
		t.Errorf("the manifest records %d paths, want the whole matched list", len(matched))
	}
}

// COVERS: FR-4.2 | positive
func TestAllPathsRunsOnceWithTheWholeSelection(t *testing.T) {
	root := project(t, `
tasks:
  - name: all
    command: "cat {all_paths} > /dev/null"
    matching: ["**/*.txt"]
`, map[string]string{"a.txt": "", "b.txt": "", "c.txt": ""})

	got := runBolt(t, root)

	entries := metadata(t, got.result)["evidence"].(map[string]any)["all"].([]any)
	if len(entries) != 1 {
		t.Fatalf("got %d executions, want one", len(entries))
	}
	if got.result["success"] != true {
		t.Errorf("the run failed: %v", got.result["reasons"])
	}
}

// COVERS: FR-4.3 | edge
func TestAPathCarryingASpaceCannotSplitTheCommandLine(t *testing.T) {
	// Unquoted, `cat one two.txt` is two arguments and the task fails looking
	// for a file called "one".
	root := project(t, `
tasks:
  - name: each
    command: "cat {each_path}"
    matching: ["**/*.txt"]
`, map[string]string{"one two.txt": "contents"})

	got := runBolt(t, root)

	if got.result["success"] != true {
		t.Errorf("a path with a space broke the command line: %v", got.result["reasons"])
	}
}

// COVERS: FR-4.3 | edge
func TestAPathCarryingAQuoteCannotInject(t *testing.T) {
	root := project(t, `
tasks:
  - name: each
    command: "cat {each_path}"
    matching: ["**/*.txt"]
`, map[string]string{"it's here.txt": "contents"})

	got := runBolt(t, root)

	if got.result["success"] != true {
		t.Errorf("a path with a quote broke the command line: %v", got.result["reasons"])
	}
}

// COVERS: FR-4.1a | positive
func TestACommandStandsAtTheBase(t *testing.T) {
	root := project(t, `
tasks:
  - name: here
    command: "test -f marker.txt"
`, map[string]string{"marker.txt": ""})

	got := runBolt(t, root)

	if got.result["success"] != true {
		t.Error("a relative path did not resolve against the base, so the command did not stand there")
	}
}

// COVERS: FR-4.1, FR-4.1c | positive
func TestEveryLocationIsAvailableToACommand(t *testing.T) {
	root := project(t, `
tasks:
  - name: locations
    command: "test -d {project_root} && test -d {base_dir} && test -d {work_dir} && test -d {config_dir} && test -d {output_dir}"
`, nil)

	got := runBolt(t, root)

	if got.result["success"] != true {
		t.Errorf("a location was not available or did not exist: %v", got.result["reasons"])
	}
}

// COVERS: FR-9.2c | positive
func TestAnArtifactArrivesInTheWorkDirectoryByBeingAddressed(t *testing.T) {
	root := project(t, `
tasks:
  - name: writes
    command: "echo produced > {work_dir}/artifact.txt"
`, nil)

	got := runBolt(t, root)

	if _, err := os.Stat(filepath.Join(got.output, "work", "writes-0", "artifact.txt")); err != nil {
		t.Errorf("the artifact is not in the work directory: %v", err)
	}
}

// COVERS: FR-2.6a | positive
func TestTheOutputDirectoryIsCreatedIfAbsent(t *testing.T) {
	root := project(t, passingJig, nil)
	nested := filepath.Join(t.TempDir(), "not", "there", "yet")

	var stdout, stderr bytes.Buffer
	status := cli.Main([]string{"--output-dir", nested, "check", root}, &stdout, &stderr)

	if status != cli.Ran {
		t.Fatalf("status %d: %s", status, stderr.String())
	}
	if _, err := os.Stat(filepath.Join(nested, "result.yaml")); err != nil {
		t.Errorf("the output directory was not created with its parents: %v", err)
	}
}

// COVERS: FR-2.6b | negative
func TestAnOutputDirectoryThatAlreadyHoldsARunIsRefused(t *testing.T) {
	root := project(t, passingJig, nil)
	output := filepath.Join(t.TempDir(), "evidence")

	var stdout, stderr bytes.Buffer
	if status := cli.Main([]string{"--output-dir", output, "check", root}, &stdout, &stderr); status != cli.Ran {
		t.Fatalf("the first run failed: %s", stderr.String())
	}

	stdout.Reset()
	stderr.Reset()
	status := cli.Main([]string{"--output-dir", output, "check", root}, &stdout, &stderr)

	if status != cli.Refused {
		t.Error("a second run wrote into the first run's directory, interleaving two runs' evidence")
	}
	if !strings.Contains(stderr.String(), "already holds a run") {
		t.Errorf("the refusal does not say why: %s", stderr.String())
	}
}

// COVERS: FR-2.5 | negative
func TestARunOverADirectoryThatIsNotThereIsRefused(t *testing.T) {
	root := project(t, passingJig, nil)
	missing := filepath.Join(root, "no", "such", "place")

	var stdout, stderr bytes.Buffer
	status := cli.Main([]string{"--config-dir", root, "check", missing}, &stdout, &stderr)

	if status != cli.Refused {
		t.Errorf("status %d, want a refusal", status)
	}
}

// COVERS: FR-1.5, FR-3.12 | negative
func TestAJigThatWillNotValidateRefusesTheRun(t *testing.T) {
	root := project(t, `
tasks:
  - description: a task with no name
    command: "true"
`, nil)

	var stdout, stderr bytes.Buffer
	status := cli.Main([]string{"check", root}, &stdout, &stderr)

	if status != cli.Refused {
		t.Errorf("status %d, want a refusal", status)
	}
	if !strings.Contains(stderr.String(), "name") {
		t.Errorf("the refusal does not name what was wrong: %s", stderr.String())
	}
}

// COVERS: FR-2.1a | negative
func TestAnInvocationNamesOneJigAndOneDirectory(t *testing.T) {
	root := project(t, passingJig, nil)

	for name, args := range map[string][]string{
		"two jigs": {"check", "other", root},
		"no jig":   {root},
		"nothing":  {},
	} {
		t.Run(name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			if status := cli.Main(args, &stdout, &stderr); status != cli.Refused {
				t.Errorf("status %d, want a refusal", status)
			}
		})
	}
}

// COVERS: FR-10.4 | positive
func TestHelpIsNotARefusal(t *testing.T) {
	var stdout, stderr bytes.Buffer

	if status := cli.Main([]string{"help"}, &stdout, &stderr); status != cli.Ran {
		t.Errorf("status %d, want 0", status)
	}
	if !strings.Contains(stdout.String(), "bolt <jig> <directory>") {
		t.Errorf("help does not say how to run it: %s", stdout.String())
	}
}
