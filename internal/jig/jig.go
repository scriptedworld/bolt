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
	// Evidence is what the task declares it produces, never discovered.
	Evidence []string
	// ShortCircuit stops the run when this task fails.
	ShortCircuit bool
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
		Name:     name,
		Path:     path,
		Requires: strings_(document["requires"]),
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
		Name:         text(mapping["name"]),
		Description:  text(mapping["description"]),
		Command:      text(mapping["command"]),
		Matching:     strings_(mapping["matching"]),
		Excluding:    strings_(mapping["excluding"]),
		Adapter:      text(mapping["adapter"]),
		Evidence:     strings_(mapping["evidence"]),
		ShortCircuit: short,
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
	each := strings.Contains(t.Command, EachPath)
	all := strings.Contains(t.Command, AllPaths)

	if each && all {
		return fmt.Errorf("names both %s and %s, so how it runs cannot be read off it", EachPath, AllPaths)
	}
	if !each && !all && (len(t.Matching) > 0 || len(t.Excluding) > 0) {
		return fmt.Errorf("declares matching or excluding but names neither %s nor %s, so the selection would be built and discarded", EachPath, AllPaths)
	}
	return nil
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
