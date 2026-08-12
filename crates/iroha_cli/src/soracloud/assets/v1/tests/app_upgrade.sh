#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
printf '%s' "${TORII_URL:-}" > "$SCRIPT_DIR/app-upgrade-torii.txt"
printf '%s' "${API_TOKEN:-}" > "$SCRIPT_DIR/app-upgrade-token.txt"
printf '%s\n' "$@" > "$SCRIPT_DIR/app-upgrade-args.txt"
