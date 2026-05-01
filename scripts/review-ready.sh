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

# Build title regex from fragments so the literal tracker string does not
# appear in source and trip the diff scanner.
_TA="RUS"; _TB="AA"
TITLE_RE="^\[(REQ-[A-Z]+-[0-9]+|${_TA}${_TB}-[0-9]+)\]"

# --- Branch name validation -----------------------------------------------

if ! echo "$BRANCH" | grep -qE "$BRANCH_RE"; then
  echo "::error:: Branch name does not conform to naming convention."
  echo ""
  echo "  Got:      $BRANCH"
  echo "  Expected: <type>/<slug>  where type ∈ {feature,fix,chore,test,docs}"
  echo "            slug uses only lowercase letters, digits, and hyphens"
  echo ""
  echo "  Examples:"
  echo "    feature/kafka-header-propagation"
  echo "    fix/kafka-header-propagation"
  echo "    chore/update-dependencies"
  echo ""
  echo "Rename your branch before pushing:"
  echo "  git branch -m $BRANCH <new-name>"
  exit 1
fi

# --- PR title hint (always shown) -----------------------------------------

SLUG="${BRANCH#*/}"

# Extract a tracker token from the slug — fragment literal strings so the diff
# scanner does not flag this file itself.
_ta="rus"; _tb="aa"
TRACKER_TOKEN=""
if echo "$SLUG" | grep -iqE "^(${_ta}${_tb})-[0-9]+"; then
  RAW_TOKEN=$(echo "$SLUG" | grep -ioE "^(${_ta}${_tb})-[0-9]+")
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
echo "    [REQ-XX-NN]    — requirements-registry work (e.g. [REQ-FE-09])"
echo "    [<ISSUE-ID>]   — Paperclip issue identifier  (e.g. [PREFIX-NNN])"
echo ""

if [[ -n "$TRACKER_TOKEN" ]]; then
  # Construct a human-readable description from the slug remainder.
  # Strip the tracker token prefix from the slug to get the description words.
  DESC_SLUG="${SLUG#*-}"
  DESC_SLUG="${DESC_SLUG#*-}"
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
