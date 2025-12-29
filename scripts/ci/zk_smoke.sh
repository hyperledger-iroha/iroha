#!/usr/bin/env bash
set -euo pipefail

# Tiny CI smoke for ZK helpers (register-asset + shield)
#
# Assumptions:
# - `iroha` CLI is available in PATH
# - Torii is reachable using the CLI config
# - No VK/proofs required; this script avoids unshield and prover ops
#
# Env overrides:
# - CLI_CONFIG: path to client config TOML (optional)
# - ASSET_ID: AssetDefinitionId (default rose#wonderland)
# - FROM: AccountId to debit (default alice@wonderland)
# - AMOUNT: amount to shield (default 1)
# - NOTE_COMMITMENT_HEX: 64-hex commitment (default zeros)

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing dependency: $1" >&2; exit 1; }; }
need iroha

CONFIG_FLAG=()
if [[ -n "${CLI_CONFIG:-}" ]]; then CONFIG_FLAG=(--config "$CLI_CONFIG"); fi

ASSET_ID="${ASSET_ID:-rose#wonderland}"
FROM="${FROM:-alice@wonderland}"
AMOUNT="${AMOUNT:-1}"
NOTE_COMMITMENT_HEX="${NOTE_COMMITMENT_HEX:-0000000000000000000000000000000000000000000000000000000000000000}"

echo "[zk-smoke] server version"
iroha "${CONFIG_FLAG[@]}" Version >/dev/null

echo "[zk-smoke] register-asset (Hybrid, allow shield/unshield)"
iroha "${CONFIG_FLAG[@]}" zk register-asset --asset "$ASSET_ID" \
  --allow-shield true --allow-unshield true >/dev/null

echo "[zk-smoke] shield $ASSET_ID from $FROM amount=$AMOUNT"
iroha "${CONFIG_FLAG[@]}" zk shield --asset "$ASSET_ID" --from "$FROM" \
  --amount "$AMOUNT" --note-commitment "$NOTE_COMMITMENT_HEX" >/dev/null

echo "[zk-smoke] OK"

