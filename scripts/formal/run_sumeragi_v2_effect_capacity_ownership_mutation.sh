#!/usr/bin/env bash
set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:-${REPO_ROOT}/target/tla2tools/${TLA2TOOLS_VERSION}/tla2tools.jar}"
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

if (($#)); then
  echo "usage: $0" >&2
  exit 2
fi

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

[[ -f "$TLA2TOOLS_JAR" ]] || {
  echo "pinned TLA2Tools v${TLA2TOOLS_VERSION} is required at ${TLA2TOOLS_JAR}" >&2
  exit 1
}
actual_sha256="$(hash_file "$TLA2TOOLS_JAR")"
[[ "$actual_sha256" == "$TLA2TOOLS_SHA256" ]] || {
  echo "TLA2Tools checksum mismatch" >&2
  echo "expected: ${TLA2TOOLS_SHA256}" >&2
  echo "actual:   ${actual_sha256}" >&2
  exit 1
}
"$JAVA_BIN" -version >/dev/null 2>&1 || {
  echo "a working Java runtime is required" >&2
  exit 1
}

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-effect-capacity.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 96 -seed 139154308881391968 -coverage 1
)

run_case() {
  local label="$1"
  local module="$2"
  local config="$3"
  local expected_status="$4"
  shift 4
  local log="${run_dir}/${label}.log"
  local actual_status
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "${run_dir}/${label}" \
      -config "$config" "$module"
  ) >"$log" 2>&1
  actual_status=$?
  set -e
  if [[ "$actual_status" -ne "$expected_status" ]]; then
    echo "${label} returned TLC status ${actual_status}, expected ${expected_status}" >&2
    cat "$log" >&2
    exit 1
  fi
  for marker in "$@"; do
    if ! grep -Fq "$marker" "$log"; then
      echo "${label} missed expected marker: ${marker}" >&2
      cat "$log" >&2
      exit 1
    fi
  done
  echo "[tlc] ${label}: expected status ${expected_status}"
}

readonly OWNERSHIP_MODULE="SumeragiV2EffectCapacityOwnershipMutation.tla"
run_case timeout-sign-lost "$OWNERSHIP_MODULE" \
  effect_capacity_timeout_sign_lost_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant PersistedTimeoutSignHasOwner is violated." \
  "State 4: <PersistTimeoutVote" \
  "4 states generated, 4 distinct states found, 0 states left on queue."

run_case timeout-sign-refill-starvation "$OWNERSHIP_MODULE" \
  effect_capacity_timeout_sign_refill_bug.cfg 13 \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "Back to state 5" \
  "7 states generated, 6 distinct states found, 0 states left on queue."

run_case timeout-sign-retained-and-protected "$OWNERSHIP_MODULE" \
  effect_capacity_timeout_sign_fixed.cfg 0 \
  "TLC2 Version 2.19" \
  "Model checking completed. No error has been found." \
  "<AdmitProposalAFetch" \
  "<AdmitDistinctPrepareQCBFetch" \
  "<PersistTimeoutVote" \
  "<DrainRetainedSignByPreemption" \
  "5 states generated, 5 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 5"

readonly CERTIFIED_REQUEST_MODULE="SumeragiV2CertifiedRequestCapacityMutation.tla"
run_case certified-request-retained-owner-drop "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_request_lost_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant RetainedFetchBIsNotDropped is violated." \
  "State 2: <RetainCapacityBlockedFetchB" \
  "retainedEffects = <<>>"

run_case certified-request-retained-owner-substitution "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_request_substitute_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant RetainedFetchBHasExactAuthorityAndTask is violated." \
  "State 2: <RetainCapacityBlockedFetchB"

run_case certified-request-retained-owner-duplication "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_request_duplicate_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant RetainedFetchBHasOneOwner is violated." \
  "State 2: <RetainCapacityBlockedFetchB"

run_case certified-request-retained-owner-overtake "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_request_overtake_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant RetainedFetchBRemainsFifoHead is violated." \
  "State 2: <RetainCapacityBlockedFetchB"

run_case certified-request-capacity-fatal "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_request_fatal_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant CertifiedRequestPressureIsNonfatal is violated." \
  "State 2: <RetainCapacityBlockedFetchB" \
  "fatal = TRUE"

run_case certified-response-count-reserve-missing "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_response_count_reserve_bug.cfg 13 \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "State 2: <RetainCapacityBlockedFetchB" \
  "outerGenericCountOwned = TRUE" \
  "responseAAdmitted = FALSE" \
  "State 3: Stuttering"

run_case certified-response-byte-reserve-missing "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_response_byte_reserve_bug.cfg 13 \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "State 2: <RetainCapacityBlockedFetchB" \
  "outerGenericBytesOwned = TRUE" \
  "responseAAdmitted = FALSE" \
  "State 3: Stuttering"

run_case certified-response-blocked-by-unrelated-retained-debt "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_response_blocked_bug.cfg 13 \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "State 2: <RetainCapacityBlockedFetchB" \
  "unrelatedRetainedT = TRUE" \
  "fatal = FALSE" \
  "State 3: <AdmitOuterTransportResponseA" \
  "responseAQueued = TRUE" \
  "State 4: Stuttering"

run_case certified-request-partial-pq-drain "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_request_partial_pq_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant DrainRetainedFetchBIsAtomic is violated." \
  "<DrainRetainedFetchB"

run_case certified-request-retained-owner-drains-atomically "$CERTIFIED_REQUEST_MODULE" \
  effect_capacity_certified_request_fixed.cfg 0 \
  "TLC2 Version 2.19" \
  "Finished computing initial states: 3 distinct states generated" \
  "Model checking completed. No error has been found." \
  "<RetainCapacityBlockedFetchB" \
  "<AdmitOuterTransportResponseA" \
  "<ConsumeTransportOnlyResponseA" \
  "<ReleaseOrdinaryWorkCapacityA" \
  "<DrainRetainedFetchB"

readonly OUTER_TRANSPORT_MODULE="SumeragiV2EffectCapacityOuterTransportMutation.tla"
run_case outer-certified-response-classification-missing "$OUTER_TRANSPORT_MODULE" \
  effect_capacity_outer_transport_response_class_bug.cfg 13 \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  'completionKind = "CertifiedBodyResponse"' \
  "State 2: Stuttering" \
  "4 states generated, 4 distinct states found, 0 states left on queue."

run_case outer-payload-chunk-classification-missing "$OUTER_TRANSPORT_MODULE" \
  effect_capacity_outer_transport_chunk_class_bug.cfg 13 \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  'completionKind = "PayloadChunk"' \
  "State 2: Stuttering" \
  "4 states generated, 4 distinct states found, 0 states left on queue."

run_case outer-transport-completion-shared-reserve "$OUTER_TRANSPORT_MODULE" \
  effect_capacity_outer_transport_class_fixed.cfg 0 \
  "TLC2 Version 2.19" \
  "Finished computing initial states: 2 distinct states generated" \
  "Model checking completed. No error has been found." \
  "<AdmitTransportCompletion" \
  "<ConsumeTransportCompletion" \
  "6 states generated, 6 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 3"

readonly RETIREMENT_MODULE="SumeragiV2EffectCapacityRetirementMutation.tla"
for blocking_kind in decided non-fetch; do
  config_kind="${blocking_kind//-/_}"
  run_case "${blocking_kind}-work-fair-retirement" "$RETIREMENT_MODULE" \
    "effect_capacity_${config_kind}_retirement_fixed.cfg" 0 \
    "TLC2 Version 2.19" \
    "Model checking completed. No error has been found." \
    "<AttemptGenuinelyNewFetchAtFullCapacity" \
    "<PersistTimeoutVoteSign" \
    "<FairlyRetireTerminatingOwner" \
    "<AdmitRetainedTimeoutVoteSign" \
    "5 states generated, 5 distinct states found, 0 states left on queue." \
    "depth of the complete state graph search is 5"
done

run_case full-capacity-fetch-head-of-line-mutant "$RETIREMENT_MODULE" \
  effect_capacity_full_fetch_hol_bug.cfg 12 \
  "TLC2 Version 2.19" \
  "Invariant FullCapacityFetchRemainsMissingReconstructibleAndUnqueued is violated." \
  "State 2: <AttemptGenuinelyNewFetchAtFullCapacity" \
  'retainedEffects = <<"NewMissingFetch">>' \
  "2 states generated, 2 distinct states found, 0 states left on queue."

run_case terminating-work-retirement-disabled "$RETIREMENT_MODULE" \
  effect_capacity_retirement_disabled_bug.cfg 13 \
  "TLC2 Version 2.19" \
  "Temporal properties were violated." \
  "State 3: <PersistTimeoutVoteSign" \
  'retainedEffects = <<"TimeoutVoteSign">>' \
  "State 4: Stuttering" \
  "3 states generated, 3 distinct states found, 0 states left on queue."

readonly PRIORITY_MODULE="SumeragiV2EffectPreemptionPriorityMutation.tla"
run_case class-and-work-id-priority-permutations "$PRIORITY_MODULE" \
  effect_preemption_priority_fixed.cfg 0 \
  "TLC2 Version 2.19" \
  "Finished computing initial states: 6 distinct states generated" \
  "Model checking completed. No error has been found." \
  "<PreemptForDurableSign" \
  "<RetainSignBehindDecidedFetch" \
  "36 states generated, 36 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 6"

run_case wrong-fetch-class-priority "$PRIORITY_MODULE" \
  effect_preemption_wrong_class_bug.cfg 12 \
  "Invariant CancellationPrefixMatchesClassAndWorkId is violated." \
  "State 2: <PreemptForDurableSign" \
  'cancelledNames = <<"LockedFetch">>' \
  "7 states generated, 7 distinct states found, 5 states left on queue."

run_case wrong-same-class-work-id "$PRIORITY_MODULE" \
  effect_preemption_wrong_work_id_bug.cfg 12 \
  "Invariant CancellationPrefixMatchesClassAndWorkId is violated." \
  "State 2: <PreemptForDurableSign" \
  'cancelledNames = <<"SpeculativeNew">>' \
  "7 states generated, 7 distinct states found, 5 states left on queue."

run_case decided-fetch-victim "$PRIORITY_MODULE" \
  effect_preemption_decided_victim_bug.cfg 12 \
  "Invariant DecidedFetchNeverPreempted is violated." \
  "State 2: <PreemptForDurableSign" \
  'cancelledNames = <<"DecidedFetch">>' \
  "7 states generated, 7 distinct states found, 5 states left on queue."

readonly BATCH_MODULE="SumeragiV2RetainedEffectBatchMutation.tla"
run_case retained-partial-fifo "$BATCH_MODULE" \
  effect_batch_partial_fifo_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "<InstallPartialFifoBatch" \
  "<DrainAvailablePrefixUntilCapacity" \
  "<FairlyRetireInitialWork" \
  "<DrainRetainedFifoTail" \
  "5 states generated, 5 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 5"

run_case retained-partial-fifo-reversed "$BATCH_MODULE" \
  effect_batch_partial_fifo_reverse_bug.cfg 12 \
  "Invariant PartialDrainIsExactFifoPrefix is violated." \
  "State 3: <DrainAvailablePrefixUntilCapacity" \
  'dispatchedEffects = <<"EquivocationReport">>' \
  "3 states generated, 3 distinct states found, 0 states left on queue."

run_case retained-second-batch-rejected "$BATCH_MODULE" \
  effect_batch_second_rejected_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "<InstallBlockedTailForSecondBatch" \
  "<AttemptOvertakingSecondBatch" \
  "3 states generated, 3 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 3"

run_case retained-second-batch-overtakes "$BATCH_MODULE" \
  effect_batch_second_accepted_bug.cfg 12 \
  "Invariant SecondBatchRejectedBeforeTailMutation is violated." \
  "State 3: <AttemptOvertakingSecondBatch" \
  'retainedEffects = <<"OvertakingBroadcast">>' \
  "3 states generated, 3 distinct states found, 0 states left on queue."

run_case retained-decision-filter "$BATCH_MODULE" \
  effect_batch_decision_filter_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "<InstallDecisionBlockedBatch" \
  "<InstallDecisionAndFilterRetainedTail" \
  "<DrainDecisionSurvivors" \
  "4 states generated, 4 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 4"

run_case retained-decision-filter-omitted "$BATCH_MODULE" \
  effect_batch_decision_no_filter_bug.cfg 12 \
  "Invariant DecisionTailContainsExactlySurvivors is violated." \
  "State 3: <InstallDecisionAndFilterRetainedTail" \
  '"StaleProposalBroadcast"' \
  "3 states generated, 3 distinct states found, 0 states left on queue."

run_case retained-source-bound "$BATCH_MODULE" \
  effect_batch_bound_fixed.cfg 0 \
  "Model checking completed. No error has been found." \
  "<InstallMaximumSizedBatch" \
  "<DrainMaximumSizedBatch" \
  "<AttemptOversizeBatch" \
  "4 states generated, 4 distinct states found, 0 states left on queue." \
  "depth of the complete state graph search is 4"

run_case retained-oversize-installed "$BATCH_MODULE" \
  effect_batch_oversize_accepted_bug.cfg 12 \
  "Invariant RetainedBatchWithinSourceBound is violated." \
  "State 4: <AttemptOversizeBatch" \
  "4 states generated, 4 distinct states found, 0 states left on queue."

echo "[tlc] a persisted TimeoutVote Sign without a retained owner fails immediately"
echo "[tlc] fair Fetch retirement plus unprotected refill has a capacity-full lasso"
echo "[tlc] bounded FIFO priority and deterministic reconstructible-Fetch preemption force rank descent and Sign admission"
echo "[tlc] Q-full Fetch B remains reconstructible Missing debt without partial P/Q/T ownership"
echo "[tlc] transport-only response A crosses unrelated retained T and releases exact P/Q ownership"
echo "[tlc] periodic retransmission atomically installs or upgrades Fetch B after request capacity is released"
echo "[tlc] certified responses and payload chunks share one independent per-validator outer transport-completion reserve"
echo "[tlc] decided/non-Fetch terminating owners retire fairly while full-capacity Fetch remains reconstructible Missing debt"
echo "[tlc] six permutations enforce stable (class, work_id) preemption and decided-owner exclusion"
echo "[tlc] retained suffix partial drain, source bound, overtaking rejection, and Decision filtering passed their mutation matrix"
