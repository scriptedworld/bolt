// Package run executes a jig's tasks and records what happened on disk.
package run

import (
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"time"

	"github.com/scriptedworld/bolt/internal/adapter"
	"github.com/scriptedworld/bolt/internal/definitions"
	"github.com/scriptedworld/bolt/internal/jig"
	"github.com/scriptedworld/bolt/internal/paths"
	"github.com/scriptedworld/wrench"
)

// Options say what a run is: which jig, and where.
type Options struct {
	Jig       *jig.Jig
	BaseDir   string
	ConfigDir string
	OutputDir string
	// Definitions is the file layer, already read from the config directory.
	// The jig's own block is on the jig, and bolt's own values are per
	// execution, so this is the only layer a caller supplies.
	Definitions map[string]string
	// Now stamps the default output directory. Passed in so a test does not
	// have to reach for a clock.
	Now time.Time
	// Progress receives a line per execution. Nothing a consumer needs to know
	// about the outcome goes here; it exists so a person watching a gate can
	// see it moving.
	Progress io.Writer
}

// An Outcome says what the run did, not whether the tools were happy. The
// authoritative verdict is the envelope.
type Outcome struct {
	OutputDir  string
	Executions int
	Skipped    []string
}

// Execute runs every task in declaration order and writes the evidence.
//
// A failing task does not stop the run: stopping early throws away the
// evidence the remaining tasks would have produced and leaves a reader unable
// to tell what else was wrong.
func Execute(options Options) (*Outcome, error) {
	outputDir := options.OutputDir
	if outputDir == "" {
		outputDir = filepath.Join(options.BaseDir, DefaultOutputName(options.Now))
	}
	absoluteOutput, err := filepath.Abs(outputDir)
	if err != nil {
		return nil, err
	}

	if err := prepareOutput(absoluteOutput); err != nil {
		return nil, err
	}

	base, err := filepath.Abs(options.BaseDir)
	if err != nil {
		return nil, err
	}
	configDir, err := filepath.Abs(options.ConfigDir)
	if err != nil {
		return nil, err
	}

	// The run never walks its own output directory. Excluding it by the path
	// this run chose is knowable; recognising some other run's by name is not.
	var withinBase []string
	if relative, relErr := filepath.Rel(base, absoluteOutput); relErr == nil && !isOutside(relative) {
		withinBase = append(withinBase, relative)
	}

	found, err := paths.Walk(base, withinBase)
	if err != nil {
		return nil, fmt.Errorf("walking %s: %w", base, err)
	}

	// Checked before the first task executes, where `requires` is checked. A
	// jig dropped at a base that does not define what it needs refuses in the
	// first second rather than partway through a gate.
	if err := definitions.Check(options.Jig, options.Definitions); err != nil {
		return nil, err
	}

	// Resolved before the first task executes. An unknown adapter is a run
	// bolt cannot carry out, and learning that after half a gate has run is
	// learning it too late.
	adapters := map[string]string{}
	for _, task := range options.Jig.Tasks {
		executable, err := adapter.Resolve(configDir, task.Adapter)
		if err != nil {
			return nil, fmt.Errorf("task %q: %w", task.Name, err)
		}
		adapters[task.Name] = executable
	}

	outcome := &Outcome{OutputDir: absoluteOutput}
	for _, task := range options.Jig.Tasks {
		ran, err := runTask(task, options, base, configDir, absoluteOutput, found, adapters[task.Name])
		if err != nil {
			return nil, err
		}
		if ran == 0 {
			outcome.Skipped = append(outcome.Skipped, task.Name)
			continue
		}
		outcome.Executions += ran
	}

	return outcome, nil
}

func runTask(task jig.Task, options Options, base, configDir, outputDir string, found []string, executable string) (int, error) {
	selection := found
	if task.ConsumesPaths() {
		var err error
		selection, err = paths.Select(found, task.Matching, task.Excluding)
		if err != nil {
			return 0, fmt.Errorf("task %q: %w", task.Name, err)
		}
		// A command naming a path variable with nothing to consume does not
		// execute, and produces no output.
		if len(selection) == 0 {
			return 0, nil
		}
	}

	each := []string{""}
	if task.PerPath() {
		each = selection
	}

	for ordinal, path := range each {
		if err := runOnce(task, options, base, configDir, outputDir, ordinal, len(each), path, selection, executable); err != nil {
			return 0, err
		}
	}
	return len(each), nil
}

func runOnce(task jig.Task, options Options, base, configDir, outputDir string, ordinal, total int, each string, selection []string, executable string) error {
	workDir := filepath.Join(outputDir, WorkSubdir, executionDir(task.Name, ordinal, total))
	if err := os.MkdirAll(workDir, 0o755); err != nil {
		return err
	}

	locations := Locations{
		ProjectRoot: base,
		BaseDir:     base,
		WorkDir:     workDir,
		ConfigDir:   configDir,
		OutputDir:   outputDir,
	}

	// Built per execution because the work directory is one of bolt's values
	// and it differs every time. The jig and file layers are settled, and
	// Execute has already refused a run they could not satisfy.
	mapping, err := definitions.Resolve(locations.values(), options.Jig.Definitions, options.Definitions)
	if err != nil {
		return err
	}
	command := substitute(task.Command, mapping, each, selection)

	// The manifest is written before the command runs, so an execution that
	// was killed, or never started, still records what was going to be
	// attempted. The case that most needs a record is the one that would
	// otherwise have none.
	if err := writeManifest(workDir, task, mapping, command, ordinal, each, selection); err != nil {
		return err
	}

	if options.Progress != nil {
		fmt.Fprintf(options.Progress, "%s\n", filepath.Base(workDir))
	}

	status, err := runCommand(command, base, workDir)
	if err != nil {
		return err
	}

	return reachVerdict(workDir, task, status, executable, locations, mapping, each, selection)
}

// runCommand executes the line as a subprocess standing at the base, capturing
// what it wrote and what it exited with.
//
// The captured streams are written as the process produces them rather than
// atomically. They are not written as a unit, and a killed command's partial
// output is exactly what has to survive.
func runCommand(command, base, workDir string) (int, error) {
	stdout, err := os.Create(filepath.Join(workDir, StdoutFile))
	if err != nil {
		return 0, err
	}
	defer stdout.Close()

	stderr, err := os.Create(filepath.Join(workDir, StderrFile))
	if err != nil {
		return 0, err
	}
	defer stderr.Close()

	process := exec.Command("sh", "-c", command)
	process.Dir = base
	process.Stdout = stdout
	process.Stderr = stderr
	process.Stdin = nil

	status := 0
	if err := process.Run(); err != nil {
		var exit *exec.ExitError
		if !errors.As(err, &exit) {
			return 0, fmt.Errorf("starting %q: %w", command, err)
		}
		status = exit.ExitCode()
	}

	if err := os.WriteFile(filepath.Join(workDir, ExitCodeFile), []byte(strconv.Itoa(status)+"\n"), 0o644); err != nil {
		return 0, err
	}
	return status, nil
}

// writeManifest records what the execution was going to be given: every key the
// three layers hold, the value that won, and the layer it came from. The same
// key means different things depending on which layer won, and the command line
// alone does not say.
func writeManifest(workDir string, task jig.Task, mapping definitions.Mapping, command string, ordinal int, each string, selection []string) error {
	variables := mapping.Record()
	if task.PerPath() {
		variables[bare(jig.EachPath)] = boltValue(each)
	}
	if task.ConsumesPaths() && !task.PerPath() {
		variables[bare(jig.AllPaths)] = boltValue(asAny(selection))
	}

	manifest := map[string]any{
		"task":      task.Name,
		"ordinal":   ordinal,
		"command":   command,
		"variables": variables,
	}
	if task.ConsumesPaths() {
		// The whole matched list, not only the path this execution was handed.
		// One path alone loses what the task was offered.
		manifest["selection"] = map[string]any{"matched": asAny(selection)}
	}

	path := filepath.Join(workDir, ManifestFile)
	return wrench.SaveFormattedFile(manifest, path, wrench.ManifestSchema, wrench.YAML, wrench.LocalFile)
}

// boltValue records a path variable the way the mapping records everything
// else, so a reader of a manifest meets one shape rather than two.
func boltValue(value any) map[string]any {
	return map[string]any{"value": value, "from": string(definitions.FromBolt)}
}

func asAny(items []string) []any {
	out := make([]any, 0, len(items))
	for _, item := range items {
		out = append(out, item)
	}
	return out
}

func isOutside(relative string) bool {
	return relative == ".." || len(relative) > 2 && relative[:3] == ".."+string(filepath.Separator)
}
