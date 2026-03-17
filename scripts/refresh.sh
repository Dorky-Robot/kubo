#!/usr/bin/env bash
set -euo pipefail

# Refresh the kubo environment: upgrade host tools + rebuild image + update containers.
#
# Usage: ./scripts/refresh.sh
#   Or add an alias:  alias kubo-refresh='path/to/kubo/scripts/refresh.sh'

log() { printf '\033[1;32m==>\033[0m %s\n' "$*"; }

# ── Upgrade host tools via Homebrew ──────────────────────────────
if command -v brew >/dev/null 2>&1; then
  log "Updating Homebrew formulae..."
  brew update --quiet

  for formula in katulong kubo; do
    if brew list --versions "$formula" >/dev/null 2>&1; then
      current=$(brew list --versions "$formula" | awk '{print $2}')
      latest=$(brew info --json=v2 "$formula" 2>/dev/null | grep -o '"stable":"[^"]*"' | head -1 | cut -d'"' -f4)
      if [ "$current" != "$latest" ] && [ -n "$latest" ]; then
        log "Upgrading $formula ($current → $latest)..."
        brew upgrade "$formula"
      else
        log "$formula is up to date ($current)"
      fi
    fi
  done
else
  log "Homebrew not found, skipping host tool upgrades"
fi

# ── Restart katulong if it was running ────────────────────────────
if command -v katulong >/dev/null 2>&1; then
  log "Restarting katulong..."
  katulong start 2>/dev/null || true
fi

# ── Rebuild kubo image and update containers ─────────────────────
log "Refreshing kubo image and containers..."
kubo refresh

log "Done!"
