#!/usr/bin/env bash
# Install git hooks that enforce SDLC policies locally.
# Safe to re-run: overwrites existing hooks with the current version.
set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel)
HOOK_DIR="${REPO_ROOT}/.git/hooks"
HOOK_FILE="${HOOK_DIR}/pre-push"

mkdir -p "$HOOK_DIR"

cat > "$HOOK_FILE" << 'HOOK'
#!/usr/bin/env bash
# Installed by scripts/install-hooks.sh — re-run `make install-hooks` to update.
exec bash "$(git rev-parse --show-toplevel)/scripts/check-no-bundle.sh" "$@"
HOOK

chmod +x "$HOOK_FILE"
echo "  OK  pre-push hook installed: ${HOOK_FILE}"
