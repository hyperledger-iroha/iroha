#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

python3 "$REPO_ROOT/scripts/nexus/lane_registry_verify.py" \
  --bundle-dir "$REPO_ROOT/fixtures/nexus/registry_bundle"

echo "[nexus] lane registry bundle fixtures validated"
