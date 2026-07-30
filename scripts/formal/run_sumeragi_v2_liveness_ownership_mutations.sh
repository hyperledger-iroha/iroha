#!/usr/bin/env bash
# Run the exact-ingress and adequate-leader ownership mutation corpus.

set -euo pipefail

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly EXPECTED_JAVA_VERSION='openjdk version "21.0.12"'
readonly TLC_FINISHED_PATTERN='^Finished in (([0-9]+d )?([0-9]+h )?([0-9]+min )?[0-9]+(ms|s)|([0-9]+d )?([0-9]+h )?[0-9]+min|([0-9]+d )?[0-9]+h|[0-9]+d) at \([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}\)$'
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/docs/formal/sumeragi_v2"
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
java_version="$($JAVA_BIN -version 2>&1)"
grep -Fq "$EXPECTED_JAVA_VERSION" <<<"$java_version" || {
  echo "frozen Java 21.0.12 is required" >&2
  printf '%s\n' "$java_version" >&2
  exit 1
}

run_dir="$(
  mktemp -d "${TMPDIR:-/tmp}/sumeragi-liveness-ownership-mutations.XXXXXX"
)"
trap 'rm -rf -- "$run_dir"' EXIT

models=(
  SumeragiV2LocalIngressSchedulerReservationMutation.tla
  SumeragiV2RestartTerminalDurabilityMutation.tla
  SumeragiV2ExactIngressTicketPriorityMutation.tla
  SumeragiV2ExactServeRestartTombstoneMutation.tla
  SumeragiV2ExactResponseClaimLifecycleMutation.tla
  SumeragiV2ExactServeFrozenPredecessorMutation.tla
  SumeragiV2ExactInstalledTcRetentionMutation.tla
  SumeragiV2ControlLivePredecessorMutation.tla
  SumeragiV2ImportedCertificateTailMutation.tla
  SumeragiV2ImportedTcTailMutation.tla
  SumeragiV2TimeoutLifecycleStageClassifierMutation.tla
  SumeragiV2PersistInstallTimeoutTagMutation.tla
  SumeragiV2PersistInstallTimeoutRootRetirementMutation.tla
  SumeragiV2AdequateLeaderWireTombstoneMutation.tla
  SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla
  SumeragiV2ExternalProducerContinuationMutation.tla
  SumeragiV2EmptyProducerHandoffMutation.tla
  SumeragiV2ProducerOriginReservationMutation.tla
  SumeragiV2ProducerContinuationCausalRankMutation.tla
  SumeragiV2RepresentativeLiveScopeMutation.tla
  SumeragiV2FixedCorridorPhysicalBudgetMutation.tla
  SumeragiV2FixedCorridorActionCreditMutation.tla
  SumeragiV2ProposalPipelineBudgetMutation.tla
  SumeragiV2AuthorityDeadlineCarryMutation.tla
  SumeragiV2FixedCorridorReceiptAcquisitionMutation.tla
)

for model in "${models[@]}"; do
  module="${model%.tla}"
  (
    cd "$FORMAL_DIR"
    "$JAVA_BIN" -cp "$TLA2TOOLS_JAR" tla2sany.SANY "$model"
  ) >"${run_dir}/${module}.sany.log" 2>&1
  sany_last_nonblank="$(
    awk 'NF { line = $0 } END { print line }' \
      "${run_dir}/${module}.sany.log"
  )"
  expected_marker="Semantic processing of module ${module}"
  [[ "$sany_last_nonblank" == "$expected_marker" ]] || {
    echo "${module}: SANY did not end at the expected marker" >&2
    cat "${run_dir}/${module}.sany.log" >&2
    exit 1
  }
done
echo "[sany] all twenty-five liveness-ownership mutation models parsed with frozen Java 21.0.12"

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 87 -seed 772441960364893113
)

run_case() {
  local label="$1"
  local model="$2"
  local config="$3"
  local expected_status="$4"
  shift 4
  local log="${run_dir}/${label}.log"
  local actual_status
  local last_nonblank
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "${run_dir}/${label}" \
      -config "$config" "$model"
  ) >"$log" 2>&1
  actual_status=$?
  set -e
  if [[ "$actual_status" -ne "$expected_status" ]]; then
    echo "${label} returned TLC status ${actual_status}, expected ${expected_status}" >&2
    cat "$log" >&2
    exit 1
  fi
  [[ "$(grep -Ec "$TLC_FINISHED_PATTERN" "$log" || true)" == 1 ]] || {
    echo "${label} did not emit exactly one TLC terminal marker" >&2
    cat "$log" >&2
    exit 1
  }
  last_nonblank="$(awk 'NF { line = $0 } END { print line }' "$log")"
  grep -Eq "$TLC_FINISHED_PATTERN" <<<"$last_nonblank" || {
    echo "${label} did not end at the TLC terminal marker" >&2
    cat "$log" >&2
    exit 1
  }
  for marker in "$@"; do
    if ! grep -Fq "$marker" "$log"; then
      echo "${label} missed expected marker: ${marker}" >&2
      cat "$log" >&2
      exit 1
    fi
  done
  echo "[tlc] ${label}: expected status ${expected_status}"
}

fixed_cases=(
  "local-ingress-scheduler-reservation|SumeragiV2LocalIngressSchedulerReservationMutation.tla|local_ingress_scheduler_reservation_fixed.cfg"
  "restart-terminal-durability|SumeragiV2RestartTerminalDurabilityMutation.tla|restart_terminal_durability_fixed.cfg"
  "exact-ingress-ticket-priority|SumeragiV2ExactIngressTicketPriorityMutation.tla|exact_ingress_ticket_priority_fixed.cfg"
  "exact-serve-restart-tombstone|SumeragiV2ExactServeRestartTombstoneMutation.tla|exact_serve_restart_tombstone_fixed.cfg"
  "exact-response-claim-lifecycle|SumeragiV2ExactResponseClaimLifecycleMutation.tla|exact_response_claim_lifecycle_fixed.cfg"
  "exact-serve-frozen-predecessor|SumeragiV2ExactServeFrozenPredecessorMutation.tla|exact_serve_frozen_predecessor_fixed.cfg"
  "exact-installed-tc-retention|SumeragiV2ExactInstalledTcRetentionMutation.tla|exact_installed_tc_retention_fixed.cfg"
  "control-live-predecessor|SumeragiV2ControlLivePredecessorMutation.tla|control_live_predecessor_fixed.cfg"
  "imported-certificate-tail|SumeragiV2ImportedCertificateTailMutation.tla|imported_certificate_tail_fixed.cfg"
  "imported-tc-tail|SumeragiV2ImportedTcTailMutation.tla|imported_tc_tail_fixed.cfg"
  "timeout-lifecycle-stage-classifier|SumeragiV2TimeoutLifecycleStageClassifierMutation.tla|timeout_lifecycle_stage_classifier_fixed.cfg"
  "persist-install-timeout-tag|SumeragiV2PersistInstallTimeoutTagMutation.tla|persist_install_timeout_tag_fixed.cfg"
  "persist-install-timeout-root-retirement|SumeragiV2PersistInstallTimeoutRootRetirementMutation.tla|persist_install_timeout_root_retirement_fixed.cfg"
  "adequate-leader-wire-tombstone|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_tombstone_fixed.cfg"
  "adequate-leader-candidate-tombstone|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_tombstone_fixed.cfg"
  "external-producer-continuation|SumeragiV2ExternalProducerContinuationMutation.tla|external_producer_continuation_fixed.cfg"
  "empty-producer-handoff|SumeragiV2EmptyProducerHandoffMutation.tla|empty_producer_handoff_fixed.cfg"
  "producer-origin-reservation|SumeragiV2ProducerOriginReservationMutation.tla|producer_origin_reservation_fixed.cfg"
  "producer-continuation-causal-rank|SumeragiV2ProducerContinuationCausalRankMutation.tla|producer_continuation_causal_rank_fixed.cfg"
  "representative-live-scope|SumeragiV2RepresentativeLiveScopeMutation.tla|representative_live_scope_fixed.cfg"
  "fixed-corridor-physical-budget|SumeragiV2FixedCorridorPhysicalBudgetMutation.tla|fixed_corridor_physical_budget_fixed.cfg"
  "fixed-corridor-action-credit|SumeragiV2FixedCorridorActionCreditMutation.tla|fixed_corridor_action_credit_fixed.cfg"
  "proposal-pipeline-budget|SumeragiV2ProposalPipelineBudgetMutation.tla|proposal_pipeline_budget_fixed.cfg"
  "authority-deadline-carry|SumeragiV2AuthorityDeadlineCarryMutation.tla|authority_deadline_carry_fixed.cfg"
  "fixed-corridor-receipt-acquisition|SumeragiV2FixedCorridorReceiptAcquisitionMutation.tla|fixed_corridor_receipt_acquisition_fixed.cfg"
)

for case_spec in "${fixed_cases[@]}"; do
  IFS='|' read -r label model config <<<"$case_spec"
  run_case "$label" "$model" "$config" 0 \
    "Model checking completed. No error has been found." \
    "states left on queue."
done

mutation_cases=(
  "local-ingress-scheduler-mutable-next|SumeragiV2LocalIngressSchedulerReservationMutation.tla|local_ingress_scheduler_reservation_mutable_next_bug.cfg|LaterLocalWorkCannotRewriteAcceptedOrdinal"
  "restart-volatile-terminal-suppression|SumeragiV2RestartTerminalDurabilityMutation.tla|restart_terminal_durability_blanket_terminal_bug.cfg|VolatileTerminalReopensExactWireIdentity"
  "exact-ingress-runtime-first|SumeragiV2ExactIngressTicketPriorityMutation.tla|exact_ingress_ticket_runtime_first_bug.cfg|ProvisionalTargetPrecedesRuntimeWork"
  "exact-serve-restart-resurrection|SumeragiV2ExactServeRestartTombstoneMutation.tla|exact_serve_restart_tombstone_bug.cfg|SameHeightRestartPreservesServeHighWatermark"
  "exact-response-duplicate|SumeragiV2ExactResponseClaimLifecycleMutation.tla|exact_response_claim_duplicate_bug.cfg|OneLogicalChargePerWaiterFamily"
  "exact-response-competing-responder|SumeragiV2ExactResponseClaimLifecycleMutation.tla|exact_response_claim_competing_responder_bug.cfg|OneLogicalChargePerWaiterFamily"
  "exact-response-post-consume-resurrection|SumeragiV2ExactResponseClaimLifecycleMutation.tla|exact_response_claim_resurrection_bug.cfg|ConsumedFamilyCannotResurrect"
  "exact-response-restart-reopen|SumeragiV2ExactResponseClaimLifecycleMutation.tla|exact_response_claim_restart_reopen_bug.cfg|SameHeightRestartReopensDurableFamily"
  "exact-serve-later-owner-churn|SumeragiV2ExactServeFrozenPredecessorMutation.tla|exact_serve_frozen_predecessor_churn_bug.cfg|ReservedServeCapacityCannotBeStolen"
  "exact-installed-tc-view-only-replacement|SumeragiV2ExactInstalledTcRetentionMutation.tla|exact_installed_tc_view_only_bug.cfg|ExactInstalledTcAuthority"
  "control-live-predecessor-replacement|SumeragiV2ControlLivePredecessorMutation.tla|control_live_predecessor_bug.cfg|LivePredecessorRetainsSlotAndPacket"
  "imported-certificate-tail-incarnation-drop|SumeragiV2ImportedCertificateTailMutation.tla|imported_certificate_tail_bug.cfg|ReceiptRetainsExactDecisionTail"
  "imported-tc-tail-generation-drop|SumeragiV2ImportedTcTailMutation.tla|imported_tc_tail_bug.cfg|TcReceiptRetainsExactInstallTail"
  "timeout-lifecycle-causal-origin-phase|SumeragiV2TimeoutLifecycleStageClassifierMutation.tla|timeout_lifecycle_stage_classifier_bug.cfg|ProposalSuccessorRetainedOriginIsNotTimeoutOwner"
  "persist-install-retains-old-timeout-tag|SumeragiV2PersistInstallTimeoutTagMutation.tla|persist_install_timeout_tag_bug.cfg|SuccessfulInstallClearsOldViewTimeoutTag"
  "persist-install-retains-old-timeout-root|SumeragiV2PersistInstallTimeoutRootRetirementMutation.tla|persist_install_timeout_root_retirement_bug.cfg|InstallRetiresOlderOrEqualTimeoutRoots"
  "adequate-wire-slot-cardinality|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_slot_cardinality_bug.cfg|SlotTableCardinalityIsRosterClassBounded"
  "adequate-wire-same-view-replacement|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_same_view_replacement_bug.cfg|SameViewIdentityCannotReplaceFirstOwner"
  "adequate-wire-terminal-identity-binding|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_terminal_identity_bug.cfg|ForeignTerminalCannotRetireExactOwner"
  "adequate-wire-retry-coalescing|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_retry_coalescing_bug.cfg|ExactRetryCoalescesIntoOneOwner"
  "adequate-wire-consumed-resurrection|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_tombstone_bug.cfg|ConsumedSameOrLowerRetryDropsWithoutCandidate"
  "adequate-wire-restart-terminal-resurrection|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_restart_resurrection_bug.cfg|SameHeightRestartPreservesStableTerminal"
  "adequate-wire-restart-immediate-owner|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_restart_reopen_owner_bug.cfg|RestartedUnconsumedIsDormantWithoutAuthority"
  "adequate-wire-restart-packet-synthesis|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_restart_packet_synthesis_bug.cfg|RestartDoesNotSynthesizePacket"
  "adequate-wire-restart-ordinal-reallocation|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_restart_ordinal_reallocation_bug.cfg|CapacityAcceptanceReactivatesSameOrdinal"
  "adequate-wire-restart-prefix-recharge|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_restart_prefix_recharge_bug.cfg|CapacityAcceptanceRetainsRestartClearedPrefix"
  "adequate-wire-dormant-potential-precharge|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_dormant_potential_precharge_bug.cfg|LaterTargetPrechargesDormantPotentialOwner"
  "adequate-wire-restart-capacity-bypass|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_restart_capacity_bypass_bug.cfg|CapacityBlockedRetryRemainsDormant"
  "adequate-wire-unconsumed-completion|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_unconsumed_completion_bug.cfg|OccupiedUnconsumedOwnerIsNotCompletion"
  "adequate-wire-rollover-reset|SumeragiV2AdequateLeaderWireTombstoneMutation.tla|adequate_leader_wire_rollover_reset_bug.cfg|SuccessorHeightRolloverResetsSlot"
  "adequate-candidate-a-b-a-resurrection|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_resurrection_bug.cfg|LiveCandidateIsNotTombstoned"
  "adequate-candidate-terminal-discard-resurrection|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_terminal_discard_resurrection_bug.cfg|TerminalDiscardCannotBeReadmitted"
  "adequate-candidate-retired-chunk-view|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_retired_chunk_view_bug.cfg|RetiredChunkStageCannotReadmitAfterViewAdvance"
  "adequate-candidate-retired-chunk-decision|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_retired_chunk_decision_bug.cfg|RetiredChunkStageCannotReadmitAfterDecision"
  "adequate-candidate-restart-resurrection|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_restart_resurrection_bug.cfg|SameHeightRestartPreservesServicedA"
  "adequate-candidate-restart-volatile-owner-loss|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_restart_volatile_owner_loss_bug.cfg|UnservicedDurableCandidateRebuiltAfterRestart"
  "adequate-candidate-signed-restart-suppression|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_signed_restart_suppression_bug.cfg|RestartScopedSignedCompletionIsReissued"
  "adequate-candidate-aggregate-evidence-identity-explosion|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_aggregate_evidence_identity_explosion_bug.cfg|EquivalentAggregateEvidenceCoalescesToOneIdentity"
  "adequate-candidate-strict-view-reclamation|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_strict_view_reclamation_bug.cfg|StrictViewAdvanceReclaimsOnlyOldView"
  "adequate-candidate-rollover-reclamation|SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla|adequate_leader_candidate_rollover_reclamation_bug.cfg|SuccessorHeightReclaimsPredecessorTombstones"
  "external-continuation-missing-conditional|SumeragiV2ExternalProducerContinuationMutation.tla|external_producer_continuation_missing_conditional_bug.cfg|ConditionalDepartureInstallsExactContinuation"
  "external-continuation-missing-volatile|SumeragiV2ExternalProducerContinuationMutation.tla|external_producer_continuation_missing_volatile_bug.cfg|VolatileDepartureInstallsExactContinuation"
  "external-continuation-synthetic-carrier|SumeragiV2ExternalProducerContinuationMutation.tla|external_producer_continuation_synthetic_carrier_bug.cfg|ExternalMaterializationRequiresExactCarrier"
  "external-continuation-resurrection|SumeragiV2ExternalProducerContinuationMutation.tla|external_producer_continuation_resurrection_bug.cfg|TerminalIdentityCannotResurrect"
  "external-continuation-missing-conditional-fairness|SumeragiV2ExternalProducerContinuationMutation.tla|external_producer_continuation_missing_conditional_fairness_bug.cfg|ExternalContinuationsReachTerminal"
  "external-continuation-missing-volatile-fairness|SumeragiV2ExternalProducerContinuationMutation.tla|external_producer_continuation_missing_volatile_fairness_bug.cfg|ExternalContinuationsReachTerminal"
  "empty-producer-handoff-missing-reservation|SumeragiV2EmptyProducerHandoffMutation.tla|empty_producer_handoff_missing_reservation_bug.cfg|EmptyProducerDepartureNeverBecomesUnowned"
  "producer-origin-missing-owner|SumeragiV2ProducerOriginReservationMutation.tla|producer_origin_reservation_missing_owner_bug.cfg|ScheduledOriginHasBoundedReservation"
  "producer-origin-new-ordinal|SumeragiV2ProducerOriginReservationMutation.tla|producer_origin_reservation_new_ordinal_bug.cfg|DepartureContinuationReusesAdmissionOrdinal"
  "producer-origin-duplicate-retry|SumeragiV2ProducerOriginReservationMutation.tla|producer_origin_reservation_duplicate_retry_bug.cfg|ExactOriginRetryCoalesces"
  "producer-continuation-stage-only-rank|SumeragiV2ProducerContinuationCausalRankMutation.tla|producer_continuation_causal_rank_stage_only_bug.cfg|FrozenCausalEpisodeCannotReplenish"
  "representative-live-missing-premise|SumeragiV2RepresentativeLiveScopeMutation.tla|representative_live_scope_missing_premise_bug.cfg|ReleaseLiveEvidenceIsRepresentative"
  "fixed-corridor-omitted-lane-cursor|SumeragiV2FixedCorridorPhysicalBudgetMutation.tla|fixed_corridor_physical_budget_omitted_lane_cursor_bug.cfg|PhysicalWindowBudgetCoversIndependentLanesAndCursorResets"
  "fixed-corridor-per-child-recharge|SumeragiV2FixedCorridorActionCreditMutation.tla|fixed_corridor_action_credit_per_child_recharge_bug.cfg|ExactSuccessorHandoffStrictlyConsumesCumulativeActionDebt"
  "proposal-pipeline-additive-budget|SumeragiV2ProposalPipelineBudgetMutation.tla|proposal_pipeline_budget_additive_bug.cfg|PipelineBudgetCoversEveryCrossSlotEpisode"
  "authority-deadline-expired-receipt|SumeragiV2AuthorityDeadlineCarryMutation.tla|authority_deadline_carry_expired_receipt_bug.cfg|ImmutableReceiptCannotExpireBeforeDecision"
  "authority-deadline-kernel-recharge|SumeragiV2AuthorityDeadlineCarryMutation.tla|authority_deadline_carry_kernel_recharge_bug.cfg|ImmutableReceiptCannotExpireBeforeDecision"
  "fixed-corridor-receipt-prestate-gap|SumeragiV2FixedCorridorReceiptAcquisitionMutation.tla|fixed_corridor_receipt_acquisition_prestate_only_bug.cfg|ReceiptAcquisitionAndRetention"
  "fixed-corridor-receipt-global-retirement|SumeragiV2FixedCorridorReceiptAcquisitionMutation.tla|fixed_corridor_receipt_acquisition_global_retire_bug.cfg|ReceiptAcquisitionAndRetention"
)

for case_spec in "${mutation_cases[@]}"; do
  IFS='|' read -r label model config invariant <<<"$case_spec"
  run_case "$label" "$model" "$config" 12 \
    "Error: Invariant ${invariant} is violated." \
    "Error: The behavior up to this point is:"
done

echo "[tlc] all 54 liveness-ownership mutations produced their exact named counterexamples; repaired models passed"
