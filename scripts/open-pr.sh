#!/usr/bin/env bash
# PR creation wrapper: enforces title format before calling `gh pr create`.
#
# Usage:
#   scripts/open-pr.sh [--title "..."] [gh pr create options]
#
# If --title is omitted, the latest commit subject is tried first.
# The script exits non-zero when the title does not match the required format.
#
# Required title format:
#   [REQ-XX-NN] <description>    — requirements-registry work
#   [RUSAA-NNN] <description>    — tooling / internal work
#
# Example:
#   scripts/open-pr.sh --title "[RUSAA-344] feat: pr hygiene scripts" \
#     --body "$(cat pr-body.md)" --base main

set -euo pipefail

TITLE_RE='^\[(REQ-[A-Z]+-[0-9]+|RUSAA-[0-9]+)\]'

# --- Parse our --title arg out of $@ without consuming gh's own flags ------

TITLE=""
REMAINING=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --title)
      TITLE="$2"
      shift 2
      ;;
    --title=*)
      TITLE="${1#--title=}"
      shift
      ;;
    *)
      REMAINING+=("$1")
      shift
      ;;
  esac
done

# --- Infer title from latest commit if not supplied -------------------------

if [[ -z "$TITLE" ]]; then
  COMMIT_SUBJECT=$(git log -1 --format="%s" 2>/dev/null || true)
  if echo "$COMMIT_SUBJECT" | grep -qE "$TITLE_RE"; then
    TITLE="$COMMIT_SUBJECT"
    echo "Using title from latest commit: $TITLE"
  else
    echo "::error:: No --title supplied and latest commit subject does not conform."
    echo ""
    echo "Latest commit: $COMMIT_SUBJECT"
    echo ""
    scripts/review-ready.sh 2>/dev/null || true
    echo "Provide --title matching: $TITLE_RE <description>"
    exit 1
  fi
fi

# --- Validate supplied or inferred title ------------------------------------

if ! echo "$TITLE" | grep -qE "$TITLE_RE"; then
  echo "::error:: PR title does not match the required format."
  echo ""
  echo "  Got:      $TITLE"
  echo "  Required: $TITLE_RE <description>"
  echo ""
  echo "  Examples:"
  echo "    [RUSAA-344] feat: pr hygiene scripts"
  echo "    [REQ-FE-09] feat: install redirect flow"
  echo ""
  scripts/review-ready.sh 2>/dev/null || true
  exit 1
fi

# Also run branch validation so the engineer catches branch issues early.
scripts/review-ready.sh 2>/dev/null || {
  echo "Fix the branch name issue above before opening the PR."
  exit 1
}

# --- Delegate to gh pr create ----------------------------------------------

echo "Title validated: $TITLE"
echo ""
exec gh pr create --title "$TITLE" "${REMAINING[@]}"
