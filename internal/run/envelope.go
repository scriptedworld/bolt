package run

import (
	"fmt"
	"path/filepath"

	"github.com/scriptedworld/bolt/internal/adapter"
	"github.com/scriptedworld/bolt/internal/jig"
	"github.com/scriptedworld/wrench"
)

// reachVerdict puts an envelope in the work directory, whatever happened.
//
// The adapter reaches the verdict. Bolt reaches it in three cases and no
// others: a task that did not produce the evidence it declared, an adapter that
// reached no authoritative result, and one whose envelope does not validate.
func reachVerdict(workDir string, task jig.Task, status int, executable string, where Locations, each string, selection []string) error {
	locations := adapter.Locations{
		ProjectRoot: where.ProjectRoot,
		BaseDir:     where.BaseDir,
		WorkDir:     where.WorkDir,
	}

	if executable == "" {
		return writeExitCodeEnvelope(workDir, task, status)
	}

	// Checked before the adapter runs. Handing it a path that is not there
	// asks it to explain an absence bolt already understands.
	if missing := adapter.MissingEvidence(workDir, task.Evidence); len(missing) > 0 {
		return boltEnvelope(workDir, "evidence-missing", fmt.Sprintf(
			"%s declared evidence it did not produce: %v", task.Name, missing,
		))
	}

	result, err := invokeAdapter(task, executable, locations, where, each, selection)
	if err != nil {
		return err
	}
	if !result.Wrote {
		return boltEnvelope(workDir, result.Kind, result.Reason)
	}

	// Present and failing validation is a failure, and a different condition
	// from absent. Canonical form is the adapter's own business: bolt checks
	// the structure against the schema and does not reparse to compare bytes.
	path := filepath.Join(workDir, adapter.Envelope)
	if _, err := wrench.LoadFormattedFile(path, wrench.EnvelopeSchema, wrench.YAML, wrench.LocalFile); err != nil {
		return boltEnvelope(workDir, "adapter-wrote-invalid", fmt.Sprintf(
			"the adapter wrote an %s that is not a valid envelope: %v", adapter.Envelope, err,
		))
	}

	return nil
}

// invokeAdapter runs the task's own invocation where it wrote one, and the
// default otherwise. An explicit invocation gets the same substitutions a
// command gets, and is still expected to leave the envelope where the default
// would.
func invokeAdapter(task jig.Task, executable string, locations adapter.Locations, where Locations, each string, selection []string) (adapter.Result, error) {
	if task.AdapterCommand == "" {
		return adapter.Run(executable, adapter.DefaultArgs(locations, task.Evidence), locations)
	}
	return adapter.RunShell(substitute(task.AdapterCommand, where, each, selection), locations)
}

// writeExitCodeEnvelope is the generic adapter a task gets when it names none.
// Every command has an exit status, so it is the one adapter that needs to know
// nothing about the tool it is reading.
func writeExitCodeEnvelope(workDir string, task jig.Task, status int) error {
	if status == 0 {
		return saveEnvelope(workDir, map[string]any{"success": true})
	}
	return boltEnvelope(workDir, "nonzero-exit", fmt.Sprintf("%s exited %d", task.Name, status))
}

func boltEnvelope(workDir, kind, message string) error {
	return saveEnvelope(workDir, map[string]any{
		"success": false,
		"reasons": []any{map[string]any{"kind": kind, "message": message}},
	})
}

func saveEnvelope(workDir string, envelope map[string]any) error {
	path := filepath.Join(workDir, adapter.Envelope)
	return wrench.SaveFormattedFile(envelope, path, wrench.EnvelopeSchema, wrench.YAML, wrench.LocalFile)
}
