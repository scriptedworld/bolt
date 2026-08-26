// Package jig loads and validates a jig.
//
// The file is read through wrench against the shipped jig schema, so a jig
// that is not well-formed fails here rather than halfway through a gate. What
// this package adds on top is the checks a schema cannot express, and turning
// a validated structure into something the rest of bolt can hold.
package jig

import (
	"fmt"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"github.com/scriptedworld/wrench"
)

// A Jig is the unit of configuration and composition. What bolt executes for a
// project is read from that project's jig.
type Jig struct {
	// Name is the <name> in bolt.<name>.yaml, which is how a jig is spoken of.
	Name string
	// Path is the file it was read from.
	Path string
	// Requires is every executable the jig invokes.
	Requires []string
	// Definitions gives the jig's own placeholders their defaults. Optional,
	// and so is any entry: a jig leaving a value to its adopter names the
	// placeholder in a command and defines nothing.
	Definitions map[string]string
	// Tasks execute in declaration order.
	Tasks []Task
}

// A Task is one thing the jig does.
type Task struct {
	Name        string
	Description string
	// Command is a shell line. How the task runs is read off it.
	Command string
	// Matching and Excluding select the paths a command task acts on.
	Matching  []string
	Excluding []string
	// Adapter names the adapter that reads what the command produced. Empty
	// means the generic exit-code adapter.
	Adapter string
	// AdapterCommand is an explicit invocation written in place of the default
	// one. The same substitutions are available as in a command.
	AdapterCommand string
	// Evidence is what the task declares it produces, never discovered.
	Evidence []string
	// ShortCircuit stops the run when this task fails.
	ShortCircuit bool

	// Jig names a jig to run in place of a command. A jig task declares what it
	// changes about the invocation it makes, as fields rather than as a command
	// line, and what it does not declare is inherited.
	Jig string
	// In is a subdirectory of the current base to run that jig in, and the only
	// thing that sets a child's base.
	In string
	// ConfigDir is where the child looks for jigs.
	ConfigDir string
	// OutputDir names the child's output directory rather than placing it.
	OutputDir string
	// Definitions names the child's definitions file, as --definitions does for
	// an invocation from the command line.
	Definitions string
}

// IsJig says whether the task runs a jig in place of a command. The schema
// refuses a task that names both.
func (t Task) IsJig() bool {
	return t.Jig != ""
}

// fields are a jig task's values, which take substitutions as a command does.
func (t Task) fields() map[string]string {
	return map[string]string{
		"in":          t.In,
		"config-dir":  t.ConfigDir,
		"output-dir":  t.OutputDir,
		"definitions": t.Definitions,
	}
}

// Filename is the shape of a jig file. A jig is named by its <name>, and the
// filename is derived from it rather than the other way round.
func Filename(name string) string {
	return "bolt." + name + ".yaml"
}

// Load reads the jig called name from configDir, validates it, and checks what
// the schema cannot.
func Load(configDir, name string) (*Jig, error) {
	path := filepath.Join(configDir, Filename(name))

	value, err := wrench.LoadFormattedFile(path, wrench.JigSchema, wrench.YAML, wrench.LocalFile)
	if err != nil {
		return nil, err
	}

	document, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("jig %s: top level is %T, want a mapping", path, value)
	}

	loaded := &Jig{
		Name:        name,
		Path:        path,
		Requires:    strings_(document["requires"]),
		Definitions: Scalars(document["definitions"]),
	}

	rawTasks, _ := document["tasks"].([]any)
	for i, item := range rawTasks {
		task, err := readTask(item)
		if err != nil {
			return nil, fmt.Errorf("jig %s: task %d: %w", path, i+1, err)
		}
		loaded.Tasks = append(loaded.Tasks, task)
	}

	if err := loaded.check(); err != nil {
		return nil, fmt.Errorf("jig %s: %w", path, err)
	}
	return loaded, nil
}

func readTask(item any) (Task, error) {
	mapping, ok := item.(map[string]any)
	if !ok {
		return Task{}, fmt.Errorf("is %T, want a mapping", item)
	}

	short, _ := mapping["short-circuit-failure"].(bool)
	return Task{
		Name:           text(mapping["name"]),
		Description:    text(mapping["description"]),
		Command:        text(mapping["command"]),
		Matching:       strings_(mapping["matching"]),
		Excluding:      strings_(mapping["excluding"]),
		Adapter:        text(mapping["adapter"]),
		AdapterCommand: text(mapping["adapter-command"]),
		Evidence:       strings_(mapping["evidence"]),
		ShortCircuit:   short,
		Jig:            text(mapping["jig"]),
		In:             text(mapping["in"]),
		ConfigDir:      text(mapping["config-dir"]),
		OutputDir:      text(mapping["output-dir"]),
		Definitions:    text(mapping["definitions"]),
	}, nil
}

// check holds the jig to the rules the schema cannot state.
func (j *Jig) check() error {
	seen := make(map[string]int, len(j.Tasks))
	for i, task := range j.Tasks {
		if first, duplicate := seen[task.Name]; duplicate {
			return fmt.Errorf("tasks %d and %d are both named %q, and a name prefixes its work directories", first+1, i+1, task.Name)
		}
		seen[task.Name] = i

		if err := task.check(); err != nil {
			return fmt.Errorf("task %q: %w", task.Name, err)
		}
	}
	return nil
}

func (t Task) check() error {
	if t.IsJig() {
		return t.checkJig()
	}

	each := strings.Contains(t.Command, EachPath)
	all := strings.Contains(t.Command, AllPaths)

	if each && all {
		return fmt.Errorf("names both %s and %s, so how it runs cannot be read off it", EachPath, AllPaths)
	}
	if !each && !all && (len(t.Matching) > 0 || len(t.Excluding) > 0) {
		return fmt.Errorf("declares matching or excluding but names neither %s nor %s, so the selection would be built and discarded", EachPath, AllPaths)
	}
	if t.AdapterCommand != "" && t.Adapter == "" {
		return fmt.Errorf("writes an adapter-command but names no adapter, so there is nothing to invoke")
	}
	return nil
}

// checkJig holds a jig task to what the schema cannot state.
//
// The location variables are what is available in a field. A jig task has no
// command consuming paths, so a path variable there has nothing to stand for,
// and substituting it would put the text into a directory name.
func (t Task) checkJig() error {
	for field, value := range t.fields() {
		for _, name := range Placeholders(value) {
			if name == bare(EachPath) || name == bare(AllPaths) {
				return fmt.Errorf("%s names {%s}, and a jig task has no command for it to stand for", field, name)
			}
		}
	}
	return nil
}

// bare strips the braces a variable is written with.
func bare(variable string) string {
	return variable[1 : len(variable)-1]
}

// ConsumesPaths says whether the task's command names a path variable, which
// is what decides both how it runs and whether an empty selection skips it.
func (t Task) ConsumesPaths() bool {
	return strings.Contains(t.Command, EachPath) || strings.Contains(t.Command, AllPaths)
}

// PerPath says whether the task runs once for each matched path.
func (t Task) PerPath() bool {
	return strings.Contains(t.Command, EachPath)
}

// Names lists the task names in declaration order.
func (j *Jig) Names() []string {
	names := make([]string, 0, len(j.Tasks))
	for _, task := range j.Tasks {
		names = append(names, task.Name)
	}
	return names
}

// SortedRequires is the dependency inventory, deduplicated and ordered so a
// refusal names what is missing the same way twice.
func (j *Jig) SortedRequires() []string {
	seen := make(map[string]bool, len(j.Requires))
	out := make([]string, 0, len(j.Requires))
	for _, entry := range j.Requires {
		if !seen[entry] {
			seen[entry] = true
			out = append(out, entry)
		}
	}
	sort.Strings(out)
	return out
}

// Scalars reads a definitions mapping out of a validated document.
//
// The schema has already held every value to a scalar, so a number or a boolean
// reaches a command line as the text it was written as. It lives here because a
// jig carries a block of the same shape as the file, and one reader serves both.
func Scalars(value any) map[string]string {
	document, ok := value.(map[string]any)
	if !ok {
		return nil
	}
	out := make(map[string]string, len(document))
	for key, item := range document {
		out[key] = scalar(item)
	}
	return out
}

func scalar(value any) string {
	switch typed := value.(type) {
	case string:
		return typed
	case bool:
		return strconv.FormatBool(typed)
	case int:
		return strconv.Itoa(typed)
	case int64:
		return strconv.FormatInt(typed, 10)
	case float64:
		return strconv.FormatFloat(typed, 'f', -1, 64)
	default:
		return fmt.Sprint(typed)
	}
}

func text(value any) string {
	s, _ := value.(string)
	return s
}

func strings_(value any) []string {
	items, ok := value.([]any)
	if !ok {
		return nil
	}
	out := make([]string, 0, len(items))
	for _, item := range items {
		if s, ok := item.(string); ok {
			out = append(out, s)
		}
	}
	return out
}
