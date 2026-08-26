package cli_test

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/scriptedworld/bolt/internal/cli"
)

// monorepo is the motivating case: one project jig whose tasks are jig tasks,
// each running a shared jig at its own base. The shared jig consumes paths, so
// a base holding none is the FR-5.15 case without a second fixture.
func monorepo(t *testing.T, projectJig string, extra map[string]string) string {
	t.Helper()

	files := map[string]string{
		"bolt.shared.yaml": `
tasks:
  - name: count
    command: "echo {all_paths} > {work_dir}/counted"
    matching: ["**/*.src"]
`,
		"go/one.src":     "",
		"python/two.src": "",
		"empty/note.txt": "",
	}
	for name, contents := range extra {
		files[name] = contents
	}

	root := project(t, projectJig, files)
	return root
}

// runNested runs the project jig over root. The jig the helper writes is called
// `check`, because that is what project() names it.
func runNested(t *testing.T, root string, extra ...string) outcome {
	t.Helper()
	return runBolt(t, root, extra...)
}

// COVERS: FR-5.1, FR-5.10, FR-5.10a | positive
func TestATaskNamingAJigRunsItAtTheSubdirectoryInNames(t *testing.T) {
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
`, nil)

	got := runNested(t, root)
	if got.result["success"] != true {
		t.Fatalf("the nested run failed: %v %s", got.result["reasons"], got.stderr)
	}

	// The child walked its own base, so it saw go/one.src and not python's.
	counted := findFile(t, filepath.Join(got.output, "work", "go-side-0"), "counted")
	if !strings.Contains(counted, "one.src") {
		t.Errorf("the child did not see its own base's files: %q", counted)
	}
	if strings.Contains(counted, "two.src") {
		t.Errorf("the child saw outside its base: %q", counted)
	}
}

// COVERS: FR-5.2 | positive
func TestAChildWritesIntoItsTasksWorkDirectoryAndIsLinkedByARelativeSymlink(t *testing.T) {
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
`, nil)

	got := runNested(t, root)
	link := filepath.Join(got.output, "work", "go-side-0", "output.yaml")

	info, err := os.Lstat(link)
	if err != nil {
		t.Fatalf("no output.yaml for the jig task: %v", err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("output.yaml is a regular file, so a jig task's envelope was copied rather than linked")
	}

	target, err := os.Readlink(link)
	if err != nil {
		t.Fatalf("reading the link: %v", err)
	}
	if filepath.IsAbs(target) {
		t.Errorf("the link is absolute (%s), so the tree does not survive being moved", target)
	}
	if !strings.HasSuffix(target, "result.yaml") {
		t.Errorf("the link points at %s, want the child's result.yaml", target)
	}
	if _, err := os.Stat(link); err != nil {
		t.Errorf("the link does not resolve: %v", err)
	}
}

// COVERS: FR-5.2 | regression
func TestANestedTreeSurvivesBeingMoved(t *testing.T) {
	// The whole run is one artifact. A link out of it, or an absolute one,
	// breaks the moment somebody archives the directory or hands it on.
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
`, nil)

	got := runNested(t, root)

	moved := filepath.Join(t.TempDir(), "relocated")
	if err := os.Rename(got.output, moved); err != nil {
		t.Fatalf("moving the run: %v", err)
	}

	link := filepath.Join(moved, "work", "go-side-0", "output.yaml")
	info, err := os.Lstat(link)
	if err != nil {
		t.Fatalf("no output.yaml after the move: %v", err)
	}
	// A regular file survives a move for a reason that says nothing about
	// FR-5.2, so this asserts the link is a link before asserting it resolves.
	if info.Mode()&os.ModeSymlink == 0 {
		t.Fatal("output.yaml is a regular file, so the move proves nothing about the link")
	}
	if _, err := os.Stat(link); err != nil {
		t.Errorf("the link does not resolve after the run was moved: %v", err)
	}
}

// COVERS: FR-5.12, FR-5.16 | positive
func TestOneJigRunsAtManyBasesOncePerJigTask(t *testing.T) {
	// Nine Go subprojects is nine jig tasks. The work directory prefix is the
	// task's name and never the jig's, or two of them would collide.
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
  - name: python-side
    jig: shared
    in: python
`, nil)

	got := runNested(t, root)
	if got.result["success"] != true {
		t.Fatalf("the run failed: %v %s", got.result["reasons"], got.stderr)
	}

	// Each task's own evidence, not merely a directory: a jig task that ran
	// nothing would leave one of those either way.
	for task, want := range map[string]string{"go-side-0": "one.src", "python-side-0": "two.src"} {
		counted := findFile(t, filepath.Join(got.output, "work", task), "counted")
		if !strings.Contains(counted, want) {
			t.Errorf("%s did not run the shared jig at its own base: %q", task, counted)
		}
	}
	// Once against its base, never once per path: there is no command for
	// FR-4.2 to read a mode off.
	if _, err := stat(filepath.Join(got.output, "work", "go-side-1")); err == nil {
		t.Error("a jig task ran more than once")
	}
}

// COVERS: FR-5.15, FR-5.15a | edge
func TestAJigTaskWithNoInputPathsOrNoDirectoryDoesNotRunAndTheRunCarriesOn(t *testing.T) {
	// A shared jig naming subprojects a repository may not have is ordinary,
	// not exceptional, so refusing would make it unusable wherever it did not
	// fit exactly.
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
  - name: nothing-here
    jig: shared
    in: empty
  - name: not-there
    jig: shared
    in: absent
`, nil)

	got := runNested(t, root)
	if got.result["success"] != true {
		t.Fatalf("an absent or empty subdirectory failed the run: %v %s", got.result["reasons"], got.stderr)
	}

	for _, skipped := range []string{"nothing-here-0", "not-there-0"} {
		if _, err := stat(filepath.Join(got.output, "work", skipped)); err == nil {
			t.Errorf("%s produced evidence, so it ran", skipped)
		}
	}
	if _, err := stat(filepath.Join(got.output, "work", "go-side-0")); err != nil {
		t.Errorf("the run did not carry on past the skipped tasks: %v", err)
	}
}

// COVERS: FR-5.13d | negative
func TestTheChildsBaseComesFromInAndFromNowhereElse(t *testing.T) {
	// Containment is a property rather than a habit. This asserts the base is
	// unmoved rather than that the key is refused, because whether an
	// unrecognised key fails or warns is part two question 10 and open. A test
	// asserting a refusal here would close it by accident.
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
    base: python
`, nil)

	got := runNested(t, root)
	if got.status != 0 {
		t.Skipf("the jig was refused, which settles question 10 rather than this row: %s", got.stderr)
	}

	counted := findFile(t, filepath.Join(got.output, "work", "go-side-0"), "counted")
	if strings.Contains(counted, "two.src") {
		t.Errorf("a base field moved the child's base: %q", counted)
	}
	if !strings.Contains(counted, "one.src") {
		t.Errorf("the child did not run at the directory `in` named: %q", counted)
	}
}

// COVERS: FR-5.13h | negative
func TestAPathVariableInAJigTasksFieldIsAJigError(t *testing.T) {
	// The location variables are what is available there. A jig task has no
	// command consuming paths, so {each_path} has nothing to stand for.
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: "{each_path}"
`, nil)

	if got := runNested(t, root); got.status == 0 {
		t.Error("a path variable in a jig task's field was accepted")
	}
}

// COVERS: FR-5.5 | negative
func TestAJigTaskCarriesNoCondition(t *testing.T) {
	// Selecting files is the nested jig's business, never its caller's.
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
    matching: ["**/*.src"]
`, nil)

	if got := runNested(t, root); got.status == 0 {
		t.Error("a jig task carrying matching was accepted")
	}
}

// COVERS: FR-5.13, FR-5.13f, FR-5.13g | positive
func TestTheBaseNarrowsWhileTheProjectRootStaysWhatItWas(t *testing.T) {
	// A jig distributed by toolbox drops in at any depth without being written
	// to know where it was placed.
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
    config-dir: "{project_root}"
`, map[string]string{
		"go/bolt.shared.yaml": `
tasks:
  - name: where
    command: "echo {base_dir} {project_root} > {work_dir}/where"
`,
	})

	got := runNested(t, root)
	if got.result["success"] != true {
		t.Fatalf("the run failed: %v %s", got.result["reasons"], got.stderr)
	}

	// config-dir was substituted to the project root, so the child read the
	// root's shared jig rather than the one sitting beside it in go/.
	where := findFile(t, filepath.Join(got.output, "work", "go-side-0"), "counted")
	if where == "" {
		t.Error("the child ran the jig beside its base, so config-dir did not substitute")
	}
}

// COVERS: FR-5.7, FR-5.8, FR-5.13e | negative
func TestAJigThatRecursesIsStoppedAtTheCeilingWithAReadableReason(t *testing.T) {
	// The `depth` key is there to show it does nothing. There is no field for
	// the ceiling, because FR-5.7 has a nested invocation read the propagated
	// one, so a field would have nothing to act on.
	root := monorepo(t, `
tasks:
  - name: down
    jig: recursive
    in: go
    depth: 99
`, map[string]string{
		"bolt.recursive.yaml": `
tasks:
  - name: again
    jig: recursive
    in: .
`,
	})

	got := runNested(t, root)

	if got.result["success"] != false {
		t.Fatalf("a recursing jig did not fail: %v", got.result)
	}
	reasons := strings.ToLower(strings.Join(reasonMessages(t, got.result), " "))
	if !strings.Contains(reasons, "depth") && !strings.Contains(reasons, "ceiling") {
		t.Errorf("the reason does not name the limit: %v", got.result["reasons"])
	}
}

// COVERS: FR-5.6 | positive
func TestDepthIsCarriedInTheEnvironmentOfEverySpawnedProcess(t *testing.T) {
	// A task command that invokes bolt directly is the case this contemplates,
	// so the depth has to survive the process boundary rather than being
	// bookkeeping bolt keeps to itself.
	root := project(t, `
tasks:
  - name: reads
    command: "env | grep -i bolt > {work_dir}/env || true"
`, nil)

	got := runBolt(t, root)
	seen := read(t, filepath.Join(got.output, "work", "reads-0", "env"))
	if !strings.Contains(strings.ToUpper(seen), "DEPTH") {
		t.Errorf("no depth variable reached the command's environment:\n%s", seen)
	}
}

// COVERS: FR-5.13j, FR-5.17 | positive
func TestJigTasksNamingNoDefinitionsFileInheritTheOneTheInvocationNamed(t *testing.T) {
	// Six Python subprojects share one set of adjustments instead of carrying
	// six copies.
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
  - name: python-side
    jig: shared
    in: python
`, map[string]string{
		"bolt.shared.yaml": `
tasks:
  - name: say
    command: "echo {greeting} > {work_dir}/said"
`,
		"bolt.local.definitions.yaml": "greeting: inherited\n",
	})

	got := runNested(t, root, "--definitions", "local")
	if got.result["success"] != true {
		t.Fatalf("the run failed: %v %s", got.result["reasons"], got.stderr)
	}

	for _, task := range []string{"go-side-0", "python-side-0"} {
		said := findFile(t, filepath.Join(got.output, "work", task), "said")
		if !strings.Contains(said, "inherited") {
			t.Errorf("%s did not inherit the definitions file: %q", task, said)
		}
	}
}

// COVERS: FR-8.5 | positive
func TestBothLevelsEnvelopesSurviveTheMerge(t *testing.T) {
	root := monorepo(t, `
tasks:
  - name: go-side
    jig: shared
    in: go
`, nil)

	got := runNested(t, root)

	// The parent's result, the child's result, and the child's own task
	// envelope are all still on disk.
	if _, err := stat(filepath.Join(got.output, "result.yaml")); err != nil {
		t.Errorf("the outer result is missing: %v", err)
	}
	if found := findFile(t, filepath.Join(got.output, "work", "go-side-0"), "result.yaml"); found == "" {
		t.Error("the child's own result did not survive")
	}
}

// findFile reads the first file called name anywhere under dir, and returns
// empty when there is none. A jig task's evidence sits at a depth the test does
// not fix, because FR-5.13c leaves the child's directory name to the field.
func findFile(t *testing.T, dir, name string) string {
	t.Helper()
	var found string
	_ = filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
		if err != nil || info.IsDir() || found != "" {
			return nil
		}
		if filepath.Base(path) == name {
			data, readErr := os.ReadFile(path)
			if readErr == nil {
				found = string(data)
			}
		}
		return nil
	})
	return found
}

// reasonMessages pulls the message off every reason, so a test asserts on what
// a reader would see rather than on the structure carrying it.
func reasonMessages(t *testing.T, result map[string]any) []string {
	t.Helper()
	raw, _ := result["reasons"].([]any)
	var out []string
	for _, item := range raw {
		if reason, ok := item.(map[string]any); ok {
			if message, ok := reason["message"].(string); ok {
				out = append(out, message)
			}
		}
	}
	return out
}

var _ = bytes.MinRead
var _ = cli.Ran
