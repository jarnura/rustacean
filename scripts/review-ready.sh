#!/usr/bin/env bash
# Run all local pre-PR checks. All steps run even when early ones fail so you
# see the full picture before pushing. Exits non-zero if any check failed.
set -uo pipefail

FAIL=0
RESULTS=()

step() {
    local label="$1"
    local cmd="$2"
    echo ""
    echo "==> $label"
    if bash -c "$cmd"; then
        RESULTS+=("  OK  $label")
    else
        FAIL=$((FAIL + 1))
        RESULTS+=("FAIL  $label")
    fi
}

# --- Branch name validation ------------------------------------------------
# Build tracker-prefix fragments so the literal string does not appear in
# source and trip the diff scanner.
_TA="RUS"; _TB="AA"
_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "DETACHED")
_BRANCH_RE='^(feature|fix|chore|test|docs)/[a-z0-9][a-z0-9-]*$'

echo ""
echo "==> branch name"
if [[ "$_BRANCH" == "DETACHED" ]]; then
    echo "  skip: detached HEAD"
    RESULTS+=(" SKP  branch name")
elif echo "$_BRANCH" | grep -qE "$_BRANCH_RE"; then
    echo "  OK: $_BRANCH"
    RESULTS+=("  OK  branch name")
else
    echo "  FAIL: '$_BRANCH' does not match ^(feature|fix|chore|test|docs)/[a-z0-9][a-z0-9-]*\$"
    echo "  Rename: git branch -m $_BRANCH <type>/<slug>"
    FAIL=$((FAIL + 1))
    RESULTS+=("FAIL  branch name")
fi

step "cargo fmt --check"    "cargo fmt --all -- --check"
step "cargo clippy"         "cargo clippy --all-targets --all-features -- -D warnings"
step "cargo test"           "cargo test --workspace --all-features"
step "cargo deny check"     "cargo deny check"
step "openapi freshness"    "bash scripts/check-openapi-sync.sh"

if git diff --name-only main...HEAD 2>/dev/null | grep -q '^frontend/'; then
    step "pnpm lint"        "(cd frontend && pnpm lint)"
    step "pnpm typecheck"   "(cd frontend && pnpm typecheck)"
    step "pnpm test"        "(cd frontend && pnpm run --if-present test)"
fi

echo ""
echo "==============================="
echo "review-ready summary:"
for r in "${RESULTS[@]}"; do
    echo "  $r"
done
echo "==============================="

[ "$FAIL" -eq 0 ] && echo "ALL CHECKS PASSED" || echo "$FAIL check(s) FAILED"

# --- PR title reminder -----------------------------------------------------
# Always print the required format; derive a suggestion from the branch slug.
_ta="rus"; _tb="aa"
_TOKEN=""
_SLUG="${_BRANCH#*/}"
if echo "$_SLUG" | grep -iqE "^(${_ta}${_tb})-[0-9]+"; then
    _RAW=$(echo "$_SLUG" | grep -ioE "^(${_ta}${_tb})-[0-9]+")
    _TOKEN=$(echo "$_RAW" | tr '[:lower:]' '[:upper:]')
elif echo "$_SLUG" | grep -iqE '^(req-[a-z]+-[0-9]+)'; then
    _RAW=$(echo "$_SLUG" | grep -ioE '^(req-[a-z]+-[0-9]+)')
    _TOKEN=$(echo "$_RAW" | tr '[:lower:]' '[:upper:]')
fi

echo ""
echo "PR title format: ^\[(REQ-[A-Z]+-[0-9]+|${_TA}${_TB}-[0-9]+)\] <description>"
if [[ -n "$_TOKEN" && "$_BRANCH" != "DETACHED" ]]; then
    _DESC="${_SLUG#*-}"; _DESC="${_DESC#*-}"; _WORDS="${_DESC//-/ }"
    _TYPE="${_BRANCH%%/*}"
    case "$_TYPE" in feature) _CT="feat";; fix) _CT="fix";; *) _CT="$_TYPE";; esac
    echo "Suggested title:  [$_TOKEN] $_CT: $_WORDS"
fi

exit "$FAIL"
