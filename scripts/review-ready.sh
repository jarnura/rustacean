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

if [ "$FAIL" -eq 0 ]; then
    echo "ALL CHECKS PASSED"
    exit 0
else
    echo "$FAIL check(s) FAILED"
    exit 1
fi
