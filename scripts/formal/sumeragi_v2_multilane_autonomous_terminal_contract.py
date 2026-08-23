"""Source-binding contract for autonomous lifecycle terminal recovery."""

from __future__ import annotations

import re
from collections.abc import Callable
from pathlib import Path
from typing import Any, Optional


RustBindingItem = Callable[
    [Path, str, str, str, str, list[str]],
    Optional[str],
]

KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE = (
    "crates/iroha_core/src/kura/pipeline_and_lane_artifacts.rs"
)
QUEUE_CANONICAL_TERMINAL_CLEANUP_RELATIVE = (
    "crates/iroha_core/src/queue/canonical_terminal_cleanup.rs"
)
QUEUE_JOURNAL_RESERVATION_COMMIT_PREFLIGHT_RELATIVE = (
    "crates/iroha_core/src/queue/journal_reservation_commit_preflight.rs"
)
KURA_LANE_ARTIFACT_BUDGET_RELATIVE = (
    "crates/iroha_core/src/kura/lane_artifact_budget.rs"
)
KURA_AUTONOMOUS_TERMINAL_CAPACITY_RELATIVE = (
    "crates/iroha_core/src/kura/autonomous_terminal_capacity.rs"
)
SUMERAGI_COMMITTED_CARRIER_CLEANUP_RELATIVE = (
    "crates/iroha_core/src/sumeragi/v2_apply/committed_carrier_cleanup.rs"
)
AUTONOMOUS_TERMINAL_RECOVERY_MODULE = "SumeragiV2AutonomousReservationCarrier"
AUTONOMOUS_TERMINAL_TLA_RELATIVE = (
    "formal/sumeragi_v2/SumeragiV2AutonomousReservationCarrier.tla"
)
AUTONOMOUS_TERMINAL_TLA_POSITIVE_ACTION_CHECKS = (
    (
        "PersistCanonicalTerminalOutcomePending",
        (
            "canonicalGroupAQueueOwned' = TRUE",
            "canonicalGroupBQueueOwned' = FALSE",
            "canonicalGroupATerminalPublished' = FALSE",
            "canonicalGroupBTerminalPublished' = FALSE",
        ),
    ),
    (
        "RestartWithPendingTerminalOutcome",
        (
            "queueGateOpen' = FALSE",
            "queueOwnershipSnapshotTaken' = FALSE",
            "queueOwnershipSnapshotReceiptValid' = FALSE",
        ),
    ),
    (
        "TakeInitialQueueOwnershipSnapshot",
        (
            "queueOwnershipSnapshotTaken' = TRUE",
            "queueOwnershipSnapshotReceiptValid' = TRUE",
            "snapshotGroupAQueueOwned' = canonicalGroupAQueueOwned",
            "snapshotGroupBQueueOwned' = canonicalGroupBQueueOwned",
        ),
    ),
    (
        "BeginTerminalOutcomeStartupSweep",
        (
            "queueOwnershipSnapshotTaken",
            "queueOwnershipSnapshotReceiptValid",
            "terminalSweepStarted' = TRUE",
        ),
    ),
    (
        "ReconstructCompleteCanonicalTerminalOutcomeSet",
        ("canonicalOutcomeSetComplete' = TRUE",),
    ),
    (
        "ReconstructCanonicalCarrierCleanupAuthorization",
        ("canonicalCarrierCleanupAuthorized' = TRUE",),
    ),
    (
        "PreflightCanonicalCarrierTerminalBatch",
        ("canonicalCarrierBatchPreflighted' = TRUE",),
    ),
    (
        "DeferCanonicalTerminalUnitWithQueueOwner",
        (
            "(snapshotGroupAQueueOwned \\/ snapshotGroupBQueueOwned)",
            "~canonicalGroupATerminalPublished",
            "~canonicalGroupBTerminalPublished",
            "canonicalCarrierUnitDeferred' = TRUE",
        ),
    ),
    (
        "FinishTerminalOutcomeStartupSweep",
        (
            'terminalOutcomeStage = "Pending"',
            "canonicalCarrierUnitDeferred",
            "terminalSweepCompleted' = TRUE",
        ),
    ),
    (
        "PlanDeferredCanonicalCarrierFromInitialSnapshot",
        (
            "queueOwnershipSnapshotReceiptValid",
            "canonicalCarrierUnitDeferred",
            "deferredCarrierPlannedFromSnapshot' = TRUE",
        ),
    ),
    (
        "PublishCanonicalQueueTerminalEvidence",
        (
            "deferredCarrierPlannedFromSnapshot",
            "canonicalGroupATerminalPublished' = TRUE",
            "canonicalGroupBTerminalPublished' = TRUE",
            "normalCarrierApplyCompleted' = normalCarrierApplyCompleted \\/ canonicalCarrierUnitDeferred",
        ),
    ),
    (
        "PromoteTerminalOutcomeComplete",
        ('terminalOutcomeStage\' = "Complete"',),
    ),
    (
        "OpenQueueAfterTerminalOutcomePlanning",
        (
            'terminalOutcomeStage # "Pending"',
            "normalCarrierApplyCompleted",
            "queueGateOpen' = TRUE",
        ),
    ),
)
AUTONOMOUS_TERMINAL_RAW_TEST_CHECKS = (
    (
        "crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_01.rs",
        "terminal_presweep_rejects_unquarantined_nonempty_queue_before_kura_inventory",
        (
            "assert!(!before.is_empty());",
            "assert!(!queue.lane_reservation_startup_reconciliation_pending());",
            "reconcile_pending_autonomous_lifecycle_terminal_outcomes(",
            "terminal pre-sweep must reject an unquarantined non-empty Queue cut",
            "published before terminal-outcome pre-sweep",
            "before,",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_01.rs",
        "empty_startup_plan_skips_canonical_cleanup_and_publishes_its_receipt",
        (
            "lane_reservation_reconciliation_snapshot()",
            ".is_empty()",
            "plan_lane_reservation_ownership(",
            "LaneReservationReconciliationPlanning::Ready(plan)",
            "apply_lane_reservation_reconciliation_plan(",
            "LaneReservationReconciliationSummary::default()",
            "assert!(!queue.lane_reservation_startup_reconciliation_pending());",
        ),
    ),
)
AUTONOMOUS_TERMINAL_ALL_BINDINGS = (
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "enum",
        "AutonomousLifecycleTerminalOutcomeStageV1",
        (
            "Pending {",
            "reserved_terminal: AutonomousLifecycleStableStateV1",
            "Complete {",
            "terminal: AutonomousLifecycleStableStateV1",
        ),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "method",
        "AutonomousLifecycleStableStateV1::terminal_outcome_pending_reservation",
        (
            "const ZERO_IDENTITY: AutonomousLifecycleCanonicalIdentityV1",
            "version: 0",
            "validator_count: 0",
            "producer: 0",
            "producer_selected_owner: 0",
            "replicated_carrier_owners: 0",
            "payload_binding_a: 0",
            "binding_a: ZERO_IDENTITY",
            "plan_state: 0",
            "selected_count: 0",
            "reservation_state: 0",
            "wsv_committed: false",
            "application_count: 0",
            "kura_retired: false",
            "fifo_restored: false",
        ),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "method",
        "AutonomousLifecycleTerminalOutcomeV1::pending",
        (
            "stage: AutonomousLifecycleTerminalOutcomeStageV1::Pending {",
            "reserved_terminal:",
            "AutonomousLifecycleStableStateV1::terminal_outcome_pending_reservation()",
            "Self::from_body(body)",
        ),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "method",
        "AutonomousLifecycleTerminalOutcomeV1::validate_body",
        (
            "match body.stage",
            "AutonomousLifecycleTerminalOutcomeStageV1::Pending { reserved_terminal }",
            "!reserved_terminal.is_terminal_outcome_pending_reservation()",
            "autonomous lifecycle pending terminal outcome changed its reserved terminal payload",
            "AutonomousLifecycleTerminalOutcomeStageV1::Complete { terminal }",
            "body.binding.validate_state(terminal)?",
        ),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "method",
        "AutonomousLifecycleCanonicalQueueSourceOutcomeAuthorization::consume_for_queue",
        (
            "lane_queue_reservation_group_binding_from_ordered_keys(",
            "derived == self.reservation_group",
            "self.source_outcome_hash",
            ".then_some((",
        ),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "method",
        "AutonomousLifecycleCanonicalCarrierSourceOutcomePublication::consume_for_v2_apply",
        (
            "let expected_count = entry.execution_batch.as_ref()?.lanes.len();",
            "self.queue_authorizations.len() == expected_count",
            "seen.insert(group.reservation_group_hash)",
            ".then_some(self.queue_authorizations)",
        ),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "method",
        "AutonomousLifecycleReleaseQueueSourceOutcomeAuthorization::consume_for_queue",
        (
            "self.barrier == *barrier",
            "self.source_outcome_hash",
            ".then_some(self.source_outcome_hash)",
        ),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "method",
        "AutonomousLifecyclePendingCanonicalCarrierRecovery::consume_for_v2_apply",
        (
            "pending_is_exact",
            "complete_is_disjoint",
            "expected_group_count",
            "self.pending_queue_authorizations.len()",
            "self.complete_reservation_groups.len()",
            "self.reference.matches_entry(&self.entry)",
        ),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "method",
        "AutonomousLifecyclePendingTerminalOutcomeRecovery::route_identities",
        (
            "pending_queue_authorizations",
            "complete_reservation_groups",
            "lane_incarnation",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "AutonomousLaneCanonicalQueueTerminalEvidence::consume_for_kura",
        (
            "IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED",
            "IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN",
            "terminal.canonical_wsv_owner",
            "terminal.commit_terminal",
            "self.source_outcome_hash",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "AutonomousLaneReleaseQueueTerminalEvidence::consume_for_kura",
        (
            "self.terminal_state.release.kura_retired",
            "self.terminal_state.release.fifo_restored",
            "terminal.ordinary_fifo_owner",
            "terminal.release_terminal",
            "self.source_outcome_hash",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "canonical_carrier_source_outcome_set_locked",
        (
            "entry_by_hash(entry_hash)?.as_ref() != Some(entry)",
            "let batch = entry.execution_batch.as_ref()",
            "for execution in &batch.lanes",
            "autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked",
            "prepare_autonomous_lifecycle_terminal_outcome_pending_locked",
            "preflight_autonomous_lifecycle_terminal_outcomes_pending_locked(",
            "publish_preflighted_autonomous_lifecycle_terminal_outcome_pending_locked(",
            "complete_reservation_groups.push(reservation_group)",
            "queue_authorizations.push((",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "persist_autonomous_lifecycle_canonical_terminal_outcomes_pending",
        (
            "canonical_carrier_source_outcome_set_locked(pending_canonical_bytes, entry, true)",
            "queue_authorizations.len() != expected_count",
            "AutonomousLifecycleCanonicalCarrierSourceOutcomePublication",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "reconstruct_autonomous_lifecycle_canonical_carrier_source_outcomes_for_group",
        (
            "read_lane_block_application_receipt_without_sidecar_repair",
            "entry_by_hash(merge_entry_hash)",
            ".canonical_carrier_source_outcome_set_locked(\n"
            "                pending_canonical_bytes,\n"
            "                &canonical_entry,\n"
            "                true,\n"
            "            )?",
            "queue_authorizations.len() != expected_count",
            "group == reservation_group",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "persist_autonomous_lifecycle_release_terminal_outcome_pending",
        (
            "record.retirement.as_ref() != Some(retirement)",
            "autonomous_lifecycle_terminal_source_matches_release_locked",
            "persist_autonomous_lifecycle_terminal_outcome_pending_locked",
            "retirement.queue_release_barrier()",
            "source_outcome_hash: outcome.outcome_hash",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "pending_autonomous_lifecycle_terminal_outcome_inventory",
        (
            "autonomous_lane_attempt_inventory_counts_locked(&entry, 1)",
            "if outcome.is_complete()",
            "queue_finalization_authorization",
            "canonical_carrier_source_outcome_set_locked(\n"
            "                pending_canonical_bytes,\n"
            "                &canonical_entry,\n"
            "                false,\n"
            "            )?",
            "pending_queue_authorizations",
            "complete_reservation_groups",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "method",
        "Kura::verify_expected_autonomous_lifecycle_terminal_outcome_stages",
        (
            "expected_groups.len() > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES",
            "self.prune_lock.lock()",
            "self.ensure_prune_recovery_not_required()",
            "self.canonical_chain_lock.lock()",
            "self.lane_geometry_lock.lock()",
            "lane_queue_reservation_group_binding_from_ordered_keys(expected_keys.iter())",
            "seen_entrypoint_hashes.insert(key.entrypoint_hash.clone())",
            "read_regular_sidecar_bytes(",
            "binding.reservation_group_binding() != expected_group",
            "payload.reservation_keys.as_slice() != expected.ordered_keys()",
            ".validate_for_payload(payload)",
            "cursor.binding() != binding",
            "autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked",
            "autonomous_lifecycle_terminal_source_matches_release_locked",
            "AutonomousLifecycleTerminalOutcomeDurableStage::Complete",
            "AutonomousLifecycleTerminalOutcomeDurableStage::Pending",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "complete_autonomous_lifecycle_terminal_outcome",
        (
            "current.outcome_hash != expected_source_outcome_hash",
            ".validate_for_payload(payload)",
            "read_autonomous_lifecycle_cursor_for_terminal_outcome_locked",
            "autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked",
            "autonomous_lifecycle_terminal_source_matches_release_locked",
            "if let Some(existing) = current",
            "let complete = current",
            "let next_bytes = complete.encode_framed()",
            "if next_bytes.len() != current_bytes.len()",
            "autonomous lifecycle terminal stage changed its fixed framed length",
            "write_atomic_synced_replace(&path, &next_bytes)",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "complete_autonomous_lifecycle_canonical_terminal_outcome",
        (
            "evidence.consume_for_kura()",
            "canonical Queue terminal evidence is malformed",
            "complete_autonomous_lifecycle_terminal_outcome(",
            "true",
            "expected_source_outcome_hash",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "complete_autonomous_lifecycle_release_terminal_outcome",
        (
            "evidence.consume_for_kura()",
            "release Queue terminal evidence is malformed",
            "complete_autonomous_lifecycle_terminal_outcome(",
            "false",
            "expected_source_outcome_hash",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "finalize_certified_merge_reservations",
        (
            "authenticate_committed_canonical_carrier",
            "persist_autonomous_lifecycle_canonical_terminal_outcomes_pending",
            ".consume_for_v2_apply(entry)",
            ".queue_cleanup_authorization()",
            "authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcomes",
            "complete_autonomous_lifecycle_canonical_terminal_outcome",
            "release_post_wsv_lane_artifact_budget_reservation",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "retire_autonomous_lane_slot_and_release_reservations",
        (
            "persist_autonomous_lane_slot_retirement",
            "prepare_lane_reservation_release_barrier_with_authorization",
            "finalize_autonomous_lane_slot_release_with_authorization",
            "persist_autonomous_lifecycle_release_terminal_outcome_pending",
            "finalize_lane_reservation_release_barrier_with_authorization",
            "complete_autonomous_lifecycle_release_terminal_outcome",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply/reconciliation_authority.rs",
        "fn",
        "recover_pending_canonical_terminal_outcome",
        (
            "recovery.consume_for_v2_apply()",
            "authenticate_committed_canonical_carrier",
            "complete_groups",
            "pending_sources",
            "authenticated_groups.is_empty()",
            "authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcomes",
            "terminal_evidence.len() != expected_terminal_evidence",
            "complete_autonomous_lifecycle_canonical_terminal_outcome",
            "release_post_wsv_lane_artifact_budget_reservation",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply/reconciliation_authority.rs",
        "fn",
        "recover_pending_release_terminal_outcome",
        (
            "lane_reservation_reconciliation_snapshot()",
            "finalize_autonomous_lifecycle_pending_release_queue_terminal_outcome",
            "authenticate_autonomous_lifecycle_pending_release_queue_terminal_outcome",
            "complete_autonomous_lifecycle_release_terminal_outcome",
        ),
    ),
    (
        SUMERAGI_COMMITTED_CARRIER_CLEANUP_RELATIVE,
        "fn",
        "finalize_startup_committed_canonical_carriers",
        (
            "if authorized_commit_groups.is_empty()",
            "let anchored_carrier_bound = authorized_commit_groups.len();",
            "let mut planned_authorizations = BTreeMap::new();",
            "let mut carrier_publications = BTreeMap::new();",
            ".reconstruct_autonomous_lifecycle_canonical_carrier_source_outcomes_for_group(",
            ".entry(publication.entry_hash())",
            "let mut carrier_heights = BTreeMap::new();",
            "let mut source_authorized_carriers = Vec::with_capacity(carrier_publications.len());",
            "authenticate_committed_canonical_carrier(state, kura, &entry, network_id)?",
            ".insert(authenticated.carrier_height, entry_hash)",
            "publication.consume_for_v2_apply(&entry)",
            "let mut carrier_groups = Vec::with_capacity(authenticated.groups.len());",
            "carrier_groups.push((source_authorization, reconstructed_authorization));",
            "source_authorized_carriers.push((",
            "if !planned_authorizations.is_empty()",
            ".sort_by_key(|(height, entry_hash, _, _, _)| (*height, *entry_hash));",
            "let mut carrier_releases = Vec::with_capacity(source_authorized_carriers.len());",
            "for (height, _, entry, carrier_block_hash, _) in &source_authorized_carriers",
            "carrier_releases.push((",
            ".authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome_carriers(",
            ".map(|(_, _, _, _, groups)| groups)",
            "anchored_carrier_bound",
            "kura.complete_autonomous_lifecycle_canonical_terminal_outcome(evidence)?;",
            "for (entry, carrier_height, carrier_block_hash) in carrier_releases",
            "release_post_wsv_lane_artifact_budget_reservation",
        ),
    ),
    (
        QUEUE_CANONICAL_TERMINAL_CLEANUP_RELATIVE,
        "method",
        "Queue::authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcomes",
        (
            "self.authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome_carriers(",
            "vec![groups]",
            "1",
        ),
    ),
    (
        QUEUE_CANONICAL_TERMINAL_CLEANUP_RELATIVE,
        "method",
        "Queue::authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome_carriers",
        (
            "let mut prepared_carriers = Vec::with_capacity(carriers.len());",
            "for groups in carriers",
            "for (pending, carrier) in groups",
            "pending.consume_for_queue()",
            "LaneQueueCarrierCleanupGate::from_authorization",
            "prepared.push(PreparedLaneQueueCarrierCleanupGroup",
            "prepared_carriers.push(prepared);",
            "self.commit_prepared_lane_reservation_carriers(prepared_carriers, anchored_carrier_bound)",
        ),
    ),
    (
        QUEUE_CANONICAL_TERMINAL_CLEANUP_RELATIVE,
        "method",
        "Queue::validate_lane_queue_carrier_cleanup_batch_bounds",
        (
            "carrier_reservation_counts.is_empty()",
            "anchored_carrier_bound == 0",
            "anchored_carrier_bound > self.capacity.get()",
            "carrier_reservation_counts.len() > anchored_carrier_bound",
            "for count in carrier_reservation_counts",
            "*count > iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS",
            "aggregate.checked_add(*count)",
        ),
    ),
    (
        QUEUE_CANONICAL_TERMINAL_CLEANUP_RELATIVE,
        "method",
        "Queue::commit_prepared_lane_reservation_groups",
        (
            "self.commit_prepared_lane_reservation_carriers(vec![groups], 1)",
        ),
    ),
    (
        QUEUE_CANONICAL_TERMINAL_CLEANUP_RELATIVE,
        "method",
        "Queue::commit_prepared_lane_reservation_carriers",
        (
            "let carrier_reservation_counts = carriers",
            "groups.iter().try_fold(0_usize",
            ".collect::<Option<Vec<_>>>()",
            "self.validate_lane_queue_carrier_cleanup_batch_bounds(",
            "let group_count = carriers",
            "carrier.len()",
            "let mut seen_group_slot_keys = BTreeSet::new();",
            "for group in carriers.iter().flatten()",
            "seen_group_hashes.insert",
            "seen_group_identities.insert",
            "seen_group_slot_keys.insert",
            "seen_hashes.insert",
            "seen_entrypoints.insert",
            "preflight_lane_reservation_group_locked",
            ".flat_map(|group| group.ordered_keys.iter())",
            "preflight_lane_reservation_plan_journal",
            "terminal_evidence\n            .try_reserve_exact(group_count)",
            "for group in carriers.into_iter().flatten()",
            "self.commit_lane_reservation(",
            "terminal_evidence.push",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::preflight_lane_reservation_plan_journal",
        (
            "let guard = self.plan_journal.lock();",
            "journal.observe_startup_replay_receipt_with_finalized_absence(",
            "&preflight.active_phases",
            "&preflight.finalized_keys",
        ),
    ),
    (
        QUEUE_JOURNAL_RESERVATION_COMMIT_PREFLIGHT_RELATIVE,
        "fn",
        "observe_startup_replay_receipt",
        (
            "self.observe_startup_replay_receipt_with_finalized_absence(phases, &[])",
        ),
    ),
    (
        QUEUE_JOURNAL_RESERVATION_COMMIT_PREFLIGHT_RELATIVE,
        "fn",
        "observe_startup_replay_receipt_with_finalized_absence",
        (
            "if phases.len() > self.limits.max_live_records",
            "for phase in phases",
            "for key in finalized_keys",
            "let mut replay = self.prepare_replay_with_removed_entrypoints(Some(&entrypoints))?;",
            "replay.verify_snapshot_content()?;",
            "let live_claims =",
            "queue_plan_startup_reservation_phase_root(phases)?",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "pending_terminal_recovery_observations",
        (
            "recovery.network_id() != network_id",
            "let route_identities = recovery.route_identities();",
            "recovery.pending_reservation_groups()",
            "pending_groups.len() != recovery.pending_outcome_count()",
            "lane_queue_reservation_group_binding_from_ordered_keys(",
            "reservation_group_hash",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "pending_terminal_group_has_exact_queue_owner",
        (
            "let expected_keys = observation",
            "phase_identity == binding.identity",
            "expected != Some(&phase.key)",
            "has_exact_owner = true;",
            "autonomous lifecycle terminal recovery transaction is owned by another Queue identity",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "reconcile_pending_autonomous_lifecycle_terminal_outcomes",
        (
            "let initial_queue_quarantine = queue.lane_reservation_startup_reconciliation_pending();",
            "let initial_snapshot = queue",
            "non-empty Queue startup snapshot was published before terminal-outcome pre-sweep",
            "active_lifecycle_routes(state, context)",
            "pending_autonomous_lifecycle_terminal_outcome_inventory",
            "pending_terminal_recovery_observations(&recovery, network_id, &active_routes)?",
            "pending_terminal_group_has_exact_queue_owner(&initial_snapshot, observation)?",
            "let deferred = !owned_group_hashes.is_empty();",
            "pending_groups: pending_groups.clone()",
            "if preflight.deferred",
            "recover_pending_autonomous_lifecycle_terminal_outcome",
            "pre-planner autonomous terminal recovery consumed a Queue owner",
            "terminal recovery changed the immutable Queue snapshot",
            "terminal recovery changed the Queue owner-quarantine state",
            "unit_has_exact_owner |=",
            "observed_deferred_units != expected_deferred_units",
            "deferred_terminal_recovery",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions",
        (
            "let queue_snapshot = queue",
            "for (unit_index, unit) in deferred.units.iter().enumerate()",
            "unit.owned_group_hashes.is_empty() || observed_identity != unit.identity",
            "for (group_position, observation) in unit.pending_groups.iter().enumerate()",
            "pending_terminal_group_has_exact_queue_owner(&queue_snapshot, observation)?",
            "verify_expected_autonomous_lifecycle_terminal_outcome_stages(",
            "deferred autonomous terminal inventory contains an unplanned Pending group",
            "previous_group_position.is_some_and(|previous| previous >= group_position)",
            "deferred autonomous terminal inventory split one whole recovery unit",
            "observed_groups != expected_pending_groups",
            "recover_pending_autonomous_lifecycle_terminal_outcome(",
            "deferred autonomous terminal completion consumed a post-plan Queue owner",
            "deferred autonomous terminal completion changed the post-plan Queue snapshot",
            "deferred terminal final stage proof failed",
            "final_stage.stage() != AutonomousLifecycleTerminalOutcomeDurableStage::Complete",
            "deferred autonomous terminal completion left a Pending source",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "exact_current_queue_group_matches",
        (
            "let expected = binding.reservation_group_binding();",
            "let Some(current_keys) = current_queue_groups.get(&expected.identity) else",
            "usize::try_from(expected.reservation_count).ok() == Some(ordered_keys.len())",
            "current_keys.as_slice() == ordered_keys",
            "lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter()).ok()",
            "lane_queue_reservation_group_binding_from_ordered_keys(current_keys.iter()).ok()",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "require_local_producer_queue_owner",
        (
            "local_actor != binding.producer_actor_projection()",
            "exact_current_queue_group_matches",
            "lost its exact current Queue reservation owner",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "reconcile_autonomous_lifecycle_startup",
        (
            "let initial_queue_quarantine",
            "let snapshot = queue",
            "if !snapshot.is_empty() && !initial_queue_quarantine",
            "bind_lane_reservation_startup_reconciliation_receipt(&snapshot)",
            "let mut seen_pending_identities = BTreeSet::new();",
            "for unit in &deferred_terminal_recovery.units",
            "pending_terminal_group_has_exact_queue_owner(&snapshot, observation)?",
            "!seen_pending_identities.insert(pending.identity)",
            "unit_has_planner_anchor |= marked_owned",
            "let mut current_queue_groups",
            "seen_pending_identities.contains(",
            "ProducerQueue bootstrap lost its exact current durable Queue owner",
            "seen_pending_identities.contains(&identity)",
            "require_local_producer_queue_owner(payload, cursor, &current_queue_groups)",
            "for authority in bootstraps",
            "complete_autonomous_lifecycle_bootstrap",
            "if recover_one_attempt(",
            "revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)",
            "changed the Queue owner-quarantine state",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "plan_lane_reservation_ownership",
        (
            "let current_snapshot = queue.lane_reservation_reconciliation_snapshot()?;",
            "handoff.into_queue_handoff()",
            "revalidate_lane_reservation_startup_reconciliation_receipt(",
            "deferred_terminal_recovery",
            "if queue.lane_reservation_reconciliation_snapshot()? != snapshot",
            "let replay_receipt = match recovered_receipt",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "apply_lane_reservation_reconciliation_plan",
        (
            "replay_receipt",
            "deferred_terminal_recovery",
            "revalidate_lane_reservation_startup_reconciliation_receipt(&replay_receipt, &snapshot)",
            "finalize_startup_committed_canonical_carriers(",
            "let final_snapshot = queue.lane_reservation_reconciliation_snapshot()?;",
            "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions(",
            "queue.complete_lane_reservation_startup_reconciliation(replay_receipt)?;",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_terminal_recovery.rs",
        "fn",
        "reconcile_lifecycle_terminal_outcomes_before_queue_planning",
        (
            "begin_fail_stop_operation()",
            "reconcile_pending_autonomous_lifecycle_terminal_outcomes(state, queue, kura, context)",
            "recovery.complete();",
            "summary.completed_outcomes()",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_non_pending_lifecycle_loop",
        (
            "if reservation_reconciliation_pending",
            "reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
            "let planning = plan_lane_reservation_ownership(",
            "reconcile_autonomous_lifecycle_startup(",
            "apply_lane_reservation_reconciliation_plan(",
            "reservation_reconciliation_pending = false;",
            "construct_after_pending_tip_application_recovery(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "reconcile_pending_lane_startup",
        (
            "reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
            "let planning = plan_lane_reservation_ownership(",
            "reconcile_autonomous_lifecycle_startup(",
            "apply_lane_reservation_reconciliation_plan(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
        "method",
        "PendingKuraProductionLifecycleV1::prepare_lane_recovery",
        (
            "lane_work.install_lane_drain_queue(Arc::clone(&queue))?;",
            "lane_work.activate_after_lane_drain_queue_install(&queue)?;",
        ),
    ),
    (
        KURA_LANE_ARTIFACT_BUDGET_RELATIVE,
        "method",
        "Kura::post_wsv_lane_artifact_budget_plan",
        (
            "carrier_height: u64",
            "carrier_hash: HashOf<BlockHeader>",
            "entry.execution_batch.as_ref()",
            "if batch.lanes.is_empty()",
            "let mut stable_components = BTreeMap::new();",
            "let mut executions = BTreeMap::new();",
            "BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES",
            "for execution in &batch.lanes",
            "LaneBlockApplicationReceiptArtifact::new_merge_execution(",
            "Self::validate_lane_block_application_receipt_artifact(&receipt)",
            "receipt.encode_framed()?",
            "Self::maximum_index_growth_for_unresolved_sidecar_write(",
            "LaneMergeApplicationFrontierV1::from_receipt(&receipt)",
            "norito::encode_canonical(&frontier)",
            "shared_transient_bytes = shared_transient_bytes.max(frontier_len);",
            "PostWsvLaneArtifactStableComponentId::Receipt(identity)",
            "PostWsvLaneArtifactStableComponentId::Frontier(identity)",
            "Self::decode_autonomous_lane_merge_bundle(",
            "Self::autonomous_lifecycle_terminal_source_from_merge_receipt(",
            "PostWsvLaneArtifactExecutionPlan {",
            "PostWsvLaneArtifactBudgetPlan {",
        ),
    ),
    (
        KURA_LANE_ARTIFACT_BUDGET_RELATIVE,
        "method",
        "PostWsvLaneArtifactBudgetPlan::initial_reserved_bytes",
        (
            "self.stable_components",
            ".values()",
            ".try_fold(self.shared_transient_bytes",
            "total.checked_add(*bytes)",
        ),
    ),
    (
        KURA_LANE_ARTIFACT_BUDGET_RELATIVE,
        "method",
        "Kura::merge_lane_application_artifact_required_bytes_for_carrier",
        (
            "self.post_wsv_lane_artifact_budget_plan(entry, carrier_height, carrier_hash)?",
            ".map_or(Ok(0), |plan| {",
            "plan.initial_reserved_bytes().ok_or_else(|| {",
            "merge application artifact byte accounting overflowed",
        ),
    ),
    (
        KURA_AUTONOMOUS_TERMINAL_CAPACITY_RELATIVE,
        "method",
        "Kura::validate_configured_autonomous_mutation_disk_peak_locked",
        (
            "self.validate_configured_autonomous_mutation_disk_peak_with_allowed_view_temp_locked(",
            "additional_physical_peak_bytes",
            "creates_lifecycle_identity",
            "consumes_terminal_cas_transient",
            "path",
            "None",
        ),
    ),
    (
        KURA_AUTONOMOUS_TERMINAL_CAPACITY_RELATIVE,
        "method",
        "Kura::validate_configured_autonomous_mutation_disk_peak_with_allowed_view_temp_locked",
        (
            "allowed_view_temp: Option<&Path>",
            "autonomous_global_terminal_reservation_counts_with_allowed_view_temp_locked(",
            "allowed_view_temp",
            "AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES",
            "resulting_missing",
            "resulting_incomplete",
            "MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES",
            "stable_terminal_reservations",
            "shared_terminal_transient",
            "consumes_terminal_cas_transient",
            "self.post_wsv_lane_artifact_budget_reserved_bytes()?",
            ".kura_disk_usage_bytes()?",
            "bytes.checked_add(stable_terminal_reservations)",
            "bytes.checked_add(post_wsv_reservations)",
            "required > self.max_disk_usage_bytes",
        ),
    ),
    (
        KURA_LANE_ARTIFACT_BUDGET_RELATIVE,
        "method",
        "Kura::merge_lane_application_artifact_required_bytes_for_block",
        (
            "Self::block_merge_reference(block)",
            "merge_entry.ok_or(Error::MissingCertifiedMergeSidecar",
            "reference.matches_entry(entry)",
            "self.merge_lane_application_artifact_required_bytes_for_carrier(",
            "block.header().height().get()",
            "block.hash()",
        ),
    ),
    (
        KURA_LANE_ARTIFACT_BUDGET_RELATIVE,
        "method",
        "Kura::lane_artifact_required_bytes_for_block",
        (
            "merge_entry: Option<&MergeLedgerEntry>",
            "self.merge_lane_application_artifact_required_bytes_for_block(block, merge_entry)?",
            "Self::maximum_index_growth_for_unresolved_sidecar_write(",
            "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::block_required_bytes_for_budget",
        (
            "merge_entry: Option<&MergeLedgerEntry>",
            "let required = Self::block_required_bytes(block)?;",
            "self.lane_artifact_required_bytes_for_block(block, merge_entry)?",
        ),
    ),
    (
        "crates/iroha_core/src/kura/tests/07f_canonical_carrier_terminal_recovery_tests.rs",
        "fn",
        "canonical_carrier_terminal_recovery_materializes_and_partitions_the_full_lane_set",
        (
            "zero-file crash boundary starts without a terminal outcome seed",
            "remove strict-prefix second outcome",
            "direct expected-stage proof must reject a deleted handoff member without reconstruction",
            "strict-prefix inventory reconstructs every missing carrier member",
            "a malformed later carrier member must prevent every recovery token from returning",
            "directly prove mixed Complete/Pending carrier stages",
            "AutonomousLifecycleTerminalOutcomeDurableStage::Complete",
            "AutonomousLifecycleTerminalOutcomeDurableStage::Pending",
            "pending.len(), 1",
            "complete.len(), 1",
            "directly prove both completed carrier members",
        ),
    ),
    (
        "crates/iroha_core/src/kura/tests/07f_canonical_carrier_terminal_recovery_tests.rs",
        "fn",
        "lifecycle_release_terminal_outcomes_are_exact_idempotent_and_ordered",
        (
            "persist exact Pending release outcome",
            "default lifecycle inventory must keep every Pending outcome fail-closed",
            "exact planner-covered Pending group must be source-validated and exposed",
            "a missing artifact namespace must not accept unused planner Pending coverage",
            "Pending must reject any non-canonical reserved terminal payload",
            "Pending reserves the exact framed length required by Complete",
            "a multi-outcome namespace exactly at its stable aggregate bound must retain completion headroom",
            "complete_len.saturating_add(1)",
            "the near-budget regression must detect any accidental Complete growth",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_recovery.rs",
        "fn",
        "local_producer_recovery_requires_the_exact_current_queue_owner",
        (
            "payload bytes alone must not replace producer Queue custody",
            "same-slot but byte-different Queue custody must fail closed",
            "the byte-exact current Queue group authenticates producer recovery custody",
        ),
    ),
    (
        "crates/iroha_core/src/queue/lane_reservation_tests.rs",
        "fn",
        "canonical_cleanup_rejects_empty_and_oversized_group_batches_before_mutation",
        (
            "canonical cleanup must reject an empty group batch",
            "non-empty carrier set",
            "two independently bounded carriers may exceed one carrier aggregate",
            "carrier batches cannot exceed their exact startup-anchor bound",
            "MAX_MERGE_EXECUTION_ENTRYPOINTS",
            "canonical cleanup must reject an oversized all-group batch",
            "exceeds hard limit",
            "whole-call bounds must fail before any Queue owner changes",
        ),
    ),
    (
        "crates/iroha_core/src/queue/lane_reservation_tests.rs",
        "fn",
        "empty_startup_reconciliation_receipt_publishes_with_gate_already_open",
        (
            "assert!(receipt.initial_snapshot.is_empty());",
            "!queue.lane_reservation_startup_reconciliation_pending()",
            "complete_lane_reservation_startup_reconciliation(receipt)",
            "an exact empty replay receipt may idempotently publish an open gate",
        ),
    ),
    (
        "crates/iroha_core/src/queue/lane_reservation_tests.rs",
        "fn",
        "reservation_restart_release_restores_exact_global_fifo",
        (
            "binds_reconciliation_snapshot(&drifted_timestamp)",
            "equal-count live owners cannot substitute another enqueue timestamp",
            "binds_reconciliation_snapshot(&drifted_fifo)",
            "equal-count proposal groups cannot reorder their exact FIFO membership",
            "equal-count proposal groups cannot replace one exact member",
            "complete_lane_reservation_startup_reconciliation(reconciliation_receipt)",
            "hashes",
        ),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        "fn",
        "finalized_carrier_absence_may_exceed_live_owner_bound_in_one_snapshot",
        (
            "QueuePlanJournal::open_with_limits(&path, limits(1), true)",
            "observe_startup_replay_receipt_with_finalized_absence(&[], &finalized_keys)",
            "two finalized carrier keys may exceed the one-live-owner bound",
            "journal.replay_scan_count(),\n            1",
            "active and finalized QueuePlan evidence must share one immutable replay snapshot",
            "replace_strict_durable(second_record)",
            "a finalized carrier key must reject its exact live QueuePlan owner",
            "the conflicting finalized owner must be classified by the same single replay",
        ),
    ),
)
AUTONOMOUS_TERMINAL_RECOVERY_BINDINGS = AUTONOMOUS_TERMINAL_ALL_BINDINGS[:-7]
AUTONOMOUS_TERMINAL_TEST_BINDINGS = AUTONOMOUS_TERMINAL_ALL_BINDINGS[-7:]
AUTONOMOUS_TERMINAL_ORDERED_SOURCE_CHECKS = (
    (
        KURA_LANE_ARTIFACT_BUDGET_RELATIVE,
        "method",
        "Kura::post_wsv_lane_artifact_budget_plan",
        (
            "let Some(batch) = entry.execution_batch.as_ref() else {",
            "let mut stable_components = BTreeMap::new();",
            "let mut executions = BTreeMap::new();",
            "let mut shared_transient_bytes = u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES)?;",
            "for execution in &batch.lanes",
            "LaneBlockApplicationReceiptArtifact::new_merge_execution(",
            "let receipt_bytes = receipt.encode_framed()?;",
            "LaneMergeApplicationFrontierV1::from_receipt(&receipt)",
            "let frontier_bytes = norito::encode_canonical(&frontier)",
            "shared_transient_bytes = shared_transient_bytes.max(frontier_len);",
            "Self::maximum_index_growth_for_unresolved_sidecar_write(",
            "PostWsvLaneArtifactStableComponentId::Receipt(identity)",
            "PostWsvLaneArtifactStableComponentId::Frontier(identity)",
            "Self::decode_autonomous_lane_merge_bundle(",
            "Self::autonomous_lifecycle_terminal_source_from_merge_receipt(",
            "PostWsvLaneArtifactExecutionPlan {",
            "Ok(Some(PostWsvLaneArtifactBudgetPlan {",
        ),
    ),
    (
        KURA_LANE_ARTIFACT_BUDGET_RELATIVE,
        "method",
        "PostWsvLaneArtifactBudgetPlan::initial_reserved_bytes",
        (
            "self.stable_components",
            ".values()",
            ".try_fold(self.shared_transient_bytes",
            "total.checked_add(*bytes)",
        ),
    ),
    (
        KURA_LANE_ARTIFACT_BUDGET_RELATIVE,
        "method",
        "Kura::merge_lane_application_artifact_required_bytes_for_carrier",
        (
            "self.post_wsv_lane_artifact_budget_plan(entry, carrier_height, carrier_hash)?",
            ".map_or(Ok(0), |plan| {",
            "plan.initial_reserved_bytes().ok_or_else(|| {",
        ),
    ),
    (
        KURA_LANE_ARTIFACT_BUDGET_RELATIVE,
        "method",
        "Kura::lane_artifact_required_bytes_for_block",
        (
            "let mut total =",
            "self.merge_lane_application_artifact_required_bytes_for_block(block, merge_entry)?",
            "if let Some(bundle) = block.execution_context()",
            "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(",
            "merge_entry,",
            "Ok(total)",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::block_required_bytes_for_budget",
        (
            "let required = Self::block_required_bytes(block)?;",
            "self.lane_artifact_required_bytes_for_block(block, merge_entry)?",
        ),
    ),
    (
        QUEUE_CANONICAL_TERMINAL_CLEANUP_RELATIVE,
        "method",
        "Queue::authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome_carriers",
        (
            "let mut prepared_carriers = Vec::with_capacity(carriers.len());",
            "for groups in carriers",
            "let mut prepared = Vec::with_capacity(groups.len());",
            "for (pending, carrier) in groups",
            "pending.consume_for_queue()",
            "prepared.push(PreparedLaneQueueCarrierCleanupGroup",
            "prepared_carriers.push(prepared);",
            "self.commit_prepared_lane_reservation_carriers(prepared_carriers, anchored_carrier_bound)",
        ),
    ),
    (
        QUEUE_CANONICAL_TERMINAL_CLEANUP_RELATIVE,
        "method",
        "Queue::validate_lane_queue_carrier_cleanup_batch_bounds",
        (
            "carrier_reservation_counts.is_empty()",
            "carrier_reservation_counts.len() > anchored_carrier_bound",
            "let mut aggregate = 0_usize;",
            "for count in carrier_reservation_counts",
            "*count > iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS",
            "aggregate.checked_add(*count)",
            "Ok(aggregate)",
        ),
    ),
    (
        QUEUE_JOURNAL_RESERVATION_COMMIT_PREFLIGHT_RELATIVE,
        "fn",
        "observe_startup_replay_receipt_with_finalized_absence",
        (
            "if phases.len() > self.limits.max_live_records",
            "for phase in phases",
            "for key in finalized_keys",
            "let mut replay = self.prepare_replay_with_removed_entrypoints(Some(&entrypoints))?;",
            "replay.verify_snapshot_content()?;",
            "for phase in phases",
            "for key in finalized_keys",
            "let live_claims =",
            "queue_plan_startup_reservation_phase_root(phases)?",
            "replay.verify_snapshot_content()?;",
            "Ok(QueuePlanStartupReplayReceiptV1",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "complete_autonomous_lifecycle_terminal_outcome",
        (
            "let complete = current",
            ".complete(terminal)",
            "let next_bytes = complete.encode_framed()",
            "if next_bytes.len() != current_bytes.len()",
            "autonomous lifecycle terminal stage changed its fixed framed length",
            "autonomous_lane_attempt_inventory_counts_locked",
            "validate_autonomous_lifecycle_terminal_outcome_budget(",
            "write_atomic_synced_replace(&path, &next_bytes)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "exact_current_queue_group_matches",
        (
            "let expected = binding.reservation_group_binding();",
            "let Some(current_keys) = current_queue_groups.get(&expected.identity) else",
            "usize::try_from(expected.reservation_count).ok() == Some(ordered_keys.len())",
            "current_keys.as_slice() == ordered_keys",
            "lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter()).ok()",
            "== Some(expected)",
            "lane_queue_reservation_group_binding_from_ordered_keys(current_keys.iter()).ok()",
            "== Some(expected)",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "method",
        "Kura::verify_expected_autonomous_lifecycle_terminal_outcome_stages",
        (
            "expected_groups.len() > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES",
            "self.prune_lock.lock()",
            "self.canonical_chain_lock.lock()",
            "self.lane_geometry_lock.lock()",
            "for expected in expected_groups",
            "lane_queue_reservation_group_binding_from_ordered_keys(expected_keys.iter())",
            "seen_entrypoint_hashes.insert(key.entrypoint_hash.clone())",
            "preflighted.push((expected, expected_group, entry, path));",
            "self.sidecar_lock.lock()",
            "for (expected, expected_group, entry, path) in preflighted",
            "read_regular_sidecar_bytes(",
            "binding.reservation_group_binding() != expected_group",
            "payload.reservation_keys.as_slice() != expected.ordered_keys()",
            ".validate_for_payload(payload)",
            "cursor.binding() != binding",
            "verified.push(",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "fn",
        "canonical_carrier_source_outcome_set_locked",
        (
            "entry_by_hash(entry_hash)?.as_ref() != Some(entry)",
            "let batch = entry.execution_batch.as_ref()",
            "for execution in &batch.lanes",
            "autonomous_lifecycle_terminal_source_matches_canonical_carrier_locked",
            "prepare_autonomous_lifecycle_terminal_outcome_pending_locked",
            "let is_complete = outcome.is_complete();",
            "queue_authorizations.push((",
            "preflight_autonomous_lifecycle_terminal_outcomes_pending_locked(",
            "for publication_plan in &terminal_publication_plans",
            "publish_preflighted_autonomous_lifecycle_terminal_outcome_pending_locked(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "finalize_certified_merge_reservations",
        (
            "authenticate_committed_canonical_carrier",
            "let carrier_height = authenticated.carrier_height;",
            "let carrier_block_hash = authenticated.carrier_block_hash;",
            "persist_autonomous_lifecycle_canonical_terminal_outcomes_pending",
            ".consume_for_v2_apply(entry)",
            "for (group, (source_group, source_authorization)) in",
            ".queue_cleanup_authorization()",
            "authorized_groups.push((source_authorization, authorization))",
            ".authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcomes(",
            "let (_, terminal_evidence) = cleanup.into_parts();",
            "for evidence in terminal_evidence",
            "kura.complete_autonomous_lifecycle_canonical_terminal_outcome(evidence)",
            "kura.release_post_wsv_lane_artifact_budget_reservation(",
            "u64::try_from(carrier_height.get())?",
            "carrier_block_hash",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "plan_lane_reservation_ownership",
        (
            "let current_snapshot = queue.lane_reservation_reconciliation_snapshot()?;",
            "handoff.into_queue_handoff()",
            "revalidate_lane_reservation_startup_reconciliation_receipt(",
            "if snapshot.is_empty()",
            "if queue.lane_reservation_reconciliation_snapshot()? != snapshot",
            "let replay_receipt = match recovered_receipt",
            "LaneReservationReconciliationPlan {",
            "deferred_terminal_recovery",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "apply_lane_reservation_reconciliation_plan",
        (
            "revalidate_lane_reservation_startup_reconciliation_receipt(&replay_receipt, &snapshot)",
            "for action in actions",
            "finalize_startup_committed_canonical_carriers(",
            "for action in remaining_actions",
            "let final_snapshot = queue.lane_reservation_reconciliation_snapshot()?;",
            "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions(",
            "queue.complete_lane_reservation_startup_reconciliation(replay_receipt)?;",
        ),
    ),
    (
        SUMERAGI_COMMITTED_CARRIER_CLEANUP_RELATIVE,
        "fn",
        "finalize_startup_committed_canonical_carriers",
        (
            "let anchored_carrier_bound = authorized_commit_groups.len();",
            "for (ordered_keys, carrier_authorization) in authorized_commit_groups",
            ".reconstruct_autonomous_lifecycle_canonical_carrier_source_outcomes_for_group(",
            "carrier_publications",
            "for (entry_hash, publication) in carrier_publications",
            "authenticate_committed_canonical_carrier(state, kura, &entry, network_id)",
            ".insert(authenticated.carrier_height, entry_hash)",
            "publication.consume_for_v2_apply(&entry)",
            "for (group, (source_group, source_authorization)) in",
            ".queue_cleanup_authorization()",
            "carrier_groups.push((source_authorization, reconstructed_authorization));",
            "source_authorized_carriers.push((",
            "if !planned_authorizations.is_empty()",
            ".sort_by_key(|(height, entry_hash, _, _, _)| (*height, *entry_hash));",
            "let mut carrier_releases = Vec::with_capacity(source_authorized_carriers.len());",
            "for (height, _, entry, carrier_block_hash, _) in &source_authorized_carriers",
            "carrier_releases.push((",
            "u64::try_from(height.get())?",
            ".authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome_carriers(",
            ".map(|(_, _, _, _, groups)| groups)",
            "anchored_carrier_bound",
            "kura.complete_autonomous_lifecycle_canonical_terminal_outcome(evidence)",
            "for (entry, carrier_height, carrier_block_hash) in carrier_releases",
            "kura.release_post_wsv_lane_artifact_budget_reservation(",
            "&entry,",
            "carrier_height,",
            "carrier_block_hash,",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "retire_autonomous_lane_slot_and_release_reservations",
        (
            "kura.persist_autonomous_lane_slot_retirement(",
            "queue.prepare_lane_reservation_release_barrier_with_authorization(",
            ".finalize_autonomous_lane_slot_release_with_authorization(",
            ".persist_autonomous_lifecycle_release_terminal_outcome_pending(",
            "queue.finalize_lane_reservation_release_barrier_with_authorization(",
            "completion.into_parts();",
            "kura.complete_autonomous_lifecycle_release_terminal_outcome(terminal_evidence)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply/reconciliation_authority.rs",
        "fn",
        "recover_pending_canonical_terminal_outcome",
        (
            "recovery.consume_for_v2_apply()",
            "authenticate_committed_canonical_carrier",
            "for complete in complete_groups",
            "if observed.reservation_group != complete",
            "for (pending_group, source_authorization) in pending_sources",
            ".queue_cleanup_authorization()",
            "queue_groups.push((source_authorization, carrier_authorization))",
            "if !authenticated_groups.is_empty()",
            ".authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcomes(",
            "terminal_evidence.len() != expected_terminal_evidence",
            "kura.complete_autonomous_lifecycle_canonical_terminal_outcome(evidence)",
            "kura.release_post_wsv_lane_artifact_budget_reservation(",
            "&entry,",
            "carrier_block_height,",
            "carrier_block_hash,",
        ),
    ),
    (
        QUEUE_CANONICAL_TERMINAL_CLEANUP_RELATIVE,
        "method",
        "Queue::commit_prepared_lane_reservation_carriers",
        (
            "let carrier_reservation_counts = carriers",
            "groups.iter().try_fold(0_usize",
            ".collect::<Option<Vec<_>>>()",
            "self.validate_lane_queue_carrier_cleanup_batch_bounds(",
            "let group_count = carriers",
            "terminal_evidence\n            .try_reserve_exact(group_count)",
            "self.transaction_selection_durability_faulted()",
            "self.lane_reservation_transition_lock.lock()",
            "self.wait_for_durability_transitions(&cleanup_hashes)",
            "let mut queue_guard = self.push_remove_lock.lock()",
            "while cleanup_hashes",
            ".any(|hash| self.durability_transition_active(hash))",
            "drop(queue_guard);\n            self.wait_for_durability_transitions(&cleanup_hashes);",
            "queue_guard = self.push_remove_lock.lock()",
            "let mut seen_group_slot_keys = BTreeSet::new();",
            "for group in carriers.iter().flatten()",
            "seen_group_hashes.insert",
            "seen_group_identities.insert",
            "seen_group_slot_keys.insert",
            "seen_hashes.insert",
            "seen_entrypoints.insert",
            "self.preflight_lane_reservation_group_locked(",
            "drop(ownership);",
            "begin_durability_transition_locked(",
            ".flat_map(|group| group.ordered_keys.iter())",
            "drop(queue_guard);",
            "self.preflight_lane_reservation_plan_journal(&journal_preflight)",
            "for group in carriers.into_iter().flatten()",
            "self.commit_lane_reservation(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "reconcile_pending_autonomous_lifecycle_terminal_outcomes",
        (
            "let initial_queue_quarantine",
            "let initial_snapshot = queue",
            "non-empty Queue startup snapshot was published before terminal-outcome pre-sweep",
            "let active_routes = active_lifecycle_routes(state, context)",
            "let network_id = context.network_id;",
            "let recoveries = kura",
            ".pending_autonomous_lifecycle_terminal_outcome_inventory()",
            "pending_terminal_recovery_observations(&recovery, network_id, &active_routes)?",
            "let mut owned_group_hashes = BTreeSet::new();",
            "pending_terminal_group_has_exact_queue_owner(&initial_snapshot, observation)?",
            "let deferred = !owned_group_hashes.is_empty();",
            "pending_groups: pending_groups.clone()",
            "preflighted.push",
            "if preflight.deferred",
            "recover_pending_autonomous_lifecycle_terminal_outcome(",
            "if finalized != 0",
            "!= initial_snapshot",
            "!= initial_queue_quarantine",
            ".pending_autonomous_lifecycle_terminal_outcome_inventory()",
            "unit_has_exact_owner |=",
            "let deferred_terminal_recovery =",
            "observed_deferred_units != expected_deferred_units",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "complete_deferred_autonomous_lifecycle_terminal_outcomes_after_queue_actions",
        (
            "let queue_snapshot = queue",
            "for (unit_index, unit) in deferred.units.iter().enumerate()",
            "pending_terminal_group_has_exact_queue_owner(&queue_snapshot, observation)?",
            "verify_expected_autonomous_lifecycle_terminal_outcome_stages(",
            ".pending_autonomous_lifecycle_terminal_outcome_inventory()",
            "preflighted.push((recovery, pending_groups.len()));",
            "for (recovery, pending_count) in preflighted",
            "recover_pending_autonomous_lifecycle_terminal_outcome(",
            "!= queue_snapshot",
            "verify_expected_autonomous_lifecycle_terminal_outcome_stages(",
            "AutonomousLifecycleTerminalOutcomeDurableStage::Complete",
            ".pending_autonomous_lifecycle_terminal_outcome_inventory()",
            ".is_empty()",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "reconcile_autonomous_lifecycle_startup",
        (
            "let initial_queue_quarantine",
            "let snapshot = queue",
            "if !snapshot.is_empty() && !initial_queue_quarantine",
            "bind_lane_reservation_startup_reconciliation_receipt(&snapshot)",
            "let mut seen_pending_identities = BTreeSet::new();",
            "for unit in &deferred_terminal_recovery.units",
            "pending_terminal_group_has_exact_queue_owner(&snapshot, observation)?",
            "!seen_pending_identities.insert(pending.identity)",
            "unit_has_planner_anchor |= marked_owned",
            "let mut current_queue_groups",
            "for phase in &snapshot.ordered_owner_phases",
            "seen_pending_identities.contains(",
            "for authority in &bootstraps",
            "ProducerQueue bootstrap lost its exact current durable Queue owner",
            ".active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(",
            "for authority in bootstraps",
            "complete_autonomous_lifecycle_bootstrap(permit)",
            "// Consume the checked action-25 stutters",
            ".active_autonomous_lifecycle_attempt_inventory_with_planner_covered_pending_groups(",
            "seen_pending_identities.contains(&identity)",
            "let cursor = attempt.cursor().ok_or_else",
            "require_local_producer_queue_owner(payload, cursor, &current_queue_groups)",
            "if recover_one_attempt(",
            "revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_terminal_recovery.rs",
        "fn",
        "reconcile_lifecycle_terminal_outcomes_before_queue_planning",
        (
            "begin_fail_stop_operation()",
            "reconcile_pending_autonomous_lifecycle_terminal_outcomes(state, queue, kura, context)",
            "recovery.complete();",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_non_pending_lifecycle_loop",
        (
            "if reservation_reconciliation_pending {",
            "let summary = loop {",
            "let deferred_terminal_recovery =",
            "reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
            "let planning = plan_lane_reservation_ownership(",
            "LaneReservationReconciliationPlanning::Ready(pre_lifecycle_plan) =>",
            "let planner_evidence =",
            "pre_lifecycle_plan.startup_snapshot_recovery_evidence()?;",
            "reconcile_autonomous_lifecycle_startup(",
            "planner_evidence,",
            "deferred_terminal_recovery,",
            "Some(lifecycle),",
            "LaneReservationReconciliationPlanning::Ready(plan) =>",
            "apply_lane_reservation_reconciliation_plan(",
            "reservation_reconciliation_pending = false;",
            "construct_after_pending_tip_application_recovery(",
            "lane_work.install_lane_drain_queue(Arc::clone(&queue))?;",
            "lane_work.activate_after_lane_drain_queue_install(&queue)?;",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "reconcile_pending_lane_startup",
        (
            "let summary = loop {",
            "let deferred_terminal_recovery =",
            "reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
            "let planning = plan_lane_reservation_ownership(",
            "LaneReservationReconciliationPlanning::Ready(pre_lifecycle_plan) =>",
            "let planner_evidence =",
            "pre_lifecycle_plan.startup_snapshot_recovery_evidence()?;",
            "reconcile_autonomous_lifecycle_startup(",
            "planner_evidence,",
            "deferred_terminal_recovery,",
            "Some(lifecycle),",
            "LaneReservationReconciliationPlanning::Ready(plan) =>",
            "apply_lane_reservation_reconciliation_plan(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
        "method",
        "PendingKuraProductionLifecycleV1::prepare_lane_recovery",
        (
            "lane_work.install_lane_drain_queue(Arc::clone(&queue))?;",
            "lane_work.activate_after_lane_drain_queue_install(&queue)?;",
        ),
    ),
)
AUTONOMOUS_TERMINAL_FORBIDDEN_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs",
        "method",
        "Kura::verify_expected_autonomous_lifecycle_terminal_outcome_stages",
        (
            "canonical_carrier_source_outcome_set_locked(",
            "pending_autonomous_lifecycle_terminal_outcome_inventory(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "finalize_certified_merge_reservations",
        (
            ".authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply/reconciliation_authority.rs",
        "fn",
        "recover_pending_canonical_terminal_outcome",
        (
            ".authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "apply_lane_reservation_reconciliation_plan",
        (
            ".authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs",
        "fn",
        "reconcile_pending_autonomous_lifecycle_terminal_outcomes",
        (
            "owned_group_hashes.len() == pending_groups.len()",
            "owned_group_hashes.len() == recovery.pending_outcome_count()",
        ),
    ),
)
AUTONOMOUS_TERMINAL_POST_COMPLETE_BUDGET_RELEASE_CHECKS = (
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "finalize_certified_merge_reservations",
        r"for\s+evidence\s+in\s+terminal_evidence\s*\{\s*"
        r"kura\.complete_autonomous_lifecycle_canonical_terminal_outcome\(evidence\)\?;\s*"
        r"\}\s*kura\.release_post_wsv_lane_artifact_budget_reservation\(",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply/reconciliation_authority.rs",
        "fn",
        "recover_pending_canonical_terminal_outcome",
        r"for\s+evidence\s+in\s+terminal_evidence\s*\{\s*"
        r"kura\.complete_autonomous_lifecycle_canonical_terminal_outcome\(evidence\)\?;\s*"
        r"\}\s*kura\.release_post_wsv_lane_artifact_budget_reservation\(",
    ),
    (
        SUMERAGI_COMMITTED_CARRIER_CLEANUP_RELATIVE,
        "fn",
        "finalize_startup_committed_canonical_carriers",
        r"for\s+evidence\s+in\s+terminal_evidence\s*\{\s*"
        r"kura\.complete_autonomous_lifecycle_canonical_terminal_outcome\(evidence\)\?;\s*"
        r"\}\s*for\s+\(entry,\s*carrier_height,\s*carrier_block_hash\)\s+"
        r"in\s+carrier_releases\s*\{\s*"
        r"kura\.release_post_wsv_lane_artifact_budget_reservation\(",
    ),
)


def _tla_operator_items(source: str) -> dict[str, str]:
    declarations = list(
        re.finditer(r"(?m)^([A-Za-z][A-Za-z0-9_]*)\s*==", source)
    )
    return {
        declaration.group(1): source[
            declaration.end() : (
                declarations[index + 1].start()
                if index + 1 < len(declarations)
                else len(source)
            )
        ]
        for index, declaration in enumerate(declarations)
    }


def _strip_tla_comments(source: str) -> str:
    source = re.sub(r"\(\*.*?\*\)", "", source, flags=re.DOTALL)
    return re.sub(r"(?m)^\s*\\\*.*$", "", source)


def _validate_terminal_tla_nonvacuity(root: Path, errors: list[str]) -> None:
    """Reject action contradictions and source-bind the fixed mixed-unit trace."""

    path = root / AUTONOMOUS_TERMINAL_TLA_RELATIVE
    try:
        source = path.read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"{path}: cannot read autonomous terminal TLA source: {error}")
        return
    stripped = _strip_tla_comments(source)
    operators = _tla_operator_items(stripped)
    next_item = operators.get("Next")
    if next_item is None:
        errors.append(f"{path}: autonomous terminal TLA source has no Next relation")
        return
    next_actions = set(
        re.findall(r"(?m)^\s*\\/\s+([A-Za-z][A-Za-z0-9_]*)\s*$", next_item)
    )

    tuple_names = {
        "carrierVars",
        "diagnosticVars",
        "recoveryVars",
        "canonicalTerminalBatchVars",
        "startupTerminalUnitVars",
        "terminalVars",
        "vars",
    }
    tuples: dict[str, set[str]] = {}
    for name in tuple_names:
        item = operators.get(name, "")
        tuple_body = item.split(">>", 1)[0]
        tuples[name] = set(
            re.findall(r"\b[A-Za-z][A-Za-z0-9_]*\b", tuple_body)
        )

    def expand_tuple_aliases(names: set[str]) -> set[str]:
        expanded: set[str] = set()
        pending = list(names)
        while pending:
            name = pending.pop()
            if name in tuples:
                pending.extend(tuples[name])
            else:
                expanded.add(name)
        return expanded

    for action in sorted(next_actions):
        item = operators.get(action)
        if item is None:
            errors.append(f"{path}: Next references missing action {action}")
            continue
        assigned = set(
            re.findall(r"\b([A-Za-z][A-Za-z0-9_]*)\s*'\s*=", item)
        )
        unchanged_aliases: set[str] = set()
        for body in re.findall(r"UNCHANGED\s*<<(.+?)>>", item, flags=re.DOTALL):
            unchanged_aliases.update(
                re.findall(r"\b[A-Za-z][A-Za-z0-9_]*\b", body)
            )
        unchanged_aliases.update(
            re.findall(r"UNCHANGED\s+([A-Za-z][A-Za-z0-9_]*)", item)
        )
        overlap = sorted(assigned & expand_tuple_aliases(unchanged_aliases))
        if overlap:
            errors.append(
                f"{path}: action {action} both assigns and leaves UNCHANGED "
                f"{', '.join(overlap)}; the fixed trace would be vacuous"
            )

    for action, required_tokens in AUTONOMOUS_TERMINAL_TLA_POSITIVE_ACTION_CHECKS:
        item = operators.get(action)
        if item is None or action not in next_actions:
            errors.append(
                f"{path}: fixed mixed-unit terminal trace is missing Next action {action}"
            )
            continue
        compact_item = re.sub(r"\s+", " ", item).strip()
        if "Mode =" in compact_item:
            errors.append(
                f"{path}: fixed mixed-unit terminal trace action {action} is mutation-gated"
            )
        for token in required_tokens:
            compact_token = re.sub(r"\s+", " ", token).strip()
            if compact_token not in compact_item:
                errors.append(
                    f"{path}: fixed mixed-unit terminal trace action {action} "
                    f"is missing token {token!r}"
                )


def _validate_terminal_raw_tests(root: Path, errors: list[str]) -> None:
    for relative, test_name, required_tokens in AUTONOMOUS_TERMINAL_RAW_TEST_CHECKS:
        path = root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(f"{path}: cannot read terminal macro test source: {error}")
            continue
        declaration = re.compile(
            r"v2_apply_test!\(\s*" + re.escape(test_name) + r"\s*,"
        )
        matches = list(declaration.finditer(source))
        if len(matches) != 1:
            errors.append(
                f"{path}: terminal macro test {test_name} must occur exactly once, "
                f"found {len(matches)}"
            )
            continue
        start = matches[0].start()
        next_test = re.search(r"v2_apply_test!\(", source[matches[0].end() :])
        end = (
            matches[0].end() + next_test.start()
            if next_test is not None
            else len(source)
        )
        item = source[start:end]
        for token in required_tokens:
            if token not in item:
                errors.append(
                    f"{path}: terminal macro test {test_name} is missing token {token!r}"
                )

def validate_autonomous_terminal_recovery_contract(
    root: Path,
    models: Any,
    errors: list[str],
    rust_binding_item: RustBindingItem,
) -> None:
    """Bind Pending-to-Queue terminal joins and producer recovery custody."""

    _validate_terminal_tla_nonvacuity(root, errors)
    _validate_terminal_raw_tests(root, errors)

    if not isinstance(models, list):
        return
    autonomous_models = [
        model
        for model in models
        if isinstance(model, dict)
        and model.get("module") == AUTONOMOUS_TERMINAL_RECOVERY_MODULE
    ]
    if len(autonomous_models) != 1:
        errors.append(
            "autonomous terminal recovery source contract requires exactly one "
            f"{AUTONOMOUS_TERMINAL_RECOVERY_MODULE} model"
        )
        return
    production_symbols = autonomous_models[0].get("production_symbols")
    if not isinstance(production_symbols, list):
        return

    binding_items: dict[tuple[str, str, str], str] = {}
    for relative, kind, symbol, expected_tokens in AUTONOMOUS_TERMINAL_RECOVERY_BINDINGS:
        matches = [
            binding
            for binding in production_symbols
            if isinstance(binding, dict)
            and binding.get("path") == relative
            and binding.get("kind") == kind
            and binding.get("symbol") == symbol
        ]
        if len(matches) != 1:
            errors.append(
                f"{AUTONOMOUS_TERMINAL_RECOVERY_MODULE}: terminal recovery "
                f"binding {relative}!{symbol} must occur exactly once, "
                f"found {len(matches)}"
            )
            continue
        actual_tokens = matches[0].get("required_tokens")
        if (
            not isinstance(actual_tokens, list)
            or len(actual_tokens) != len(set(actual_tokens))
            or any(token not in actual_tokens for token in expected_tokens)
        ):
            errors.append(
                f"{AUTONOMOUS_TERMINAL_RECOVERY_MODULE}: reviewed terminal "
                f"recovery tokens changed for {relative}!{symbol}"
            )

        item = rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "autonomous terminal recovery source binding",
            errors,
        )
        if item is None:
            continue
        binding_items[(relative, kind, symbol)] = item
        for token in expected_tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: autonomous terminal recovery item "
                    f"{symbol} is missing source-bound token {token!r}"
                )

    for relative, kind, symbol, expected_tokens in AUTONOMOUS_TERMINAL_TEST_BINDINGS:
        item = rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "autonomous terminal recovery negative-control binding",
            errors,
        )
        if item is None:
            continue
        for token in expected_tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: autonomous terminal recovery negative "
                    f"control {symbol} is missing token {token!r}"
                )

    for relative, kind, symbol, ordered_tokens in (
        AUTONOMOUS_TERMINAL_ORDERED_SOURCE_CHECKS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "ordered autonomous terminal recovery source binding",
                errors,
            )
        if item is None:
            continue
        cursor = -1
        for token in ordered_tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: autonomous terminal recovery item "
                    f"{symbol} is missing or reorders token {token!r}"
                )
                break
            cursor = position

    for relative, kind, symbol, post_complete_release_pattern in (
        AUTONOMOUS_TERMINAL_POST_COMPLETE_BUDGET_RELEASE_CHECKS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "post-Complete autonomous terminal budget-release binding",
                errors,
            )
        if item is None:
            continue
        release_call = "kura.release_post_wsv_lane_artifact_budget_reservation("
        if item.count(release_call) != 1:
            errors.append(
                f"{root / relative}: autonomous terminal recovery item {symbol} "
                "must release exactly one exact post-WSV carrier reservation"
            )
            continue
        if re.search(post_complete_release_pattern, item) is None:
            errors.append(
                f"{root / relative}: autonomous terminal recovery item {symbol} "
                "must release its post-WSV carrier reservation only after the full "
                "Kura Complete publication loop"
            )

    for relative, kind, symbol, forbidden_tokens in (
        AUTONOMOUS_TERMINAL_FORBIDDEN_SOURCE_CHECKS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "forbidden autonomous terminal recovery source binding",
                errors,
            )
        if item is None:
            continue
        for token in forbidden_tokens:
            if token in item:
                errors.append(
                    f"{root / relative}: autonomous terminal recovery item "
                    f"{symbol} contains forbidden per-group cleanup token {token!r}"
                )

    journal_key = (
        QUEUE_JOURNAL_RESERVATION_COMMIT_PREFLIGHT_RELATIVE,
        "fn",
        "observe_startup_replay_receipt_with_finalized_absence",
    )
    journal_item = binding_items.get(journal_key)
    if journal_item is not None:
        replay_call = "prepare_replay_with_removed_entrypoints("
        if journal_item.count(replay_call) != 1:
            errors.append(
                f"{root / QUEUE_JOURNAL_RESERVATION_COMMIT_PREFLIGHT_RELATIVE}: "
                "combined carrier cleanup must authenticate exactly one immutable "
                "QueuePlan replay snapshot"
            )
        if journal_item.count("replay.verify_snapshot_content()?;") != 2:
            errors.append(
                f"{root / QUEUE_JOURNAL_RESERVATION_COMMIT_PREFLIGHT_RELATIVE}: "
                "combined carrier cleanup must verify the same QueuePlan snapshot "
                "before and after classification"
            )
        compact_journal = re.sub(r"\s+", "", journal_item)
        aggregate_cap_patterns = (
            r"phases\.len\(\)\+finalized_keys\.len\(\)",
            r"phases\.len\(\)\.checked_add\(finalized_keys\.len\(\)\)",
            r"finalized_keys\.len\(\)>self\.limits\.max_live_records",
            r"owner_hashes\.len\(\)>self\.limits\.max_live_records",
        )
        if any(re.search(pattern, compact_journal) for pattern in aggregate_cap_patterns):
            errors.append(
                f"{root / QUEUE_JOURNAL_RESERVATION_COMMIT_PREFLIGHT_RELATIVE}: "
                "authenticated absent carrier siblings must not share the active "
                "QueuePlan max-live-record cap"
            )
