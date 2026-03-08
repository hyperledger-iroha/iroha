#!/usr/bin/env bash
set -euo pipefail

# Configuration
IROHA_BIN="${IROHA_BIN:-target/release/iroha}"
TORII0="${TORII0:-http://127.0.0.1:8080/}"
TORII1="${TORII1:-http://127.0.0.1:8081/}"
ASSET_DEFINITION_ID="${ASSET_DEFINITION_ID:-testasset#wonderland}"
ASSET_ID="${ASSET_ID:-norito:4e52543000000001}"
SENDER_ACCOUNT="${SENDER_ACCOUNT:-6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9}"
RECIPIENT_ACCOUNT="${RECIPIENT_ACCOUNT:-6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ}"
MINT_QTY="${MINT_QTY:-100}"
XFER_QTY="${XFER_QTY:-25}"

if [ ! -x "$IROHA_BIN" ]; then
  echo "IROHA_BIN not found or not executable: $IROHA_BIN" >&2
  exit 1
fi

echo "[1/4] Register asset definition: $ASSET_DEFINITION_ID"
"$IROHA_BIN" --config defaults/client.toml asset definition register --id "$ASSET_DEFINITION_ID" -t Numeric || true

echo "[2/4] Mint $MINT_QTY to $SENDER_ACCOUNT"
"$IROHA_BIN" --config defaults/client.toml --torii-url "$TORII0" asset mint --id "$ASSET_ID" --quantity "$MINT_QTY"

echo "[3/4] Transfer $XFER_QTY to $RECIPIENT_ACCOUNT"
"$IROHA_BIN" --config defaults/client.toml --torii-url "$TORII0" asset transfer --id "$ASSET_ID" --to "$RECIPIENT_ACCOUNT" --quantity "$XFER_QTY"

echo "[4/4] Query recipient balance from node1 ($TORII1)"
"$IROHA_BIN" --config defaults/client.toml --torii-url "$TORII1" asset get --id "$ASSET_ID"
