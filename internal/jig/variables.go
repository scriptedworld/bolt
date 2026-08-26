package jig

import "regexp"

// The template variables a command may name.
//
// Variables are underscored and command-line flags are hyphenated, as a rule
// rather than as an accident: {config_dir} and --config-dir name one thing in
// the two shapes their contexts use.
const (
	// ProjectRoot is where the project starts. The outermost run is assumed to
	// sit there; a nested one is not.
	ProjectRoot = "{project_root}"
	// BaseDir is what this run operates from, and where a command stands.
	BaseDir = "{base_dir}"
	// WorkDir is the execution's own directory, and the only place a command
	// can put an artifact that becomes evidence.
	WorkDir = "{work_dir}"
	// ConfigDir is where jigs and the adapters they name are found.
	ConfigDir = "{config_dir}"
	// OutputDir is the directory this run writes into.
	OutputDir = "{output_dir}"

	// EachPath means one execution per matched path.
	EachPath = "{each_path}"
	// AllPaths means one execution with the whole selection substituted.
	AllPaths = "{all_paths}"
)

// Locations lists the variables that name a directory, which are available to
// every task whatever its command says. All five rather than the three a task
// acts within, so there is no carve-out to remember.
var Locations = []string{ProjectRoot, BaseDir, WorkDir, ConfigDir, OutputDir}

// placeholder matches a template variable as a command writes it.
//
// The name shape is the one a definitions file's keys are held to, so what a
// command can name and what a file can define are the same set. It is also
// narrow enough that a shell's own brace expansion is not mistaken for one:
// {a,b} carries a comma and {} is empty, and neither matches.
var placeholder = regexp.MustCompile(`\{([a-z][a-z0-9_]*)\}`)

// reserved is every name bolt supplies. A jig or a definitions file defining
// one is refused, because a redefined {base_dir} would substitute something
// other than where the command stands.
var reserved = func() map[string]bool {
	names := map[string]bool{}
	for _, variable := range append(append([]string{}, Locations...), EachPath, AllPaths) {
		names[variable[1:len(variable)-1]] = true
	}
	return names
}()

// Placeholders lists the template variable names a command names, in first
// appearance order and without repeats.
func Placeholders(command string) []string {
	var names []string
	seen := map[string]bool{}
	for _, match := range placeholder.FindAllStringSubmatch(command, -1) {
		if name := match[1]; !seen[name] {
			seen[name] = true
			names = append(names, name)
		}
	}
	return names
}

// Reserved says whether a bare name is one bolt supplies.
func Reserved(name string) bool {
	return reserved[name]
}
