#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
NPM_BIN="${NPM_BIN:-npm}"
LIVE_PORT="${SORACLOUD_LIVE_DEV_PORT:-8787}"
VAULT_PORT="${SORACLOUD_VAULT_DEV_PORT:-8788}"
FRONTEND_PORT="${FRONTEND_PORT:-5173}"

cleanup() {
  if [[ -n "${LIVE_PID:-}" ]]; then
    kill "$LIVE_PID" 2>/dev/null || true
  fi
  if [[ -n "${VAULT_PID:-}" ]]; then
    kill "$VAULT_PID" 2>/dev/null || true
  fi
  wait "${LIVE_PID:-}" "${VAULT_PID:-}" 2>/dev/null || true
}

trap cleanup EXIT INT TERM

(
  cd "$SCRIPT_DIR/services/live"
  PORT="$LIVE_PORT" ./dev.sh
) &
LIVE_PID=$!

(
  cd "$SCRIPT_DIR/services/vault"
  PORT="$VAULT_PORT" ./dev.sh
) &
VAULT_PID=$!

cd "$SCRIPT_DIR/frontend"
if [[ ! -d node_modules ]]; then
  "$NPM_BIN" install
fi

export SORACLOUD_LIVE_DEV_PROXY_TARGET="${SORACLOUD_LIVE_DEV_PROXY_TARGET:-http://127.0.0.1:$LIVE_PORT}"
export SORACLOUD_VAULT_DEV_PROXY_TARGET="${SORACLOUD_VAULT_DEV_PROXY_TARGET:-http://127.0.0.1:$VAULT_PORT}"
export VITE_PUBLIC_API_BASE="${VITE_PUBLIC_API_BASE:-/api}"
export VITE_DATA_MODE="${VITE_DATA_MODE:-local}"

"$NPM_BIN" run dev -- --host 127.0.0.1 --port "$FRONTEND_PORT"
