---
name: sweep
description: Check the memory notes waiting at stage investigate against the live repository, then mark each one verified, rule or superseded with a dated reason. Use when the user asks to sweep, verify or clean up memory, or on a weekly schedule.
---

# Sweep the memory queue

1. Run `almanac mem queue --json`. If the list is empty, say so and stop.
2. Run the `almanac:memory-sweep` workflow with these args:
   - `repo`: the repository's absolute path
   - `notes`: the list from step 1
3. The workflow returns `{ verdicts: [{path, verdict, reason}] }`. Write that result as it is to a file in your scratchpad and run `almanac mem apply < that-file`. `apply` edits only the frontmatter, and only of notes inside the memory folder. It then rebuilds `MEMORY.md`.
4. Report the count for each verdict. List the superseded notes, with their reasons, so the user can disagree.
