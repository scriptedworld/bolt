package paths_test

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/scriptedworld/bolt/internal/paths"
)

// tree writes a set of files, and directories for any path ending in a slash.
func tree(t *testing.T, files map[string]string) string {
	t.Helper()
	root := t.TempDir()
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

// COVERS: FR-2.2, FR-2.2d | property
func TestWalkReturnsEveryFileInSortedOrder(t *testing.T) {
	root := tree(t, map[string]string{
		"zebra.go":       "",
		"alpha.go":       "",
		"sub/nested.go":  "",
		"sub/deep/x.txt": "",
	})

	found, err := paths.Walk(root, nil)
	if err != nil {
		t.Fatalf("walk: %v", err)
	}

	want := []string{"alpha.go", "sub/deep/x.txt", "sub/nested.go", "zebra.go"}
	if !reflect.DeepEqual(found, want) {
		t.Errorf("got %v, want %v", found, want)
	}
}

// COVERS: FR-2.2a, FR-2.2b | positive
func TestWalkHonoursGitignoreWithoutInvokingGit(t *testing.T) {
	root := tree(t, map[string]string{
		".gitignore":  "ignored.go\nbuild/\n",
		"kept.go":     "",
		"ignored.go":  "",
		"build/out.o": "",
	})

	found, err := paths.Walk(root, nil)
	if err != nil {
		t.Fatalf("walk: %v", err)
	}

	for _, path := range found {
		if path == "ignored.go" || path == "build/out.o" {
			t.Errorf("%s was walked despite .gitignore", path)
		}
	}
	if !contains(found, "kept.go") {
		t.Error("kept.go was not walked")
	}
}

// COVERS: FR-2.2a | edge
func TestANestedGitignoreAppliesToItsOwnSubtree(t *testing.T) {
	// A root-level read alone would miss this, and a project keeping one
	// further down is ordinary rather than unusual.
	root := tree(t, map[string]string{
		"keep.tmp":          "",
		"sub/.gitignore":    "*.tmp\n",
		"sub/drop.tmp":      "",
		"sub/keep.go":       "",
		"other/keep.tmp":    "",
		"sub/deep/gone.tmp": "",
	})

	found, err := paths.Walk(root, nil)
	if err != nil {
		t.Fatalf("walk: %v", err)
	}

	if contains(found, "sub/drop.tmp") {
		t.Error("sub/.gitignore did not apply to its own directory")
	}
	if contains(found, "sub/deep/gone.tmp") {
		t.Error("sub/.gitignore did not apply below itself")
	}
	if !contains(found, "keep.tmp") || !contains(found, "other/keep.tmp") {
		t.Error("sub/.gitignore applied outside its subtree")
	}
}

// COVERS: FR-2.2c | positive
func TestWalkSkipsTheDirectoriesItIsToldTo(t *testing.T) {
	root := tree(t, map[string]string{
		"src.go":                  "",
		".bolt-run/work/x/stdout": "",
		".bolt-run/result.yaml":   "",
	})

	found, err := paths.Walk(root, []string{".bolt-run"})
	if err != nil {
		t.Fatalf("walk: %v", err)
	}

	for _, path := range found {
		if len(path) > 9 && path[:9] == ".bolt-run" {
			t.Errorf("%s was walked, so a run would read its own evidence", path)
		}
	}
	if !contains(found, "src.go") {
		t.Error("src.go was not walked")
	}
}

// COVERS: FR-2.2e | negative
func TestWalkDoesNotFollowSymlinks(t *testing.T) {
	root := tree(t, map[string]string{"real.go": ""})
	outside := tree(t, map[string]string{"beyond.go": ""})

	if err := os.Symlink(outside, filepath.Join(root, "linked")); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}

	found, err := paths.Walk(root, nil)
	if err != nil {
		t.Fatalf("walk: %v", err)
	}

	for _, path := range found {
		if path != "real.go" {
			t.Errorf("walked %s, which is through a symlink and outside the base", path)
		}
	}
}

// COVERS: FR-2.7 | property
func TestATreeThatIsNotARepositoryWalksTheSameWay(t *testing.T) {
	// No .git anywhere. Bolt reads no git, so this is not a special case and
	// the walk must not treat it as one.
	root := tree(t, map[string]string{"only.go": ""})

	found, err := paths.Walk(root, nil)
	if err != nil {
		t.Fatalf("walk: %v", err)
	}
	if !reflect.DeepEqual(found, []string{"only.go"}) {
		t.Errorf("got %v, want [only.go]", found)
	}
}

// COVERS: FR-3.4 | positive
func TestMatchingSelectsByPatternAcrossDirectoryLevels(t *testing.T) {
	found := []string{"a.go", "sub/b.go", "sub/deep/c.go", "notes.md"}

	selected, err := paths.Select(found, []string{"**/*.go"}, nil)
	if err != nil {
		t.Fatalf("select: %v", err)
	}

	want := []string{"a.go", "sub/b.go", "sub/deep/c.go"}
	if !reflect.DeepEqual(selected, want) {
		t.Errorf("got %v, want %v", selected, want)
	}
}

// COVERS: FR-3.4a | positive
func TestExcludingRemovesFromWhatMatchingSelected(t *testing.T) {
	found := []string{"a.go", "a_test.go", "sub/b.go", "sub/b_test.go"}

	selected, err := paths.Select(found, []string{"**/*.go"}, []string{"**/*_test.go"})
	if err != nil {
		t.Fatalf("select: %v", err)
	}

	want := []string{"a.go", "sub/b.go"}
	if !reflect.DeepEqual(selected, want) {
		t.Errorf("got %v, want %v", selected, want)
	}
}

// COVERS: FR-3.4a | edge
func TestASingleKnownBadFileIsNamedOutright(t *testing.T) {
	// A literal path is a pattern with nothing special in it, so naming one
	// file needs no separate mechanism.
	found := []string{"a.go", "generated/legacy.go", "b.go"}

	selected, err := paths.Select(found, nil, []string{"generated/legacy.go"})
	if err != nil {
		t.Fatalf("select: %v", err)
	}

	if contains(selected, "generated/legacy.go") {
		t.Error("the named file was not excluded")
	}
	if len(selected) != 2 {
		t.Errorf("got %v, want the other two", selected)
	}
}

// COVERS: FR-3.4 | edge
func TestNoMatchingSelectsEverythingTheWalkFound(t *testing.T) {
	found := []string{"a.go", "b.md"}

	selected, err := paths.Select(found, nil, nil)
	if err != nil {
		t.Fatalf("select: %v", err)
	}
	if !reflect.DeepEqual(selected, found) {
		t.Errorf("got %v, want everything", selected)
	}
}

// COVERS: FR-3.4 | negative
func TestAnUnusablePatternIsAnError(t *testing.T) {
	if _, err := paths.Select([]string{"a.go"}, []string{"["}, nil); err == nil {
		t.Error("a malformed pattern was accepted, so it would silently match nothing")
	}
}

func contains(items []string, want string) bool {
	for _, item := range items {
		if item == want {
			return true
		}
	}
	return false
}
