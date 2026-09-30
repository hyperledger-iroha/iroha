#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"

export PORT="${PORT:-${SORACLOUD_HTTP_PORT:-8787}}"
exec node "$SCRIPT_DIR/dev-server.mjs"
