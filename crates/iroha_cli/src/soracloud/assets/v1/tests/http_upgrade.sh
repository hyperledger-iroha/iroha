#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
printf '%s' "${TORII_URL:-}" > "$SCRIPT_DIR/upgrade-torii.txt"
printf '%s' "${SORAFS_RETENTION_EPOCH:-}" > "$SCRIPT_DIR/upgrade-retention-epoch.txt"
printf '%s\n' "$@" > "$SCRIPT_DIR/upgrade-args.txt"
