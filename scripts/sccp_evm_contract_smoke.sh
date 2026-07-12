#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sccp-evm-smoke.XXXXXX")"
PYTHON_BIN="${SCCP_CORRIDOR_PYTHON_BIN:-python3}"
NODE_BIN="${SCCP_CORRIDOR_NODE_BIN:-node}"
NPM_BIN="${SCCP_CORRIDOR_NPM_BIN:-npm}"
PINNED_SOLC_BUILD="0.7.4+commit.3f05b770.Emscripten.clang"
PINNED_SOLC_URL="https://binaries.soliditylang.org/wasm/soljson-v0.7.4+commit.3f05b770.js"
PINNED_SOLC_SHA256="2b55ed5fec4d9625b6c7b3ab1abd2b7fb7dd2a9c68543bf0323db2c7e2d55af2"

cleanup() {
  rm -rf "$WORK_DIR"
}

trap cleanup EXIT

for retired_contract in \
  contracts/evm/sccp/SccpSecp256k1MessageVerifier.sol \
  contracts/evm/sccp/SccpMessageBridge.sol \
  contracts/evm/sccp/SccpMessageBridgeDeployer.sol \
  contracts/evm/sccp/SccpEvmSourceBridge.sol \
  contracts/evm/sccp/Ownable.sol \
  contracts/ethereum/sccp/SccpEthereumSourceBridge.sol \
  contracts/bsc/sccp/SccpBscSourceBridge.sol \
  contracts/tron/sccp/SccpTronSourceBridge.sol
do
  if [[ -e "$retired_contract" ]]; then
    echo "retired generic SCCP contract must remain deleted: $retired_contract" >&2
    exit 1
  fi
done

ARTIFACT_DIR="${SCCP_CONTRACT_ARTIFACT_DIR:-$WORK_DIR/artifacts}"
MANIFEST="$ARTIFACT_DIR/sccp-contract-artifacts-v1.json"
if [[ -f "$MANIFEST" && ! -L "$MANIFEST" ]]; then
  "$PYTHON_BIN" scripts/contract_artifact_corridor.py verify \
    --manifest "$MANIFEST" \
    --repo-root . \
    --check-source-inputs
else
  "$PYTHON_BIN" scripts/contract_artifact_corridor.py build \
    --repo-root . \
    --output-dir "$ARTIFACT_DIR" \
    --node "$NODE_BIN"
fi

# A reviewed artifact is deployable only while it remains byte-for-byte bound
# to this checkout. Exercise both mutation and source-staleness failures before
# the runtime process is allowed to create a provider.
MUTATED_MANIFEST="$WORK_DIR/mutated-artifact-manifest.json"
"$PYTHON_BIN" - "$MANIFEST" "$MUTATED_MANIFEST" <<'PY'
import copy
import sys
from pathlib import Path

sys.path.insert(0, str(Path("scripts").resolve()))
import contract_artifact_corridor as corridor

manifest = corridor.load_manifest(Path(sys.argv[1]))
mutated = copy.deepcopy(manifest)
record = mutated["targets"]["evm"]["contracts"][0]["creation_bytecode"]
record["hex"] = record["hex"] + "00"
corridor.write_canonical_file(Path(sys.argv[2]), mutated)
PY
if "$PYTHON_BIN" scripts/contract_artifact_corridor.py verify \
  --manifest "$MUTATED_MANIFEST" \
  --repo-root . \
  --check-source-inputs >/dev/null 2>&1
then
  echo "mutated SCCP artifact manifest was accepted" >&2
  exit 1
fi

STALE_REPO="$WORK_DIR/stale-checkout"
"$PYTHON_BIN" - "$STALE_REPO" <<'PY'
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path("scripts").resolve()))
import contract_artifact_corridor as corridor

destination = Path(sys.argv[1])
config = corridor.load_corridor_config()
for relative in sorted(set(config.sources["evm"] + config.sources["tron"])):
    target = destination / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(relative, target)
stale = destination / config.sources["evm"][0]
stale.write_bytes(stale.read_bytes() + b"\n// adversarial stale source\n")
PY
if "$PYTHON_BIN" scripts/contract_artifact_corridor.py verify \
  --manifest "$MANIFEST" \
  --repo-root "$STALE_REPO" \
  --check-source-inputs >/dev/null 2>&1
then
  echo "stale SCCP contract source was accepted by the artifact verifier" >&2
  exit 1
fi
RUNTIME_MANIFEST="$WORK_DIR/runtime-artifact-manifest.json"
cp "$MANIFEST" "$RUNTIME_MANIFEST"
chmod 0444 "$RUNTIME_MANIFEST"
RUNTIME_ARTIFACT_LOCK="$WORK_DIR/runtime-artifact-lock.json"
cp scripts/contract_tooling/artifact-lock.json "$RUNTIME_ARTIFACT_LOCK"
chmod 0444 "$RUNTIME_ARTIFACT_LOCK"

EVM_SOLJSON="$WORK_DIR/soljson-evm-0.7.4.js"
EVM_SOLJSON_SHA256="$("$PYTHON_BIN" - \
  "$EVM_SOLJSON" \
  "$PINNED_SOLC_URL" \
  "$PINNED_SOLC_SHA256" \
  "$PINNED_SOLC_BUILD" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, str(Path("scripts").resolve()))
import contract_artifact_corridor as corridor

compiler = corridor.CompilerSpec(
    target="evm-compatibility",
    identity="solc-evm-0.7.4+commit.3f05b770",
    reported_version=sys.argv[4],
    url=sys.argv[2],
    sha256=sys.argv[3],
)
corridor.materialize_verified_compiler(compiler, Path(sys.argv[1]))
print(compiler.sha256)
PY
)"
if ! [[ "$EVM_SOLJSON_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "authenticated EVM compiler receipt did not contain one SHA-256 digest" >&2
  exit 1
fi

MUTATED_SOLJSON="$WORK_DIR/mutated-soljson-evm-0.7.4.js"
cp "$EVM_SOLJSON" "$MUTATED_SOLJSON"
"$PYTHON_BIN" - "$MUTATED_SOLJSON" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
path.write_bytes(path.read_bytes() + b"\n")
PY
if SCCP_SOLJSON_PATH="$MUTATED_SOLJSON" \
  SCCP_SOLJSON_SHA256="$EVM_SOLJSON_SHA256" \
  "$NODE_BIN" -e 'require("./scripts/contract_tooling/authenticated-solc")' >/dev/null 2>&1
then
  echo "mutated authenticated Solidity compiler was accepted" >&2
  exit 1
fi

cp -R scripts/contract_tooling "$WORK_DIR/contract_tooling"
(
  cd "$WORK_DIR/contract_tooling/evm-runtime"
  "$NPM_BIN" ci --ignore-scripts --no-audit --no-fund --loglevel=error
  "$NPM_BIN" audit --omit=dev --audit-level=low
)

echo "Running exact-manifest SCCP EVM runtime and test-only TRON compatibility smoke with authenticated $PINNED_SOLC_BUILD."
echo "Execution uses the locked Hardhat runtime directly through its EIP-1193 provider."
NODE_PATH="$WORK_DIR/contract_tooling/evm-runtime/node_modules" \
SCCP_SOLJSON_PATH="$EVM_SOLJSON" \
SCCP_SOLJSON_SHA256="$EVM_SOLJSON_SHA256" \
SCCP_EXPECTED_SOLC_BUILD="$PINNED_SOLC_BUILD" \
SCCP_CONTRACT_ARTIFACT_MANIFEST="$RUNTIME_MANIFEST" \
SCCP_CONTRACT_ARTIFACT_LOCK="$RUNTIME_ARTIFACT_LOCK" \
  "$NODE_BIN" contracts/evm/sccp/test/sccp_message_bridge_smoke.js

echo "Pinned Solidity $PINNED_SOLC_BUILD compile and runtime smoke passed."
echo "Authenticated EVM and TRON compiler/artifact smoke passed."
echo "No EVM execution is accepted as TVM evidence; the separate real-TRE gate is mandatory."
