#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
printf 'ok' > "$SCRIPT_DIR/http-service-build-and-sync-ran.txt"
