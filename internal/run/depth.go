package run

import (
	"fmt"
	"os"
	"strconv"
)

// The depth is carried in the environment of every process bolt spawns, so it
// survives reparenting, backgrounding, and a task command that invokes bolt
// directly rather than through a jig task.
const (
	// DepthVariable holds how many invocations deep the spawning bolt was.
	DepthVariable = "BOLT_DEPTH"
	// CeilingVariable holds the limit in force. Bolt exports the resolved
	// ceiling rather than reading a fresh one at each level, which is what
	// stops a jig raising the limit it is running under.
	CeilingVariable = "BOLT_MAX_DEPTH"
	// DefaultCeiling is a guard against accident and runaway.
	DefaultCeiling = 4
)

// resolveDepth says how deep this invocation is and what limit it runs under.
//
// The ceiling is read from the environment only where the depth variable is
// absent, which is the outermost invocation and the only one entitled to set
// it. Every level below takes the resolved value its parent exported.
func resolveDepth(options Options) (depth, ceiling int) {
	if options.Depth > 0 {
		// Nested in process. The parent passed both down.
		return options.Depth, options.Ceiling
	}

	if raw, set := os.LookupEnv(DepthVariable); set {
		// A process bolt spawned, which is not outermost however it was
		// reached. FR-5.6 contemplates a task command invoking bolt directly.
		parent, err := strconv.Atoi(raw)
		if err != nil {
			parent = 0
		}
		return parent + 1, ceilingFromEnvironment()
	}

	return 0, ceilingFromEnvironment()
}

func ceilingFromEnvironment() int {
	raw, set := os.LookupEnv(CeilingVariable)
	if !set {
		return DefaultCeiling
	}
	value, err := strconv.Atoi(raw)
	if err != nil || value < 1 {
		return DefaultCeiling
	}
	return value
}

// environment is what every process this run spawns inherits, carrying the
// depth and the ceiling so a bolt reached any way at all knows both.
func environment(depth, ceiling int) []string {
	out := make([]string, 0, len(os.Environ())+2)
	for _, entry := range os.Environ() {
		if hasPrefix(entry, DepthVariable+"=") || hasPrefix(entry, CeilingVariable+"=") {
			continue
		}
		out = append(out, entry)
	}
	return append(out,
		fmt.Sprintf("%s=%d", DepthVariable, depth),
		fmt.Sprintf("%s=%d", CeilingVariable, ceiling),
	)
}

func hasPrefix(value, prefix string) bool {
	return len(value) >= len(prefix) && value[:len(prefix)] == prefix
}

// tooDeep says whether this invocation has passed the ceiling.
//
// The guard is against accident and runaway, not against a jig trying to defeat
// it. FR-5.6 contemplates a task command invoking bolt directly, and such a
// command can unset the variable and be believed outermost. Closing that needs
// the ancestry cross-check, which is a question rather than a row.
func tooDeep(depth, ceiling int) error {
	if depth <= ceiling {
		return nil
	}
	return fmt.Errorf("nesting reached depth %d, past the ceiling of %d", depth, ceiling)
}
