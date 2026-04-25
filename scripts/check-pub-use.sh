#!/usr/bin/env bash
# REQ-MD-02: Public surface lint for workspace library crates.
#
# Every src/lib.rs in the workspace must satisfy two rules:
#
#   Rule 1 — No bare `pub mod name;`
#     Using `pub mod` makes an entire internal module part of the public API,
#     leaking implementation structure.  Use private `mod name;` declarations
#     and re-export only the specific items callers need via explicit `pub use`.
#     Note: `pub(crate) mod` and `pub(super) mod` are allowed.
#
#   Rule 2 — No wildcard re-exports (`pub use path::*;`)
#     Wildcard re-exports silently pull in every public item from a module,
#     making the crate's public surface opaque and hard to audit.  Name each
#     re-exported item explicitly.
#
# When a new lib.rs is added it is checked automatically — no allowlist needed.
set -euo pipefail

FAILED=0

while IFS= read -r -d '' file; do
    # Rule 1: bare `pub mod name;` (pub(crate)/pub(super)/etc. are permitted)
    # Regex: optional whitespace, then literally `pub mod`, then identifier and semicolon.
    # A parenthesised qualifier like `pub(crate)` contains `(` immediately after `pub`,
    # so the negative lookahead `(?!\s*\()` excludes those forms.
    if grep -Pn '^\s*pub(?!\s*\()\s+mod\s+\w+\s*;' "$file" 2>/dev/null | grep -q .; then
        echo "FAIL [$file]: bare 'pub mod' declaration(s) found:" >&2
        grep -Pn '^\s*pub(?!\s*\()\s+mod\s+\w+\s*;' "$file" >&2
        echo "  Fix: change to 'mod name;' and add 'pub use name::Item;' for each public item." >&2
        FAILED=1
    fi

    # Rule 2: wildcard re-exports — `pub use path::*;` (any pub visibility modifier)
    # Plain `use path::*;` inside test modules is permitted; only public re-exports are flagged.
    if grep -Pn '^\s*pub(\([^)]+\))?\s+use\s+\S+::\*\s*;' "$file" 2>/dev/null | grep -q .; then
        echo "FAIL [$file]: wildcard re-export(s) found:" >&2
        grep -Pn '^\s*pub(\([^)]+\))?\s+use\s+\S+::\*\s*;' "$file" >&2
        echo "  Fix: replace 'pub use foo::*;' with named re-exports, e.g. 'pub use foo::{A, B};'." >&2
        FAILED=1
    fi
done < <(find . -name "lib.rs" \
    ! -path "*/target/*" \
    ! -path "*/.git/*" \
    ! -path "*/vendor/*" \
    ! -path "*/node_modules/*" \
    -print0)

if (( FAILED == 1 )); then
    echo "" >&2
    echo "public-surface lint FAILED." >&2
    exit 1
fi

echo "public-surface lint PASSED"
