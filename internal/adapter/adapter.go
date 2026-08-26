// Package adapter turns one task execution's captured output into a result
// envelope, by running the adapter the task named.
//
// An adapter is a separate process and it reaches the verdict. Bolt decides
// whether an execution passed in exactly three cases, each named where it
// arises: an execution bolt terminated, an adapter that reached no
// authoritative result, and a task that did not produce the evidence it
// declared.
package adapter

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// Envelope names the file an adapter writes. The path is the work directory it
// was given and the name never varies, so no flag says where the envelope goes
// and no task can put it somewhere else.
const Envelope = "output.yaml"

// ExitCode is the generic adapter every task gets when it names none. Its name
// is not a path: it is resolved before the config directory is consulted, so a
// project cannot shadow it by accident.
const ExitCode = "exit-code"

// A Result says what happened to the adapter itself, not what it concluded.
// What it concluded is in the envelope it wrote.
type Result struct {
	// Wrote is true when an envelope is on disk to be read.
	Wrote bool
	// Reason, when Wrote is false, says which of FR-6.11's three happened.
	Reason string
	// Kind pairs with Reason so a consumer tells them apart without reading
	// English.
	Kind string
}

// Locations are the three directories an adapter is handed, the same three
// every task gets.
type Locations struct {
	ProjectRoot string
	BaseDir     string
	WorkDir     string
}

// Resolve finds the executable for an adapter named by a task.
//
// It comes from the config directory, where jigs already come from, so a jig
// and the adapters it names travel together and `link-jigs` places both or
// neither.
func Resolve(configDir, name string) (string, error) {
	if name == "" || name == ExitCode {
		return "", nil
	}

	path := filepath.Join(configDir, name)
	info, err := os.Stat(path)
	if err != nil {
		return "", fmt.Errorf("adapter %q is not in %s: %w", name, configDir, err)
	}
	if info.IsDir() {
		return "", fmt.Errorf("adapter %q in %s is a directory", name, configDir)
	}
	if info.Mode()&0o111 == 0 {
		return "", fmt.Errorf("adapter %q in %s is not executable", name, configDir)
	}
	return path, nil
}

// DefaultArgs is the invocation bolt writes when a task does not write its own.
//
// It names the captured files and the three locations. Evidence is repeated
// once per declared file, because a task declares its evidence and those are
// what --evidence names; discovery would hand an adapter whatever a tool
// happened to leave behind.
func DefaultArgs(locations Locations, evidence []string) []string {
	args := []string{
		"--stdout", filepath.Join(locations.WorkDir, "stdout"),
		"--stderr", filepath.Join(locations.WorkDir, "stderr"),
		"--exitcode", filepath.Join(locations.WorkDir, "exitcode"),
		"--project-root", locations.ProjectRoot,
		"--base-dir", locations.BaseDir,
		"--work-dir", locations.WorkDir,
	}
	for _, path := range evidence {
		args = append(args, "--evidence", filepath.Join(locations.WorkDir, path))
	}
	return args
}

// MissingEvidence returns the declared evidence files the task did not
// produce, relative to the work directory.
//
// A task declaring evidence it did not write did not do what it said, and the
// refusal to discover means nothing else notices.
func MissingEvidence(workDir string, evidence []string) []string {
	var missing []string
	for _, path := range evidence {
		if _, err := os.Stat(filepath.Join(workDir, path)); err != nil {
			missing = append(missing, path)
		}
	}
	return missing
}

// Run invokes the adapter with the invocation bolt wrote.
func Run(executable string, args []string, locations Locations) (Result, error) {
	return finish(exec.Command(executable, args...), locations)
}

// RunShell invokes an explicit adapter invocation, written by the task in place
// of the default one. It is a shell line like a command, and the envelope is
// still expected at the same place, because no flag says where it goes.
func RunShell(command string, locations Locations) (Result, error) {
	return finish(exec.Command("sh", "-c", command), locations)
}

// finish runs a prepared adapter process and says whether an authoritative
// envelope resulted.
//
// It removes any envelope already in the work directory first, so "the adapter
// wrote one" cannot be satisfied by a leftover from a previous fold.
func finish(process *exec.Cmd, locations Locations) (Result, error) {
	envelope := filepath.Join(locations.WorkDir, Envelope)
	if err := os.Remove(envelope); err != nil && !errors.Is(err, os.ErrNotExist) {
		return Result{}, err
	}

	process.Dir = locations.BaseDir
	process.Stdin = nil

	// An adapter's own streams are captured beside the command's, so a broken
	// adapter can be read rather than guessed at.
	output, runErr := process.CombinedOutput()
	if writeErr := os.WriteFile(
		filepath.Join(locations.WorkDir, "adapter-output"), output, 0o644,
	); writeErr != nil {
		return Result{}, writeErr
	}

	if runErr != nil {
		var exit *exec.ExitError
		if !errors.As(runErr, &exit) {
			return Result{}, fmt.Errorf("running adapter %s: %w", process.Path, runErr)
		}
		return Result{
			Kind:   "adapter-failed",
			Reason: fmt.Sprintf("the adapter exited %d; its output is in adapter-output", exit.ExitCode()),
		}, nil
	}

	if _, err := os.Stat(envelope); err != nil {
		return Result{
			Kind:   "adapter-wrote-nothing",
			Reason: fmt.Sprintf("the adapter exited 0 and wrote no %s", Envelope),
		}, nil
	}

	return Result{Wrote: true}, nil
}
