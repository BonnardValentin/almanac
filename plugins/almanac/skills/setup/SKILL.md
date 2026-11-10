---
name: setup
description: Link a project's memory into the almanac vault. Use when the almanac SessionStart message says the project's memory is not in the vault yet, or when the user asks to set up almanac.
---

# Set up almanac for this project

The notes stay in Claude Code's own memory folder for the repository, which every worktree of the repo shares. The vault gets a link to that folder, named after the repository. No Claude Code setting changes.

1. Run `almanac setup --vault <vault>` from the project folder. Take `<vault>` from the almanac SessionStart message, or ask the user. It links the memory folder into the vault, writes the vault's `Memory.base` if it is missing, and prints `memory_dir` and `vault_link`.

2. Read each note in `memory_dir` except `MEMORY.md`. Use the Glob and Read tools.
   - Add `stage: investigate` and `status: active` to each note that has no `stage`, under its `metadata:` block if it has one. Keep `type` where it is. Nothing is verified until something has checked it. Do not promote any note to `rule`.
   - List the notes that only record history that git already holds, and propose deleting them. Delete only the ones the user confirms.

3. Run `almanac mem index`. Tell the user:
   - Open the vault in Obsidian to see `Memory.base`.
   - From the next session, only notes at `stage: rule` load at session start. Run the `sweep` skill to check the others.
