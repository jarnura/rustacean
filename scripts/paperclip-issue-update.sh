#!/usr/bin/env bash
# Update a Paperclip issue status and post a multiline markdown comment.
#
# Usage:
#   scripts/paperclip-issue-update.sh --issue-id <id> --status <status> <<'MD'
#   ## Update
#   - did the thing
#   MD
#
# Required env: PAPERCLIP_API_KEY, PAPERCLIP_API_URL
# Optional env: PAPERCLIP_RUN_ID (included as X-Paperclip-Run-Id when set)
set -euo pipefail

ISSUE_ID=""
STATUS=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --issue-id) ISSUE_ID="$2"; shift 2 ;;
        --status)   STATUS="$2";   shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

if [[ -z "$ISSUE_ID" || -z "$STATUS" ]]; then
    echo "Usage: $0 --issue-id <id> --status <status> < comment-body" >&2
    exit 1
fi

COMMENT=$(cat)

PAYLOAD=$(jq -n \
    --arg status  "$STATUS" \
    --arg comment "$COMMENT" \
    '{"status": $status, "comment": $comment}')

CURL_ARGS=(
    -sf -X PATCH
    -H "Authorization: Bearer ${PAPERCLIP_API_KEY}"
    -H "Content-Type: application/json"
    -d "$PAYLOAD"
)

if [[ -n "${PAPERCLIP_RUN_ID:-}" ]]; then
    CURL_ARGS+=(-H "X-Paperclip-Run-Id: ${PAPERCLIP_RUN_ID}")
fi

curl "${CURL_ARGS[@]}" "${PAPERCLIP_API_URL}/api/issues/${ISSUE_ID}"
