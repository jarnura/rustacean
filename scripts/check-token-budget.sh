#!/usr/bin/env bash
# Reads per-agent token usage from Paperclip NDJSON run logs.
# Outputs peak context window usage for the N most recent completed runs.
#
# Usage:
#   scripts/check-token-budget.sh [COMPANY_ID] [AGENT_ID] [N_RUNS]
#
# Exit codes:
#   0 — below 80K (healthy)
#   1 — at or above 80K (soft warning)
#   2 — at or above 100K (hard escalation required)

set -euo pipefail

PAPERCLIP_DATA_DIR="${PAPERCLIP_DATA_DIR:-$HOME/.paperclip/instances/default/data}"
RUN_LOG_BASE="$PAPERCLIP_DATA_DIR/run-logs"

COMPANY_ID="${1:-}"
AGENT_ID="${2:-}"
N_RUNS="${3:-5}"

SOFT_WARN=80000
HARD_ESCALATE=100000

if [[ -z "$COMPANY_ID" || -z "$AGENT_ID" ]]; then
  echo "Usage: $0 <company-id> <agent-id> [n-runs]" >&2
  exit 1
fi

AGENT_LOG_DIR="$RUN_LOG_BASE/$COMPANY_ID/$AGENT_ID"
if [[ ! -d "$AGENT_LOG_DIR" ]]; then
  echo "No run logs found at $AGENT_LOG_DIR" >&2
  exit 0
fi

python3 - "$AGENT_LOG_DIR" "$N_RUNS" "$SOFT_WARN" "$HARD_ESCALATE" <<'PYEOF'
import sys, json, os, glob

agent_log_dir = sys.argv[1]
n_runs = int(sys.argv[2])
soft_warn = int(sys.argv[3])
hard_esc = int(sys.argv[4])

files = sorted(glob.glob(f"{agent_log_dir}/*.ndjson"), key=os.path.getmtime, reverse=True)[:n_runs]

if not files:
    print("No run log files found.")
    sys.exit(0)

max_peak = 0
results = []

for fpath in files:
    fname = os.path.basename(fpath)
    peak = 0
    turns = 0
    with open(fpath) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                d = json.loads(line)
                chunk_raw = d.get("chunk", "{}")
                chunk = json.loads(chunk_raw) if isinstance(chunk_raw, str) else chunk_raw
                if chunk.get("type") == "assistant":
                    msg = chunk.get("message", {})
                    u = msg.get("usage", {})
                    total = (
                        u.get("input_tokens", 0)
                        + u.get("cache_creation_input_tokens", 0)
                        + u.get("cache_read_input_tokens", 0)
                    )
                    if total > peak:
                        peak = total
                    turns += 1
            except Exception:
                pass

    status = "OK"
    if peak >= hard_esc:
        status = "HARD_ESCALATE"
    elif peak >= soft_warn:
        status = "SOFT_WARN"

    results.append((fname[:8], peak, turns, status))
    if peak > max_peak:
        max_peak = peak

print(f"{'RUN':8}  {'PEAK_CTX':>10}  {'TURNS':>6}  STATUS")
print("-" * 46)
for run_id, peak, turns, status in results:
    flag = " ⚠️" if status == "SOFT_WARN" else (" 🚨" if status == "HARD_ESCALATE" else "")
    print(f"{run_id}  {peak:>10,}  {turns:>6}  {status}{flag}")

print()
print(f"Peak across last {len(results)} run(s): {max_peak:,} tokens")

if max_peak >= hard_esc:
    print(f"\n🚨 HARD ESCALATION: context exceeded {hard_esc:,} tokens.")
    print("  Per COMPANY.md § Token Budget Management: spawn a continuation issue and exit.")
    sys.exit(2)
elif max_peak >= soft_warn:
    print(f"\n⚠️  SOFT WARNING: context reached {max_peak:,} tokens (threshold {soft_warn:,}).")
    print("  Trim memory and tool output. Consider compacting before next heartbeat.")
    sys.exit(1)
else:
    print(f"\n✓ Context healthy (below {soft_warn:,} token soft-warning threshold).")
    sys.exit(0)
PYEOF
