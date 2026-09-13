#!/usr/bin/env bash
# Verify native SCCP artifacts and run audited EVM diagnostics. Requires Python 3,
# Node/npm, HTTPS and native x86-64 compiler execution (Rosetta on macOS arm64).
# SCCP_CORRIDOR_{PYTHON,NODE,NPM}_BIN select developer tools; optional
# SCCP_CONTRACT_ARTIFACT_DIR selects an already verified artifact directory.
# All generated tooling and compiler files live in a disposable private directory.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sccp-evm-smoke.XXXXXX")"
PYTHON_BIN="${SCCP_CORRIDOR_PYTHON_BIN:-python3}"
NODE_BIN="${SCCP_CORRIDOR_NODE_BIN:-node}"
NPM_BIN="${SCCP_CORRIDOR_NPM_BIN:-npm}"

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
    --output-dir "$ARTIFACT_DIR"
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

EVM_NATIVE_SOLC="$WORK_DIR/solc-evm"
"$PYTHON_BIN" scripts/contract_artifact_corridor.py materialize \
  --target evm --output "$EVM_NATIVE_SOLC"
MUTATED_NATIVE_SOLC="$WORK_DIR/mutated-solc-evm"
cp "$EVM_NATIVE_SOLC" "$MUTATED_NATIVE_SOLC"
chmod u+w "$MUTATED_NATIVE_SOLC"
"$PYTHON_BIN" - "$MUTATED_NATIVE_SOLC" <<'INNER_PY'
import sys
from pathlib import Path
path = Path(sys.argv[1])
path.write_bytes(path.read_bytes() + b"\n")
INNER_PY
"$PYTHON_BIN" - "$MUTATED_NATIVE_SOLC" <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, str(Path("scripts").resolve()))
import contract_artifact_corridor as corridor
config = corridor.load_corridor_config()
value, _ = corridor.standard_json_input(Path("."), config, "evm")
try:
    corridor.run_native_solc(Path(sys.argv[1]), config.compilers["evm"],
                             corridor.canonical_json_bytes(value))
except corridor.CorridorError as error:
    if "digest mismatch before execution" not in str(error):
        raise
else:
    raise SystemExit("mutated authenticated native compiler was accepted")
PY

cp -R scripts/contract_tooling "$WORK_DIR/contract_tooling"
(
  cd "$WORK_DIR/contract_tooling"
  "$NPM_BIN" ci --ignore-scripts --no-audit --no-fund --loglevel=error
  "$NPM_BIN" audit --omit=dev --audit-level=low
)
NODE_PATH="$WORK_DIR/contract_tooling/node_modules" \
SCCP_TVM_STATIC_ONLY=1 \
  "$NODE_BIN" scripts/contract_tvm_smoke.mjs \
    "$RUNTIME_MANIFEST" \
    fixtures/sccp/native_transfer_event_v1.json

(
  cd "$WORK_DIR/contract_tooling/evm-runtime"
  "$NPM_BIN" ci --ignore-scripts --no-audit --no-fund --loglevel=error
  "$NPM_BIN" audit --omit=dev --audit-level=low
)

echo "Running exact-manifest SCCP EVM runtime and test-only TRON compatibility smoke with authenticated native Solidity 0.7.6."
echo "Execution uses the locked native EDR runtime directly through its EIP-1193 provider."
NODE_PATH="$WORK_DIR/contract_tooling/evm-runtime/node_modules" \
  "$NODE_BIN" --test scripts/tests/contract_edr_provider_test.cjs
NODE_PATH="$WORK_DIR/contract_tooling/evm-runtime/node_modules" \
SCCP_NATIVE_SOLC_PATH="$EVM_NATIVE_SOLC" \
SCCP_CORRIDOR_PYTHON_BIN="$PYTHON_BIN" \
SCCP_CONTRACT_ARTIFACT_MANIFEST="$RUNTIME_MANIFEST" \
SCCP_CONTRACT_ARTIFACT_LOCK="$RUNTIME_ARTIFACT_LOCK" \
  "$NODE_BIN" contracts/evm/sccp/test/sccp_message_bridge_smoke.js

echo "Pinned native Solidity 0.7.6 compile and runtime smoke passed."
echo "Authenticated EVM and TRON compiler/artifact smoke passed."
echo "No EVM execution is accepted as TVM evidence; run the explicit real-TRE phase for release evidence."
