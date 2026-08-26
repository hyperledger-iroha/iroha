#!/usr/bin/env bash
set -euo pipefail

# Auto-run ZK CLI sample sequence
#
# Prerequisites:
# - iroha CLI in PATH
# - jq, base64 utilities
# - A running Torii node reachable by the CLI config
#
# Configuration (env vars):
# - CLI_CONFIG: optional path to client config TOML (passed via --config)
# - The configured client account and key sign VK transactions.
# - ELECTION_ID: optional vote id (default: demo-election-1)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FLAG=()
if [[ -n "${CLI_CONFIG:-}" ]]; then
  CONFIG_FLAG=(--config "$CLI_CONFIG")
fi

ELECTION_ID="${ELECTION_ID:-demo-election-1}"

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing dependency: $1" >&2; exit 1; }; }
need iroha
need jq
need base64

# [0] Health check
echo "[0/5] Checking Torii health (server version)"
if ! iroha "${CONFIG_FLAG[@]}" Version >/dev/null; then
  echo "Torii health check failed. Verify config and that the server is running." >&2
  exit 1
fi

echo "[1/5] VK register/update with the configured signer"
iroha "${CONFIG_FLAG[@]}" zk vk register --json "$SCRIPT_DIR/vk_register.json"
iroha "${CONFIG_FLAG[@]}" zk vk update --json "$SCRIPT_DIR/vk_update.json"
iroha "${CONFIG_FLAG[@]}" zk vk get --backend halo2/ipa --name vk_add

echo "[2/5] Upload JSON attachment"
ATT_META_JSON=$(iroha "${CONFIG_FLAG[@]}" zk attachments upload --file "$SCRIPT_DIR/proof.json" --content-type application/json)
echo "$ATT_META_JSON" | jq -C .

echo "[3/5] Upload minimal ZK1 Norito envelope"
ZK1_BIN="$SCRIPT_DIR/zk1_min.bin"
if base64 --help 2>&1 | grep -q -- '--decode'; then
  base64 --decode "$SCRIPT_DIR/zk1_min.b64" >"$ZK1_BIN"
else
  base64 -D "$SCRIPT_DIR/zk1_min.b64" >"$ZK1_BIN"
fi
iroha "${CONFIG_FLAG[@]}" zk attachments upload --file "$ZK1_BIN" --content-type application/x-norito >/dev/null

echo "[4/5] List attachments"
iroha "${CONFIG_FLAG[@]}" zk attachments list | jq -C .

echo "[5/5] Vote tally helper"
iroha "${CONFIG_FLAG[@]}" zk vote tally --election-id "$ELECTION_ID" | jq -C .

echo "Done."
