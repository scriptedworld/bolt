package run

import (
	"strings"

	"github.com/scriptedworld/bolt/internal/jig"
)

// Locations are the five directories every task can name, whatever its command
// says.
type Locations struct {
	ProjectRoot string
	BaseDir     string
	WorkDir     string
	ConfigDir   string
	OutputDir   string
}

// values maps each variable to what it stands for, which is also what the
// manifest records. A variable added later is recorded because it is a
// variable, not because somebody remembered to add it.
func (l Locations) values() map[string]string {
	return map[string]string{
		jig.ProjectRoot: l.ProjectRoot,
		jig.BaseDir:     l.BaseDir,
		jig.WorkDir:     l.WorkDir,
		jig.ConfigDir:   l.ConfigDir,
		jig.OutputDir:   l.OutputDir,
	}
}

// substitute puts the locations and the paths into a command line.
//
// Every path is quoted individually, so one carrying a space, a quote or a
// semicolon can neither split the command line nor inject into it. Quoting the
// whole substitution instead would let a selection of two files become one
// argument.
func substitute(command string, locations Locations, each string, all []string) string {
	replacements := make([]string, 0, 14)
	for variable, value := range locations.values() {
		replacements = append(replacements, variable, shellQuote(value))
	}

	if strings.Contains(command, jig.EachPath) {
		replacements = append(replacements, jig.EachPath, shellQuote(each))
	}
	if strings.Contains(command, jig.AllPaths) {
		quoted := make([]string, 0, len(all))
		for _, path := range all {
			quoted = append(quoted, shellQuote(path))
		}
		replacements = append(replacements, jig.AllPaths, strings.Join(quoted, " "))
	}

	return strings.NewReplacer(replacements...).Replace(command)
}

// shellQuote wraps a value in single quotes, which is the one quoting a POSIX
// shell does not interpret at all. An embedded single quote ends the string,
// escapes itself outside it, and starts a new one.
func shellQuote(value string) string {
	return "'" + strings.ReplaceAll(value, "'", `'\''`) + "'"
}
