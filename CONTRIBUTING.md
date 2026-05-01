# Contributing

## Branch naming

All feature branches must follow this convention:

```
<type>/<slug>
```

Where `<type>` is one of `feature`, `fix`, `chore`, `test`, `docs` and `<slug>` uses only lowercase letters, digits, and hyphens.

Examples:
- `feature/rusaa-344-pr-hygiene-scripts`
- `fix/kafka-header-propagation`
- `chore/update-dependencies`

## PR title format

Every PR title must start with a tracker-issue bracket prefix:

```
[REQ-XX-NN] <description>    # requirements-registry work
[RUSAA-NNN] <description>    # tooling / internal work
```

Use `REQ-XX-NN` when implementing a formal requirement from the requirements registry. Use `RUSAA-NNN` for tooling, infrastructure, or internal issues.

The `pr-hygiene` CI check enforces this on every PR — titles that don't match the pattern block the merge.

## Pre-push helpers

Two scripts live in `scripts/` to catch problems before you push.

### `scripts/review-ready.sh`

Validates your current branch name and prints a suggested PR title:

```bash
scripts/review-ready.sh
```

Run this before opening a PR. It exits non-zero when the branch name is invalid.

### `scripts/open-pr.sh`

Wrapper around `gh pr create` that validates the title before creating the PR:

```bash
scripts/open-pr.sh --title "[RUSAA-344] feat: pr hygiene scripts" \
  --body "$(cat pr-body.md)" --base main
```

If `--title` is omitted, the script tries to use the latest commit subject. If the commit subject already conforms to the required format it is used as-is; otherwise the script exits with instructions.

All other flags are forwarded to `gh pr create` unchanged.

## When CI hygiene rules change on `main`

The `pr-hygiene.yml` workflow runs the YAML **at your branch HEAD**, not at `main`. This means if the title regex or other hygiene rules change on `main` after your branch was created, your branch runs the old version of the check.

**To pick up updated rules, rebase your branch onto `main`:**

```bash
git fetch origin
git rebase origin/main
git push --force-with-lease
```

You will know a rule change landed on `main` (but not on your branch) when:
- The check passes locally but fails in CI on another engineer's fresh branch.
- The CI error references a pattern that differs from what you see in `.github/workflows/pr-hygiene.yml` on your branch.

When in doubt, rebase first.
