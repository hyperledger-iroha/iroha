#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
NPM_BIN="${NPM_BIN:-npm}"
API_PORT="${SORACLOUD_SINGLE_API_DEV_PORT:-8787}"
FRONTEND_PORT="${FRONTEND_PORT:-5173}"

cleanup() {
  if [[ -n "${API_PID:-}" ]]; then
    kill "$API_PID" 2>/dev/null || true
  fi
  wait "${API_PID:-}" 2>/dev/null || true
}

trap cleanup EXIT INT TERM

(
  cd "$SCRIPT_DIR/services/api"
  PORT="$API_PORT" ./dev.sh
) &
API_PID=$!

cd "$SCRIPT_DIR/web"
if [[ ! -d node_modules ]]; then
  "$NPM_BIN" install
fi

export SORACLOUD_SINGLE_API_DEV_PROXY_TARGET="${SORACLOUD_SINGLE_API_DEV_PROXY_TARGET:-http://127.0.0.1:$API_PORT}"
"$NPM_BIN" run dev -- --host 127.0.0.1 --port "$FRONTEND_PORT"
