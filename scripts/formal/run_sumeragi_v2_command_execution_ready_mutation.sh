#!/usr/bin/env bash
# Reject missing, reordered, tautological, or disconnected command readiness.

set -euo pipefail

readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly NETWORK="${FORMAL_DIR}/SumeragiV2AsyncNetwork.tla"
readonly PROOF="${FORMAL_DIR}/SumeragiV2CommandExecutionReadyProofs.tla"
readonly REGULAR_FRAMED_HELPER="${FORMAL_DIR}/SumeragiV2RegularCommandFramedReadyProofs.tla"
readonly REGULAR_HELPER="${FORMAL_DIR}/SumeragiV2RegularCommandExecutionReadyProofs.tla"
readonly NON_REGULAR_HELPER="${FORMAL_DIR}/SumeragiV2NonRegularCommandExecutionReadyProofs.tla"
readonly CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_command_execution_ready_contract.py"

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-command-ready.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

python3 "$CHECKER" "$NETWORK" "$PROOF"

python3 - "$NETWORK" "$PROOF" "$REGULAR_FRAMED_HELPER" \
  "$REGULAR_HELPER" "$NON_REGULAR_HELPER" "$run_dir" <<'PY'
from pathlib import Path
import sys

(
    network_path,
    proof_path,
    regular_framed_helper_path,
    regular_helper_path,
    non_regular_helper_path,
    output_path,
) = map(Path, sys.argv[1:])
output_path.mkdir(parents=True, exist_ok=True)
network = network_path.read_text(encoding="utf-8")
proof = proof_path.read_text(encoding="utf-8")
regular_framed_helper = regular_framed_helper_path.read_text(encoding="utf-8")
regular_helper = regular_helper_path.read_text(encoding="utf-8")
non_regular_helper = non_regular_helper_path.read_text(encoding="utf-8")

for helper_path, helper in (
    (regular_framed_helper_path, regular_framed_helper),
    (regular_helper_path, regular_helper),
    (non_regular_helper_path, non_regular_helper),
):
    (output_path / helper_path.name).write_text(helper, encoding="utf-8")


def replace_once(source: str, old: str, new: str, label: str) -> str:
    count = source.count(old)
    if count != 1:
        raise SystemExit(
            f"mutation fixture drift for {label}: expected one source fragment, "
            f"found {count}"
        )
    return source.replace(old, new, 1)


omitted = replace_once(
    network,
    "    \\/ ExecuteDecisionFetchReady(selectedCommand)\n",
    "",
    "omitted-ready-arm",
)
(output_path / "omitted-ready-arm.tla").write_text(omitted, encoding="utf-8")

swapped_ready = replace_once(
    network,
    "    \\/ ExecuteSignProposalReady(selectedCommand)\n"
    "    \\/ ExecuteSignVoteReady(selectedCommand)\n",
    "    \\/ ExecuteSignVoteReady(selectedCommand)\n"
    "    \\/ ExecuteSignProposalReady(selectedCommand)\n",
    "swapped-ready-arms",
)
(output_path / "swapped-ready-arms.tla").write_text(
    swapped_ready, encoding="utf-8"
)

swapped_executor = replace_once(
    network,
    "  \\/ ExecuteApply(command)\n"
    "  \\/ ExecuteCoreDelivery(command)\n",
    "  \\/ ExecuteCoreDelivery(command)\n"
    "  \\/ ExecuteApply(command)\n",
    "swapped-executor-arms",
)
(output_path / "swapped-executor-arms.tla").write_text(
    swapped_executor, encoding="utf-8"
)

tautology = replace_once(
    network,
    "ExecuteDecisionFetchReady(command) ==\n"
    "  CertifiedRecoveryFetchFrontier(command)",
    "ExecuteDecisionFetchReady(command) == TRUE",
    "tautological-ready-arm",
)
(output_path / "tautological-ready-arm.tla").write_text(
    tautology, encoding="utf-8"
)

ownerless_core_delivery = replace_once(
    network,
    "ExecuteCoreDeliveryReady(command) ==\n"
    "  LET item == command.item\n"
    "  IN /\\ item \\in asyncSentItems\n"
    "     /\\ AsyncControlServiceOccurrenceIsCurrentOwner(item)\n",
    "ExecuteCoreDeliveryReady(command) ==\n"
    "  LET item == command.item\n"
    "  IN /\\ item \\in asyncSentItems\n",
    "ownerless-core-delivery-ready",
)
(output_path / "ownerless-core-delivery-ready.tla").write_text(
    ownerless_core_delivery, encoding="utf-8"
)

disconnected = replace_once(
    network,
    "  /\\ CandidateConsumerCurrent(command)\n"
    "  /\\ CommandExecutionReady(command)\n"
    "  /\\ (NodeIdle(command.node)\n"
    "        \\/ command.class = \"Completion\"\n"
    "        \\/ LocalAssemblyBusyDispatchAllowed(command))",
    "  /\\ CandidateConsumerCurrent(command)\n"
    "  /\\ TRUE\n"
    "  /\\ (NodeIdle(command.node)\n"
    "        \\/ command.class = \"Completion\"\n"
    "        \\/ LocalAssemblyBusyDispatchAllowed(command))",
    "disconnected-dispatch-call",
)
(output_path / "disconnected-dispatch-call.tla").write_text(
    disconnected, encoding="utf-8"
)

vacuous_proof = replace_once(
    proof,
    "    CommandExecutionReady(command) <=> ENABLED ExecuteCommand(command)\n",
    "    CommandExecutionReady(command) <=> TRUE\n",
    "vacuous-aggregate-proof",
)
(output_path / "vacuous-aggregate-proof.tla").write_text(
    vacuous_proof, encoding="utf-8"
)

vacuous_arm_proof = replace_once(
    proof,
    "BY ExecuteDecisionFetchReadyIffEnabledComposed\n",
    "BY TRUE\n",
    "vacuous-arm-proof",
)
(output_path / "vacuous-arm-proof.tla").write_text(
    vacuous_arm_proof, encoding="utf-8"
)

broken_cardinality_base = replace_once(
    proof,
    "    BY FS_Singleton\n",
    "    BY TRUE\n",
    "broken-cardinality-base",
)
(output_path / "broken-cardinality-base.tla").write_text(
    broken_cardinality_base, encoding="utf-8"
)

vacuous_helper_dir = output_path / "vacuous-composed-helper"
vacuous_helper_dir.mkdir()
(vacuous_helper_dir / proof_path.name).write_text(proof, encoding="utf-8")
(vacuous_helper_dir / regular_framed_helper_path.name).write_text(
    regular_framed_helper, encoding="utf-8"
)
(vacuous_helper_dir / regular_helper_path.name).write_text(
    regular_helper, encoding="utf-8"
)
vacuous_composed_helper = replace_once(
    non_regular_helper,
    "BY ExecuteDecisionFetchReadyImpliesEnabled,\n"
    "   ExecuteDecisionFetchEnabledImpliesReady\n",
    "BY TRUE\n",
    "vacuous-composed-helper",
)
(vacuous_helper_dir / non_regular_helper_path.name).write_text(
    vacuous_composed_helper, encoding="utf-8"
)
PY

run_rejected() {
  local label="$1"
  local network="$2"
  local proof="$3"
  local marker="$4"
  local stdout="${run_dir}/${label}.stdout"
  local stderr="${run_dir}/${label}.stderr"
  local status

  set +e
  python3 "$CHECKER" "$network" "$proof" >"$stdout" 2>"$stderr"
  status=$?
  set -e
  if [[ $status -eq 0 ]]; then
    echo "CommandExecutionReady source mutation was accepted: ${label}" >&2
    exit 1
  fi
  if ! grep -Fq "$marker" "$stderr"; then
    echo "${label} missed expected rejection marker: ${marker}" >&2
    cat "$stderr" >&2
    exit 1
  fi
  echo "[source] ${label}: rejected"
}

run_rejected omitted-ready-arm \
  "$run_dir/omitted-ready-arm.tla" "$PROOF" \
  "CommandExecutionReady must retain the exact 13-arm canonical order"
run_rejected swapped-ready-arms \
  "$run_dir/swapped-ready-arms.tla" "$PROOF" \
  "CommandExecutionReady must retain the exact 13-arm canonical order"
run_rejected swapped-executor-arms \
  "$run_dir/swapped-executor-arms.tla" "$PROOF" \
  "ExecuteCommand must retain the matching exact 13-arm canonical order"
run_rejected tautological-ready-arm \
  "$run_dir/tautological-ready-arm.tla" "$PROOF" \
  "ExecuteDecisionFetchReady must be a non-tautological pure guard"
run_rejected ownerless-core-delivery-ready \
  "$run_dir/ownerless-core-delivery-ready.tla" "$PROOF" \
  "ExecuteCoreDeliveryReady must retain its exact normalized production guard body"
run_rejected disconnected-dispatch-call \
  "$run_dir/disconnected-dispatch-call.tla" "$PROOF" \
  "CommandDispatchable must call the exact pure CommandExecutionReady kernel once"
run_rejected vacuous-aggregate-proof \
  "$NETWORK" "$run_dir/vacuous-aggregate-proof.tla" \
  "must state only the bidirectional pure-readiness/ENABLED equivalence"
run_rejected vacuous-arm-proof \
  "$NETWORK" "$run_dir/vacuous-arm-proof.tla" \
  "ExecuteDecisionFetchReadyIffEnabled must be the exact source-fidelity alias"
run_rejected broken-cardinality-base \
  "$NETWORK" "$run_dir/broken-cardinality-base.tla" \
  "arm-domain theorem must retain its exact 13-member finite-set induction"
run_rejected vacuous-composed-helper \
  "$NETWORK" \
  "$run_dir/vacuous-composed-helper/SumeragiV2CommandExecutionReadyProofs.tla" \
  "ExecuteDecisionFetchReadyIffEnabledComposed must compose the exact two directional production proofs"

echo "[source] exact 13-arm readiness rejects omissions, swaps, tautologies, ownerless Core delivery, disconnected callers, broken cardinality induction, and vacuous arm/helper/aggregate proofs"
