#!/usr/bin/env bash
set -euo pipefail

# Configuration
IROHA_BIN="${IROHA_BIN:-target/release/iroha}"
TORII0="${TORII0:-http://127.0.0.1:8080/}"
TORII1="${TORII1:-http://127.0.0.1:8081/}"
ASSET_DEFINITION_ID="${ASSET_DEFINITION_ID:-7EAD8EFYUx1aVKZPUU1fyKvr8dF1}"
ASSET_DEFINITION_NAME="${ASSET_DEFINITION_NAME:-USD}"
SENDER_ACCOUNT="${SENDER_ACCOUNT:-sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D}"
RECIPIENT_ACCOUNT="${RECIPIENT_ACCOUNT:-sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB}"
ASSET_ID="${ASSET_ID:-${ASSET_DEFINITION_ID}#${SENDER_ACCOUNT}}"
MINT_QTY="${MINT_QTY:-100}"
XFER_QTY="${XFER_QTY:-25}"

if [ ! -x "$IROHA_BIN" ]; then
  echo "IROHA_BIN not found or not executable: $IROHA_BIN" >&2
  exit 1
fi

echo "[1/4] Register asset definition: $ASSET_DEFINITION_ID"
"$IROHA_BIN" --config defaults/client.toml asset definition register --id "$ASSET_DEFINITION_ID" --name "$ASSET_DEFINITION_NAME" --scale 0 || true

echo "[2/4] Mint $MINT_QTY to $SENDER_ACCOUNT"
"$IROHA_BIN" --config defaults/client.toml --torii-url "$TORII0" asset mint --id "$ASSET_ID" --quantity "$MINT_QTY"

echo "[3/4] Transfer $XFER_QTY to $RECIPIENT_ACCOUNT"
"$IROHA_BIN" --config defaults/client.toml --torii-url "$TORII0" asset transfer --id "$ASSET_ID" --to "$RECIPIENT_ACCOUNT" --quantity "$XFER_QTY"

echo "[4/4] Query recipient balance from node1 ($TORII1)"
"$IROHA_BIN" --config defaults/client.toml --torii-url "$TORII1" asset get --id "$ASSET_ID"
