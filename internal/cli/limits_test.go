package cli_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// COVERS: FR-4.11 | positive
func TestBothLimitsAreOptional(t *testing.T) {
	// Unset means a tool is allowed to finish.
	root := project(t, `
tasks:
  - name: unhurried
    command: "sleep 0.2 && echo done > {work_dir}/said"
`, nil)

	got := runBolt(t, root)
	if got.result["success"] != true {
		t.Fatalf("a jig setting no limit failed: %v %s", got.result["reasons"], got.stderr)
	}
	if said := read(t, filepath.Join(got.output, "work", "unhurried-0", "said")); !strings.Contains(said, "done") {
		t.Errorf("the command did not run to completion: %q", said)
	}
}

// COVERS: FR-4.12, FR-4.12b | negative
func TestATaskExceedingItsLimitFailsAndTheRunCarriesOn(t *testing.T) {
	// A slow task is no more reason to discard the rest than a failing one.
	root := project(t, `
tasks:
  - name: hangs
    command: "sleep 30"
    time-limit: 50ms
  - name: after
    command: "echo ran > {work_dir}/marker"
`, nil)

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Errorf("a task that passed its limit did not fail the run: %v", got.result)
	}
	reasons := strings.ToLower(strings.Join(reasonMessages(t, got.result), " "))
	if !strings.Contains(reasons, "limit") {
		t.Errorf("no reason says the limit was passed: %q", reasons)
	}
	if _, err := os.Stat(filepath.Join(got.output, "work", "after-0", "marker")); err != nil {
		t.Errorf("the run did not carry on past the timeout: %v", err)
	}
}

// COVERS: FR-4.11a, FR-4.11b | property
func TestATasksLimitCoversAllItsInvocationsTakenTogether(t *testing.T) {
	// Thirty seconds over four hundred paths is thirty seconds for the task,
	// not for every path in turn. Reaching it kills the execution in flight and
	// the executions after it do not start.
	root := project(t, `
tasks:
  - name: each
    command: "sleep 5 && echo {each_path} > {work_dir}/said"
    matching: ["**/*.txt"]
    time-limit: 80ms
`, map[string]string{"a.txt": "", "b.txt": "", "c.txt": "", "d.txt": ""})

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Errorf("the task did not fail: %v", got.result)
	}
	// The first execution was killed; the ones after it never started, so they
	// left no work directory.
	if _, err := stat(filepath.Join(got.output, "work", "each-0")); err != nil {
		t.Errorf("the execution in flight left no evidence: %v", err)
	}
	for _, later := range []string{"each-1", "each-2", "each-3"} {
		if _, err := stat(filepath.Join(got.output, "work", later)); err == nil {
			t.Errorf("%s started after the task's budget was spent", later)
		}
	}
}

// COVERS: FR-4.11c, FR-4.12a, FR-4.12d | positive
func TestAKilledCommandKeepsItsOutputAndItsAdapterStillRuns(t *testing.T) {
	// A tool that reported forty problems before hanging reported forty real
	// problems. The adapter runs outside the limit, because it is what records
	// that the limit fired, and a budget exhausted killing the command would
	// leave nothing to write the envelope.
	root := project(t, `
tasks:
  - name: partial
    command: "echo forty-problems; sleep 30"
    time-limit: 80ms
`, nil)

	got := runBolt(t, root)

	captured := read(t, filepath.Join(got.output, "work", "partial-0", "stdout"))
	if !strings.Contains(captured, "forty-problems") {
		t.Errorf("what the command gathered before it was killed was discarded: %q", captured)
	}
	// A valid envelope either way, which is what distinguishes a timeout from
	// an adapter that died and left none.
	if _, err := os.Stat(filepath.Join(got.output, "work", "partial-0", "output.yaml")); err != nil {
		t.Errorf("a timed-out execution has no envelope: %v", err)
	}
	if got.result["success"] != false {
		t.Errorf("the execution passed despite being killed: %v", got.result)
	}
}

// COVERS: FR-4.12b | edge
func TestATimedOutExecutionFailsWhateverItsAdapterConcluded(t *testing.T) {
	// A partial run cannot report a pass, because what it did not reach is
	// exactly what is unknown about it. This adapter always says success.
	root := project(t, `
requires: [sh]
tasks:
  - name: partial
    command: "sleep 30"
    time-limit: 80ms
    adapter: always-passes
`, map[string]string{
		"always-passes": "#!/bin/sh\nfor a in \"$@\"; do case $a in --work-dir) shift; printf 'success: true\\n' > \"$1/output.yaml\";; esac; shift 2>/dev/null || true; done\n",
	})
	if err := os.Chmod(filepath.Join(root, "always-passes"), 0o755); err != nil {
		t.Fatalf("chmod: %v", err)
	}

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Errorf("an adapter saying success made a timed-out execution pass: %v", got.result)
	}
	reasons := strings.ToLower(strings.Join(reasonMessages(t, got.result), " "))
	if !strings.Contains(reasons, "limit") {
		t.Errorf("the reasons do not carry the limit being passed: %q", reasons)
	}
}

// COVERS: FR-4.13, FR-4.14 | negative
func TestARunExceedingItsLimitFailsAndStillWritesItsResult(t *testing.T) {
	// Bolt is alive and in control when the limit passes, so the rule is the
	// one FR-5.8 already sets for a refusal: only a bolt that dies leaves
	// nothing behind.
	root := project(t, `
time-limit: 80ms
tasks:
  - name: quick
    command: "echo done > {work_dir}/said"
  - name: hangs
    command: "sleep 30"
  - name: never
    command: "echo ran > {work_dir}/marker"
`, nil)

	got := runBolt(t, root)

	if got.result["success"] != false {
		t.Errorf("a run past its limit did not fail: %v", got.result)
	}
	reasons := strings.ToLower(strings.Join(reasonMessages(t, got.result), " "))
	if !strings.Contains(reasons, "limit") {
		t.Errorf("no reason says the run's limit was passed: %q", reasons)
	}

	// Carrying what completed before the limit.
	if _, err := os.Stat(filepath.Join(got.output, "work", "quick-0", "said")); err != nil {
		t.Errorf("the result does not carry what completed: %v", err)
	}
	if _, err := stat(filepath.Join(got.output, "work", "never-0")); err == nil {
		t.Error("a task after the run's limit executed")
	}
}

// COVERS: FR-4.11b | regression
func TestATimedOutCommandLeavesNoDescendantsRunning(t *testing.T) {
	// A command that spawns its own children leaves them running when only the
	// child is signalled, and they go on writing into a work directory bolt has
	// finished with, and into the streams an adapter is about to read.
	root := project(t, `
tasks:
  - name: spawns
    command: "sh -c 'sleep 30 > {work_dir}/orphan &' ; sleep 30"
    time-limit: 80ms
`, nil)

	got := runBolt(t, root)
	if got.result["success"] != false {
		t.Fatalf("the task did not time out: %v", got.result)
	}

	// The orphan would still be holding the file open and writing. Its absence
	// of growth is what says it went with its parent.
	before := size(t, filepath.Join(got.output, "work", "spawns-0", "orphan"))
	time.Sleep(300 * time.Millisecond)
	if after := size(t, filepath.Join(got.output, "work", "spawns-0", "orphan")); after != before {
		t.Errorf("a descendant is still writing: %d then %d", before, after)
	}
}

func size(t *testing.T, path string) int64 {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		return -1
	}
	return info.Size()
}
