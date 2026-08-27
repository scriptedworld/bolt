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
	// ProjectRoot is where the project starts. Empty means this invocation is
	// the outermost, which is assumed to sit at the base; a nested one carries
	// its parent's, because narrowing the base leaves the root what it was.
	ProjectRoot string
	// StandDir is where this run's commands stand, when that is not the base.
	// Only FR-5.14 sets it, and it reaches the working directory and nothing
	// else: the walk, the containment and the patterns stay with the base.
	StandDir string
	// Ceiling is the nesting limit in force, resolved by the outermost.
	Ceiling int
	// Depth is how many invocations deep this one is, zero at the outermost.
	Depth int
	// Fold turns a finished run directory into its one result. A nested run
	// needs it, and the package that folds reads what this package writes, so
	// it arrives as a value rather than as an import that would be a cycle.
	Fold func(outputDir, base string, outermost bool) (bool, error)
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

	// Checked before anything runs, and after the output directory exists so
	// the refusal has somewhere to be written that a caller will look.
	depth, ceiling := resolveDepth(options)
	if err := tooDeep(depth, ceiling); err != nil {
		return nil, err
	}
	options.Depth, options.Ceiling = depth, ceiling

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
	if task.IsJig() {
		return runNestedJig(task, options, base, configDir, outputDir)
	}

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
		ProjectRoot: options.projectRoot(base),
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

	status, err := runCommand(command, options.standDir(base), workDir, options)
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
func runCommand(command, base, workDir string, options Options) (int, error) {
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
	// The depth and the ceiling ride in the environment of every process bolt
	// spawns, so a task command that invokes bolt directly is nested too.
	process.Env = environment(options.Depth, options.Ceiling)

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

// saveManifest writes one execution's manifest, validated on the way out.
func saveManifest(workDir string, manifest map[string]any) error {
	path := filepath.Join(workDir, ManifestFile)
	return wrench.SaveFormattedFile(manifest, path, wrench.ManifestSchema, wrench.YAML, wrench.LocalFile)
}

// projectRoot is where the project starts. The outermost run is assumed to sit
// there and a nested one is not, so a jig based on a subtree still reaches a
// config file at the root without giving up its base.
// standDir is where a command stands, which is the base unless a jig declared
// it has to stand at the repository root.
func (o Options) standDir(base string) string {
	if o.StandDir != "" {
		return o.StandDir
	}
	return base
}

func (o Options) projectRoot(base string) string {
	if o.ProjectRoot != "" {
		return o.ProjectRoot
	}
	return base
}

// Refuse writes a refusal where a caller will find it, in the shape every
// refusal takes: a result carrying success false and a reason, so a caller
// parses one thing whatever went wrong.
func Refuse(outputDir, base string, cause error) error {
	if err := os.MkdirAll(outputDir, 0o755); err != nil {
		return err
	}
	result := map[string]any{
		"success": false,
		"reasons": []any{
			map[string]any{"kind": "bolt-refused", "message": cause.Error()},
		},
		"metadata": map[string]any{"base": base},
	}
	path := filepath.Join(outputDir, ResultFile)
	return wrench.SaveFormattedFile(result, path, wrench.EnvelopeSchema, wrench.YAML, wrench.LocalFile)
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
