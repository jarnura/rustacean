# Contributing

## Before pushing any branch

Run `make review-ready` from the repo root before opening a PR. It runs `cargo fmt --check`, `cargo clippy`, the full workspace test suite, `cargo deny check`, and an OpenAPI freshness check — and also runs `pnpm lint`, `pnpm typecheck`, and `pnpm test` automatically if your branch touches `frontend/`. All steps execute even if earlier ones fail so you see the complete picture in one pass. Fix everything flagged before pushing; PRs that fail these checks are returned without review.

## Regenerating generated artefacts

The repository tracks two generated files. Regenerate them whenever you change the API surface:

### OpenAPI spec (`openapi.json`)

```bash
cargo run -p control-api -- print-openapi > openapi.json
```

### Frontend TypeScript schema (`frontend/src/api/generated/schema.ts`)

```bash
cd frontend && npm run gen:api
```

Both commands must be re-run (in that order) whenever control-api handler signatures change. The CI `codegen-drift` job enforces this by running `git diff --exit-code` over both files on every PR.

## Branch naming

Branch names must match `^(feature|fix|chore|test|docs)/[a-z0-9][a-z0-9-]*$`. `make review-ready` enforces this and prints a rename hint on failure.

## PR title format

Every PR title must start with a tracker-issue bracket prefix:

- `[REQ-XX-NN] <description>` — for requirements-registry work (e.g. `[REQ-FE-09]`)
- `[<ISSUE-ID>] <description>` — for tooling / internal work (use the Paperclip issue identifier, e.g. `PREFIX-NNN`)

The `pr-hygiene` CI check enforces this on every PR. `make review-ready` prints the required regex and derives a suggested title from the branch slug so you can copy-paste it.

## Opening a PR

Use `scripts/open-pr.sh` instead of calling `gh pr create` directly. It validates branch name and title before creating the PR and falls back to the latest commit subject when `--title` is omitted:

```bash
scripts/open-pr.sh --title "[REQ-FE-09] feat: install redirect flow" \
  --body "$(cat pr-body.md)" --base main
```

All extra flags are forwarded to `gh pr create` unchanged.

## When CI hygiene rules change on `main`

The `pr-hygiene.yml` workflow runs **at your branch HEAD**, not at `main`. When the title regex or other rules change on `main` after your branch was cut, your branch runs the old version of the check.

**To pick up updated rules, rebase your branch:**

```bash
git fetch origin
git rebase origin/main
git push --force-with-lease
```

This is the deliberate policy — rebase to ship. You will know a rule change landed on `main` (but not on your branch) when the CI error references a pattern that differs from what you see in `.github/workflows/pr-hygiene.yml` on your branch.

## Getting started

See `docs/getting-started.md` for environment setup, local stack, and migration steps.
