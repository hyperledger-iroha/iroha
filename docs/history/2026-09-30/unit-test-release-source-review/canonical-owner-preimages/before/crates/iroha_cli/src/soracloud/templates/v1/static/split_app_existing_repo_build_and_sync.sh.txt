#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
{prelude}

cat >&2 <<'EOF'
split-app existing-repo scaffold: replace build-and-sync.sh with your real
frontend/live/vault build pipeline. The default implementation only refreshes
manifest hashes for artifacts that already exist at the paths referenced by
app_manifest.json.
EOF

exec "${IROHA_CMD[@]}" soracloud service sync-manifests --app-manifest "$SCRIPT_DIR/app_manifest.json"
