# COMPANY.md — Rust-brain-by-GOV Governance Rules

Shared rules for every agent. Do **not** duplicate sections from this file in per-agent
`agents/*/AGENTS.md` files — reference them instead with a one-liner like:
`> See [COMPANY.md § Section Name](../../COMPANY.md#section-anchor).`

---

## Cross-Cutting Rules

### Serve Mode Rules

You run headless in OpenCode serve mode. These tools will block forever — **never call them**:
- `submit_plan` — blocks waiting for human input
- `question` — blocks waiting for human input
- `ask_permission` — blocks waiting for human input
- `confirm` — blocks waiting for human input

If you feel the urge to ask a clarifying question, post it as a Paperclip comment instead and set
the issue to `blocked-on-board`. Then stop. Do not call blocking tools.

---

### GitHub Hygiene

Every PR MUST follow these rules. The `pr hygiene` CI job will reject violations.

1. **Title format**: `[REQ-XX-NN] type(scope): description` — bracket prefix FIRST.
   - Example: `[REQ-TN-03] feat: TenantPool with fully-qualified table names`
   - WRONG: `feat(infra): Docker service [REQ-DV-07]`

2. **GitHub issue required**: Create a GH issue before opening the PR. Include `Closes #XX` (or
   `Fixes #XX`) in the PR body. The linked issue must be self-contained — no Paperclip references.

3. **No internal tracker IDs**: Never put `RUSAA-XX` or `RUSTBRAINBYGOV-XX` in source files,
   comments, commit messages, PR titles, or PR bodies. CI scans diffs for these patterns.

4. **One REQ per PR**: Never combine multiple requirements in a single PR.

5. **Branch naming**: `feature/<desc>`, `fix/<desc>`, or `docs/<desc>`. Never
   `feature/RUSAA-XX-…` or `feature/RUSTBRAINBYGOV-XX-…`.

6. **Conventional commits only**: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`,
   `perf:`, `ci:`. Never `[RUSTBRAINBYGOV-XX] feat: …`.

7. **No Paperclip references on GitHub**: No `RUSTBRAINBYGOV-XX`, `RUSAA-XX`, or
   `localhost:3100` URLs in PR titles, bodies, or commits. Post technical content only on the
   GitHub side.

8. **Always open a PR**: A branch on `origin` without a PR is a hygiene violation. No orphaned
   branches.

9. **Squash-merge default**: `gh pr merge <num> --squash` ensures branch commit pollution never
   reaches `main`.

---

### PR Creation Protocol

```bash
git checkout main && git pull
git checkout -b feature/<short-description>
# ... implement and test ...
git push -u origin feature/<short-description>
gh pr create --repo jarnura/rustacean \
  --title "[REQ-XX-NN] type: description" \
  --body "$(cat <<'EOF'
## Summary
<what and why>

## GitHub Issue
Closes #XX

## Checklist
- [ ] All tests pass
- [ ] No internal Paperclip references
- [ ] Docs updated if architecture changed
EOF
)"
```

Never push to `main` directly. Never stack branches (unless the Sequential Phase Exception applies
— see `agents/eng-platform/AGENTS.md`). Jarnura is the sole merge authority.

---

### Hygiene-Close Protocol

When you find a PR with `RUSTBRAINBYGOV-XX` or `RUSAA-XX` in the title, body, or commit messages,
**never close it outright**. Past incident: PRs closed for hygiene violations left no replacements;
engine phases vanished from GitHub for days. This cannot happen again.

- **Bot-authored PR** (author = `rust-brain-by-gov-bot` or another agent): **rename in place** via
  `curl PATCH /repos/jarnura/rustacean/pulls/<num>` with a clean `title` and `body`. Do not use
  `gh pr edit` — it has failed on this repo due to a Projects-classic deprecation bug. Use the
  REST API directly. Squash-merge handles commit-message pollution at merge time.
- **Human-authored PR**: `gh pr review <num> --request-changes` with:
  > "This PR contains internal references that should not appear in GitHub. Please remove all
  > Paperclip references from the PR title, body, and commit messages."
- **Abandoned PR** (no commits in ≥14 days, no linked active Paperclip issue): may be closed, but
  **only** with a comment on the linked Paperclip issue explaining the closure.

If you were about to close a dirty PR, stop and rename it instead. Closing deletes work from the
team's view of the world; renaming preserves it.

---

### Wave Execution Order

The authoritative wave list lives in the relevant Paperclip issue (see CTO delegation comments for
the current wave). Only accept work for the current active wave. If assigned a higher-wave issue:

1. Comment: `"Wave guard: Wave N+1 issue. Escalating to CTO per COMPANY.md"`.
2. Set the issue to `blocked`, tag the CTO.
3. Do **not** self-promote backlog items.

If Wave Guard routines are enabled in your deployment, they enforce wave constraints every 15
minutes. If disabled, skip Wave Guard tick references in self-approval workflows.

---

### Memory

Your cross-session memory lives at `$AGENT_HOME` (set by Paperclip at runtime).

```bash
skill("para-memory-files")
```

Use the `para-memory-files` skill to store and retrieve facts, decisions, and patterns across
sessions. Write to memory after completing significant work. Read from memory at the start of each
run to restore context.

---

### Git Commit Attribution

All commits must use:
```
Co-Authored-By: Rust-brain-by-GOV Bot <bot@example.com>
```

Add this trailer to every commit message alongside the standard
`Rust-brain-by-GOV Bot <bot@example.com>` identity.

---

## Done-Gate Standard

Work is `done` only when its primary artifact is on `main` — not when submitted for review.
Role-specific criteria live in each agent's `AGENTS.md`; the universal rules:

- **No issue transitions to `done` until the merged-PR check passes** (`gh pr view --json mergedAt`
  shows non-null).
- Roles transition to `in_review` when work is submitted; the **CTO closes** the issue after
  artifact-on-main is confirmed.
- **Never self-transition to `done`** from `in_progress` — go to `in_review` and tag CTO.

Done-gate evidence block (required on every `in_review` or `done` transition):

```
Done-gate evidence:
- Type: <code|qa|docs|security>
- Artifact: <GitHub PR URL or Paperclip document URL>
- Verified by: <command or check used to confirm>
```

---

## Delegation & Approval Rules

### Gate 1 — Design Approval

**Jarnura-approval required** (post plan document, escalate to Jarnura, wait for explicit approval
comment):
- New service or binary
- New shared crate (`rb-*`)
- Cross-service boundary change
- Change to the `crates ← services` one-way dependency rule

**Architect self-approval allowed** (post plan document, wait one heartbeat for PR Reviewer to
flag, then proceed):
- New agent or `@tool` inside an existing service
- Refactor within a single service or crate
- Bug-fix redesign
- Small feature tweak on existing functionality

Never self-approve an ADR touching cross-service boundaries. When in doubt, escalate.

### Gate 2 — Technical Guardrails

The following require **board (Jarnura) approval** — do not merge without it:
- Destructive schema migration (`DROP`, `RENAME`, `ALTER TYPE`, `ALTER … DROP`)
- New dependency with a non-allowlisted license (allowlist: MIT, Apache 2.0, BSD-2-Clause,
  BSD-3-Clause, ISC)
- Core/crate code importing from services (dependency-flow violation)
- PRs labelled `security`

**Additive migrations** (new nullable columns, new indexes, new tables) need Architect + Platform
Engineer comment-approval on the Paperclip issue — no board escalation needed.

### Gate 3 — QA PASS + Reviewer Approval

1. QA report: (a) PASS verdict, (b) references current branch HEAD SHA, (c) under 24 hours old.
2. PR Reviewer posts review on GitHub (no Paperclip refs).
3. Merge: `gh pr merge $PR_NUM --squash`.
4. Close Paperclip issue with Done-gate evidence block.
