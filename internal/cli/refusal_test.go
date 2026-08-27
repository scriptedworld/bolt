package cli_test

import (
	"bytes"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"syscall"
	"testing"
	"time"

	"github.com/scriptedworld/bolt/internal/cli"
	"github.com/scriptedworld/wrench"
)

// resultAt reads a result.yaml, which is what a caller parses whatever went
// wrong.
func resultAt(t *testing.T, output string) map[string]any {
	t.Helper()
	value, err := wrench.LoadFormattedFile(
		filepath.Join(output, "result.yaml"), wrench.EnvelopeSchema, wrench.YAML, wrench.LocalFile,
	)
	if err != nil {
		t.Fatalf("no result at %s: %v", output, err)
	}
	return value.(map[string]any)
}

// COVERS: FR-2.5a, FR-10.7 | negative
func TestAMissingBaseRefusesInTheShapeEveryRefusalTakes(t *testing.T) {
	// A caller parses one thing whatever went wrong. Bolt is alive and in
	// control here, so it writes a result; only a bolt that died leaves none.
	output := filepath.Join(t.TempDir(), "evidence")
	absent := filepath.Join(t.TempDir(), "not-there")

	var stdout, stderr bytes.Buffer
	status := cli.Main([]string{"--output-dir", output, "check", absent}, &stdout, &stderr)

	if status == 0 {
		t.Error("a run over a directory that is not there exited 0")
	}

	result := resultAt(t, output)
	if result["success"] != false {
		t.Errorf("the refusal does not carry success: false: %v", result)
	}
	reasons := strings.Join(reasonMessages(t, result), " ")
	if !strings.Contains(reasons, absent) {
		t.Errorf("no reason names the directory: %q", reasons)
	}
}

// COVERS: FR-10.7a, FR-10.7b | edge
func TestTheOneRefusalThatCannotWriteAResultSaysSo(t *testing.T) {
	// The default output directory sits at the base, so writing the refusal
	// would create the base, and the base not being there is what is being
	// refused. Naming one outside the tree gets a result as everything else
	// does, which is what a graph node's .ephemera/ already is.
	absent := filepath.Join(t.TempDir(), "not-there")

	var stdout, stderr bytes.Buffer
	status := cli.Main([]string{"check", absent}, &stdout, &stderr)

	if status == 0 {
		t.Error("exited 0 over a directory that is not there")
	}
	if _, err := os.Stat(absent); err == nil {
		t.Fatal("the base was created, so the refusal undid itself")
	}
	if !strings.Contains(stderr.String(), "no result written") {
		t.Errorf("stderr does not say a result was not written: %q", stderr.String())
	}
}

// COVERS: FR-10.5, FR-10.7 | property
func TestEveryRefusalWritesAResultAndExitsOne(t *testing.T) {
	// A jig that will not parse, an unknown adapter and a base that is not
	// there are one kind of outcome: bolt could not carry the run out.
	cases := map[string]func(t *testing.T) (jig string, base string){
		"a jig that will not parse": func(t *testing.T) (string, string) {
			return "tasks: this is not a list\n", project(t, passingJig, nil)
		},
		"an unknown adapter": func(t *testing.T) (string, string) {
			return "tasks:\n  - name: a\n    command: \"true\"\n    adapter: nowhere\n", project(t, passingJig, nil)
		},
		"a placeholder nothing defines": func(t *testing.T) (string, string) {
			return "tasks:\n  - name: a\n    command: \"check {absent}\"\n", project(t, passingJig, nil)
		},
	}

	for what, build := range cases {
		t.Run(what, func(t *testing.T) {
			jigBody, root := build(t)
			if err := os.WriteFile(filepath.Join(root, "bolt.check.yaml"), []byte(jigBody), 0o644); err != nil {
				t.Fatalf("writing the jig: %v", err)
			}
			output := filepath.Join(t.TempDir(), "evidence")

			var stdout, stderr bytes.Buffer
			if status := cli.Main([]string{"--output-dir", output, "check", root}, &stdout, &stderr); status != 1 {
				t.Errorf("exited %d, want 1", status)
			}
			if result := resultAt(t, output); result["success"] != false {
				t.Errorf("no failing result was written: %v", result)
			}
		})
	}
}

// COVERS: FR-10.2, FR-10.3 | property
func TestTheExitStatusDoesNotMoveWhenTheVerdictDoes(t *testing.T) {
	// Bolt's status answers one question: could bolt carry out the run. A
	// caller reading it to learn whether the tools passed has read the wrong
	// thing, so a passing run and a failing one exit the same way.
	for what, command := range map[string]string{"passing": "true", "failing": "false"} {
		root := project(t, "tasks:\n  - name: a\n    command: \""+command+"\"\n", nil)
		got := runBolt(t, root)

		if got.status != 0 {
			t.Errorf("%s: exited %d, want 0 because the run completed", what, got.status)
		}
		want := command == "true"
		if got.result["success"] != want {
			t.Errorf("%s: the envelope says %v, want %v", what, got.result["success"], want)
		}
	}
}

// COVERS: FR-2.6, FR-2.6c, FR-2.6d | positive
func TestTheDefaultOutputDirectorySitsAtTheBaseWithAFilesystemSafeStamp(t *testing.T) {
	// Bolt reads no git, so there is no repository root to prefer, and the base
	// is the one directory every invocation names.
	root := project(t, passingJig, nil)

	var stdout, stderr bytes.Buffer
	if status := cli.Main([]string{"check", root}, &stdout, &stderr); status != 0 {
		t.Fatalf("the run was refused: %s", stderr.String())
	}

	entries, err := os.ReadDir(root)
	if err != nil {
		t.Fatalf("reading the base: %v", err)
	}

	// The strict form's colons are legal here and hostile to a Windows
	// checkout, so a colon becomes a hyphen and the offset is spelled the same
	// way.
	stamp := regexp.MustCompile(`^\.bolt-\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}[+-]\d{2}-\d{2}$`)
	var found string
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".bolt-") {
			found = entry.Name()
		}
	}
	if found == "" {
		t.Fatalf("no .bolt-<iso8601> at the base: %v", entries)
	}
	if !stamp.MatchString(found) {
		t.Errorf("%s is not the filesystem-safe form", found)
	}
	if strings.Contains(found, ":") {
		t.Errorf("%s carries a colon, which is a path on every platform", found)
	}
	if _, err := os.Stat(filepath.Join(root, found, "result.yaml")); err != nil {
		t.Errorf("the default output directory holds no result: %v", err)
	}
}

// helperMarker makes a test binary re-execute itself as the process under test,
// so a signal reaches a real bolt rather than an in-process call.
const helperMarker = "BOLT_TEST_HELPER_BASE"

// TestHelperBoltRuns is not a test. It is the child process the signal case
// needs, and it exits immediately unless the marker names a base for it.
func TestHelperBoltRuns(t *testing.T) {
	base := os.Getenv(helperMarker)
	if base == "" {
		t.Skip("not the helper process")
	}
	os.Exit(cli.Main([]string{"check", base}, os.Stdout, os.Stderr))
}

// COVERS: FR-10.6 | edge
func TestABoltKilledBySignalExitsOneHundredAndTwentyEightPlusIt(t *testing.T) {
	// The shell's convention, and the one case where bolt does not choose its
	// own status. What this really asserts is that bolt installs no handler
	// that swallows the signal and reports something of its own.
	root := project(t, "tasks:\n  - name: slow\n    command: \"sleep 30\"\n", nil)

	child := exec.Command(os.Args[0], "-test.run=TestHelperBoltRuns", "-test.timeout=60s")
	child.Env = append(os.Environ(), helperMarker+"="+root)
	if err := child.Start(); err != nil {
		t.Fatalf("starting the helper: %v", err)
	}

	// Wait for the task to be running before signalling, so the signal lands on
	// a bolt doing work rather than one still starting.
	deadline := time.Now().Add(20 * time.Second)
	for time.Now().Before(deadline) {
		if entries, err := os.ReadDir(root); err == nil && len(entries) > 1 {
			break
		}
		time.Sleep(20 * time.Millisecond)
	}

	if err := child.Process.Signal(syscall.SIGTERM); err != nil {
		t.Fatalf("signalling: %v", err)
	}
	err := child.Wait()

	exit, ok := err.(*exec.ExitError)
	if !ok {
		t.Fatalf("the helper exited %v, want a signal death", err)
	}
	status, ok := exit.Sys().(syscall.WaitStatus)
	if !ok {
		t.Skip("no wait status on this platform")
	}
	if !status.Signaled() {
		t.Fatalf("the helper exited %d rather than dying of the signal, so something swallowed it", status.ExitStatus())
	}
	if got, want := 128+int(status.Signal()), 128+int(syscall.SIGTERM); got != want {
		t.Errorf("the shell would report %d, want %d", got, want)
	}
}
