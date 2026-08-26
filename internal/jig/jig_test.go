package jig_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/scriptedworld/bolt/internal/jig"
)

// written puts a jig on disk and returns the directory holding it.
func written(t *testing.T, body string) string {
	t.Helper()
	dir := t.TempDir()
	if err := os.WriteFile(filepath.Join(dir, jig.Filename("check")), []byte(body), 0o644); err != nil {
		t.Fatalf("writing the jig: %v", err)
	}
	return dir
}

// COVERS: FR-3.9 | positive
func TestAJigFileIsNamedFromTheJig(t *testing.T) {
	if got := jig.Filename("go-quality"); got != "bolt.go-quality.yaml" {
		t.Errorf("got %s, want bolt.go-quality.yaml", got)
	}
}

// COVERS: FR-3.1, FR-3.2 | positive
func TestAJigLoadsItsTasksInDeclarationOrder(t *testing.T) {
	dir := written(t, `
requires: [go, gofmt]
tasks:
  - name: first
    description: the first one
    command: "true"
  - name: second
    command: "false"
    short-circuit-failure: true
`)

	loaded, err := jig.Load(dir, "check")
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	if names := loaded.Names(); names[0] != "first" || names[1] != "second" {
		t.Errorf("got %v, want declaration order", names)
	}
	if loaded.Tasks[0].Description != "the first one" {
		t.Errorf("description is %q", loaded.Tasks[0].Description)
	}
	if !loaded.Tasks[1].ShortCircuit {
		t.Error("short-circuit-failure did not survive loading")
	}
}

// COVERS: FR-3.10 | positive
func TestAJigDeclaresItsWholeDependencyInventory(t *testing.T) {
	dir := written(t, `
requires: [go, gofmt, go]
tasks:
  - name: build
    command: "go build ./..."
`)

	loaded, err := jig.Load(dir, "check")
	if err != nil {
		t.Fatalf("load: %v", err)
	}

	got := loaded.SortedRequires()
	if len(got) != 2 || got[0] != "go" || got[1] != "gofmt" {
		t.Errorf("got %v, want [go gofmt] deduplicated and ordered", got)
	}
}

// COVERS: FR-3.3a | negative
func TestTwoTasksCannotShareAName(t *testing.T) {
	dir := written(t, `
tasks:
  - name: same
    command: "true"
  - name: same
    command: "false"
`)

	_, err := jig.Load(dir, "check")
	if err == nil {
		t.Fatal("a duplicate name was accepted, so two tasks would share a work directory")
	}
	if !strings.Contains(err.Error(), "same") {
		t.Errorf("the error does not name the duplicate: %v", err)
	}
}

// COVERS: FR-4.2 | negative
func TestACommandNamingBothPathVariablesIsAJigError(t *testing.T) {
	dir := written(t, `
tasks:
  - name: confused
    command: "lint {each_path} {all_paths}"
`)

	_, err := jig.Load(dir, "check")
	if err == nil {
		t.Fatal("a command naming both was accepted, so how it runs cannot be read off it")
	}
}

// COVERS: FR-3.4b | negative
func TestMatchingOnACommandThatConsumesNoPathsIsAJigError(t *testing.T) {
	// Caught in validation rather than quietly ignored: a selection built and
	// discarded reads as a filter that works.
	dir := written(t, `
tasks:
  - name: whole
    command: "go build ./..."
    matching: ["**/*.go"]
`)

	if _, err := jig.Load(dir, "check"); err == nil {
		t.Fatal("matching was accepted on a command naming no path variable")
	}
}

// COVERS: FR-1.5 | negative
func TestAJigThatIsNotWellFormedFailsValidation(t *testing.T) {
	for name, body := range map[string]string{
		"no tasks":                "requires: [go]\n",
		"a task with no name":     "tasks:\n  - command: \"true\"\n",
		"neither command nor jig": "tasks:\n  - name: empty\n",
		"both command and jig":    "tasks:\n  - name: both\n    command: \"true\"\n    jig: other\n",
	} {
		t.Run(name, func(t *testing.T) {
			if _, err := jig.Load(written(t, body), "check"); err == nil {
				t.Error("it validated, so a broken jig would surface halfway through a gate")
			}
		})
	}
}

// COVERS: FR-3.12 | negative
func TestAJigThatIsNotThereIsAnError(t *testing.T) {
	if _, err := jig.Load(t.TempDir(), "absent"); err == nil {
		t.Error("a missing jig loaded, so the run would proceed with no tasks")
	}
}

// COVERS: FR-4.2 | property
func TestHowATaskRunsIsReadOffItsCommand(t *testing.T) {
	cases := map[string]struct {
		command  string
		consumes bool
		perPath  bool
	}{
		"names each_path": {"lint {each_path}", true, true},
		"names all_paths": {"lint {all_paths}", true, false},
		"names neither":   {"go build ./...", false, false},
	}

	for name, want := range cases {
		t.Run(name, func(t *testing.T) {
			task := jig.Task{Command: want.command}
			if task.ConsumesPaths() != want.consumes {
				t.Errorf("ConsumesPaths is %v, want %v", task.ConsumesPaths(), want.consumes)
			}
			if task.PerPath() != want.perPath {
				t.Errorf("PerPath is %v, want %v", task.PerPath(), want.perPath)
			}
		})
	}
}
