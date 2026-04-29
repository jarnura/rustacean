#!/usr/bin/env bash
# Reads per-agent token usage from Paperclip NDJSON run logs and pushes
# Prometheus metrics to the Pushgateway.
#
# Usage:
#   scripts/push-token-metrics.sh [COMPANY_ID] [PUSHGATEWAY_URL]
#
# Metrics emitted (gauge, per agent):
#   rb_agent_context_tokens_peak_last_run{agent_id,agent_name}
#   rb_agent_context_tokens_current_run{agent_id,agent_name}
#   rb_agent_context_budget_ratio_last_run{agent_id,agent_name}  # 0.0–1.0
#   rb_agent_context_turns_last_run{agent_id,agent_name}

set -euo pipefail

PAPERCLIP_DATA_DIR="${PAPERCLIP_DATA_DIR:-$HOME/.paperclip/instances/default/data}"
RUN_LOG_BASE="$PAPERCLIP_DATA_DIR/run-logs"

COMPANY_ID="${1:-}"
PUSHGATEWAY_URL="${2:-http://localhost:9091}"
CONTEXT_WINDOW=200000

if [[ -z "$COMPANY_ID" ]]; then
  echo "Usage: $0 <company-id> [pushgateway-url]" >&2
  exit 1
fi

COMPANY_LOG_DIR="$RUN_LOG_BASE/$COMPANY_ID"
if [[ ! -d "$COMPANY_LOG_DIR" ]]; then
  echo "No run logs at $COMPANY_LOG_DIR" >&2
  exit 0
fi

python3 - "$COMPANY_LOG_DIR" "$PUSHGATEWAY_URL" "$CONTEXT_WINDOW" <<'PYEOF'
import sys, json, os, glob, urllib.request, urllib.error

company_log_dir = sys.argv[1]
pushgateway_url = sys.argv[2]
context_window = int(sys.argv[3])

metrics_lines = []

agent_dirs = [d for d in os.listdir(company_log_dir)
              if os.path.isdir(os.path.join(company_log_dir, d))]

for agent_id in agent_dirs:
    agent_dir = os.path.join(company_log_dir, agent_id)
    files = sorted(glob.glob(f"{agent_dir}/*.ndjson"), key=os.path.getmtime, reverse=True)

    if not files:
        continue

    def peak_for_file(fpath):
        peak, turns = 0, 0
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
        return peak, turns

    current_peak, current_turns = peak_for_file(files[0])
    last_peak, last_turns = (current_peak, current_turns)
    if len(files) > 1:
        last_peak, last_turns = peak_for_file(files[1])

    label = f'agent_id="{agent_id}"'
    ratio = round(last_peak / context_window, 4) if context_window else 0

    metrics_lines.append(f'# HELP rb_agent_context_tokens_peak_last_run Peak context tokens in last completed run')
    metrics_lines.append(f'# TYPE rb_agent_context_tokens_peak_last_run gauge')
    metrics_lines.append(f'rb_agent_context_tokens_peak_last_run{{{label}}} {last_peak}')
    metrics_lines.append(f'# HELP rb_agent_context_tokens_current_run Peak context tokens in current (or most recent) run')
    metrics_lines.append(f'# TYPE rb_agent_context_tokens_current_run gauge')
    metrics_lines.append(f'rb_agent_context_tokens_current_run{{{label}}} {current_peak}')
    metrics_lines.append(f'# HELP rb_agent_context_budget_ratio_last_run Fraction of context window used (0.0–1.0)')
    metrics_lines.append(f'# TYPE rb_agent_context_budget_ratio_last_run gauge')
    metrics_lines.append(f'rb_agent_context_budget_ratio_last_run{{{label}}} {ratio}')
    metrics_lines.append(f'# HELP rb_agent_context_turns_last_run Number of assistant turns in last completed run')
    metrics_lines.append(f'# TYPE rb_agent_context_turns_last_run gauge')
    metrics_lines.append(f'rb_agent_context_turns_last_run{{{label}}} {last_turns}')

if not metrics_lines:
    print("No agent data found.")
    sys.exit(0)

payload = "\n".join(metrics_lines) + "\n"
url = f"{pushgateway_url}/metrics/job/agent_token_budget"

try:
    req = urllib.request.Request(
        url,
        data=payload.encode("utf-8"),
        method="PUT",
        headers={"Content-Type": "text/plain"},
    )
    with urllib.request.urlopen(req, timeout=5) as resp:
        print(f"Pushed {len(agent_dirs)} agents to {url} — HTTP {resp.status}")
except urllib.error.URLError as e:
    print(f"Push failed: {e}")
    # Print metrics locally so callers can debug
    print("\nMetrics that would have been pushed:")
    print(payload)
    sys.exit(1)
PYEOF
