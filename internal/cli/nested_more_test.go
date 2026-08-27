package cli_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// COVERS: FR-5.3, FR-5.4 | property
func TestAJigTaskCarriesTheSameBookkeepingAsACommandTask(t *testing.T) {
	// Nothing reading work/*/ needs to know which kind it is looking at, and
	// the merge does not know a constituent was a nested run.
	root := monorepo(t, `
tasks:
  - name: plain
    command: "true"
  - name: nested
    jig: shared
    in: go
`, nil)

	got := runNested(t, root)

	for _, task := range []string{"plain-0", "nested-0"} {
		for _, file := range []string{"manifest.yaml", "output.yaml"} {
			if _, err := os.Stat(filepath.Join(got.output, "work", task, file)); err != nil {
				t.Errorf("%s has no %s: %v", task, file, err)
			}
		}
	}

	// Both appear in the evidence the same way, keyed by task, each with args
	// and a result. The merge folded one of each without distinguishing them.
	evidence := metadata(t, got.result)["evidence"].(map[string]any)
	for _, task := range []string{"plain", "nested"} {
		records, ok := evidence[task].([]any)
		if !ok || len(records) != 1 {
			t.Fatalf("%s is not one evidence record: %v", task, evidence[task])
		}
		record := records[0].(map[string]any)
		if record["result"] == nil || record["args"] == nil {
			t.Errorf("%s's record is missing args or result: %v", task, record)
		}
	}
}

// COVERS: FR-5.1b | positive
func TestAParentKnowsTheJigsNameAndWhereAndNothingInside(t *testing.T) {
	root := monorepo(t, `
tasks:
  - name: nested
    jig: shared
    in: go
`, nil)

	got := runNested(t, root)
	evidence := metadata(t, got.result)["evidence"].(map[string]any)

	if _, leaked := evidence["count"]; leaked {
		t.Error("the child's own task appears in the parent's evidence, so something rolled up")
	}
	if len(evidence) != 1 {
		t.Errorf("the parent's evidence has %d keys, want only its own task: %v", len(evidence), evidence)
	}
}

// COVERS: FR-5.9, FR-8.6 | property
func TestOnlyTheOutermostRelativisesAndAChildLeavesPathsAbsolute(t *testing.T) {
	// A path means the same thing to a child and to its parent, so a nested
	// run's evidence folds in with nothing rewritten.
	root := monorepo(t, `
tasks:
  - name: nested
    jig: shared
    in: go
`, nil)

	got := runNested(t, root)

	outer := metadata(t, got.result)["evidence"].(map[string]any)["nested"].([]any)[0].(map[string]any)
	if filepath.IsAbs(outer["result"].(string)) {
		t.Errorf("the outermost result carries an absolute path: %v", outer["result"])
	}

	child := read(t, filepath.Join(got.output, "work", "nested-0", "run", "result.yaml"))
	if !strings.Contains(child, root) {
		t.Errorf("the child's result does not carry absolute paths:\n%s", child)
	}
}

// COVERS: FR-5.11 | negative
func TestTheSubdirectoryIsAWrittenPathNotAPattern(t *testing.T) {
	// A pattern can say which files look like Go and never that a directory is
	// a Go module.
	root := monorepo(t, `
tasks:
  - name: nested
    jig: shared
    in: "go*"
`, nil)

	got := runNested(t, root)

	// `go*` names a directory that is not there, which FR-5.15a treats as one
	// holding nothing rather than expanding.
	if _, err := stat(filepath.Join(got.output, "work", "nested-0")); err == nil {
		t.Error("the subdirectory was treated as a pattern and matched go/")
	}
}

// COVERS: FR-5.13a, FR-5.13b | positive
func TestAFieldLeftOutIsInheritedAndConfigDirSendsTheChildElsewhere(t *testing.T) {
	root := monorepo(t, `
tasks:
  - name: inherits
    jig: shared
    in: go
  - name: elsewhere
    jig: shared
    in: python
    config-dir: tooling
`, map[string]string{
		"tooling/bolt.shared.yaml": `
tasks:
  - name: other
    command: "echo from-tooling > {work_dir}/said"
`,
	})

	got := runNested(t, root)

	// The inheriting task read the root's jig, so its child ran `count`.
	if found := findFile(t, filepath.Join(got.output, "work", "inherits-0"), "counted"); found == "" {
		t.Error("the inheriting task did not use its parent's config directory")
	}
	// The one naming config-dir read the other jig entirely.
	if said := findFile(t, filepath.Join(got.output, "work", "elsewhere-0"), "said"); !strings.Contains(said, "from-tooling") {
		t.Errorf("config-dir did not send the child elsewhere: %q", said)
	}
}

// COVERS: FR-5.13c | negative
func TestOutputDirRenamesWithinTheWorkDirectoryAndCannotRelocate(t *testing.T) {
	// Renaming is expressible and relocating is not sayable, so FR-5.2's
	// layout cannot be undone by a field.
	root := monorepo(t, `
tasks:
  - name: renamed
    jig: shared
    in: go
    output-dir: evidence
  - name: escapes
    jig: shared
    in: python
    output-dir: ../../../outside
`, nil)

	got := runNested(t, root)

	if _, err := stat(filepath.Join(got.output, "work", "renamed-0", "evidence")); err != nil {
		t.Errorf("output-dir did not rename the child's directory: %v", err)
	}

	entries, err := os.ReadDir(filepath.Join(got.output, "work", "escapes-0"))
	if err != nil {
		t.Fatalf("reading the work directory: %v", err)
	}
	for _, entry := range entries {
		if entry.IsDir() && strings.Contains(entry.Name(), "..") {
			t.Errorf("output-dir escaped the work directory as %s", entry.Name())
		}
	}
	if _, err := stat(filepath.Join(got.output, "outside")); err == nil {
		t.Error("output-dir placed the child outside its task's work directory")
	}
}

// COVERS: FR-5.13i | property
func TestAJigTasksFieldsAreRefusedByTheSchema(t *testing.T) {
	// Every one of them is schema-checkable, which a command line would not
	// have been. The part with the most power over a nested run is not the part
	// exempt from validation.
	root := monorepo(t, `
tasks:
  - name: nested
    jig: shared
    in: 42
`, nil)

	got := runNested(t, root)
	if got.status == 0 {
		t.Error("a jig task field of the wrong type was accepted")
	}
	if !strings.Contains(got.stderr, "jig.schema.json") {
		t.Errorf("the refusal did not come from the schema: %s", got.stderr)
	}
}

// COVERS: FR-5.7a | edge
func TestTheDepthGuardIsAgainstAccidentAndNotAgainstEvasion(t *testing.T) {
	// FR-5.6 contemplates a task command invoking bolt directly, and such a
	// command can unset the variable and be believed outermost. This is the
	// row's own admission written as a test, so nobody reads FR-5.7 as a
	// containment guarantee.
	root := project(t, `
tasks:
  - name: clears
    command: "sh -c 'unset BOLT_DEPTH; echo ${BOLT_DEPTH:-cleared}' > {work_dir}/said"
`, nil)

	got := runBolt(t, root)
	if said := strings.TrimSpace(read(t, filepath.Join(got.output, "work", "clears-0", "said"))); said != "cleared" {
		t.Errorf("a task command could not clear the depth variable, got %q; if that is now true, FR-5.7a is stronger than it says", said)
	}
}

// COVERS: FR-8.7 | edge
func TestRewritingReachesStructuredReferencesAndLeavesAToolsTextAlone(t *testing.T) {
	// Text a tool emitted, carried up inside a reason, stays as the tool wrote
	// it and may still name an absolute path.
	root := project(t, `
tasks:
  - name: fails
    command: "echo {base_dir} && false"
`, nil)

	got := runBolt(t, root)

	evidence := metadata(t, got.result)["evidence"].(map[string]any)
	record := evidence["fails"].([]any)[0].(map[string]any)
	if filepath.IsAbs(record["result"].(string)) {
		t.Errorf("the structured reference was not relativised: %v", record["result"])
	}

	// The task's own command line, which the manifest recorded and the merge
	// carried up as args, still names the base absolutely.
	if !strings.Contains(record["args"].(string), root) {
		t.Errorf("a tool's own text was rewritten: %v", record["args"])
	}
}

// COVERS: FR-5.14, FR-5.14a, FR-5.14b | positive
func TestAJigThatNeedsTheRepositoryRootOverridesASubdirectoryBase(t *testing.T) {
	// A tool that must be standing at the root says so in the jig it is in,
	// and that beats the base its caller named. The field is on the jig rather
	// than on the jig task, because only the jig knows the tool.
	root := monorepo(t, `
tasks:
  - name: nested
    jig: rooted
    in: go
`, map[string]string{
		"bolt.rooted.yaml": `
needs-repository-root: true
tasks:
  - name: where
    command: "pwd > {work_dir}/where; echo {base_dir} > {work_dir}/base"
  - name: sees
    command: "echo {all_paths} > {work_dir}/saw"
    matching: ["**/*.src"]
`,
	})

	got := runNested(t, root)
	if got.result["success"] != true {
		t.Fatalf("the run failed: %v %s", got.result["reasons"], got.stderr)
	}

	child := filepath.Join(got.output, "work", "nested-0")
	if where := strings.TrimSpace(findFile(t, child, "where")); where != root {
		t.Errorf("the command stood at %q, want the repository root %q", where, root)
	}
	if base := strings.TrimSpace(findFile(t, child, "base")); base != filepath.Join(root, "go") {
		t.Errorf("{base_dir} is %q, want the base the caller granted", base)
	}
}

// COVERS: FR-5.14b, FR-5.14c | regression
func TestStandingAtTheRootDoesNotWidenWhatTheChildCanRead(t *testing.T) {
	// Built the other way first, and measured: a parent naming `in: go` ran a
	// child that read a file outside the grant, with nothing in the parent's
	// jig recording it. FR-5.13 makes narrowing the base and narrowing the
	// containment check one act, so a mechanism undoing one undoes both.
	root := monorepo(t, `
tasks:
  - name: nested
    jig: rooted
    in: go
`, map[string]string{
		"bolt.rooted.yaml": `
needs-repository-root: true
tasks:
  - name: sees
    command: "echo {all_paths} > {work_dir}/saw"
    matching: ["**/*.src"]
`,
		"outside/private.src": "",
	})

	got := runNested(t, root)
	saw := findFile(t, filepath.Join(got.output, "work", "nested-0"), "saw")

	if strings.Contains(saw, "private.src") {
		t.Errorf("the child read outside the base its caller granted: %q", saw)
	}
	if !strings.Contains(saw, "one.src") {
		t.Errorf("the child did not see its own base: %q", saw)
	}
}

// COVERS: FR-5.1a | property
func TestANestedRunIsTheSameOperationAsOneFromTheCommandLine(t *testing.T) {
	// Inside its subdirectory it is identical to the same jig run on that
	// directory from the command line: one operation, one code path, two
	// callers.
	root := monorepo(t, `
tasks:
  - name: nested
    jig: shared
    in: go
`, nil)

	nested := runNested(t, root)
	nestedResult := read(t, filepath.Join(nested.output, "work", "nested-0", "run", "result.yaml"))

	direct := runBoltAt(t, root, filepath.Join(root, "go"), "shared")
	directResult := read(t, filepath.Join(direct, "result.yaml"))

	// The two differ only where a path names the output directory each was
	// given, which is the one thing the caller chose.
	if taskNames(nestedResult) != taskNames(directResult) {
		t.Errorf("the two runs folded different tasks:\nnested: %s\ndirect: %s", nestedResult, directResult)
	}
}

// taskNames is the evidence keys as they appear in a result, which is what two
// runs of one jig have to agree on however they were reached.
func taskNames(result string) string {
	var names []string
	for _, line := range strings.Split(result, "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasSuffix(trimmed, "\":") && strings.HasPrefix(trimmed, "\"") {
			names = append(names, trimmed)
		}
	}
	return strings.Join(names, ",")
}
