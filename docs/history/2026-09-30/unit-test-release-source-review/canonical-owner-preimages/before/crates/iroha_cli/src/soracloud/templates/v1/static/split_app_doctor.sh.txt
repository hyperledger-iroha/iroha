#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
{prelude}

"$SCRIPT_DIR/build-and-sync.sh"
exec "${IROHA_CMD[@]}" soracloud app doctor --manifest "$SCRIPT_DIR/app_manifest.json" "$@"
