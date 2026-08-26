package run

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

// WorkSubdir holds one directory per task execution.
const WorkSubdir = "work"

// ResultFile is the run's one result, at the top of the output directory.
const ResultFile = "result.yaml"

// EnvelopeFile is what an adapter writes into an execution's work directory.
// The name never varies, so no flag says where the envelope goes.
const EnvelopeFile = "output.yaml"

// The bookkeeping files an execution carries whatever kind of task it was.
const (
	ManifestFile = "manifest.yaml"
	StdoutFile   = "stdout"
	StderrFile   = "stderr"
	ExitCodeFile = "exitcode"
)

// DefaultOutputPrefix begins the name of a directory a run creates for itself
// when none was named.
const DefaultOutputPrefix = ".bolt-"

// DefaultOutputName is `.bolt-<iso8601>`, filesystem-safe.
//
// The strict form carries colons, which are legal on this filesystem and
// hostile to a Windows checkout, so a colon becomes a hyphen and the local
// offset is spelled the same way. Local rather than UTC: a run directory is
// read by whoever made it, on the day they made it.
func DefaultOutputName(at time.Time) string {
	strict := at.Format("2006-01-02T15:04:05-07:00")
	return DefaultOutputPrefix + strings.ReplaceAll(strict, ":", "-")
}

// executionDir is the directory one execution of a task writes into.
//
// The task's name prefixes it, so a task's evidence is identifiable on disk
// without opening anything. The ordinal is the execution's index within the
// task, zero-padded to the width that task's execution count needs, so a
// listing sorts into execution order rather than into 1, 10, 2.
func executionDir(task string, ordinal, total int) string {
	width := len(strconv.Itoa(max(total-1, 0)))
	return fmt.Sprintf("%s-%0*d", task, width, ordinal)
}

// prepareOutput makes the output directory, refusing one that already holds a
// run rather than interleaving two runs' evidence.
func prepareOutput(dir string) error {
	entries, err := os.ReadDir(dir)
	switch {
	case os.IsNotExist(err):
		// Created with its parents. A graph node's .ephemera/ may not exist
		// yet, and making the caller create it first buys nothing.
		return os.MkdirAll(filepath.Join(dir, WorkSubdir), 0o755)
	case err != nil:
		return err
	}

	for _, entry := range entries {
		if entry.Name() == WorkSubdir || entry.Name() == ResultFile {
			return fmt.Errorf("%s already holds a run; removing it is yours to decide", dir)
		}
	}
	return os.MkdirAll(filepath.Join(dir, WorkSubdir), 0o755)
}
