package run

import (
	"strings"

	"github.com/scriptedworld/bolt/internal/definitions"
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

// values maps each location's bare name to what it stands for, which is bolt's
// layer of the mapping substitution resolves against. A variable added later is
// carried because it is a variable, not because somebody remembered to add it.
func (l Locations) values() map[string]string {
	return map[string]string{
		bare(jig.ProjectRoot): l.ProjectRoot,
		bare(jig.BaseDir):     l.BaseDir,
		bare(jig.WorkDir):     l.WorkDir,
		bare(jig.ConfigDir):   l.ConfigDir,
		bare(jig.OutputDir):   l.OutputDir,
	}
}

// bare strips the braces a variable is written with, because a mapping is keyed
// by the name and a command writes it as {name}.
func bare(variable string) string {
	return variable[1 : len(variable)-1]
}

// substitute puts the resolved mapping and the paths into a command line.
//
// Every value is quoted individually, so one carrying a space, a quote or a
// semicolon can neither split the command line nor inject into it. Quoting the
// whole substitution instead would let a selection of two files become one
// argument.
//
// A defined value is quoted like a location, which is what makes it one
// argument. A definition meaning several arguments is not expressible, and that
// is the same trade FR-4.16c already made by holding a value to a scalar.
func substitute(command string, mapping definitions.Mapping, each string, all []string) string {
	replacements := make([]string, 0, 2*len(mapping)+4)
	for name, value := range mapping {
		replacements = append(replacements, "{"+name+"}", shellQuote(value.Value))
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
