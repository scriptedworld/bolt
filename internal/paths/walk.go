// Package paths finds the files a run may act on and narrows them per task.
package paths

import (
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"

	ignore "github.com/sabhiram/go-gitignore"
)

// gitDir is never walked. It is not source, and nothing in a jig can want it.
const gitDir = ".git"

// A Matcher decides whether a path is ignored. Reading `.gitignore` as text is
// the whole of it: bolt invokes no git, so a run over a tree that is not a
// repository behaves exactly as one over a repository.
//
// It is an interface because the pattern matching behind it is the one part of
// the walk borrowed rather than written, and swapping it should be one file.
type Matcher interface {
	Ignored(relative string, isDir bool) bool
}

// Walk returns every file under base that a task may act on, as paths relative
// to base, in sorted order.
//
// Sorted because FR-9.4's identical work directory names across two runs rest
// on the matched list being the same list, and nothing else gives that.
//
// exclude names directories the walk must not descend into whatever
// `.gitignore` says. A run's own output directory goes here, because a tree
// accumulating runs would otherwise feed each run the last one's evidence.
func Walk(base string, exclude []string) ([]string, error) {
	excluded := make(map[string]bool, len(exclude))
	for _, dir := range exclude {
		if dir != "" {
			excluded[filepath.Clean(dir)] = true
		}
	}

	stack := newIgnoreStack(base)
	var found []string

	err := filepath.WalkDir(base, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}

		relative, relErr := filepath.Rel(base, path)
		if relErr != nil {
			return relErr
		}
		if relative == "." {
			return nil
		}

		if entry.IsDir() {
			if entry.Name() == gitDir || excluded[relative] {
				return filepath.SkipDir
			}
			if stack.ignored(relative, true) {
				return filepath.SkipDir
			}
			stack.enter(relative)
			return nil
		}

		// A symlink is not followed. Following one leaves the base and breaks
		// containment, and shared jigs arrive as tracked symlinks, so this is
		// the ordinary case rather than a hostile one.
		if entry.Type()&os.ModeSymlink != 0 {
			return nil
		}
		if !entry.Type().IsRegular() {
			return nil
		}
		if stack.ignored(relative, false) {
			return nil
		}

		found = append(found, filepath.ToSlash(relative))
		return nil
	})
	if err != nil {
		return nil, err
	}

	sort.Strings(found)
	return found, nil
}

// ignoreStack applies each `.gitignore` to the subtree it sits in, which is
// what git does and what a single root-level read would get wrong for any
// project keeping one further down.
type ignoreStack struct {
	base  string
	rules map[string]*ignore.GitIgnore
}

func newIgnoreStack(base string) *ignoreStack {
	stack := &ignoreStack{base: base, rules: map[string]*ignore.GitIgnore{}}
	stack.enter(".")
	return stack
}

// enter reads the `.gitignore` in dir, if there is one, so it applies from
// here down.
func (s *ignoreStack) enter(dir string) {
	path := filepath.Join(s.base, dir, ".gitignore")
	compiled, err := ignore.CompileIgnoreFile(path)
	if err != nil {
		// No `.gitignore` here, or an unreadable one. Neither is a reason to
		// refuse a run: the tree may not be a repository at all.
		return
	}
	s.rules[filepath.Clean(dir)] = compiled
}

func (s *ignoreStack) ignored(relative string, isDir bool) bool {
	subject := filepath.ToSlash(relative)
	if isDir {
		subject += "/"
	}

	for dir, rules := range s.rules {
		within, ok := under(dir, relative)
		if !ok {
			continue
		}
		candidate := filepath.ToSlash(within)
		if isDir {
			candidate += "/"
		}
		if rules.MatchesPath(candidate) {
			return true
		}
	}
	return false
}

// under says whether relative sits inside dir, and what it is called from
// there, which is the form a `.gitignore` in dir matches against.
func under(dir, relative string) (string, bool) {
	if dir == "." {
		return relative, true
	}
	prefix := dir + string(filepath.Separator)
	if !strings.HasPrefix(relative, prefix) {
		return "", false
	}
	return strings.TrimPrefix(relative, prefix), true
}
