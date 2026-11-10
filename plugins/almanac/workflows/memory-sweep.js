export const meta = {
  name: 'memory-sweep',
  description:
    'Check almanac memory notes against the live repo, then verify, promote or supersede each one',
  whenToUse:
    'Run by the almanac sweep skill, with args {repo, notes} from `almanac mem queue --json`.',
  phases: [
    {
      title: 'Check',
      detail:
        'one agent extracts the claims of a note, a separate one checks them',
    },
    {
      title: 'Distill',
      detail: 'merge duplicates and promote general notes to rule',
    },
  ],
};

const CLAIMS = {
  type: 'object',
  properties: {
    claims: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          claim: { type: 'string' },
          how_to_check: { type: 'string' },
        },
        required: ['claim', 'how_to_check'],
      },
    },
  },
  required: ['claims'],
};

const CHECKS = {
  type: 'object',
  properties: {
    results: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          claim: { type: 'string' },
          holds: { type: 'string', enum: ['yes', 'no', 'unknown'] },
          evidence: { type: 'string' },
        },
        required: ['claim', 'holds', 'evidence'],
      },
    },
  },
  required: ['results'],
};

const DISTILL = {
  type: 'object',
  properties: {
    promote: {
      type: 'array',
      items: {
        type: 'object',
        properties: { path: { type: 'string' }, reason: { type: 'string' } },
        required: ['path', 'reason'],
      },
    },
    supersede: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          path: { type: 'string' },
          kept: { type: 'string' },
          reason: { type: 'string' },
        },
        required: ['path', 'kept', 'reason'],
      },
    },
  },
  required: ['promote', 'supersede'],
};

const { repo, notes } = args ?? {};
if (!repo || !Array.isArray(notes) || notes.length === 0) {
  return { verdicts: [], skipped: 'args need a repo path and a non-empty notes list' }
}

phase('Check');
// The checker never sees the note, only the claims: an agent that sees the reasoning behind a
// claim tends to agree with it.
const checked = await pipeline(
  notes,
  (note) =>
    agent(
      `Read the memory note at ${note.path}. List every claim in it that can be checked against the ` +
        `repository at ${repo} or its tooling: a file path, a flag, an identifier, a command, a setting. ` +
        `Write each claim as one short sentence and say how to check it. Do not judge whether a claim is true. ` +
        `A [[wikilink]] is not a claim: links to notes that do not exist yet are allowed. ` +
        `Return an empty list when the note holds only preferences or judgment.`,
      {
        label: `extract:${note.name}`,
        phase: 'Check',
        schema: CLAIMS,
        model: 'haiku',
        effort: 'low',
      }
    ),
  (extracted, note) =>
    extracted.claims.length === 0
      ? { note, results: [] }
      : agent(
          `Check each claim against the live repository at ${repo} and its tooling. Read files and run ` +
            `read-only commands only. Change nothing. Answer yes, no or unknown for each claim, with the ` +
            `evidence you saw.\n\nClaims:\n${JSON.stringify(extracted.claims, null, 2)}`,
          { label: `check:${note.name}`, phase: 'Check', schema: CHECKS }
        ).then((r) => ({ note, results: r.results }))
);

const done = checked.filter(Boolean);
if (done.length < notes.length)
  log(
    `${notes.length - done.length} notes were not checked and keep their stage`
  );

function firstVerdict({ note, results }) {
  const broken = results.filter((r) => r.holds === 'no');
  if (broken.length > 0) {
    const what = broken.map((r) => `${r.claim} (${r.evidence})`).join('; ');
    return {
      path: note.path,
      verdict: 'superseded',
      reason: `claims no longer hold: ${what}`,
    };
  }
  if (results.length > 0 && results.every((r) => r.holds === 'yes')) {
    return {
      path: note.path,
      verdict: 'verified',
      reason: `${results.length} claims checked against the repo`,
    };
  }
  const why =
    results.length === 0
      ? 'no claims that the repo can check'
      : 'some claims could not be checked';
  return { path: note.path, verdict: 'investigate', reason: why };
}

const verdicts = new Map(done.map((c) => [c.note.path, firstVerdict(c)]));

// A preference the user stated cannot be checked against the repo, but the user is its source,
// so distill may promote it. Any other unchecked note stays a lead.
const candidates = done
  .filter((c) => {
    const v = verdicts.get(c.note.path).verdict;
    return (
      v === 'verified' ||
      (v === 'investigate' &&
        c.results.length === 0 &&
        ['feedback', 'user'].includes(c.note.type))
    );
  })
  .map((c) => c.note);

phase('Distill');
if (candidates.length > 0) {
  const result = await agent(
    `These memory notes survived a check against the repository at ${repo}. Read each one.\n\n` +
      `${candidates.map((n) => `- ${n.path}: ${n.description}`).join('\n')}\n\n` +
      `1. Find notes that state the same fact. For each duplicate, name the note to keep and supersede the others.\n` +
      `2. Promote a note to rule only when it applies beyond the one case it was written for, and a future ` +
      `session would act differently because of it. Leave the rest as they are.\n` +
      `Use only the paths listed above.`,
    { label: 'distill', phase: 'Distill', schema: DISTILL, effort: 'high' }
  );
  const allowed = new Set(candidates.map((n) => n.path));
  for (const p of result?.promote ?? []) {
    if (allowed.has(p.path))
      verdicts.set(p.path, { path: p.path, verdict: 'rule', reason: p.reason });
  }
  for (const s of result?.supersede ?? []) {
    if (allowed.has(s.path) && allowed.has(s.kept) && s.path !== s.kept) {
      verdicts.set(s.path, {
        path: s.path,
        verdict: 'superseded',
        reason: `duplicate of ${s.kept}: ${s.reason}`,
      });
    }
  }
}

return { verdicts: [...verdicts.values()] }
