#!/usr/bin/env bash
# Reject weakened, disconnected, or vacuously proved timeout readiness guards.

set -euo pipefail

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
readonly CORE="${FORMAL_DIR}/SumeragiV2Core.tla"
readonly NETWORK="${FORMAL_DIR}/SumeragiV2AsyncNetwork.tla"
readonly PROOF="${FORMAL_DIR}/SumeragiV2BeginTimeoutReadyProofs.tla"
readonly CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_begin_timeout_ready_contract.py"

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-begin-timeout-ready.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

python3 "$CHECKER" "$CORE" "$NETWORK" "$PROOF"

python3 - "$CORE" "$NETWORK" "$PROOF" "$run_dir" <<'PY'
from pathlib import Path
import sys

core_path, network_path, proof_path, output_path = map(Path, sys.argv[1:])
output_path.mkdir(parents=True, exist_ok=True)
core = core_path.read_text(encoding="utf-8")
network = network_path.read_text(encoding="utf-8")
proof = proof_path.read_text(encoding="utf-8")


def replace_once(source: str, old: str, new: str, label: str) -> str:
    count = source.count(old)
    if count != 1:
        raise SystemExit(
            f"mutation fixture drift for {label}: expected one source fragment, "
            f"found {count}"
        )
    return source.replace(old, new, 1)


weak_ready = replace_once(
    core,
    "     /\\ NodeIdle(node)\n"
    "     /\\ NoDecisionForNode(node)\n"
    "     /\\ ~NodeTimedOut(node, roundView)\n",
    "     /\\ NodeIdle(node)\n"
    "     /\\ ~NodeTimedOut(node, roundView)\n",
    "weak-ready-missing-decision-guard",
)
(output_path / "weak-ready-core.tla").write_text(weak_ready, encoding="utf-8")

disconnected_network = replace_once(
    network,
    "BeginTimeoutEnabled(node) == BeginTimeoutReady(node)",
    "BeginTimeoutEnabled(node) == TRUE",
    "disconnected-scheduler-guard",
)
(output_path / "disconnected-network.tla").write_text(
    disconnected_network, encoding="utf-8"
)

bypassed_core = replace_once(
    core,
    "  IN /\\ BeginTimeoutReady(node)\n"
    "     /\\ pendingTimeout' = pendingTimeout \\cup {request}\n",
    "  IN /\\ TRUE\n"
    "     /\\ pendingTimeout' = pendingTimeout \\cup {request}\n",
    "bypassed-core-call-site",
)
(output_path / "bypassed-core.tla").write_text(bypassed_core, encoding="utf-8")

vacuous_proof = replace_once(
    proof,
    "BY ExpandENABLED, Isa\n"
    "   DEF BeginTimeoutReady, BeginTimeout, TimeoutRequestFor, vars\n",
    "BY TRUE\n",
    "vacuous-enabledness-proof",
)
(output_path / "vacuous-proof.tla").write_text(vacuous_proof, encoding="utf-8")
PY

run_rejected() {
  local label="$1"
  local core="$2"
  local network="$3"
  local proof="$4"
  local marker="$5"
  local stdout="${run_dir}/${label}.stdout"
  local stderr="${run_dir}/${label}.stderr"
  local status

  set +e
  python3 "$CHECKER" "$core" "$network" "$proof" >"$stdout" 2>"$stderr"
  status=$?
  set -e
  if [[ $status -eq 0 ]]; then
    echo "BeginTimeout readiness source mutation was accepted: ${label}" >&2
    exit 1
  fi
  if ! grep -Fq "$marker" "$stderr"; then
    echo "${label} missed expected rejection marker: ${marker}" >&2
    cat "$stderr" >&2
    exit 1
  fi
  echo "[source] ${label}: rejected"
}

run_rejected weak-ready-missing-decision-guard \
  "$run_dir/weak-ready-core.tla" "$NETWORK" "$PROOF" \
  "BeginTimeoutReady must retain the exact complete unprimed Core guard"
run_rejected disconnected-scheduler-guard \
  "$CORE" "$run_dir/disconnected-network.tla" "$PROOF" \
  "BeginTimeoutEnabled must equal only the shared BeginTimeoutReady(node) kernel"
run_rejected bypassed-core-call-site \
  "$run_dir/bypassed-core.tla" "$NETWORK" "$PROOF" \
  "BeginTimeout must consume BeginTimeoutReady as its first and only guard"
run_rejected vacuous-enabledness-proof \
  "$CORE" "$NETWORK" "$run_dir/vacuous-proof.tla" \
  "must retain the exact proof dependency"

echo "[source] exact BeginTimeout readiness rejects weak, disconnected, bypassed, and vacuous mutations"
