# almanac

Agent memory for Claude Code that gets better with use. Memory lives in an Obsidian vault as one note per fact. Every note starts as a lead, and a note is loaded into every session only after something has checked it against reality.

Claude Code's own memory is a folder of notes plus a hand-written `MEMORY.md` index. Only the first 200 lines or 25KB of that index load at session start, and the rest is dropped silently. almanac keeps that contract but generates `MEMORY.md` from the notes whose `stage` is `rule`, so the index holds only what has earned its place.

## Layers

Each layer works without the next one.

| Layer | What it does | Parts |
|---|---|---|
| Substrate | Notes with `type`, `stage`, `status`, `verified_on` frontmatter, at the top level or under `metadata:` as Claude Code writes them. `Memory.base` shows the Rules, Needs verification and Stale views in Obsidian. | `almanac mem`, `templates/Memory.base` |
| Discipline | The SessionStart hook rebuilds `MEMORY.md`. The Stop hook asks for a note when a session changed files and wrote none. It blocks at most twice per session. | `hook session-start`, `hook stop` |
| Decisions | A PreToolUse gate on shell commands: hard rules in code first, then an optional classifier. The same classifier can decide the Stop hook and triage loop items. | `hook gate`, `triage`, `shadow report` |
| Verification | A workflow that checks each queued note against the live repo, with separate agents for extracting and checking, then merges duplicates and promotes general notes. | `workflows/memory-sweep.js`, `sweep` skill |

## Install

```bash
claude plugin marketplace add kepano/obsidian-skills
claude plugin marketplace add BonnardValentin/almanac
claude plugin install almanac@almanac
```

The install asks for:

- `vault`: the Obsidian vault folder. Each project gets a subfolder named after its repository.
- `gate_mode`: `off`, `shadow` (the default) or `enforce`.
- `typesafe_api_key`: optional, see below.

Then, in each project, start a session and run the `setup` skill. The notes stay in Claude Code's own memory folder for the repository, which all its worktrees share, so Claude Code writes them without a permission prompt. The vault gets a symlink to that folder, named after the repository. Setup changes no Claude Code setting, and marks existing notes `stage: investigate`.

## The gate

Hard rules in [`src/gate.rs`](src/gate.rs) always apply, unless `gate_mode` is `off`. They deny force pushes and piping into a shell, and they ask before recursive deletes, `git reset --hard`, `git clean -f` and pushes. A rule is a regex, not a model, because a command can be written to mislead a model.

[`test/fixtures/gate.jsonl`](test/fixtures/gate.jsonl) is the public eval for the rules. When a command gets the wrong decision, add it there.

## The classifier (optional)

With a TypeSafe key, a small calibrated model answers the yes/no questions: what class a command is, whether the session's lessons are in memory, and whether a loop item needs work. Thresholds live in [`src/decide.rs`](src/decide.rs).

**Privacy:** with a key set, command text and the end of the session transcript are sent to TypeSafe.

v0.1 has no live client yet. Without a key, the gate uses the hard rules only, the Stop hook blocks and lets the agent decide, and triage passes every item. To run the classifier paths offline, set `ALMANAC_JEV_REPLAY` to a JSON file of answers, as described in [`src/jev.rs`](src/jev.rs).

Leave `gate_mode` at `shadow` for a week. Then run `almanac shadow report` to see, for each decision, how often the command still ran. Switch to `enforce` only for the bands that matched what you did.

## Sweep

Ask Claude to sweep memory, or schedule it weekly. The `sweep` skill runs `almanac mem queue --json`, then the `memory-sweep` workflow, then `almanac mem apply`, which records each verdict with a dated reason.

## Triage

```bash
gh issue list --json number,title,body | jq -c '.[] | {id: .number, text: (.title + "\n" + .body)}' | almanac triage
```

This prints the ids that need an agent turn. Start `claude -p` only for those, from cron or `/schedule`.

## Development

```bash
cargo test
ALMANAC_BIN=$PWD/target/debug/almanac claude --plugin-dir plugins/almanac
claude plugin validate ./
```

`ALMANAC_BIN` makes the shim run a local build instead of downloading a release.

To release, push a tag `vX.Y.Z`. CI builds macOS and Linux binaries, publishes them, then commits the new version and `checksums.txt` to main. The shim runs a downloaded binary only when its SHA-256 matches that file.

## License

MIT
