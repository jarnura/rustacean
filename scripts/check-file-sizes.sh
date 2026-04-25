#!/usr/bin/env bash
# REQ-MD-01: Hard cap — no source file may exceed 600 lines.
# Checks all .rs, .ts, .tsx, .py, and .go files outside of generated / vendor paths.
# RUSAA-25 owns any refinements to scope or thresholds.
set -euo pipefail

MAX_LINES=600
FAILED=0

while IFS= read -r -d '' file; do
    lines=$(wc -l < "$file")
    if (( lines > MAX_LINES )); then
        echo "FAIL: $file has $lines lines (limit: $MAX_LINES)" >&2
        FAILED=1
    fi
done < <(find . -type f \( \
    -name "*.rs" -o \
    -name "*.ts" -o \
    -name "*.tsx" -o \
    -name "*.py"  -o \
    -name "*.go" \
  \) \
  ! -path "*/target/*" \
  ! -path "*/.git/*" \
  ! -path "*/node_modules/*" \
  ! -path "*/vendor/*" \
  ! -path "*/generated/*" \
  ! -name "*.generated.rs" \
  -print0)

if (( FAILED == 1 )); then
    echo "file-size check FAILED: one or more files exceed the ${MAX_LINES}-line limit" >&2
    exit 1
fi

echo "file-size check PASSED: all files are within the ${MAX_LINES}-line limit"
