// Package cli turns a command line into a run.
package cli

import (
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/scriptedworld/bolt/internal/definitions"
	"github.com/scriptedworld/bolt/internal/jig"
	"github.com/scriptedworld/bolt/internal/merge"
	"github.com/scriptedworld/bolt/internal/run"
)

// Exit statuses. Bolt's status answers one question: could bolt carry out the
// run it was asked for. Whether the tools were happy is the envelope's to say,
// and a caller reading this to learn that has read the wrong thing.
const (
	// Ran means the run completed, whatever the tools concluded.
	Ran = 0
	// Refused means bolt could not carry the run out.
	Refused = 1
)

const usage = `bolt runs a jig over a directory and records what happened.

    bolt <jig> <directory>

A jig is named by its <name>, and read from bolt.<name>.yaml in the config
directory. The directory is the run's base: patterns resolve against it and
commands stand in it.

    --config-dir   where bolt.<name>.yaml is found (default: the directory)
    --output-dir   where evidence is written (default: .bolt-<iso8601> at the base)
    --definitions  a <name> read from the config directory as
                   bolt.<name>.definitions.yaml, supplying values for the jig's
                   placeholders. It merges over the jig's own definitions block,
                   which merges over the locations bolt exposes.

Exits 0 when the run completed, whatever the tools concluded, and 1 when bolt
could not carry it out. The verdict is success in result.yaml.
`

// Main is the whole of bolt, reachable by an ordinary test.
func Main(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("bolt", flag.ContinueOnError)
	flags.SetOutput(stderr)
	flags.Usage = func() { fmt.Fprint(stderr, usage) }

	configDir := flags.String("config-dir", "", "where bolt.<name>.yaml is found")
	outputDir := flags.String("output-dir", "", "where evidence is written")
	definitionsName := flags.String("definitions", "", "a <name> read as bolt.<name>.definitions.yaml")

	if err := flags.Parse(args); err != nil {
		return Refused
	}

	rest := flags.Args()
	if len(rest) == 1 && rest[0] == "help" {
		fmt.Fprint(stdout, usage)
		return Ran
	}
	if len(rest) != 2 {
		fmt.Fprint(stderr, usage)
		return Refused
	}

	return execute(request{
		jigName:     rest[0],
		baseDir:     rest[1],
		configDir:   *configDir,
		outputDir:   *outputDir,
		definitions: *definitionsName,
		now:         time.Now(),
	}, stdout, stderr)
}

type request struct {
	jigName     string
	baseDir     string
	configDir   string
	definitions string
	outputDir   string
	now         time.Time
}

func execute(req request, stdout, stderr io.Writer) int {
	if req.configDir == "" {
		req.configDir = req.baseDir
	}

	base, err := filepath.Abs(req.baseDir)
	if err != nil {
		fmt.Fprintf(stderr, "bolt: %v\n", err)
		return Refused
	}

	output := req.outputDir
	if output == "" {
		output = filepath.Join(base, run.DefaultOutputName(req.now))
	}
	output, err = filepath.Abs(output)
	if err != nil {
		fmt.Fprintf(stderr, "bolt: %v\n", err)
		return Refused
	}

	// A run refuses to start if the directory it was given is not there.
	// Checked before anything is created, because preparing the output
	// directory would otherwise bring the base into being as a side effect and
	// the run would go on to check an empty tree.
	if info, statErr := os.Stat(base); statErr != nil || !info.IsDir() {
		return refuseInto(output, base, fmt.Errorf("%s is not a directory to run over", base), stderr)
	}

	loaded, err := jig.Load(req.configDir, req.jigName)
	if err != nil {
		return refuseInto(output, base, err, stderr)
	}

	// Read where the jig was, and before the run, so a file that will not parse
	// is a refusal with nothing created rather than a run that got partway.
	defined, err := definitions.Load(req.configDir, req.definitions)
	if err != nil {
		return refuseInto(output, base, err, stderr)
	}

	outcome, err := run.Execute(run.Options{
		Jig:         loaded,
		BaseDir:     base,
		ConfigDir:   req.configDir,
		OutputDir:   output,
		Definitions: defined,
		Now:         req.now,
		Fold:        merge.Fold,
		Progress:    stdout,
	})
	if err != nil {
		return refuseInto(output, base, err, stderr)
	}

	passed, err := merge.Fold(outcome.OutputDir, base, true)
	if err != nil {
		return refuseInto(output, base, err, stderr)
	}

	report(stdout, outcome, passed)
	return Ran
}

// refuseInto writes the refusal where a caller will find it and says so on
// stderr. Every refusal takes one shape, so a caller parses one thing whatever
// went wrong, and only a bolt that dies leaves nothing behind.
func refuseInto(output, base string, cause error, stderr io.Writer) int {
	fmt.Fprintf(stderr, "bolt: %v\n", cause)

	// The one case with nowhere to write. Creating the output directory would
	// create the base along with it, and the base not being there is what is
	// being refused, so writing the result would undo the refusal. A caller
	// that named --output-dir outside the base gets one either way.
	if within(output, base) {
		if _, err := os.Stat(base); err != nil {
			fmt.Fprintf(stderr, "bolt: no result written: %s would have created %s\n", output, base)
			return Refused
		}
	}

	if err := run.Refuse(output, base, cause); err != nil {
		fmt.Fprintf(stderr, "bolt: could not write the refusal: %v\n", err)
	}
	return Refused
}

// within says whether a path sits inside a directory, which decides whether
// creating it would bring that directory into being.
func within(path, dir string) bool {
	relative, err := filepath.Rel(dir, path)
	if err != nil {
		return false
	}
	return relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator))
}

func report(stdout io.Writer, outcome *run.Outcome, passed bool) {
	verdict := "failed"
	if passed {
		verdict = "passed"
	}

	fmt.Fprintf(stdout, "\n%s: %d execution(s)", verdict, outcome.Executions)
	if len(outcome.Skipped) > 0 {
		fmt.Fprintf(stdout, ", %d task(s) skipped for an empty selection: %v", len(outcome.Skipped), outcome.Skipped)
	}
	// What a short-circuiting failure kept from running, so a reader sees what
	// was not attempted rather than inferring it from what is absent.
	if len(outcome.Stopped) > 0 {
		fmt.Fprintf(stdout, ", %d task(s) not reached: %v", len(outcome.Stopped), outcome.Stopped)
	}
	fmt.Fprintf(stdout, "\n%s\n", filepath.Join(outcome.OutputDir, run.ResultFile))
}
