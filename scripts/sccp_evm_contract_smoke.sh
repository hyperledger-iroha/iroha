#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

cleanup() {
  rm -rf node_modules
}

trap cleanup EXIT

npx --yes solc@0.7.4 --bin --base-path . \
  contracts/evm/sccp/SccpMessageBridge.sol \
  contracts/evm/sccp/SccpMessageBridgeDeployer.sol \
  contracts/evm/sccp/ISccpMessageVerifier.sol \
  contracts/evm/sccp/SccpSecp256k1MessageVerifier.sol \
  contracts/evm/sccp/Ownable.sol

npm install --no-save --no-package-lock solc@0.7.4 ganache ethers
node contracts/evm/sccp/test/sccp_message_bridge_smoke.js
