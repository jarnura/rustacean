# Contributing

## First-time setup

After cloning, install the local git hooks once:

```bash
make install-hooks
```

This installs a `pre-push` hook that runs the bundle detector before every push. It is safe to re-run and will overwrite stale hooks.

## Before pushing any branch

Run `make review-ready` from the repo root before opening a PR. It runs `cargo fmt --check`, `cargo clippy`, the full workspace test suite, `cargo deny check`, and an OpenAPI freshness check — and also runs `pnpm lint`, `pnpm typecheck`, and `pnpm test` automatically if your branch touches `frontend/`. All steps execute even if earlier ones fail so you see the complete picture in one pass. Fix everything flagged before pushing; PRs that fail these checks are returned without review.

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

## One issue per PR (bundle policy)

Each PR must address exactly one tracker issue. The `pr-bundle-check` CI gate and the `pre-push` git hook both enforce this.

**What counts as a bundle violation:**
- A PR whose title names more than one issue.
- A PR whose body or commits mention an issue not declared in the title.
- A push whose commit messages reference more than one distinct issue.

**Waiver (board or CTO approval required):** if you have explicit approval to land multiple issues together, add this trailer to one of your commits before pushing:

```
bundle-waiver: board
```

or

```
bundle-waiver: cto
```

CI will verify the trailer value is an authorized role (`board` or `cto`). Without a valid waiver the push and the PR check both fail.

## Getting started

See `docs/getting-started.md` for environment setup, local stack, and migration steps.
