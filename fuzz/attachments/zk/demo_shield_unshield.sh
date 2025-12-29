#!/usr/bin/env bash
set -euo pipefail

# Tiny end-to-end ZK demo script (placeholders)
#
# Sequence:
#   1) Register VKs (transfer, unshield)
#   2) Register Hybrid asset with VK bindings
#   3) Shield: public -> shielded (note commitment)
#   4) Unshield: shielded -> public (requires real proof JSON; skipped by default)
#
# Requirements:
#   - `iroha` CLI in PATH
#   - `jq`
#   - Running Torii instance accessible via CLI config
#
# Env vars:
#   - CLI_CONFIG: optional path to client config TOML (passed via --config)
#   - AUTHORITY: AccountId for VK ops (e.g., alice@wonderland)
#   - PRIVATE_KEY: ExposedPrivateKey for AUTHORITY
#   - BACKEND: proof backend (default: halo2/ipa)
#   - ASSET_ID: AssetDefinitionId (default: rose#wonderland)
#   - FROM: debit account for shield (default: alice@wonderland)
#   - TO: credit account for unshield (default: alice@wonderland)
#   - AMOUNT: amount for shield/unshield (default: 1000)
#   - NOTE_COMMITMENT_HEX: 64-hex for shield (default: 32 zero bytes)
#   - VK_TRANSFER_NAME: name for transfer VK (default: vk_transfer)
#   - VK_UNSHIELD_NAME: name for unshield VK (default: vk_unshield)
#   - PROOF_JSON: path to proof JSON for unshield (default: none; step skipped)
#   - RUN_UNSHIELD: set to 1 to attempt unshield if PROOF_JSON is set

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing dependency: $1" >&2; exit 1; }; }
need iroha
need jq

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FLAG=()
if [[ -n "${CLI_CONFIG:-}" ]]; then
  CONFIG_FLAG=(--config "$CLI_CONFIG")
fi

AUTHORITY="${AUTHORITY:-alice@wonderland}"
PRIVATE_KEY="${PRIVATE_KEY:-ed0120...}"
BACKEND="${BACKEND:-halo2/ipa}"
ASSET_ID="${ASSET_ID:-rose#wonderland}"
FROM="${FROM:-alice@wonderland}"
TO="${TO:-alice@wonderland}"
AMOUNT="${AMOUNT:-1000}"
NOTE_COMMITMENT_HEX="${NOTE_COMMITMENT_HEX:-0000000000000000000000000000000000000000000000000000000000000000}"
VK_TRANSFER_NAME="${VK_TRANSFER_NAME:-vk_transfer}"
VK_UNSHIELD_NAME="${VK_UNSHIELD_NAME:-vk_unshield}"
PROOF_JSON="${PROOF_JSON:-}"
RUN_UNSHIELD="${RUN_UNSHIELD:-0}"

echo "[0/4] Health check (server version)"
if ! iroha "${CONFIG_FLAG[@]}" Version >/dev/null; then
  echo "Torii health check failed. Verify config and that the server is running." >&2
  exit 1
fi

echo "[1/4] Register verifying keys (transfer, unshield)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT
reg_template="$SCRIPT_DIR/vk_register.json"
if [[ ! -f "$reg_template" ]]; then
  echo "Missing $reg_template; see corpora under fuzz/attachments/zk" >&2
  exit 1
fi

# Register transfer VK
jq --arg a "$AUTHORITY" --arg k "$PRIVATE_KEY" --arg b "$BACKEND" --arg n "$VK_TRANSFER_NAME" \
  '.authority=$a | .private_key=$k | .backend=$b | .name=$n' \
  "$reg_template" >"$tmp_dir/vk_transfer.json"
iroha "${CONFIG_FLAG[@]}" zk vk register --json "$tmp_dir/vk_transfer.json"

# Register unshield VK
jq --arg a "$AUTHORITY" --arg k "$PRIVATE_KEY" --arg b "$BACKEND" --arg n "$VK_UNSHIELD_NAME" \
  '.authority=$a | .private_key=$k | .backend=$b | .name=$n' \
  "$reg_template" >"$tmp_dir/vk_unshield.json"
iroha "${CONFIG_FLAG[@]}" zk vk register --json "$tmp_dir/vk_unshield.json"

echo "[2/4] Register Hybrid asset with VK bindings"
iroha "${CONFIG_FLAG[@]}" zk register-asset --asset "$ASSET_ID" \
  --allow-shield true --allow-unshield true \
  --vk-transfer "$BACKEND:$VK_TRANSFER_NAME" \
  --vk-unshield "$BACKEND:$VK_UNSHIELD_NAME"

echo "[3/4] Shield: $FROM -> shielded ($ASSET_ID) amount=$AMOUNT"
iroha "${CONFIG_FLAG[@]}" zk shield --asset "$ASSET_ID" --from "$FROM" \
  --amount "$AMOUNT" --note-commitment "$NOTE_COMMITMENT_HEX"

echo "[4/4] Unshield: shielded -> $TO ($ASSET_ID) amount=$AMOUNT"
if [[ "$RUN_UNSHIELD" == "1" && -n "$PROOF_JSON" ]]; then
  echo "  Attempting unshield using proof JSON: $PROOF_JSON"
  # Nullifiers placeholder (two zeros); replace with real values for a working demo
  NULLIFIERS="0000000000000000000000000000000000000000000000000000000000000000,0000000000000000000000000000000000000000000000000000000000000000"
  iroha "${CONFIG_FLAG[@]}" zk unshield --asset "$ASSET_ID" --to "$TO" --amount "$AMOUNT" \
    --inputs "$NULLIFIERS" --proof-json "$PROOF_JSON" || \
    echo "  Unshield likely failed due to placeholder proof (expected). Provide a real proof to succeed." >&2
else
  echo "  Skipping unshield. Set RUN_UNSHIELD=1 and PROOF_JSON=/path/to/proof.json to attempt."
fi

echo "Done."

