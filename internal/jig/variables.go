package jig

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
