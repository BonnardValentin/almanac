---
name: setup
description: Move a project's memory into the almanac vault. Use when the almanac SessionStart message says the project's memory is not in the vault yet, or when the user asks to set up almanac.
---

# Set up almanac for this project

1. Run `almanac setup --vault <vault>` from the project folder. Take `<vault>` from the almanac SessionStart message, or ask the user. It creates the project's memory folder and the vault's `Memory.base`, and prints JSON with:
   - `memory_dir`: the project's folder in the vault
   - `settings_file`: the project's `.claude/settings.local.json`
   - `configured`: whether Claude Code already uses `memory_dir`
   - `old_memory_dir`: Claude Code's previous memory folder for this project, if it has one

2. If `configured` is false, tell the user you will set `autoMemoryDirectory` to `memory_dir` in `settings_file`, and wait for a yes. Then merge the key with `jq`, keeping every other setting:

   ```bash
   f=<settings_file>; mkdir -p "$(dirname "$f")"; [ -f "$f" ] || echo '{}' > "$f"
   jq --arg d "<memory_dir>" '.autoMemoryDirectory = $d' "$f" > "$f.tmp" && mv "$f.tmp" "$f"
   ```

3. If `old_memory_dir` is set, migrate its notes:
   - Copy each `*.md` except `MEMORY.md` into `memory_dir`, keeping the file name.
   - Add `stage: investigate` and `status: active` to each copy, under its `metadata:` block if it has one. Keep `type` where it is. Nothing is verified until something has checked it. Do not promote any note to `rule`.
   - List the notes that only record history that git already holds, and propose deleting them. Delete only the ones the user confirms.
   - Leave `old_memory_dir` in place. The user removes it when they are satisfied.

4. Run `almanac mem index --dir <memory_dir>`. Tell the user to open the vault in Obsidian and look at `Memory.base`, and that the change takes effect in the next session.
