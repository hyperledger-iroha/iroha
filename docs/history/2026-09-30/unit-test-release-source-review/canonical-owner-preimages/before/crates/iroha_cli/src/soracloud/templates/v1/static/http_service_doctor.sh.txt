#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
{prelude}

"$SCRIPT_DIR/build-and-sync.sh"
exec "${IROHA_CMD[@]}" soracloud service plan \
  --container "$SCRIPT_DIR/container_manifest.json" \
  --service "$SCRIPT_DIR/service_manifest.json" \
  "$@"
