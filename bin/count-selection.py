"""Count tasks, and how many let bolt select their files, per jig.

The first attempt counted LINES matching a pattern, so a task carrying
matching, excluding and {each_path} counted three times. This counts tasks.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path.home() / ".projects"
TASK = re.compile(r"^\s*-\s+name:")
SELECTS = re.compile(r"\{each_path\}|\{all_paths\}")
MATCHING = re.compile(r"^\s*matching:")

total_tasks = 0
total_selecting = 0
rows = []

for jig in sorted(ROOT.glob("*/bolt.*.yaml")):
    if ".bolt-" in str(jig):
        continue
    text = jig.read_text(errors="replace")
    # Split into task blocks on the "- name:" boundary.
    lines = text.splitlines()
    starts = [i for i, line in enumerate(lines) if TASK.match(line)]
    tasks = 0
    selecting = 0
    for index, start in enumerate(starts):
        end = starts[index + 1] if index + 1 < len(starts) else len(lines)
        block = "\n".join(lines[start:end])
        tasks += 1
        if SELECTS.search(block) or MATCHING.search(block):
            selecting += 1
    total_tasks += tasks
    total_selecting += selecting
    if tasks:
        rows.append((str(jig.relative_to(ROOT)), tasks, selecting))

for name, tasks, selecting in rows:
    if selecting:
        print(f"  {name:<46} tasks={tasks:<3} selecting={selecting}")

jigs = len([r for r in rows if r[1]])
print()
print(f"jig files with tasks: {jigs}")
print(f"tasks total:          {total_tasks}")
print(f"tasks bolt selects:   {total_selecting}")
print(f"jigs that select:     {len([r for r in rows if r[2]])}", file=sys.stdout)
