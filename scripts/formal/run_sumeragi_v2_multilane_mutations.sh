#!/usr/bin/env bash
set -euo pipefail

# Run the bounded multilane lifecycle/evidence/carrier negative-control corpus.
# Prerequisites: the pinned TLA2Tools jar installed by
# install_sumeragi_v2_tla2tools.sh and a working Java runtime. No environment
# variables are required; TLA2TOOLS_JAR and JAVA_BIN may override safe defaults.

if (($#)); then
  if (($# == 1)) && [[ "$1" == "--help" ]]; then
    echo "usage: $0" >&2
    exit 0
  fi
  echo "usage: $0" >&2
  exit 2
fi

readonly TLA2TOOLS_VERSION="1.7.4"
readonly TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly FORMAL_DIR="${REPO_ROOT}/formal/sumeragi_v2"
readonly TLA2TOOLS_JAR="${TLA2TOOLS_JAR:?TLA2TOOLS_JAR must name the authenticated external tool}"
readonly CONTRACT_CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_models.py"
source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"
if [[ -n "${JAVA_BIN:-}" ]]; then
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh" "$JAVA_BIN")"
else
  resolved_java_bin="$("${REPO_ROOT}/scripts/formal/resolve_java.sh")"
fi
readonly JAVA_BIN="$resolved_java_bin"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

python3 -I -S "$CONTRACT_CHECKER"
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

run_dir="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-multilane.XXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT

common=(
  "$JAVA_BIN" -XX:+UseParallelGC -cp "$TLA2TOOLS_JAR" tlc2.TLC
  -cleanup -workers 1 -fp 94 -seed 20260723
)

run_mutant() {
  local name="$1"
  local module="$2"
  local config="$3"
  local invariant="$4"
  local log="$run_dir/${name}.log"
  local invariant_marker="Error: Invariant ${invariant} is violated."
  local primary_diagnostic_count
  set +e
  (
    cd "$FORMAL_DIR"
    "${common[@]}" -metadir "$run_dir/${name}" \
      -config "$config" "$module"
  ) >"$log" 2>&1
  local status=$?
  set -e
  if [[ "$status" -ne 12 ]]; then
    echo "${name} did not produce the expected ${invariant} counterexample (status=${status})" >&2
    cat "$log" >&2
    exit 1
  fi
  sumeragi_v2_tlc_assert_nonzero_state_space "$name" "$log"
  sumeragi_v2_tlc_assert_exact_line "$name" "$log" "$invariant_marker"
  sumeragi_v2_tlc_assert_exact_line \
    "$name" "$log" "Error: The behavior up to this point is:"
  primary_diagnostic_count="$(
    grep -Ec \
      "$SUMERAGI_V2_TLC_PRIMARY_DIAGNOSTIC_PATTERN" \
      "$log" || true
  )"
  [[ "$primary_diagnostic_count" -eq 1 ]] || {
    echo "${name} emitted ${primary_diagnostic_count} primary failure diagnostics" >&2
    cat "$log" >&2
    exit 1
  }
  sumeragi_v2_tlc_assert_terminal "$name" "$log"
  grep -Fq "TLC2 Version 2.19" "$log" || {
    echo "${name} did not run with the pinned TLC engine" >&2
    cat "$log" >&2
    exit 1
  }
  echo "[tlc] observed ${name}: ${invariant}"
}

readonly AUTOSCALE_MODULE="SumeragiV2AutoscaleLifecycle.tla"
run_mutant autoscale-early-drain "$AUTOSCALE_MODULE" \
  multilane_autoscale_early_drain_bug.cfg MLDrainImpliesNoOwnedWork
run_mutant autoscale-destroy-before-archive "$AUTOSCALE_MODULE" \
  multilane_autoscale_destroy_before_archive_bug.cfg \
  ArchiveBeforeDestroyInvariant
run_mutant autoscale-incarnation-reuse "$AUTOSCALE_MODULE" \
  multilane_autoscale_incarnation_reuse_bug.cfg \
  MLRetirementConsumesExactIncarnation
run_mutant autoscale-activation-before-storage "$AUTOSCALE_MODULE" \
  multilane_autoscale_activation_before_storage_bug.cfg \
  MLActivationAfterAtomicCreate
run_mutant autoscale-weak-drain-certificate "$AUTOSCALE_MODULE" \
  multilane_autoscale_weak_drain_certificate_bug.cfg \
  MLDrainCertificateMonotonic
run_mutant autoscale-cleanup-by-lane-id "$AUTOSCALE_MODULE" \
  multilane_autoscale_cleanup_by_lane_id_bug.cfg \
  MLRetirementConsumesExactIncarnation

readonly NATIVE_MODULE="SumeragiV2NativeApplicationEvidence.tla"
run_mutant native-frontier-before-sidecars "$NATIVE_MODULE" \
  multilane_native_frontier_before_sidecars_bug.cfg \
  MLNativeDurabilityPrecedesFrontier
run_mutant native-hash-only-pruning "$NATIVE_MODULE" \
  multilane_native_hash_only_pruning_bug.cfg \
  PrunedEvidenceVerifiableInvariant
run_mutant native-same-route-marker "$NATIVE_MODULE" \
  multilane_native_same_route_marker_bug.cfg MLSeparateParticipantApplication
run_mutant native-source-claim-equivocation "$NATIVE_MODULE" \
  multilane_native_source_claim_equivocation_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-source-id-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_source_id_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-entrypoint-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_tx_entrypoint_hash_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-plan-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_plan_digest_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-round-context-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_round_context_id_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-round-height-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_round_height_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-round-view-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_round_view_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-epoch-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_epoch_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-chain-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_network_id_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-authority-height-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_authority_context_height_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-coordinator-lane-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_coordinator_lane_id_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-coordinator-dataspace-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_coordinator_dataspace_id_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-coordinator-incarnation-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_coordinator_lane_incarnation_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-coordinator-height-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_planned_coordinator_block_height_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-coordinator-view-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_coordinator_lane_block_view_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-coordinator-proposal-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_coordinator_proposal_hash_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-participant-lane-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_participant_lane_id_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-participant-dataspace-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_participant_dataspace_id_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-participant-incarnation-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_participant_lane_incarnation_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-source-claim-participant-membership-drift "$NATIVE_MODULE" \
  multilane_native_source_claim_participant_membership_drift_bug.cfg \
  MLNativeSourceClaimInjective
run_mutant native-noncontiguous-route "$NATIVE_MODULE" \
  multilane_native_noncontiguous_route_bug.cfg \
  MLNativeContiguousActiveRoute
run_mutant native-partial-group-application "$NATIVE_MODULE" \
  multilane_native_partial_group_application_bug.cfg \
  MLNativeGroupExactCover
run_mutant native-forged-manifest-leaf "$NATIVE_MODULE" \
  multilane_native_forged_manifest_leaf_bug.cfg \
  MLNativeManifestAuthenticates
run_mutant native-dropped-startup-repair "$NATIVE_MODULE" \
  multilane_native_dropped_startup_repair_bug.cfg \
  MLNativeDurabilityPrecedesFrontier
run_mutant native-ambiguous-latest-index "$NATIVE_MODULE" \
  multilane_native_ambiguous_latest_index_bug.cfg \
  MLNativeLatestIndexExact
run_mutant native-discard-authenticated-latest-temp "$NATIVE_MODULE" \
  multilane_native_discard_authenticated_latest_temp_bug.cfg \
  MLNativeLatestIndexExact
run_mutant native-shared-evidence-budget "$NATIVE_MODULE" \
  multilane_native_shared_evidence_budget_bug.cfg \
  MLNativeSharedEvidenceBudget
run_mutant native-second-incoming-pair "$NATIVE_MODULE" \
  multilane_native_second_incoming_pair_bug.cfg \
  MLNativeSingleIncomingPairHeadroom
run_mutant native-unauthenticated-temp-promotion "$NATIVE_MODULE" \
  multilane_native_unauthenticated_temp_promotion_bug.cfg \
  MLNativeTempPromotionAuthenticated
run_mutant native-punctured-retained-history "$NATIVE_MODULE" \
  multilane_native_punctured_retained_history_bug.cfg \
  MLNativeRetainedHistoryExact
run_mutant native-nonoldest-prefix-prune "$NATIVE_MODULE" \
  multilane_native_nonoldest_prefix_prune_bug.cfg \
  MLNativePruneOldestPrefix
run_mutant native-nonhighest-repair-half "$NATIVE_MODULE" \
  multilane_native_nonhighest_repair_half_bug.cfg \
  MLNativeRetainedHistoryExact
run_mutant native-multiple-repair-halves "$NATIVE_MODULE" \
  multilane_native_multiple_repair_halves_bug.cfg \
  MLNativeRetainedHistoryExact
run_mutant native-conflicting-retained-pair "$NATIVE_MODULE" \
  multilane_native_conflicting_retained_pair_bug.cfg \
  MLNativeRetainedHistoryExact
run_mutant native-retained-predecessor-drift "$NATIVE_MODULE" \
  multilane_native_retained_predecessor_drift_bug.cfg \
  MLNativeRetainedHistoryExact
run_mutant native-mutating-unified-startup-plan "$NATIVE_MODULE" \
  multilane_native_mutating_unified_startup_plan_bug.cfg \
  MLUnifiedStartupEvidenceRepairSafe
run_mutant native-uncoalesced-canonical-body-needs "$NATIVE_MODULE" \
  multilane_native_uncoalesced_canonical_body_needs_bug.cfg \
  MLUnifiedStartupEvidenceRepairSafe
run_mutant native-partial-unified-startup-preflight "$NATIVE_MODULE" \
  multilane_native_partial_unified_startup_preflight_bug.cfg \
  MLUnifiedStartupEvidenceRepairSafe
run_mutant native-queue-before-evidence-readback "$NATIVE_MODULE" \
  multilane_native_queue_before_evidence_readback_bug.cfg \
  MLUnifiedStartupEvidenceRepairSafe
run_mutant native-missing-reverse-merge-carrier "$NATIVE_MODULE" \
  multilane_native_missing_reverse_merge_carrier_bug.cfg \
  MLUnifiedStartupEvidenceRepairSafe
run_mutant native-orphan-merge-carrier "$NATIVE_MODULE" \
  multilane_native_orphan_merge_carrier_bug.cfg \
  MLUnifiedStartupEvidenceRepairSafe
run_mutant native-skip-post-cache-carrier-reconcile "$NATIVE_MODULE" \
  multilane_native_skip_post_cache_carrier_reconcile_bug.cfg \
  MLUnifiedStartupEvidenceRepairSafe
run_mutant native-repair-historical-sibling-as-active "$NATIVE_MODULE" \
  multilane_native_repair_historical_sibling_as_active_bug.cfg \
  MLUnifiedStartupEvidenceRepairSafe
run_mutant native-prune-without-protected-latest "$NATIVE_MODULE" \
  multilane_native_prune_without_protected_latest_bug.cfg \
  MLNativePruneProtectedLatestExact
run_mutant native-prune-namespace-rebind "$NATIVE_MODULE" \
  multilane_native_prune_namespace_rebind_bug.cfg \
  MLNativePruneExactObjectRemoval

readonly AUTONOMOUS_MODULE="SumeragiV2AutonomousReservationCarrier.tla"
run_mutant autonomous-carrier-drift "$AUTONOMOUS_MODULE" \
  multilane_autonomous_carrier_drift_bug.cfg MLReservationIdentityStable
run_mutant autonomous-duplicate-application "$AUTONOMOUS_MODULE" \
  multilane_autonomous_duplicate_application_bug.cfg \
  MLCarrierExactlyOnce
run_mutant autonomous-release-after-apply "$AUTONOMOUS_MODULE" \
  multilane_autonomous_release_after_apply_bug.cfg \
  NoReleaseAfterApplicationInvariant
run_mutant autonomous-release-before-barrier "$AUTONOMOUS_MODULE" \
  multilane_autonomous_release_before_barrier_bug.cfg \
  ReleaseOrderingInvariant
run_mutant autonomous-aba-release "$AUTONOMOUS_MODULE" \
  multilane_autonomous_aba_release_bug.cfg \
  MLRestartOwnershipPartition
run_mutant autonomous-digest-only-authorization "$AUTONOMOUS_MODULE" \
  multilane_autonomous_digest_only_authorization_bug.cfg \
  MLCertifiedBundleDurable
run_mutant autonomous-ordinary-anchor-execution "$AUTONOMOUS_MODULE" \
  multilane_autonomous_ordinary_anchor_execution_bug.cfg \
  MLCarrierExactlyOnce
run_mutant autonomous-reserve-before-durable "$AUTONOMOUS_MODULE" \
  multilane_autonomous_reserve_before_durable_bug.cfg \
  MLReservationIdentityStable
run_mutant autonomous-noncanonical-merge-prefix "$AUTONOMOUS_MODULE" \
  multilane_autonomous_noncanonical_merge_prefix_bug.cfg \
  MLMergeCandidateExactPrefix
run_mutant autonomous-skip-canonical-reexecution "$AUTONOMOUS_MODULE" \
  multilane_autonomous_skip_canonical_reexecution_bug.cfg \
  MLCarrierExactlyOnce
run_mutant autonomous-prevote-commit-surface-drift "$AUTONOMOUS_MODULE" \
  multilane_autonomous_prevote_commit_surface_drift_bug.cfg \
  MLCarrierCommitSurfaceExact
run_mutant autonomous-event-prefix-drift "$AUTONOMOUS_MODULE" \
  multilane_autonomous_event_prefix_drift_bug.cfg \
  MLCarrierCommitSurfaceExact
run_mutant autonomous-post-validation-event-surface-drift "$AUTONOMOUS_MODULE" \
  multilane_autonomous_post_validation_event_surface_drift_bug.cfg \
  MLCarrierCommitSurfaceExact
run_mutant autonomous-restart-drops-ownership "$AUTONOMOUS_MODULE" \
  multilane_autonomous_restart_drops_ownership_bug.cfg \
  MLReservationSingleOwner
run_mutant autonomous-unauthenticated-recovery-body "$AUTONOMOUS_MODULE" \
  multilane_autonomous_unauthenticated_recovery_body_bug.cfg \
  MLRecoveredCarrierBodyAuthenticated
run_mutant autonomous-mixed-signer-recovery-body "$AUTONOMOUS_MODULE" \
  multilane_autonomous_mixed_signer_recovery_body_bug.cfg \
  MLRecoveredCarrierBodyAuthenticated
run_mutant autonomous-inflated-recovery-wire-length "$AUTONOMOUS_MODULE" \
  multilane_autonomous_inflated_recovery_wire_length_bug.cfg \
  MLRecoveredCarrierLengthAuthenticated
run_mutant autonomous-historical-context-drift "$AUTONOMOUS_MODULE" \
  multilane_autonomous_historical_context_drift_bug.cfg \
  MLHistoricalRecoveryContextExact
run_mutant autonomous-open-queue-before-recovery-install "$AUTONOMOUS_MODULE" \
  multilane_autonomous_open_queue_before_recovery_install_bug.cfg \
  MLHistoricalQueueGateOrder
run_mutant autonomous-partial-recovery-group-preflight "$AUTONOMOUS_MODULE" \
  multilane_autonomous_partial_recovery_group_preflight_bug.cfg \
  MLHistoricalAllGroupsPreflight
run_mutant autonomous-pending-only-canonical-terminal "$AUTONOMOUS_MODULE" \
  multilane_autonomous_pending_only_canonical_terminal_bug.cfg \
  MLTerminalOutcomeJoinAuthenticated
run_mutant autonomous-release-without-finalization-authority \
  "$AUTONOMOUS_MODULE" \
  multilane_autonomous_release_without_finalization_authority_bug.cfg \
  MLTerminalOutcomeJoinAuthenticated
run_mutant autonomous-complete-without-queue-evidence "$AUTONOMOUS_MODULE" \
  multilane_autonomous_complete_without_queue_evidence_bug.cfg \
  MLTerminalOutcomeJoinAuthenticated
run_mutant autonomous-partial-terminal-unit-sweep "$AUTONOMOUS_MODULE" \
  multilane_autonomous_partial_terminal_unit_sweep_bug.cfg \
  MLCanonicalTerminalBatchAtomic
run_mutant autonomous-owned-group-mutation-before-planner "$AUTONOMOUS_MODULE" \
  multilane_autonomous_owned_group_mutation_before_planner_bug.cfg \
  MLTerminalStartupSweepOrder
run_mutant autonomous-open-queue-before-deferred-carrier-apply \
  "$AUTONOMOUS_MODULE" \
  multilane_autonomous_open_queue_before_deferred_carrier_apply_bug.cfg \
  MLTerminalStartupSweepOrder
run_mutant autonomous-producer-recovery-without-queue-owner \
  "$AUTONOMOUS_MODULE" \
  multilane_autonomous_producer_recovery_without_queue_owner_bug.cfg \
  MLLocalProducerRecoveryRequiresQueueOwner
run_mutant autonomous-volatile-stage-diagnostics "$AUTONOMOUS_MODULE" \
  multilane_autonomous_volatile_stage_diagnostics_bug.cfg \
  MLStageEvidenceMonotonic

readonly QUEUE_PLAN_ADMISSION_MODULE="SumeragiV2QueuePlanAdmissionRegistry.tla"
run_mutant queue-plan-split-route-public-acceptance \
  "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_split_route_public_acceptance_bug.cfg \
  MLPublic202Exact
run_mutant queue-plan-execution-before-global-cas \
  "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_execution_before_global_cas_bug.cfg \
  MLQueueEligibilityExact
run_mutant queue-plan-conflicting-cas "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_conflicting_cas_bug.cfg \
  MLAdmissionCasUnique
run_mutant queue-plan-restart-aba "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_restart_aba_bug.cfg \
  MLQueueEligibilityExact
run_mutant queue-plan-local-expiry-clears-tombstone \
  "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_local_expiry_clears_tombstone_bug.cfg \
  MLImmutableAdmissionTombstone
run_mutant queue-plan-deferred-bypass "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_deferred_bypass_bug.cfg \
  MLPublic202Exact
run_mutant queue-plan-cancellation-bypass "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_cancellation_bypass_bug.cfg \
  MLCancellationStopsExecution
run_mutant queue-plan-guard-drop-deletes-durable-owner \
  "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_guard_drop_deletes_durable_owner_bug.cfg \
  MLCertificateDurable
run_mutant queue-plan-execution-without-exact-binding \
  "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_execution_without_exact_binding_bug.cfg \
  MLExecutionRequiresExactBinding
run_mutant queue-plan-duplicate-execution "$QUEUE_PLAN_ADMISSION_MODULE" \
  multilane_queue_plan_duplicate_execution_bug.cfg \
  MLAdmissionAtMostOnceExecution

readonly KURA_RETENTION_MODULE="SumeragiV2KuraReplicaRetention.tla"
run_mutant kura-replica-forged-signature "$KURA_RETENTION_MODULE" \
  kura_replica_forged_signature_bug.cfg KRAdmittedAdvertsSigned
run_mutant kura-replica-relayed-advert "$KURA_RETENTION_MODULE" \
  kura_replica_relayed_advert_bug.cfg KRAdmittedAdvertsDirectAuthenticated
run_mutant kura-replica-wrong-finality-identity "$KURA_RETENTION_MODULE" \
  kura_replica_wrong_finality_identity_bug.cfg \
  KRAdmittedAdvertsBindExactFinality
run_mutant kura-replica-wrong-wire-identity "$KURA_RETENTION_MODULE" \
  kura_replica_wrong_wire_identity_bug.cfg KRAdmittedAdvertsBindExactWire
run_mutant kura-replica-keeper-cardinality "$KURA_RETENTION_MODULE" \
  kura_replica_keeper_cardinality_bug.cfg KRDeterministicFPlusOneKeepers
run_mutant kura-replica-nonsigner-keeper "$KURA_RETENTION_MODULE" \
  kura_replica_nonsigner_keeper_bug.cfg KRDeterministicFPlusOneKeepers
run_mutant kura-replica-local-keeper-evict "$KURA_RETENTION_MODULE" \
  kura_replica_local_keeper_evict_bug.cfg KRLocalSelectedKeeperPinsBody
run_mutant kura-replica-partial-remote-freshness "$KURA_RETENTION_MODULE" \
  kura_replica_partial_remote_freshness_bug.cfg \
  KREvictionRequiresAllSelectedRemoteFresh
run_mutant kura-replica-ttl-expiry "$KURA_RETENTION_MODULE" \
  kura_replica_ttl_expiry_bug.cfg KRExpiredAdvertsCannotAuthorize
run_mutant kura-replica-restart-registry-reuse "$KURA_RETENTION_MODULE" \
  kura_replica_restart_registry_reuse_bug.cfg KRRestartClearsAdvertRegistry
run_mutant kura-replica-registry-capacity-overflow "$KURA_RETENTION_MODULE" \
  kura_replica_registry_capacity_overflow_bug.cfg KRRegistryCapacityBounded
run_mutant kura-replica-refresh-window-oversize "$KURA_RETENTION_MODULE" \
  kura_replica_refresh_window_oversize_bug.cfg KRRefreshWindowBounded
run_mutant kura-replica-refresh-cursor-skip "$KURA_RETENTION_MODULE" \
  kura_replica_refresh_cursor_skip_bug.cfg KRRefreshCursorExact
run_mutant kura-replica-skip-final-prestage-recheck "$KURA_RETENTION_MODULE" \
  kura_replica_skip_final_prestage_recheck_bug.cfg KRFinalPreStageRecheck

echo "[tlc] all 106 multilane mutations produced their exact named counterexamples; no deductive proof status was changed"
