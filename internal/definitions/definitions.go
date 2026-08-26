// Package definitions resolves what a jig's placeholders stand for.
//
// Substitution resolves against one mapping, built in three layers, each
// winning over the one before it: bolt's own values, then the jig's
// definitions block, then the definitions file named on the invocation. Every
// key in the result is a template variable, so a value a jig defined and a
// location bolt exposed are written and read the same way.
package definitions

import (
	"fmt"
	"path/filepath"
	"sort"

	"github.com/scriptedworld/bolt/internal/jig"
	"github.com/scriptedworld/wrench"
)

// Layer says where a value came from, which is what the manifest records. The
// same key means different things depending on which layer won, and a command
// line alone does not say.
type Layer string

const (
	// FromBolt is a location or a path variable. Reserved, never overridden.
	FromBolt Layer = "bolt"
	// FromJig is the jig's own definitions block.
	FromJig Layer = "jig"
	// FromFile is the definitions file named on the invocation.
	FromFile Layer = "file"
)

// A Value is what a placeholder stands for and which layer supplied it.
type Value struct {
	Value string
	Layer Layer
}

// Mapping is the resolved set. A key is present or it is not, which is the
// distinction FR-4.18b rests on: a value holding the empty string is defined,
// and a key nothing holds is the case that refuses the run.
type Mapping map[string]Value

// Filename is the shape of a definitions file. It is named by its <name> and
// read from the config directory, as a jig is, so a shared one is adopted and
// linked exactly as a shared jig is.
func Filename(name string) string {
	return "bolt." + name + ".definitions.yaml"
}

// Load reads the definitions file called name from configDir.
//
// An empty name is no file, which is ordinary: a jig whose own block covers its
// placeholders runs without one.
func Load(configDir, name string) (map[string]string, error) {
	if name == "" {
		return nil, nil
	}

	path := filepath.Join(configDir, Filename(name))
	value, err := wrench.LoadFormattedFile(path, wrench.DefinitionsSchema, wrench.YAML, wrench.LocalFile)
	if err != nil {
		return nil, err
	}

	if _, ok := value.(map[string]any); !ok {
		return nil, fmt.Errorf("definitions %s: top level is %T, want a mapping", path, value)
	}
	return jig.Scalars(value), nil
}

// Resolve builds the mapping from the three layers.
//
// It refuses a jig or a file that defines a name bolt exposes, because
// {base_dir} redefined would substitute something other than where the command
// stands, and the jig would say one thing while the process did another.
func Resolve(bolt map[string]string, ownJig, file map[string]string) (Mapping, error) {
	if err := refuseReserved(ownJig, "the jig"); err != nil {
		return nil, err
	}
	if err := refuseReserved(file, "the definitions file"); err != nil {
		return nil, err
	}

	mapping := make(Mapping, len(bolt)+len(ownJig)+len(file))
	for key, value := range bolt {
		mapping[key] = Value{Value: value, Layer: FromBolt}
	}
	// Each layer adds the keys the layers below did not have, replaces the
	// values of those they did, and leaves every key it does not name standing.
	for key, value := range ownJig {
		mapping[key] = Value{Value: value, Layer: FromJig}
	}
	for key, value := range file {
		mapping[key] = Value{Value: value, Layer: FromFile}
	}
	return mapping, nil
}

// Check holds the jig and the file to what has to be settled before anything
// executes: neither may shadow a name bolt supplies, and every placeholder the
// jig's commands name has to have a value somewhere.
//
// Bolt's own layer is left out because its keys are reserved and therefore
// always present, so what is missing here is missing whatever the run's
// locations turn out to be.
func Check(ownJig *jig.Jig, file map[string]string) error {
	mapping, err := Resolve(nil, ownJig.Definitions, file)
	if err != nil {
		return err
	}

	var commands []string
	for _, task := range ownJig.Tasks {
		commands = append(commands, task.Command, task.AdapterCommand)
	}
	if missing := mapping.Undefined(commands); len(missing) > 0 {
		return fmt.Errorf("nothing defines %v, which the jig's commands name", missing)
	}
	return nil
}

// refuseReserved refuses a layer that names one of bolt's own values.
//
// The path variables are reserved with the locations, though they are not in
// the mapping: they are per-execution and substituted where they are known, and
// a file defining `all_paths` would still be saying something bolt decides.
func refuseReserved(layer map[string]string, what string) error {
	var clashes []string
	for key := range layer {
		if jig.Reserved(key) {
			clashes = append(clashes, key)
		}
	}
	if len(clashes) == 0 {
		return nil
	}
	sort.Strings(clashes)
	return fmt.Errorf("%s defines %v, which bolt exposes and reserves", what, clashes)
}

// Undefined lists the placeholders the commands name that no layer supplies,
// sorted so a refusal names them the same way twice.
//
// Checked when `requires` is, before the first task executes, so a jig run
// where nothing defines what it needs refuses in the first second rather than
// partway through a gate.
func (m Mapping) Undefined(commands []string) []string {
	seen := map[string]bool{}
	var missing []string
	for _, command := range commands {
		for _, name := range jig.Placeholders(command) {
			// A path variable is bolt's and is substituted per execution, so it
			// is never in the mapping and is never missing from it.
			if _, defined := m[name]; defined || seen[name] || jig.Reserved(name) {
				continue
			}
			seen[name] = true
			missing = append(missing, name)
		}
	}
	sort.Strings(missing)
	return missing
}

// Record is the mapping as the manifest holds it: every key the layers hold,
// with the value that won and the layer it came from.
func (m Mapping) Record() map[string]any {
	out := make(map[string]any, len(m))
	for key, value := range m {
		out[key] = map[string]any{"value": value.Value, "from": string(value.Layer)}
	}
	return out
}
