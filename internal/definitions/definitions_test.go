package definitions_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/scriptedworld/bolt/internal/definitions"
	"github.com/scriptedworld/bolt/internal/jig"
)

// resolve is the three layers with the errors checked, for the tests that are
// about what the mapping holds rather than about what it refuses.
func resolve(t *testing.T, bolt, ownJig, file map[string]string) definitions.Mapping {
	t.Helper()
	mapping, err := definitions.Resolve(bolt, ownJig, file)
	if err != nil {
		t.Fatalf("resolving: %v", err)
	}
	return mapping
}

// COVERS: FR-4.16, FR-4.17 | positive
func TestALayerAddsKeysAndReplacesValuesAndLeavesTheRestStanding(t *testing.T) {
	// default < jig-file < definitions-file. Each layer adds the keys the ones
	// below did not have, replaces the values of those they did, and leaves
	// every key it does not name alone.
	mapping := resolve(t,
		map[string]string{"base_dir": "/base"},
		map[string]string{"requirements": "REQUIREMENTS.md", "line_length": "88"},
		map[string]string{"line_length": "100", "strict": "true"},
	)

	want := map[string]string{
		"base_dir":     "/base",           // untouched by either layer above
		"requirements": "REQUIREMENTS.md", // the jig's, which the file did not name
		"line_length":  "100",             // the file replaced the jig's 88
		"strict":       "true",            // added by the file
	}
	for name, expected := range want {
		if got := mapping[name].Value; got != expected {
			t.Errorf("%s resolved to %q, want %q", name, got, expected)
		}
	}
	if len(mapping) != len(want) {
		t.Errorf("the mapping holds %d keys, want %d: %v", len(mapping), len(want), mapping)
	}
}

// COVERS: FR-9.5g | positive
func TestTheRecordSaysWhichLayerEachValueCameFrom(t *testing.T) {
	// The same key means different things depending on which layer won, and a
	// command line alone does not say.
	mapping := resolve(t,
		map[string]string{"base_dir": "/base"},
		map[string]string{"line_length": "88", "requirements": "R.md"},
		map[string]string{"line_length": "100"},
	)

	record := mapping.Record()
	for name, layer := range map[string]string{
		"base_dir":     "bolt",
		"requirements": "jig",
		"line_length":  "file",
	} {
		entry, ok := record[name].(map[string]any)
		if !ok {
			t.Errorf("%s is not recorded", name)
			continue
		}
		if entry["from"] != layer {
			t.Errorf("%s says it came from %v, want %s", name, entry["from"], layer)
		}
	}
}

// COVERS: FR-4.19, FR-4.16d | negative
func TestNeitherLayerMayDefineANameBoltSupplies(t *testing.T) {
	// A redefined {base_dir} would substitute something other than where the
	// command stands, so the jig would say one thing and the process do another.
	//
	// Refusing here is what makes bolt's layer the exception to the precedence
	// ordering: nothing above it can win, so the rule that the later layer wins
	// only ever settles a key the jig and the file both set.
	for what, layers := range map[string][2]map[string]string{
		"the jig redefining a location":       {{"base_dir": "/elsewhere"}, nil},
		"the file redefining a location":      {nil, {"work_dir": "/elsewhere"}},
		"the file redefining a path variable": {nil, {"all_paths": "everything"}},
		"the jig redefining a path variable":  {{"each_path": "one"}, nil},
	} {
		_, err := definitions.Resolve(map[string]string{"base_dir": "/base"}, layers[0], layers[1])
		if err == nil {
			t.Errorf("%s was accepted", what)
			continue
		}
		if !strings.Contains(err.Error(), "reserves") {
			t.Errorf("%s was refused without saying the name is reserved: %v", what, err)
		}
	}
}

// COVERS: FR-4.18, FR-4.18a | negative
func TestAPlaceholderNoLayerSuppliesIsNamedBeforeAnythingRuns(t *testing.T) {
	mapping := resolve(t, nil, map[string]string{"requirements": "R.md"}, nil)

	missing := mapping.Undefined([]string{
		"check --requirements {requirements} --line-length {line_length} {targets}",
	})
	if len(missing) != 2 || missing[0] != "line_length" || missing[1] != "targets" {
		t.Errorf("undefined placeholders are %v, want [line_length targets] sorted", missing)
	}
}

// COVERS: FR-4.18 | edge
func TestAPathVariableIsNeverMissingFromTheMapping(t *testing.T) {
	// It is bolt's and it is substituted per execution, so it is not in the
	// mapping and reporting it as undefined would refuse every ordinary jig.
	mapping := resolve(t, nil, nil, nil)

	if missing := mapping.Undefined([]string{"fmt {each_path}", "vet {all_paths} {base_dir}"}); missing != nil {
		t.Errorf("bolt's own variables were reported undefined: %v", missing)
	}
}

// COVERS: FR-4.18b | edge
func TestADefinitionHoldingAnEmptyValueIsDefined(t *testing.T) {
	// Not found and found empty are different states. A jig wanting a flag to
	// carry nothing defines it empty rather than leaving it out.
	mapping := resolve(t, nil, nil, map[string]string{"extra_flags": ""})

	if missing := mapping.Undefined([]string{"check {extra_flags}"}); missing != nil {
		t.Errorf("an empty definition was reported undefined: %v", missing)
	}
	if got, ok := mapping["extra_flags"]; !ok || got.Value != "" {
		t.Errorf("the empty value did not survive: %#v", got)
	}
}

// COVERS: FR-4.16a | positive
func TestADefinitionsFileIsNamedAndFoundLikeAJig(t *testing.T) {
	if got := definitions.Filename("python-override"); got != "bolt.python-override.definitions.yaml" {
		t.Errorf("the filename is %q", got)
	}

	configDir := t.TempDir()
	write(t, configDir, definitions.Filename("q"), "requirements: ../REQUIREMENTS.md\nline_length: 100\nstrict: true\n")

	loaded, err := definitions.Load(configDir, "q")
	if err != nil {
		t.Fatalf("loading: %v", err)
	}
	// A number and a boolean reach a command line as the text they were
	// written as, because a command line carries text.
	for name, want := range map[string]string{
		"requirements": "../REQUIREMENTS.md",
		"line_length":  "100",
		"strict":       "true",
	} {
		if loaded[name] != want {
			t.Errorf("%s loaded as %q, want %q", name, loaded[name], want)
		}
	}
}

// COVERS: FR-4.16b | edge
func TestNamingNoDefinitionsFileIsOrdinary(t *testing.T) {
	// A jig whose own block covers its placeholders runs without one.
	loaded, err := definitions.Load(t.TempDir(), "")
	if err != nil {
		t.Fatalf("naming no file was an error: %v", err)
	}
	if loaded != nil {
		t.Errorf("naming no file produced %v", loaded)
	}
}

// COVERS: FR-4.20 | negative
func TestADefinitionsFileThatWillNotValidateIsNotTakenForAnAbsentOne(t *testing.T) {
	configDir := t.TempDir()
	write(t, configDir, definitions.Filename("nested"), "python:\n  line_length: 100\n")

	if _, err := definitions.Load(configDir, "nested"); err == nil {
		t.Error("a nested definitions file was accepted")
	}

	if _, err := definitions.Load(configDir, "absent"); err == nil {
		t.Error("a file that is not there was accepted, so a missing one reads as empty")
	}
}

// COVERS: FR-4.16 | property
func TestEveryReservedNameIsOneACommandCanWrite(t *testing.T) {
	// The reserved set and the variables bolt substitutes are the same set. A
	// variable added to one and not the other is either unreserved or unusable.
	for _, variable := range append(append([]string{}, jig.Locations...), jig.EachPath, jig.AllPaths) {
		names := jig.Placeholders(variable)
		if len(names) != 1 {
			t.Errorf("%s is not written the way a command writes a placeholder", variable)
			continue
		}
		if !jig.Reserved(names[0]) {
			t.Errorf("%s is substituted by bolt and not reserved against being redefined", variable)
		}
	}
}

func write(t *testing.T, dir, name, contents string) {
	t.Helper()
	if err := os.WriteFile(filepath.Join(dir, name), []byte(contents), 0o644); err != nil {
		t.Fatalf("writing %s: %v", name, err)
	}
}
