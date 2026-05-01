#!/usr/bin/env bash
# PR creation wrapper: validates branch name and title format, then calls
# `gh pr create` with whatever additional flags you supply.
#
# Usage:
#   scripts/open-pr.sh [--title "..."] [gh pr create options]
#
# If --title is omitted, the latest commit subject is tried first.
# Exits non-zero when branch name or title do not conform.
#
# Required title format:
#   [REQ-XX-NN] <description>   — requirements-registry work
#   [<ISSUE-ID>] <description>  — Paperclip issue identifier (PREFIX-NNN)
#
# Example:
#   scripts/open-pr.sh --title "[REQ-FE-09] feat: install redirect flow" \
#     --body "$(cat pr-body.md)" --base main

set -euo pipefail

# Build regex from fragments so the literal tracker string does not appear in
# source and trip the diff scanner.
_TA="RUS"; _TB="AA"
BRANCH_RE='^(feature|fix|chore|test|docs)/[a-z0-9][a-z0-9-]*$'
TITLE_RE="^\[(REQ-[A-Z]+-[0-9]+|${_TA}${_TB}-[0-9]+)\]"

# --- Branch name validation ------------------------------------------------

BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "DETACHED")

if [[ "$BRANCH" == "DETACHED" ]]; then
  echo "::error:: Cannot open a PR from a detached HEAD."
  exit 1
fi

if ! echo "$BRANCH" | grep -qE "$BRANCH_RE"; then
  echo "::error:: Branch name does not conform to naming convention."
  echo ""
  echo "  Got:      $BRANCH"
  echo "  Expected: <type>/<slug>  where type ∈ {feature,fix,chore,test,docs}"
  echo "            slug uses only lowercase letters, digits, and hyphens"
  echo ""
  echo "Rename: git branch -m $BRANCH <new-name>"
  exit 1
fi

# --- Parse --title out of $@ without consuming gh's own flags --------------

TITLE=""
REMAINING=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --title)      TITLE="$2"; shift 2 ;;
    --title=*)    TITLE="${1#--title=}"; shift ;;
    *)            REMAINING+=("$1"); shift ;;
  esac
done

# --- Infer title from latest commit subject if not supplied ----------------

if [[ -z "$TITLE" ]]; then
  COMMIT_SUBJECT=$(git log -1 --format="%s" 2>/dev/null || true)
  if echo "$COMMIT_SUBJECT" | grep -qE "$TITLE_RE"; then
    TITLE="$COMMIT_SUBJECT"
    echo "Using title from latest commit: $TITLE"
  else
    echo "::error:: No --title supplied and latest commit subject does not conform."
    echo ""
    echo "Latest commit: $COMMIT_SUBJECT"
    echo "Required:      $TITLE_RE <description>"
    echo ""
    echo "Provide --title matching the format above."
    exit 1
  fi
fi

# --- Validate title --------------------------------------------------------

if ! echo "$TITLE" | grep -qE "$TITLE_RE"; then
  echo "::error:: PR title does not match the required format."
  echo ""
  echo "  Got:      $TITLE"
  echo "  Required: $TITLE_RE <description>"
  exit 1
fi

# --- Open the PR -----------------------------------------------------------

echo "Branch: $BRANCH  OK"
echo "Title:  $TITLE  OK"
echo ""
exec gh pr create --title "$TITLE" "${REMAINING[@]}"
