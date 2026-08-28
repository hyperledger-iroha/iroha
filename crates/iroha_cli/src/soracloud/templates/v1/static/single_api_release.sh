#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${{BASH_SOURCE[0]}}")" && pwd)"
{prelude}

: "${TORII_URL:?Set TORII_URL to the Torii base URL, for example http://127.0.0.1:8080}"
: "${SORAFS_RETENTION_EPOCH:?Set SORAFS_RETENTION_EPOCH to the exact Unix-second SoraFS retention boundary and reuse it for retries}"

args=(
  soracloud app release
  --manifest "$SCRIPT_DIR/app_manifest.json"
  --torii-url "$TORII_URL"
  --sorafs-retention-epoch "$SORAFS_RETENTION_EPOCH"
)

if [[ -n "${API_TOKEN:-}" ]]; then
  args+=(--api-token "$API_TOKEN")
fi

exec "${IROHA_CMD[@]}" "${args[@]}" "$@"
