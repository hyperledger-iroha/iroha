#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
NPM_BIN="${NPM_BIN:-npm}"
{prelude}

(
  cd "$SCRIPT_DIR/web"
  "$NPM_BIN" install
  "$NPM_BIN" run build
)

(
  cd "$SCRIPT_DIR/services/api"
  ./build.sh
  ./verify-build.sh
)

"${IROHA_CMD[@]}" soracloud service sync-manifests --app-manifest "$SCRIPT_DIR/app_manifest.json"
