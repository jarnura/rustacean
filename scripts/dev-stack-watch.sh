#!/usr/bin/env bash
# Polls origin/main for new commits and triggers dev-stack-auto-rebuild.sh on changes.
# Designed to run as a persistent systemd service on mars.
#
# Usage:
#   scripts/dev-stack-watch.sh [REPO_PATH]
#
# Environment:
#   POLL_INTERVAL   Seconds between git fetch polls (default: 60)
#   REMOTE          Remote name to poll (default: origin)
#   BRANCH          Branch to track (default: main)
#   COMPOSE_CMD     Passed through to dev-stack-auto-rebuild.sh
#
# Logs: journald captures stdout/stderr when run as a systemd service.
# Rebuild records are written by dev-stack-auto-rebuild.sh to
#   $HOME/.local/state/rustbrain/dev-stack-rebuilds.ndjson
#
# See docs/dev-stack-auto-rebuild.md for setup instructions.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${1:-"$(cd "$SCRIPT_DIR/.." && pwd)"}"
POLL_INTERVAL="${POLL_INTERVAL:-60}"
REMOTE="${REMOTE:-origin}"
BRANCH="${BRANCH:-main}"

REBUILD_SCRIPT="$SCRIPT_DIR/dev-stack-auto-rebuild.sh"

# Persist LAST_KNOWN_SHA across restarts so commits that land during an outage
# are not silently dropped when the service comes back up.
STATE_DIR="${RB_STATE_DIR:-"$HOME/.local/state/rustbrain"}"
SHA_STATE_FILE="$STATE_DIR/dev-stack-watch-last-sha"
mkdir -p "$STATE_DIR"

echo "[dev-stack-watch] started — polling $REMOTE/$BRANCH every ${POLL_INTERVAL}s"
echo "[dev-stack-watch] repo: $REPO_ROOT"
echo "[dev-stack-watch] rebuild script: $REBUILD_SCRIPT"

cd "$REPO_ROOT"

# Initialise LAST_KNOWN_SHA: restore from state file to catch commits that arrived
# during an outage; fall back to current origin/main only on first run.
git fetch "$REMOTE" "$BRANCH" --quiet 2>/dev/null || \
  echo "[dev-stack-watch] initial fetch failed; will retry on next cycle" >&2
if [[ -f "$SHA_STATE_FILE" ]]; then
  LAST_KNOWN_SHA="$(cat "$SHA_STATE_FILE")"
  echo "[dev-stack-watch] resuming from persisted SHA $LAST_KNOWN_SHA"
else
  LAST_KNOWN_SHA="$(git rev-parse "$REMOTE/$BRANCH" 2>/dev/null || git rev-parse HEAD)"
  echo "$LAST_KNOWN_SHA" > "$SHA_STATE_FILE"
fi

echo "[dev-stack-watch] tracking from $LAST_KNOWN_SHA"

while true; do
  sleep "$POLL_INTERVAL"

  if ! git fetch "$REMOTE" "$BRANCH" --quiet 2>/dev/null; then
    echo "[dev-stack-watch] fetch failed — will retry" >&2
    continue
  fi

  NEW_SHA="$(git rev-parse "$REMOTE/$BRANCH")"

  if [[ "$NEW_SHA" == "$LAST_KNOWN_SHA" ]]; then
    continue
  fi

  echo "[dev-stack-watch] new commit: $LAST_KNOWN_SHA → $NEW_SHA"

  # Fast-forward the local branch if it is currently checked out on main.
  CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
  if [[ "$CURRENT_BRANCH" == "$BRANCH" ]]; then
    git merge --ff-only "$REMOTE/$BRANCH" --quiet 2>/dev/null || true
  fi

  PREV_SHA="$LAST_KNOWN_SHA"
  LAST_KNOWN_SHA="$NEW_SHA"
  echo "$LAST_KNOWN_SHA" > "$SHA_STATE_FILE"

  echo "[dev-stack-watch] triggering rebuild: $PREV_SHA → $NEW_SHA"
  # Never let a rebuild failure crash the watch loop.
  "$REBUILD_SCRIPT" "$PREV_SHA" "$NEW_SHA" || \
    echo "[dev-stack-watch] rebuild exited non-zero — see logs for details" >&2
done
