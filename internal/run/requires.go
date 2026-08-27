package run

import (
	"fmt"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/scriptedworld/bolt/internal/jig"
	"github.com/scriptedworld/wrench"
)

// checkRequires resolves every executable the jig declares, before any task
// executes.
//
// An incomplete toolchain is known in the first second rather than partway
// through a gate, which is the whole of what this buys. It names every missing
// entry rather than the first, so a caller fixing them does not pay a round
// trip per tool.
//
// It is a guarantee about `requires` and not about every way a process fails to
// launch. A command invoking something the jig never declared still fails its
// own task, by FR-4.10.
func checkRequires(declared *jig.Jig) error {
	var missing []string
	for _, entry := range declared.SortedRequires() {
		if _, err := exec.LookPath(entry); err != nil {
			missing = append(missing, entry)
		}
	}
	if len(missing) == 0 {
		return nil
	}

	return fmt.Errorf(
		"the jig requires %s, which %s not on PATH",
		strings.Join(missing, ", "), were(len(missing)),
	)
}

func were(count int) string {
	if count == 1 {
		return "is"
	}
	return "are"
}

// executionPassed reads the verdict back off the envelope the execution just
// wrote.
//
// The envelope is the authoritative result, so short-circuiting reads it rather
// than keeping a parallel record that could disagree with the evidence on disk.
func executionPassed(workDir string) (bool, error) {
	path := filepath.Join(workDir, EnvelopeFile)
	value, err := wrench.LoadFormattedFile(path, wrench.EnvelopeSchema, wrench.YAML, wrench.LocalFile)
	if err != nil {
		return false, err
	}
	envelope, ok := value.(map[string]any)
	if !ok {
		return false, fmt.Errorf("%s is %T, want a mapping", path, value)
	}
	success, _ := envelope["success"].(bool)
	return success, nil
}
