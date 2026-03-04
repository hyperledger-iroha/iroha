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
# - AUTHORITY: account id like 'alice@wonderland' (required for VK ops)
# - PRIVATE_KEY: ExposedPrivateKey string (multihash) for AUTHORITY (required for VK ops)
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
echo "[0/7] Checking Torii health (server version)"
if ! iroha "${CONFIG_FLAG[@]}" Version >/dev/null; then
  echo "Torii health check failed. Verify config and that the server is running." >&2
  exit 1
fi

echo "[1/7] VK register/update/deprecate (requires AUTHORITY and PRIVATE_KEY)"
if [[ -z "${AUTHORITY:-}" || -z "${PRIVATE_KEY:-}" ]]; then
  echo "  Skipping VK ops: AUTHORITY and/or PRIVATE_KEY not set" >&2
else
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' EXIT
  # Register
  jq --arg a "$AUTHORITY" --arg k "$PRIVATE_KEY" '.authority=$a | .private_key=$k' \
    "$SCRIPT_DIR/vk_register.json" >"$tmp_dir/vk_register.eff.json"
  iroha "${CONFIG_FLAG[@]}" zk vk register --json "$tmp_dir/vk_register.eff.json"
  # Update (bump version)
  jq --arg a "$AUTHORITY" --arg k "$PRIVATE_KEY" '.authority=$a | .private_key=$k' \
    "$SCRIPT_DIR/vk_update.json" >"$tmp_dir/vk_update.eff.json"
  iroha "${CONFIG_FLAG[@]}" zk vk update --json "$tmp_dir/vk_update.eff.json"
  # Deprecate
  jq --arg a "$AUTHORITY" --arg k "$PRIVATE_KEY" '.authority=$a | .private_key=$k' \
    "$SCRIPT_DIR/vk_deprecate.json" >"$tmp_dir/vk_deprecate.eff.json"
  iroha "${CONFIG_FLAG[@]}" zk vk deprecate --json "$tmp_dir/vk_deprecate.eff.json"
  # Read
  iroha "${CONFIG_FLAG[@]}" zk vk get --backend halo2/ipa --name vk_add
fi

echo "[2/7] Upload JSON attachment"
ATT_META_JSON=$(iroha "${CONFIG_FLAG[@]}" zk attachments upload --file "$SCRIPT_DIR/proof.json" --content-type application/json)
echo "$ATT_META_JSON" | jq -C .
ATT_ID=$(echo "$ATT_META_JSON" | jq -r .id)

echo "[3/7] Upload minimal ZK1 Norito envelope"
ZK1_BIN="$SCRIPT_DIR/zk1_min.bin"
if base64 --help 2>&1 | grep -q -- '--decode'; then
  base64 --decode "$SCRIPT_DIR/zk1_min.b64" >"$ZK1_BIN"
else
  base64 -D "$SCRIPT_DIR/zk1_min.b64" >"$ZK1_BIN"
fi
iroha "${CONFIG_FLAG[@]}" zk attachments upload --file "$ZK1_BIN" --content-type application/x-norito >/dev/null

echo "[4/7] List attachments"
iroha "${CONFIG_FLAG[@]}" zk attachments list | jq -C .

echo "[5/7] List prover reports"
iroha "${CONFIG_FLAG[@]}" zk prover reports list | jq -C '.[:5]'

echo "[6/7] Get a single prover report (first if available)"
FIRST_REP=$(iroha "${CONFIG_FLAG[@]}" zk prover reports list | jq -r '.[0].id // empty')
if [[ -n "$FIRST_REP" ]]; then
  iroha "${CONFIG_FLAG[@]}" zk prover reports get --id "$FIRST_REP" | jq -C .
else
  echo "  No reports yet"
fi

echo "[7/7] Vote tally helper"
iroha "${CONFIG_FLAG[@]}" zk vote tally --election-id "$ELECTION_ID" | jq -C .

echo "Done."
