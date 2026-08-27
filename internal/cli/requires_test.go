package cli_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// COVERS: FR-3.10b, FR-3.10d | negative
func TestAJigRequiringAToolThatIsNotInstalledRefusesBeforeAnythingRuns(t *testing.T) {
	// An incomplete toolchain is known before half a gate has run rather than
	// partway through it, so this asserts no task executed and not only that
	// the status was non-zero. A tool the base image lacks is this same
	// refusal and not a separate path.
	root := project(t, `
requires: [sh, definitely-not-installed-anywhere]
tasks:
  - name: a
    command: "echo ran > {work_dir}/marker"
`, nil)

	got := runBolt(t, root)

	if got.status == 0 {
		t.Error("a jig requiring a missing tool ran anyway")
	}
	if !strings.Contains(got.stderr, "definitely-not-installed-anywhere") {
		t.Errorf("the refusal does not name what is missing: %s", got.stderr)
	}
	if _, err := stat(filepath.Join(got.output, "work", "a-0")); err == nil {
		t.Error("a task executed before the check, so the guarantee is not up front")
	}
}

// COVERS: FR-3.10b | edge
func TestTheRefusalNamesEveryMissingToolNotTheFirst(t *testing.T) {
	// A caller fixing one at a time pays a round trip per tool.
	root := project(t, `
requires: [sh, absent-alpha, absent-beta]
tasks:
  - name: a
    command: "true"
`, nil)

	got := runBolt(t, root)
	for _, missing := range []string{"absent-alpha", "absent-beta"} {
		if !strings.Contains(got.stderr, missing) {
			t.Errorf("the refusal does not name %s: %s", missing, got.stderr)
		}
	}
}

// COVERS: FR-3.10, FR-3.10b | positive
func TestAJigRequiringNothingMissingRunsAsBefore(t *testing.T) {
	root := project(t, `
requires: [sh, echo]
tasks:
  - name: a
    command: "true"
`, nil)

	got := runBolt(t, root)
	if got.status != 0 || got.result["success"] != true {
		t.Errorf("a satisfiable requires refused the run: %v %s", got.result, got.stderr)
	}
}

// COVERS: FR-3.10a | negative
func TestAnAdapterIsResolvedInTheSameUpFrontPass(t *testing.T) {
	// An adapter a task names appears in requires too, so an adapter no entry
	// covers is found before a run instead of when the task reaches it.
	root := project(t, `
requires: [sh, not-an-adapter-here]
tasks:
  - name: first
    command: "echo ran > {work_dir}/marker"
  - name: second
    command: "true"
    adapter: not-an-adapter-here
`, nil)

	got := runBolt(t, root)

	if got.status == 0 {
		t.Error("an unresolvable adapter ran anyway")
	}
	if _, err := stat(filepath.Join(got.output, "work", "first-0")); err == nil {
		t.Error("a task ran before the adapter was resolved")
	}
}

// COVERS: FR-4.9 | negative
func TestShortCircuitFailureStopsTheRun(t *testing.T) {
	// Stopping is what a jig asks for rather than what it gets.
	root := project(t, `
tasks:
  - name: stops
    command: "false"
    short-circuit-failure: true
  - name: after
    command: "echo ran > {work_dir}/marker"
`, nil)

	got := runBolt(t, root)

	if _, err := stat(filepath.Join(got.output, "work", "after-0")); err == nil {
		t.Error("the task after a short-circuiting failure executed")
	}
	if got.result["success"] != false {
		t.Errorf("the run passed despite a short-circuiting failure: %v", got.result)
	}
	if got.status != 0 {
		t.Errorf("exited %d; stopping early is the run completing as asked, not a refusal", got.status)
	}
}

// COVERS: FR-4.9 | positive
func TestShortCircuitFailureStopsNothingWhenTheTaskPasses(t *testing.T) {
	root := project(t, `
tasks:
  - name: passes
    command: "true"
    short-circuit-failure: true
  - name: after
    command: "echo ran > {work_dir}/marker"
`, nil)

	got := runBolt(t, root)
	if _, err := stat(filepath.Join(got.output, "work", "after-0")); err != nil {
		t.Errorf("the run stopped after a task that passed: %v", err)
	}
}

// COVERS: FR-4.8 | positive
func TestWithoutItAFailingTaskDoesNotStopTheRun(t *testing.T) {
	// Stopping early throws away the evidence the remaining tasks would have
	// produced and leaves a reader unable to tell what else was wrong.
	root := project(t, `
tasks:
  - name: fails
    command: "false"
  - name: after
    command: "echo ran > {work_dir}/marker"
`, nil)

	got := runBolt(t, root)
	if _, err := os.Stat(filepath.Join(got.output, "work", "after-0", "marker")); err != nil {
		t.Errorf("the run stopped at the failure: %v", err)
	}
}

// COVERS: FR-3.10c, FR-4.10, FR-4.10a, FR-4.10b | negative
func TestACommandThatCannotStartFailsItsOwnTaskAndTheRunCarriesOn(t *testing.T) {
	// Checking requires up front is a guarantee about requires, not about every
	// way a process fails to launch. A jig that under-declares still has a
	// command that cannot start, and it is that task's failure.
	root := project(t, `
requires: [sh]
tasks:
  - name: undeclared
    command: "no-such-tool-was-declared"
  - name: after
    command: "echo ran > {work_dir}/marker"
`, nil)

	got := runBolt(t, root)

	if got.status != 0 {
		t.Errorf("exited %d; a command that cannot start is a failing task, not a refusal", got.status)
	}
	if got.result["success"] != false {
		t.Errorf("the run passed with a command that could not start: %v", got.result)
	}
	if _, err := os.Stat(filepath.Join(got.output, "work", "after-0", "marker")); err != nil {
		t.Errorf("the run did not carry on: %v", err)
	}

	reasons := strings.Join(reasonMessages(t, got.result), " ")
	if !strings.Contains(reasons, "undeclared") {
		t.Errorf("no reason names the task that could not start: %q", reasons)
	}
}
