#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sccp-evm-smoke.XXXXXX")"

cleanup() {
  rm -rf "$WORK_DIR"
}

trap cleanup EXIT

mkdir -p "$WORK_DIR/solc-bin"

SOLC_VERSION="0.7.4"
GANACHE_VERSION="7.9.2"
ETHERS_VERSION="6.16.0"

npx --yes "solc@$SOLC_VERSION" --bin --base-path . -o "$WORK_DIR/solc-bin" \
  contracts/evm/sccp/SccpMessageBridge.sol \
  contracts/evm/sccp/SccpMessageBridgeDeployer.sol \
  contracts/evm/sccp/ISccpMessageVerifier.sol \
  contracts/evm/sccp/SccpGroth16Bn254MessageVerifier.sol \
  contracts/evm/sccp/SccpSecp256k1MessageVerifier.sol \
  contracts/evm/sccp/Ownable.sol \
  contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol \
  contracts/tron/sccp/SccpTronSourceBridge.sol \
  contracts/tron/sccp/TairaXOR.sol \
  contracts/tron/sccp/TairaXorSccpBridge.sol

npm --prefix "$WORK_DIR" install --no-save --no-package-lock --loglevel=error \
  "solc@$SOLC_VERSION" \
  "ganache@$GANACHE_VERSION" \
  "ethers@$ETHERS_VERSION"
NODE_PATH="$WORK_DIR/node_modules" node contracts/evm/sccp/test/sccp_message_bridge_smoke.js
