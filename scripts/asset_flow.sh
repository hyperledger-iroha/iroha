#!/usr/bin/env bash
set -euo pipefail

# Configuration
IROHA_BIN="${IROHA_BIN:-target/release/iroha}"
TORII0="${TORII0:-http://127.0.0.1:8080/}"
TORII1="${TORII1:-http://127.0.0.1:8081/}"
ASSET_ID="${ASSET_ID:-testasset#wonderland}"
SENDER_ACCOUNT="${SENDER_ACCOUNT:-ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland}"
RECIPIENT_ACCOUNT="${RECIPIENT_ACCOUNT:-ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016@wonderland}"
MINT_QTY="${MINT_QTY:-100}"
XFER_QTY="${XFER_QTY:-25}"

if [ ! -x "$IROHA_BIN" ]; then
  echo "IROHA_BIN not found or not executable: $IROHA_BIN" >&2
  exit 1
fi

echo "[1/4] Register asset definition: $ASSET_ID"
"$IROHA_BIN" --config defaults/client.toml asset definition register --id "$ASSET_ID" -t Numeric || true

echo "[2/4] Mint $MINT_QTY to $SENDER_ACCOUNT"
"$IROHA_BIN" --config defaults/client.toml --torii-url "$TORII0" asset mint --id "$ASSET_ID#$SENDER_ACCOUNT" --quantity "$MINT_QTY"

echo "[3/4] Transfer $XFER_QTY to $RECIPIENT_ACCOUNT"
"$IROHA_BIN" --config defaults/client.toml --torii-url "$TORII0" asset transfer --id "$ASSET_ID#$SENDER_ACCOUNT" --to "$RECIPIENT_ACCOUNT" --quantity "$XFER_QTY"

echo "[4/4] Query recipient balance from node1 ($TORII1)"
"$IROHA_BIN" --config defaults/client.toml --torii-url "$TORII1" asset get --id "$ASSET_ID#$RECIPIENT_ACCOUNT"
