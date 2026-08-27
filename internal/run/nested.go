package run

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/scriptedworld/bolt/internal/definitions"
	"github.com/scriptedworld/bolt/internal/jig"
	"github.com/scriptedworld/bolt/internal/paths"
)

// ChildDirName is where a nested run writes inside its task's work directory,
// when the task does not rename it. `output-dir` names the directory rather
// than placing it, so this is the default name and never the default location.
const ChildDirName = "run"

// runNestedJig runs a jig in place of a command.
//
// A nested run is not a mode. Inside its subdirectory it is identical to the
// same jig run on that directory from the command line, so this prepares the
// invocation and calls Execute, which is the one code path both callers reach.
func runNestedJig(task jig.Task, options Options, base, configDir, outputDir string) (int, error) {
	childBase, err := childBase(task, options, base, configDir, outputDir)
	if err != nil {
		return 0, err
	}

	// A subdirectory that is not there is treated as one holding nothing, and a
	// base holding no input paths does not run. A shared jig naming
	// subprojects a repository may not have is ordinary rather than
	// exceptional, so refusing would make it unusable wherever it did not fit.
	if info, statErr := os.Stat(childBase); statErr != nil || !info.IsDir() {
		return 0, nil
	}
	found, err := paths.Walk(childBase, nil)
	if err != nil {
		return 0, fmt.Errorf("task %q: walking %s: %w", task.Name, childBase, err)
	}
	if len(found) == 0 {
		return 0, nil
	}

	// A jig task runs once against its base, so the ordinal is always the
	// first and the width is one.
	workDir := filepath.Join(outputDir, WorkSubdir, executionDir(task.Name, 0, 1))
	if err := os.MkdirAll(workDir, 0o755); err != nil {
		return 0, err
	}

	child, err := childOptions(task, options, base, configDir, outputDir, childBase, workDir)
	if err != nil {
		return 0, fmt.Errorf("task %q: %w", task.Name, err)
	}

	if options.Progress != nil {
		fmt.Fprintf(options.Progress, "%s\n", filepath.Base(workDir))
	}

	if err := writeJigManifest(workDir, task, child, base); err != nil {
		return 0, err
	}

	// The child follows its own process: its own requires, its own tasks, its
	// own filtering. Nothing rolls up and no parent reads a child's content.
	if err := runChild(child); err != nil {
		return 0, err
	}
	return 1, linkChildResult(workDir, child.OutputDir)
}

// childBase resolves the subdirectory `in` names against the current base.
//
// It is the only thing that sets a child's base, which is what makes
// containment a property rather than a habit.
func childBase(task jig.Task, options Options, base, configDir, outputDir string) (string, error) {
	if task.In == "" {
		return base, nil
	}

	resolved, err := substituteField(task.In, options, base, configDir, outputDir, "")
	if err != nil {
		return "", err
	}
	// A substituted location is already absolute, so the relative-path rule
	// never applies to it. Naming a location and writing a relative path are
	// two ways of saying where, and they do not compose into a third.
	if filepath.IsAbs(resolved) {
		return resolved, nil
	}
	return filepath.Join(base, resolved), nil
}

// childOptions is the invocation the jig task makes. What it does not declare
// is inherited, so a nested jig runs with its parent's settings until a field
// says otherwise.
func childOptions(task jig.Task, options Options, base, configDir, outputDir, childBase, workDir string) (Options, error) {
	childConfig := configDir
	if task.ConfigDir != "" {
		resolved, err := substituteField(task.ConfigDir, options, base, configDir, outputDir, workDir)
		if err != nil {
			return Options{}, err
		}
		if !filepath.IsAbs(resolved) {
			resolved = filepath.Join(base, resolved)
		}
		childConfig = resolved
	}

	// output-dir names the child's directory rather than placing it. Whatever
	// it is set to, the result is a subdirectory of this task's work directory,
	// so renaming is expressible and relocating is not sayable.
	name := ChildDirName
	if task.OutputDir != "" {
		resolved, err := substituteField(task.OutputDir, options, base, configDir, outputDir, workDir)
		if err != nil {
			return Options{}, err
		}
		name = filepath.Base(filepath.Clean(resolved))
	}

	defined := options.Definitions
	if task.Definitions != "" {
		loaded, err := definitions.Load(childConfig, task.Definitions)
		if err != nil {
			return Options{}, err
		}
		defined = loaded
	}

	loaded, err := jig.Load(childConfig, task.Jig)
	if err != nil {
		return Options{}, err
	}

	// A jig that genuinely needs the repository root says so, and its commands
	// stand there. It is a property of the tool the jig runs, so only the jig
	// can know it, which is why this is read after loading rather than decided
	// by the task.
	//
	// It reaches where a command stands and nothing else. Overriding the base
	// would let a child widen past the grant its caller wrote, with nothing in
	// the parent's jig recording it, and FR-5.13 makes narrowing the base and
	// narrowing the containment check one act.
	standIn := ""
	if loaded.NeedsRepositoryRoot {
		standIn = options.projectRoot(base)
	}

	return Options{
		Jig:         loaded,
		BaseDir:     childBase,
		StandDir:    standIn,
		ConfigDir:   childConfig,
		OutputDir:   filepath.Join(workDir, name),
		Definitions: defined,
		ProjectRoot: options.projectRoot(base),
		Depth:       options.Depth + 1,
		Ceiling:     options.Ceiling,
		Fold:        options.Fold,
		Now:         options.Now,
		Progress:    options.Progress,
	}, nil
}

// substituteField puts the locations into a field's value, as they go into a
// command. The work directory is only named where there is one.
func substituteField(value string, options Options, base, configDir, outputDir, workDir string) (string, error) {
	locations := Locations{
		ProjectRoot: options.projectRoot(base),
		BaseDir:     base,
		WorkDir:     workDir,
		ConfigDir:   configDir,
		OutputDir:   outputDir,
	}
	mapping, err := definitions.Resolve(locations.values(), options.Jig.Definitions, options.Definitions)
	if err != nil {
		return "", err
	}

	// Unquoted, because a field's value is a path bolt uses rather than a word
	// a shell parses. Quoting would put the quotes into the directory name.
	out := value
	for name, resolved := range mapping {
		out = replaceAll(out, "{"+name+"}", resolved.Value)
	}
	return out, nil
}

// runChild carries out the nested invocation and folds its result, which is
// what makes the child's `result.yaml` exist for the parent to link.
//
// Its failure is an ordinary constituent failure rather than the parent's
// error: the merge does not know a constituent was a nested run.
func runChild(child Options) error {
	outcome, err := Execute(child)
	if err != nil {
		// The child could not be carried out. It writes its own refusal where
		// its result belongs, so the parent's link resolves and the merge folds
		// a failure like any other. Only a bolt that dies leaves nothing.
		return Refuse(child.OutputDir, child.BaseDir, err)
	}

	if child.Fold == nil {
		return fmt.Errorf("no merge was supplied, so a nested run has no result to link")
	}
	// Not outermost: a nested fold leaves paths absolute, because a path means
	// the same thing to a child and to its parent and only the outermost
	// invocation rewrites them.
	_, err = child.Fold(outcome.OutputDir, child.BaseDir, false)
	return err
}

// writeJigManifest records what the nested invocation was going to be, before
// it runs. A jig task carries the same bookkeeping as a command task, so
// nothing reading work/*/ needs to know which kind it is looking at.
func writeJigManifest(workDir string, task jig.Task, child Options, base string) error {
	return saveManifest(workDir, map[string]any{
		"task":    task.Name,
		"ordinal": 0,
		"command": fmt.Sprintf("bolt %s %s", task.Jig, child.BaseDir),
		"variables": map[string]any{
			bare(jig.ProjectRoot): boltValue(child.ProjectRoot),
			bare(jig.BaseDir):     boltValue(child.BaseDir),
			bare(jig.WorkDir):     boltValue(workDir),
			bare(jig.ConfigDir):   boltValue(child.ConfigDir),
			bare(jig.OutputDir):   boltValue(child.OutputDir),
		},
	})
}

// replaceAll is strings.ReplaceAll, named here so the substitution in a field
// reads the same as the one in a command.
func replaceAll(value, from, to string) string {
	return strings.ReplaceAll(value, from, to)
}

// linkChildResult points the task's envelope at the child's result with a
// relative symlink, so the whole tree survives being moved or archived.
func linkChildResult(workDir, childOutput string) error {
	target, err := filepath.Rel(workDir, filepath.Join(childOutput, ResultFile))
	if err != nil {
		return err
	}

	link := filepath.Join(workDir, EnvelopeFile)
	if err := os.Remove(link); err != nil && !os.IsNotExist(err) {
		return err
	}
	return os.Symlink(target, link)
}
