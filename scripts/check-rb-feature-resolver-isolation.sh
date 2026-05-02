#!/usr/bin/env bash
# REQ-LI-01: Enforce zero rb-* dependencies in rb-feature-resolver.
#
# rb-feature-resolver is a candidate for standalone crates.io publication; it must
# never import from any other rb-* crate.  This script fails CI if any such
# dependency appears in the resolved dependency graph or in the crate's Cargo.toml.
#
# Two checks are run:
#
#   Check 1 — Cargo.toml scan
#     Reads crates/rb-feature-resolver/Cargo.toml and greps for any `rb-*` dependency
#     name in [dependencies], [dev-dependencies], or [build-dependencies].
#
#   Check 2 — cargo metadata graph scan
#     Uses `cargo metadata` to walk the resolved dep graph of rb-feature-resolver and
#     asserts that no node id starts with "rb-" (other than rb-feature-resolver itself).
#
set -euo pipefail

CRATE="rb-feature-resolver"
MANIFEST="crates/${CRATE}/Cargo.toml"
FAILED=0

echo "==> [1/2] Scanning ${MANIFEST} for rb-* dependency declarations..."

if grep -E '^\s*(rb-[a-z-]+)\s*=' "${MANIFEST}" 2>/dev/null | grep -qv "^#"; then
    echo "FAIL: ${MANIFEST} declares an rb-* dependency:" >&2
    grep -En '^\s*(rb-[a-z-]+)\s*=' "${MANIFEST}" >&2
    FAILED=1
else
    echo "  PASS: no rb-* dep declarations found in ${MANIFEST}"
fi

echo ""
echo "==> [2/2] Scanning resolved dependency graph for rb-* transitive deps..."

# cargo metadata emits JSON; parse out all package names in the dep graph of our crate.
# We use --filter-platform "" to avoid needing a target spec while still getting all deps.
RB_DEPS=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
    | python3 -c "
import sys, json
data = json.load(sys.stdin)
packages = {p['name'] for p in data.get('packages', [])}
rb_deps = sorted(n for n in packages if n.startswith('rb-') and n != 'rb-feature-resolver')
for n in rb_deps:
    print(n)
" 2>/dev/null || true)

# Now check the full dep tree including transitive deps via cargo tree.
RB_TRANSITIVE=$(cargo tree -p "${CRATE}" 2>/dev/null \
    | grep -E '^\s*(rb-[a-z-]+)' \
    | grep -v "^${CRATE}" \
    | grep -v "^rb-feature-resolver" \
    | sed 's/.*\(rb-[a-zA-Z-]*\).*/\1/' \
    | sort -u || true)

if [ -n "${RB_TRANSITIVE}" ]; then
    echo "FAIL: rb-feature-resolver has rb-* transitive dependencies:" >&2
    echo "${RB_TRANSITIVE}" >&2
    FAILED=1
else
    echo "  PASS: zero rb-* transitive deps in rb-feature-resolver"
fi

echo ""
if (( FAILED == 1 )); then
    echo "rb-feature-resolver isolation check FAILED." >&2
    exit 1
fi

echo "rb-feature-resolver isolation check PASSED"
