package cli_test

import (
	"bytes"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"

	"github.com/scriptedworld/bolt/internal/cli"
	"github.com/scriptedworld/bolt/internal/run"
)

// snapshot lists every path under root with its size, excluding one directory,
// so a test can say what a run changed rather than what it wrote.
func snapshot(t *testing.T, root, excluding string) []string {
	t.Helper()
	var seen []string
	err := filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() && info.Name() == excluding {
			return filepath.SkipDir
		}
		if !info.IsDir() {
			relative, _ := filepath.Rel(root, path)
			seen = append(seen, relative+":"+strconv.FormatInt(info.Size(), 10))
		}
		return nil
	})
	if err != nil {
		t.Fatalf("walking %s: %v", root, err)
	}
	sort.Strings(seen)
	return seen
}

// COVERS: FR-11.2 | property
func TestARunsWholeEffectIsTheDirectoryItWrites(t *testing.T) {
	// It changes no graph state, no task state and no other record. The
	// commands here write into their work directories, which is where FR-9.2c
	// says an artifact arrives by being addressed.
	root := project(t, `
tasks:
  - name: writes
    command: "echo produced > {work_dir}/artifact"
  - name: reads
    command: "cat bolt.check.yaml > {work_dir}/copy"
`, map[string]string{"src/a.txt": "one", "src/b.txt": "two"})

	before := snapshot(t, root, "")
	got := runBolt(t, root)
	if got.result["success"] != true {
		t.Fatalf("the run failed: %v %s", got.result["reasons"], got.stderr)
	}
	after := snapshot(t, root, "")

	if strings.Join(before, "\n") != strings.Join(after, "\n") {
		t.Errorf("the run changed the tree it was pointed at:\nbefore:\n%s\nafter:\n%s",
			strings.Join(before, "\n"), strings.Join(after, "\n"))
	}
}

// COVERS: FR-11.1, FR-11.3 | property
func TestTheSameJigAgainstAThrowawayCopyGivesTheSameVerdict(t *testing.T) {
	// A run needs nothing beyond the jig it was named, the paths it was handed
	// and the tree those paths sit in, which is what lets one run in a worker
	// sandbox or against a copy prepared to test a prospective merge.
	body := `
tasks:
  - name: counts
    command: "wc -l < {each_path} > {work_dir}/lines"
    matching: ["**/*.txt"]
`
	files := map[string]string{"src/a.txt": "one\ntwo\n", "src/b.txt": "three\n"}

	original := project(t, body, files)
	copied := project(t, body, files)

	first := runBolt(t, original)
	second := runBolt(t, copied)

	if first.result["success"] != second.result["success"] {
		t.Errorf("the verdicts differ: %v against %v", first.result["success"], second.result["success"])
	}
	if names(t, first) != names(t, second) {
		t.Errorf("the evidence differs:\n%s\n%s", names(t, first), names(t, second))
	}
}

// names is the tasks a result carries, which two runs of one jig over the same
// content have to agree on wherever the tree happens to sit.
func names(t *testing.T, got outcome) string {
	t.Helper()
	evidence, _ := metadata(t, got.result)["evidence"].(map[string]any)
	var out []string
	for task := range evidence {
		out = append(out, task)
	}
	sort.Strings(out)
	return strings.Join(out, ",")
}

// COVERS: NFR-12.4, NFR-12.2 | property
func TestBoltBuildsWithoutACToolchainAndLinksStatically(t *testing.T) {
	// An image carries one file and a cross-build needs no target compiler.
	// Asserted against a real build rather than against a flag in a script,
	// because the flag is what drifts.
	if testing.Short() {
		t.Skip("builds a binary")
	}

	binary := filepath.Join(t.TempDir(), "bolt")
	build := exec.Command("go", "build", "-o", binary, "github.com/scriptedworld/bolt/cmd/bolt")
	build.Env = append(os.Environ(), "CGO_ENABLED=0")
	if out, err := build.CombinedOutput(); err != nil {
		t.Fatalf("building with CGO_ENABLED=0: %v\n%s", err, out)
	}

	// One file, and nothing beside it. `file` is not everywhere, so this reads
	// the binary for the interpreter a dynamically linked ELF names.
	data, err := os.ReadFile(binary)
	if err != nil {
		t.Fatalf("reading the binary: %v", err)
	}
	if bytes.Contains(data, []byte("/lib64/ld-linux")) || bytes.Contains(data, []byte("/lib/ld-linux")) {
		t.Error("the binary names a dynamic loader, so it is not statically linked")
	}

	// It runs with nothing else present, which is what installing beside an
	// unknown toolchain means.
	probe := exec.Command(binary, "help")
	probe.Env = []string{}
	if out, err := probe.CombinedOutput(); err != nil {
		t.Errorf("the binary does not run with an empty environment: %v\n%s", err, out)
	}
}

// COVERS: NFR-12.1 | positive
func TestBoltsOwnGateIsABoltRunOverItsOwnRepository(t *testing.T) {
	// Until this is true, every claim bolt makes about being a usable gate is
	// untested by the project in the best position to test it.
	if testing.Short() {
		t.Skip("runs the real gate")
	}

	// Already inside a bolt run, which is this row holding rather than a reason
	// to skip. The gate's `tests` task runs `go test`, which reaches this test,
	// so running the gate again here nests one inside itself: the depth ceiling
	// stops it at four but the cost multiplies at every level on the way.
	//
	// The variable is the evidence. Bolt exports it into every process it
	// spawns, so finding it set means a bolt run is what started this suite.
	if depth, nested := os.LookupEnv(run.DepthVariable); nested {
		if _, err := os.Stat(filepath.Join(os.Getenv("PWD"), "..", "..", "bolt.go-quality.yaml")); err != nil {
			t.Errorf("running under bolt at depth %s but the gate's jig is not where it should be: %v", depth, err)
		}
		return
	}

	repo, err := filepath.Abs("../..")
	if err != nil {
		t.Fatalf("finding the repository: %v", err)
	}
	if _, err := os.Stat(filepath.Join(repo, "bolt.go-quality.yaml")); err != nil {
		t.Fatalf("bolt has no jig of its own: %v", err)
	}

	// Pointed at a directory outside the repository, so the gate's own run does
	// not land in the tree while the suite is running.
	output := filepath.Join(t.TempDir(), "gate")

	var stdout, stderr bytes.Buffer
	status := cli.Main([]string{
		"--output-dir", output, "--config-dir", repo, "go-quality", repo,
	}, &stdout, &stderr)

	if status != 0 {
		t.Fatalf("bolt could not carry out its own gate: %d %s", status, stderr.String())
	}

	result := resultAt(t, output)
	if _, ok := result["success"].(bool); !ok {
		t.Errorf("the gate produced no verdict: %v", result)
	}
	// The traceability task is in it, which is the check that every test cites
	// a requirement and every cited requirement exists.
	evidence := metadata(t, result)["evidence"].(map[string]any)
	if _, ran := evidence["traceability"]; !ran {
		t.Errorf("the gate did not run traceability: %v", evidence)
	}
}
