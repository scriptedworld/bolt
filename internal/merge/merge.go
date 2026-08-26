// Package merge folds a finished run's evidence into one result.
package merge

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"

	"github.com/scriptedworld/bolt/internal/run"
	"github.com/scriptedworld/wrench"
)

// Fold reads every work/*/output.yaml under outputDir and writes one
// result.yaml at the top of it.
//
// It is mechanical and repeatable over a finished directory: fixing an adapter
// and folding again costs no re-execution, because every input is already on
// disk.
//
// base is the directory the run was pointed at. It goes into the result,
// because it is the first thing anybody reading one asks and the per-execution
// manifests answer it only for somebody already inside the run directory.
func Fold(outputDir, base string, outermost bool) (bool, error) {
	workRoot := filepath.Join(outputDir, run.WorkSubdir)
	entries, err := os.ReadDir(workRoot)
	if err != nil {
		return false, fmt.Errorf("reading %s: %w", workRoot, err)
	}

	var reasons []any
	evidence := map[string]any{}
	constituents := 0
	passed := true

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		workDir := filepath.Join(workRoot, entry.Name())

		envelope, err := readEnvelope(workDir)
		if err != nil {
			return false, err
		}
		if envelope == nil {
			// No output.yaml means no authoritative result was reached. In the
			// skeleton every execution writes one, so this is a bug rather
			// than a state, and saying so beats folding silently around it.
			return false, fmt.Errorf("%s holds no %s", workDir, run.EnvelopeFile)
		}
		constituents++

		if success, _ := envelope["success"].(bool); !success {
			passed = false
		}
		if carried, ok := envelope["reasons"].([]any); ok {
			reasons = append(reasons, carried...)
		}

		task, args, err := readManifest(workDir)
		if err != nil {
			return false, err
		}
		record := map[string]any{
			"args":   args,
			"result": reference(filepath.Join(workDir, run.EnvelopeFile), base, outermost),
		}
		appendEvidence(evidence, task, record)
	}

	// A merge finding no constituent fails. Every constituent passing holds
	// when there are none, and a green result is read as checked and fine,
	// which over zero checks it is not.
	if constituents == 0 {
		passed = false
		reasons = append(reasons, map[string]any{
			"kind":    "nothing-ran",
			"message": "no task produced a result, so nothing was checked",
		})
	}

	result := map[string]any{
		"success": passed,
		"metadata": map[string]any{
			"base":     base,
			"evidence": evidence,
		},
	}
	if len(reasons) > 0 {
		result["reasons"] = reasons
	}

	path := filepath.Join(outputDir, run.ResultFile)
	if err := wrench.SaveFormattedFile(result, path, wrench.EnvelopeSchema, wrench.YAML, wrench.LocalFile); err != nil {
		return false, err
	}
	return passed, nil
}

func readEnvelope(workDir string) (map[string]any, error) {
	path := filepath.Join(workDir, run.EnvelopeFile)
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return nil, nil
	}

	value, err := wrench.LoadFormattedFile(path, wrench.EnvelopeSchema, wrench.YAML, wrench.LocalFile)
	if err != nil {
		return nil, err
	}
	envelope, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("%s is %T, want a mapping", path, value)
	}
	return envelope, nil
}

// readManifest takes the task name and the args from the manifest rather than
// from the envelope, so an adapter never has to know what task it was run for.
// The work directory name carries the task too, and the manifest is the record
// that says so without parsing a filename.
func readManifest(workDir string) (task string, args string, err error) {
	path := filepath.Join(workDir, run.ManifestFile)
	value, err := wrench.LoadFormattedFile(path, wrench.ManifestSchema, wrench.YAML, wrench.LocalFile)
	if err != nil {
		return "", "", err
	}

	manifest, ok := value.(map[string]any)
	if !ok {
		return "", "", fmt.Errorf("%s is %T, want a mapping", path, value)
	}
	task, _ = manifest["task"].(string)
	args, _ = manifest["command"].(string)
	return task, args, nil
}

func appendEvidence(evidence map[string]any, task string, record map[string]any) {
	existing, _ := evidence[task].([]any)
	evidence[task] = append(existing, record)
}

// reference is how a path goes into a result.
//
// Only the outermost invocation relativises. Paths are absolute at every depth
// so a nested run's evidence folds into its parent with nothing rewritten, and
// a path means the same thing to a child and to its parent. Rewriting at each
// level would rewrite the same path twice.
//
// It reaches the structured path references and stops there. Text a tool
// emitted, carried up inside a reason, stays as the tool wrote it.
func reference(path, base string, outermost bool) string {
	if !outermost {
		return path
	}
	relative, err := filepath.Rel(base, path)
	if err != nil {
		return path
	}
	return filepath.ToSlash(relative)
}

// Tasks lists the task names the evidence mapping holds, sorted, which is what
// a caller prints when it wants to say what ran.
func Tasks(evidence map[string]any) []string {
	names := make([]string, 0, len(evidence))
	for name := range evidence {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}
