#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
NPM_BIN="${NPM_BIN:-npm}"
{prelude}

(
  cd "$SCRIPT_DIR/frontend"
  "$NPM_BIN" install
  VITE_PUBLIC_API_BASE=/api VITE_DATA_MODE=live "$NPM_BIN" run build
)

(
  cd "$SCRIPT_DIR/services/live"
  ./build.sh
)

(
  cd "$SCRIPT_DIR/services/vault"
  ./build.sh
  ./verify-build.sh
)

"${IROHA_CMD[@]}" soracloud service sync-manifests --app-manifest "$SCRIPT_DIR/app_manifest.json"
