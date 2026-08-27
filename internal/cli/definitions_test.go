package cli_test

import (
	"bytes"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/scriptedworld/bolt/internal/cli"
	"github.com/scriptedworld/wrench"
)

// read is a file a command wrote into its work directory, which is how these
// tests see what the command was actually handed.
func read(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("reading %s: %v", path, err)
	}
	return string(data)
}

// stat says whether an execution left a directory behind, which is how a test
// tells "did not run" from "ran and failed".
func stat(path string) (fs.FileInfo, error) {
	return os.Stat(path)
}

// runBoltAt runs a named jig directly on a base, which is the command-line
// invocation a nested run has to be identical to.
func runBoltAt(t *testing.T, configDir, base, jigName string) string {
	t.Helper()
	output := filepath.Join(t.TempDir(), "direct")

	var stdout, stderr bytes.Buffer
	if status := cli.Main([]string{
		"--output-dir", output, "--config-dir", configDir, jigName, base,
	}, &stdout, &stderr); status != 0 {
		t.Fatalf("the direct run was refused: %s", stderr.String())
	}
	return output
}

// variables reads one execution's manifest and returns what it recorded, which
// is where the layer a value came from is written down.
func variables(t *testing.T, output, execution string) map[string]any {
	t.Helper()
	value, err := wrench.LoadFormattedFile(
		filepath.Join(output, "work", execution, "manifest.yaml"),
		wrench.ManifestSchema, wrench.YAML, wrench.LocalFile,
	)
	if err != nil {
		t.Fatalf("reading the manifest: %v", err)
	}
	return value.(map[string]any)["variables"].(map[string]any)
}

// COVERS: FR-4.16, FR-4.16a, FR-4.17 | positive
func TestADefinitionsFileMergesOverTheJigsOwnBlock(t *testing.T) {
	// The jig ships working values and the file overrides the one that differs
	// here, so an adopter writes the line that changed and inherits the rest.
	root := project(t, `
definitions:
  line_length: "88"
  target: py312
tasks:
  - name: show
    command: "echo {line_length} {target} > {work_dir}/said"
`, map[string]string{
		"bolt.local.definitions.yaml": "line_length: 100\n",
	})

	got := runBolt(t, root, "--definitions", "local")
	if got.result["success"] != true {
		t.Fatalf("the run failed: %v %s", got.result["reasons"], got.stderr)
	}

	said := read(t, filepath.Join(got.output, "work", "show-0", "said"))
	if strings.TrimSpace(said) != "100 py312" {
		t.Errorf("the command saw %q, want %q: the file should win and the jig's other value stand", strings.TrimSpace(said), "100 py312")
	}
}

// COVERS: FR-9.5g | positive
func TestAManifestRecordsTheLayerEachValueCameFrom(t *testing.T) {
	root := project(t, `
definitions:
  line_length: "88"
  target: py312
tasks:
  - name: show
    command: "echo {line_length} {target}"
`, map[string]string{
		"bolt.local.definitions.yaml": "line_length: 100\n",
	})

	got := runBolt(t, root, "--definitions", "local")
	recorded := variables(t, got.output, "show-0")

	for name, want := range map[string]string{
		"base_dir":    "bolt",
		"target":      "jig",
		"line_length": "file",
	} {
		entry, ok := recorded[name].(map[string]any)
		if !ok {
			t.Errorf("%s is not in the manifest", name)
			continue
		}
		if entry["from"] != want {
			t.Errorf("%s says it came from %v, want %s", name, entry["from"], want)
		}
	}
}

// COVERS: FR-4.18, FR-4.18a | negative
func TestAPlaceholderNothingDefinesRefusesBeforeAnythingExecutes(t *testing.T) {
	// Substituting empty is the reading that fails silently: the command line
	// would be short an argument and the tool would report something else.
	root := project(t, `
tasks:
  - name: first
    command: "echo ran > {work_dir}/marker"
  - name: needs
    command: "check --requirements {requirements}"
`, nil)

	got := runBolt(t, root)

	if got.status == 0 {
		t.Errorf("a jig naming an undefined placeholder ran anyway")
	}
	if !strings.Contains(got.stderr, "requirements") {
		t.Errorf("the refusal does not name the placeholder: %s", got.stderr)
	}
	if _, err := stat(filepath.Join(got.output, "work", "first-0")); err == nil {
		t.Error("a task executed before the check, so the refusal was not up front")
	}
}

// COVERS: FR-4.19 | negative
func TestAJigRedefiningALocationIsRefused(t *testing.T) {
	root := project(t, `
definitions:
  base_dir: /somewhere/else
tasks:
  - name: show
    command: "echo {base_dir}"
`, nil)

	got := runBolt(t, root)

	if got.status == 0 {
		t.Errorf("a jig redefining a location ran anyway")
	}
	if !strings.Contains(got.stderr, "base_dir") {
		t.Errorf("the refusal does not name what was redefined: %s", got.stderr)
	}
}

// COVERS: FR-4.17c | property
func TestADefinedValueCannotIntroduceAPathVariable(t *testing.T) {
	// A value is a literal, so FR-4.2 still reads how a task runs off the
	// command as written. A definition holding {each_path} reaches the command
	// as those characters and does not make the task run once per path.
	root := project(t, `
definitions:
  sneaky: "{each_path}"
tasks:
  - name: once
    command: "echo {sneaky} > {work_dir}/said"
`, map[string]string{"a.txt": "", "b.txt": ""})

	got := runBolt(t, root)
	if got.result["success"] != true {
		t.Fatalf("the run failed: %v %s", got.result["reasons"], got.stderr)
	}

	// One execution, not one per path.
	if _, err := stat(filepath.Join(got.output, "work", "once-1")); err == nil {
		t.Error("the task ran more than once, so a definition changed how it runs")
	}
	if said := strings.TrimSpace(read(t, filepath.Join(got.output, "work", "once-0", "said"))); said != "{each_path}" {
		t.Errorf("the value reached the command as %q, want it left literal", said)
	}
}

// COVERS: FR-4.17a | property
func TestAValueIsALiteralAndCarriesNoSubstitutionsOfItsOwn(t *testing.T) {
	// Reading a definition settles it. A value naming another definition is not
	// a reference to it, so nothing resolves in terms of anything else and
	// there is no order to resolve them in.
	root := project(t, `
definitions:
  inner: resolved
  outer: "{inner}"
tasks:
  - name: show
    command: "echo {outer} > {work_dir}/said"
`, nil)

	got := runBolt(t, root)
	if got.result["success"] != true {
		t.Fatalf("the run failed: %v %s", got.result["reasons"], got.stderr)
	}

	if said := strings.TrimSpace(read(t, filepath.Join(got.output, "work", "show-0", "said"))); said != "{inner}" {
		t.Errorf("the value reached the command as %q, want it left literal", said)
	}
}

// COVERS: FR-4.17b | positive
func TestARelativeValueResolvesAgainstTheBaseBecauseTheCommandStandsThere(t *testing.T) {
	// wrench's case: one requirements document at the root, serving runs based
	// at go/ and at python/ where no such file sits. Bolt does not rewrite the
	// value, because nothing tells ../REQUIREMENTS.md from 100. The command
	// standing at the base is what makes it resolve.
	root := project(t, `
definitions:
  contract: ../CONTRACT.md
tasks:
  - name: reads
    command: "cat {contract} > {work_dir}/said"
`, map[string]string{
		"CONTRACT.md": "one document, two packs\n",
		"go/keep.txt": "",
	})

	base := filepath.Join(root, "go")
	output := filepath.Join(t.TempDir(), "evidence")

	var stdout, stderr bytes.Buffer
	status := cli.Main([]string{
		"--output-dir", output, "--config-dir", root, "check", base,
	}, &stdout, &stderr)
	if status != 0 {
		t.Fatalf("the run was refused: %s", stderr.String())
	}

	said := strings.TrimSpace(read(t, filepath.Join(output, "work", "reads-0", "said")))
	if said != "one document, two packs" {
		t.Errorf("the command read %q, so the relative value did not resolve against the base", said)
	}
}

// COVERS: FR-4.16c, FR-4.20 | negative
func TestADefinitionsFileThatIsNotOneLevelOfScalarsRefusesTheRun(t *testing.T) {
	root := project(t, `
tasks:
  - name: show
    command: "echo {line_length}"
`, map[string]string{
		"bolt.local.definitions.yaml": "python:\n  line_length: 100\n",
	})

	got := runBolt(t, root, "--definitions", "local")

	if got.status == 0 {
		t.Error("a nested definitions file was accepted")
	}
}

// COVERS: FR-4.18b | edge
func TestADefinitionHoldingAnEmptyValueSatisfiesItsPlaceholder(t *testing.T) {
	root := project(t, `
definitions:
  extra_flags: ""
tasks:
  - name: show
    command: "echo begin {extra_flags} end > {work_dir}/said"
`, nil)

	got := runBolt(t, root)
	if got.result["success"] != true {
		t.Fatalf("an empty definition refused the run: %v %s", got.result["reasons"], got.stderr)
	}

	if said := strings.TrimSpace(read(t, filepath.Join(got.output, "work", "show-0", "said"))); said != "begin  end" {
		t.Errorf("the command saw %q, want the empty value substituted as one empty argument", said)
	}
}
