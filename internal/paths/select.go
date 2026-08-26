package paths

import (
	"fmt"

	"github.com/bmatcuk/doublestar/v4"
)

// Select narrows what the walk found to what one task acts on.
//
// matching is a condition on the task: a list of patterns or literal paths,
// where ** matches zero or more directory levels. excluding is its
// counterpart, removing from what matching selected, so a task wanting
// everything but one shape of file says so directly instead of writing a
// pattern that means "not that".
//
// Patterns are relative to the base of the run they are declared in, which is
// what makes a jig written for reuse the same jig at the repository root and at
// backend/.
//
// No matching at all selects everything the walk found. A task that consumes
// paths and says nothing about which has asked for all of them.
func Select(found, matching, excluding []string) ([]string, error) {
	selected, err := keep(found, matching)
	if err != nil {
		return nil, err
	}

	removed, err := keep(selected, excluding)
	if err != nil {
		return nil, err
	}

	if len(excluding) == 0 {
		return selected, nil
	}

	drop := make(map[string]bool, len(removed))
	for _, path := range removed {
		drop[path] = true
	}

	out := make([]string, 0, len(selected))
	for _, path := range selected {
		if !drop[path] {
			out = append(out, path)
		}
	}
	return out, nil
}

// keep returns the paths any pattern matches. With no patterns it returns
// everything, which is what makes Select's two calls compose.
func keep(paths, patterns []string) ([]string, error) {
	if len(patterns) == 0 {
		return paths, nil
	}

	out := make([]string, 0, len(paths))
	for _, path := range paths {
		matched, err := matchesAny(path, patterns)
		if err != nil {
			return nil, err
		}
		if matched {
			out = append(out, path)
		}
	}
	return out, nil
}

func matchesAny(path string, patterns []string) (bool, error) {
	for _, pattern := range patterns {
		// A literal path is a pattern with nothing special in it, so naming a
		// single known-bad file outright needs no separate mechanism.
		matched, err := doublestar.Match(pattern, path)
		if err != nil {
			return false, fmt.Errorf("pattern %q: %w", pattern, err)
		}
		if matched {
			return true, nil
		}
	}
	return false, nil
}
