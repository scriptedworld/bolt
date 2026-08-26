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

// COVERS: FR-3.15 | positive
func TestAJigsDefinitionsBlockIsOptionalAndSoIsAnyEntry(t *testing.T) {
	// A jig with no placeholders writes none. One deliberately leaving a value
	// to its adopter names the placeholder in a command and defines nothing,
	// which is what FR-4.18 then refuses when nothing else supplies it.
	bare, err := jig.Load(written(t, "tasks:\n  - name: a\n    command: \"true\"\n"), "check")
	if err != nil {
		t.Fatalf("a jig with no definitions block was refused: %v", err)
	}
	if len(bare.Definitions) != 0 {
		t.Errorf("a jig with no block loaded definitions: %v", bare.Definitions)
	}

	partial, err := jig.Load(written(t, `
definitions:
  line_length: 100
  strict: true
  empty: ""
tasks:
  - name: a
    command: "check {line_length} {strict} {empty} {supplied_elsewhere}"
`), "check")
	if err != nil {
		t.Fatalf("loading: %v", err)
	}

	// A number and a boolean reach a command line as the text they were written
	// as, and an empty value is a value.
	for name, want := range map[string]string{"line_length": "100", "strict": "true", "empty": ""} {
		got, defined := partial.Definitions[name]
		if !defined {
			t.Errorf("%s is not defined", name)
			continue
		}
		if got != want {
			t.Errorf("%s loaded as %q, want %q", name, got, want)
		}
	}
	if _, defined := partial.Definitions["supplied_elsewhere"]; defined {
		t.Error("a placeholder the jig did not define was given a value anyway")
	}
}

// COVERS: FR-3.15 | edge
func TestAPlaceholderIsReadOffACommandAndABraceExpansionIsNot(t *testing.T) {
	// The name shape is the one a definitions file's keys are held to, so what
	// a command can name and what a file can define are one set. It is narrow
	// enough that a shell's own braces are not mistaken for a placeholder.
	got := jig.Placeholders("cp {base_dir}/{name}.txt out.{a,b} {} {Upper} {name}")

	want := []string{"base_dir", "name"}
	if len(got) != len(want) {
		t.Fatalf("read %v, want %v", got, want)
	}
	for i, name := range want {
		if got[i] != name {
			t.Errorf("placeholder %d is %q, want %q", i, got[i], name)
		}
	}
}
