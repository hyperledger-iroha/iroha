#!/usr/bin/env bash
# SCCP v1 EVM/TRON contract smoke (`specs/sccp.md` §5.2, §5.5, §11).
#
# Builds `contracts/evm/sccp/SccpTairaXor.sol` with the pinned native Solidity
# 0.8.31 compilers (solc for ETH/BSC, tronprotocol tv_0.8.31 for TRON) through
# `scripts/contract_artifact_corridor.py`, proves that a mutated manifest, a
# stale source and a mutated compiler are refused, installs and audits the
# locked EDR runtime in a private copy, and runs the EDR suite under chain ids
# 1, 56 and 0x2b6653dc. TRON-compiler bytecode itself is qualified on java-tron
# (TRE) separately; nothing here is TVM evidence.
#
# Prerequisites: Python 3.9+, Node.js >= 22 with npm, HTTPS for the first
# compiler download (cached by SHA-256 under target/sccp-contract-tooling).
# Runs natively on macOS arm64/x86-64 and Linux x86-64/arm64; no Rosetta or
# Docker. Developer overrides: SCCP_CORRIDOR_{PYTHON,NODE,NPM}_BIN select the
# tool binaries. All generated files live in a disposable private directory.
set -euo pipefail

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PYTHON_BIN="${SCCP_CORRIDOR_PYTHON_BIN:-python3}"
NODE_BIN="${SCCP_CORRIDOR_NODE_BIN:-node}"
NPM_BIN="${SCCP_CORRIDOR_NPM_BIN:-npm}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sccp-evm-smoke.XXXXXX")"
chmod 700 "$WORK_DIR"

cleanup() {
  chmod -R u+w "$WORK_DIR" >/dev/null 2>&1 || true
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

shopt -s nullglob
sources=(contracts/evm/sccp/*.sol)
shopt -u nullglob
if [[ "${sources[*]}" != "contracts/evm/sccp/SccpTairaXor.sol" ]]; then
  echo "contracts/evm/sccp must contain exactly SccpTairaXor.sol, found: ${sources[*]}" >&2
  exit 1
fi

ARTIFACT_DIR="$WORK_DIR/artifacts"
MANIFEST="$ARTIFACT_DIR/sccp-contract-artifacts-v1.json"
"$PYTHON_BIN" scripts/contract_artifact_corridor.py build --output-dir "$ARTIFACT_DIR"
"$PYTHON_BIN" scripts/contract_artifact_corridor.py verify --manifest "$MANIFEST"

# A manifest is usable only while it is byte-for-byte bound to the locks and to
# this checkout: a mutated artifact and a stale source must both be refused.
MUTATED_MANIFEST="$WORK_DIR/mutated-manifest.json"
"$PYTHON_BIN" - "$MANIFEST" "$MUTATED_MANIFEST" <<'PY'
import copy
import sys
from pathlib import Path

sys.path.insert(0, str(Path("scripts").resolve()))
import contract_artifact_corridor as corridor

manifest = corridor.load_manifest(Path(sys.argv[1]))
mutated = copy.deepcopy(manifest)
record = mutated["targets"]["tron"]["contracts"][0]["runtime_bytecode"]
record["hex"] = record["hex"][:-2] + ("00" if not record["hex"].endswith("00") else "01")
corridor.write_canonical_file(Path(sys.argv[2]), mutated)
PY
if "$PYTHON_BIN" scripts/contract_artifact_corridor.py verify --manifest "$MUTATED_MANIFEST" >/dev/null 2>&1; then
  echo "a mutated SCCP artifact manifest was accepted" >&2
  exit 1
fi

STALE_REPO="$WORK_DIR/stale-checkout"
mkdir -p "$STALE_REPO/contracts/evm/sccp"
cp contracts/evm/sccp/SccpTairaXor.sol "$STALE_REPO/contracts/evm/sccp/SccpTairaXor.sol"
printf '\n// adversarial stale source\n' >>"$STALE_REPO/contracts/evm/sccp/SccpTairaXor.sol"
if "$PYTHON_BIN" scripts/contract_artifact_corridor.py verify \
  --manifest "$MANIFEST" --repo-root "$STALE_REPO" >/dev/null 2>&1
then
  echo "a stale SCCP contract source was accepted by the artifact verifier" >&2
  exit 1
fi

MUTATED_SOLC="$WORK_DIR/mutated-solc"
"$PYTHON_BIN" scripts/contract_artifact_corridor.py materialize --target evm --output "$WORK_DIR/solc-evm" >/dev/null
cp "$WORK_DIR/solc-evm" "$MUTATED_SOLC"
chmod u+w "$MUTATED_SOLC"
printf '\n' >>"$MUTATED_SOLC"
"$PYTHON_BIN" - "$MUTATED_SOLC" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, str(Path("scripts").resolve()))
import contract_artifact_corridor as corridor

config = corridor.load_corridor_config()
value, _ = corridor.standard_json_input(Path("."), config, "evm")
try:
    corridor.run_native_solc(Path(sys.argv[1]), config.compilers["evm"], corridor.canonical_json_bytes(value))
except corridor.CorridorError as error:
    if "digest mismatch before execution" not in str(error):
        raise
else:
    raise SystemExit("a mutated authenticated native compiler was accepted")
PY

# Install and audit the locked EDR runtime in a private copy.
RUNTIME_DIR="$WORK_DIR/evm-runtime"
mkdir -p "$RUNTIME_DIR"
cp scripts/contract_tooling/evm-runtime/package.json \
  scripts/contract_tooling/evm-runtime/package-lock.json \
  scripts/contract_tooling/evm-runtime/edr-provider.js \
  scripts/contract_tooling/evm-runtime/evm-errors.js \
  "$RUNTIME_DIR/"
(
  cd "$RUNTIME_DIR"
  "$NPM_BIN" ci --ignore-scripts --no-audit --no-fund --loglevel=error
  "$NPM_BIN" audit --omit=dev --audit-level=low
)

NODE_PATH="$RUNTIME_DIR/node_modules" "$NODE_BIN" --test scripts/tests/contract_edr_provider_test.cjs

echo "Running the SccpTairaXor EDR suite (chain ids 1, 56 and 0x2b6653dc) on the locked native EDR runtime."
GAS_REPORT="$WORK_DIR/gas.json"
SCCP_EVM_RUNTIME_DIR="$RUNTIME_DIR" \
SCCP_CONTRACT_ARTIFACT_MANIFEST="$MANIFEST" \
SCCP_CORRIDOR_PYTHON_BIN="$PYTHON_BIN" \
SCCP_GAS_REPORT="$GAS_REPORT" \
  "$NODE_BIN" --test contracts/evm/sccp/test/sccp_taira_xor.test.js

"$PYTHON_BIN" - "$GAS_REPORT" <<'PY'
import json
import sys

report = json.load(open(sys.argv[1], encoding="utf-8"))
required = (
    "finalizeFromTaira n=4 t=3 (new balance, same bitmap word)",
    "finalizeFromTaira n=31 t=21 (new balance, same bitmap word)",
    "applyControl n=4 t=3",
    "applyControl n=31 t=21",
    "rotateRosters 1 rotation n=4 t=3 (steady state)",
    "rotateRosters 1 rotation n=31 t=21 (steady state)",
    "voidExpired n=4 t=3",
    "voidFrozen 256 nonces (one bitmap word)",
    "transferToTaira (34-byte Taira recipient)",
)
missing = [label for label in required if set(report.get(label, {})) != {"ethereum", "bsc", "tron"}]
if missing:
    raise SystemExit(f"measured gas is missing for: {missing}")
PY

echo "SCCP v1 EVM contract smoke passed: authenticated native builds, fail-closed verification and the EDR suite."
echo "TRON bytecode from tv_0.8.31 still requires the separate java-tron (TRE) qualification."
