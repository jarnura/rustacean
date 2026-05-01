#!/usr/bin/env bash
# Pre-push validation: branch name conformance + PR title reminder.
#
# Usage:
#   scripts/review-ready.sh           # validates current branch
#   scripts/review-ready.sh <branch>  # validates named branch
#
# Exit codes:
#   0 — branch name is valid (title hint always printed as a reminder)
#   1 — branch name violates naming convention

set -euo pipefail

BRANCH="${1:-$(git rev-parse --abbrev-ref HEAD)}"

BRANCH_RE='^(feature|fix|chore|test|docs)/[a-z0-9][a-z0-9-]*$'
TITLE_RE='^\[(REQ-[A-Z]+-[0-9]+|RUSAA-[0-9]+)\]'

# --- Branch name validation -----------------------------------------------

if ! echo "$BRANCH" | grep -qE "$BRANCH_RE"; then
  echo "::error:: Branch name does not conform to naming convention."
  echo ""
  echo "  Got:      $BRANCH"
  echo "  Expected: <type>/<slug>  where type ∈ {feature,fix,chore,test,docs}"
  echo "            slug uses only lowercase letters, digits, and hyphens"
  echo ""
  echo "  Examples:"
  echo "    feature/rusaa-344-pr-hygiene-scripts"
  echo "    fix/kafka-header-propagation"
  echo "    chore/update-dependencies"
  echo ""
  echo "Rename your branch before pushing:"
  echo "  git branch -m $BRANCH <new-name>"
  exit 1
fi

# --- PR title hint (always shown) -----------------------------------------

# Derive a worked example from the branch name.
# feature/rusaa-344-pr-hygiene-scripts → slug = rusaa-344-pr-hygiene-scripts
SLUG="${BRANCH#*/}"

# Try to extract a tracker token from the slug (e.g. rusaa-344 → RUSAA-344 or req-fe-09 → REQ-FE-09).
TRACKER_TOKEN=""
if echo "$SLUG" | grep -iqE '^(rusaa)-[0-9]+'; then
  RAW_TOKEN=$(echo "$SLUG" | grep -ioE '^(rusaa)-[0-9]+')
  TRACKER_TOKEN=$(echo "$RAW_TOKEN" | tr '[:lower:]' '[:upper:]')
elif echo "$SLUG" | grep -iqE '^(req-[a-z]+-[0-9]+)'; then
  RAW_TOKEN=$(echo "$SLUG" | grep -ioE '^(req-[a-z]+-[0-9]+)')
  TRACKER_TOKEN=$(echo "$RAW_TOKEN" | tr '[:lower:]' '[:upper:]')
fi

echo "✓ Branch name OK: $BRANCH"
echo ""
echo "PR title format required:"
echo "  $TITLE_RE <description>"
echo ""
echo "  Where the prefix is:"
echo "    [REQ-XX-NN]  — requirements-registry work (e.g. [REQ-FE-09])"
echo "    [RUSAA-NNN]  — tooling / internal work     (e.g. [RUSAA-344])"
echo ""

if [[ -n "$TRACKER_TOKEN" ]]; then
  # Construct a human-readable description from the slug remainder
  DESC_SLUG="${SLUG#*-}"
  DESC_SLUG="${DESC_SLUG#*-}"   # strip leading NNN- for RUSAA or prefix for REQ
  DESC_WORDS="${DESC_SLUG//-/ }"
  TYPE_PREFIX="${BRANCH%%/*}"
  case "$TYPE_PREFIX" in
    feature) CONV_TYPE="feat" ;;
    fix)     CONV_TYPE="fix" ;;
    *)       CONV_TYPE="$TYPE_PREFIX" ;;
  esac
  echo "Suggested title for this branch:"
  echo "  [$TRACKER_TOKEN] $CONV_TYPE: $DESC_WORDS"
  echo ""
fi
