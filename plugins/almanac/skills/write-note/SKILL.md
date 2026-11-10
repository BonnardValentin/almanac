---
name: write-note
description: Write or update one almanac memory note - one fact with frontmatter the vault's Base can filter on. Use whenever you save something to memory, and when the Stop hook asks whether this session left a lesson.
---

# Write a memory note

Memory is a folder of notes in an Obsidian vault. `MEMORY.md` in that folder is generated from the notes whose `stage` is `rule`. Never edit `MEMORY.md`: the next index run overwrites it.

## Before writing

- Look for a note that already covers the fact, with the Glob and Read tools on the memory folder. Bash may not be allowed there. Update that note instead of adding a second one.
- Do not save what the repository already records: code structure, past fixes, git history.
- One note holds one fact.

## Schema

Claude Code keeps a memory note's fields under `metadata:`, and almanac reads them there or at the top level. Write new notes this way:

```markdown
---
name: short-kebab-case-slug
description: one-line summary, used to decide relevance
metadata:
  type: project | feedback | user | reference
  stage: investigate
  status: active
  source: pr-123 | conversation | docs/some-file.md
  tags: [area, subsystem]
---

The fact.

**Why:** the reason or the incident behind it.
**How to apply:** what a future session does differently because of it.

Related: [[other-note]]
```

- `type`: `project` for ongoing work and constraints, `feedback` for how the agent should work, `user` for who the person is, `reference` for pointers to external material.
- `stage`: always `investigate` when you write a note. A note moves to `verified` or `rule` only when the sweep checks it, or when the user says so. Do not promote your own notes.
- `status`: `superseded` when a newer note replaces this one. Say which note in the body.
- File name: the `name` slug plus `.md`.
- A `[[wikilink]]` to a note that does not exist yet is fine. It marks something worth writing later.
