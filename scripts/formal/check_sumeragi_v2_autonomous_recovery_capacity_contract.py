#!/usr/bin/env python3
"""Validate the autonomous recovery/capacity model's static source contract.

This checker intentionally does not invoke TLC, Apalache, Cargo, or rustc. It
verifies the finite model/config surface, exact mutation routing, stable Rust
anchors, and fail-closed editor placeholders. A placeholder is never accepted
as completed production binding evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import stat
import sys
from pathlib import Path
from typing import Any


ROOT_DIR = Path(__file__).resolve().parents[2]
FORMAL_RELATIVE = Path("formal/sumeragi_v2")
CONTRACT_RELATIVE = FORMAL_RELATIVE / "autonomous_recovery_capacity_source_bindings.json"
MODEL_NAME = "SumeragiV2AutonomousRecoveryCapacity"
MODEL_RELATIVE = FORMAL_RELATIVE / f"{MODEL_NAME}.tla"
EVIDENCE_RELATIVE = FORMAL_RELATIVE / "AUTONOMOUS_RECOVERY_CAPACITY_CONTRACT.md"
MAX_STATIC_FILE_BYTES = 8 * 1024 * 1024

INVARIANTS = (
    "MLIncompleteCarrierNRecoverable",
    "MLAutonomousPredecessorGloballyApplied",
    "MLStartupRepairAfterCarrierEnvelopes",
    "MLCertifiedFrontierCapacityReconstructable",
    "MLMutationPeaksAdmittedBeforeFirstWrite",
    "MLDebugAppendReservationAndRestartAccounting",
)

POSITIVE_CONFIG = "multilane_autonomous_recovery_capacity_fixed.cfg"
MUTATIONS = (
    (
        "RouteLatestOnlySkip",
        "multilane_autonomous_recovery_capacity_route_latest_only_skip_bug.cfg",
        "MLIncompleteCarrierNRecoverable",
    ),
    (
        "HashOnlyAutonomousPredecessor",
        "multilane_autonomous_recovery_capacity_hash_only_predecessor_bug.cfg",
        "MLAutonomousPredecessorGloballyApplied",
    ),
    (
        "StartupRepairBeforeEnvelope",
        "multilane_autonomous_recovery_capacity_startup_repair_before_envelopes_bug.cfg",
        "MLStartupRepairAfterCarrierEnvelopes",
    ),
    (
        "FrontierMissingBundleEnvelope",
        "multilane_autonomous_recovery_capacity_frontier_missing_bundle_obligation_bug.cfg",
        "MLCertifiedFrontierCapacityReconstructable",
    ),
    (
        "ClaimPeakAfterMutation",
        "multilane_autonomous_recovery_capacity_claim_peak_after_mutation_bug.cfg",
        "MLMutationPeaksAdmittedBeforeFirstWrite",
    ),
    (
        "AssociationPeakAfterMutation",
        "multilane_autonomous_recovery_capacity_association_peak_after_mutation_bug.cfg",
        "MLMutationPeaksAdmittedBeforeFirstWrite",
    ),
    (
        "PrunePeakAfterMutation",
        "multilane_autonomous_recovery_capacity_prune_peak_after_mutation_bug.cfg",
        "MLMutationPeaksAdmittedBeforeFirstWrite",
    ),
    (
        "PrunePeakDropsRosterGeneration",
        "multilane_autonomous_recovery_capacity_prune_peak_drops_roster_generation_bug.cfg",
        "MLMutationPeaksAdmittedBeforeFirstWrite",
    ),
    (
        "PrunePeakDropsReservationEnvelope",
        "multilane_autonomous_recovery_capacity_prune_peak_drops_reservation_envelope_bug.cfg",
        "MLMutationPeaksAdmittedBeforeFirstWrite",
    ),
    (
        "DebugAppendBeforeCarrierReservation",
        "multilane_autonomous_recovery_capacity_debug_append_before_carrier_reservation_bug.cfg",
        "MLDebugAppendReservationAndRestartAccounting",
    ),
    (
        "DebugRestartDropsAccounting",
        "multilane_autonomous_recovery_capacity_debug_restart_drops_accounting_bug.cfg",
        "MLDebugAppendReservationAndRestartAccounting",
    ),
)

EXPECTED_TOP_LEVEL_KEYS = {
    "schema_version",
    "module",
    "positive_config",
    "evidence_document",
    "production_refinement_obligation",
    "integration_status",
    "formal_engine_status",
    "invariants",
    "mutations",
    "stable_bindings",
    "editor_placeholders",
}

STABLE_BINDING_IDENTITIES = {
    "durable_reservation_reconciliation_snapshot": (
        "MLIncompleteCarrierNRecoverable",
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::lane_reservation_reconciliation_snapshot",
    ),
    "exact_incomplete_carrier_history_scan": (
        "MLIncompleteCarrierNRecoverable",
        "crates/iroha_core/src/kura/merge_ledger_latest_execution_index.rs",
        "method",
        "MergeLedgerLog::execution_entries_for_bounded_identities",
    ),
    "exact_incomplete_carrier_reservation_rebuild": (
        "MLIncompleteCarrierNRecoverable",
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "method",
        "Kura::rebuild_post_wsv_lane_artifact_budget_reservations_on_startup",
    ),
    "autonomous_predecessor_role_dispatch": (
        "MLAutonomousPredecessorGloballyApplied",
        "crates/iroha_core/src/state/autonomous_predecessor_application.rs",
        "method",
        "State::certified_lane_block_session_predecessor_is_applied_cached",
    ),
    "autonomous_predecessor_ordinary_receipt_filter": (
        "MLAutonomousPredecessorGloballyApplied",
        "crates/iroha_core/src/state/autonomous_predecessor_application.rs",
        "method",
        "State::ordinary_application_receipt_repair_session",
    ),
    "autonomous_predecessor_global_application_gate": (
        "MLAutonomousPredecessorGloballyApplied",
        "crates/iroha_core/src/state/autonomous_predecessor_application.rs",
        "method",
        "State::certified_autonomous_lane_block_predecessor_is_globally_applied_cached",
    ),
    "autonomous_predecessor_merge_receipt_revalidator": (
        "MLAutonomousPredecessorGloballyApplied",
        "crates/iroha_core/src/kura/autonomous_application_evidence.rs",
        "method",
        "Kura::autonomous_lane_block_predecessor_merge_receipt_revalidates_without_sidecar_repair",
    ),
    "autonomous_current_merge_receipt_revalidator": (
        "MLAutonomousPredecessorGloballyApplied",
        "crates/iroha_core/src/kura/autonomous_application_evidence.rs",
        "method",
        "Kura::autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair",
    ),
    "certified_merge_stage_consumer": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/state.rs",
        "method",
        "StateBlock::stage_certified_merge_entry",
    ),
    "certified_bundle_reservation_sum": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity_reservation_types.rs",
        "method",
        "CertifiedBundleCapacityReservation::reserved_bytes",
    ),
    "certified_bundle_admission_before_first_write": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::persist_committed_lane_block_session_inner",
    ),
    "certified_bundle_exact_pair_capacity_projection": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::certified_bundle_pair_remaining_capacity_locked",
    ),
    "certified_bundle_complete_plan": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::certified_bundle_capacity_plan",
    ),
    "certified_bundle_history_preflight": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::preflight_certified_bundle_inventory_locked",
    ),
    "certified_bundle_aggregate_capacity_gate": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::ensure_certified_bundle_capacity_plan_locked",
    ),
    "certified_bundle_capacity_snapshot_order": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::ensure_certified_bundle_capacity_reservation_under_prune_guard",
    ),
    "certified_bundle_durable_component_consumption": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::consume_certified_bundle_capacity_component",
    ),
    "certified_bundle_retirement_blocker": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::ensure_lane_has_no_certified_bundle_capacity_reservation",
    ),
    "certified_bundle_transactional_startup_rebuild": (
        "MLCertifiedFrontierCapacityReconstructable",
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::rebuild_certified_bundle_capacity_reservations_on_startup",
    ),
    "lane_history_compaction_recovery_before_capacity": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura/lane_history_compaction.rs",
        "method",
        "Kura::compact_lane_histories_through_merge_frontier_locked",
    ),
    "prune_roster_generation_peak_projection": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/commit_roster_journal.rs",
        "method",
        "CommitRosterJournalPruneProjectionV2::allocation_peak_with_sidecar",
    ),
    "prune_transaction_peak_projection": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura/prune_commit_merge_support.rs",
        "method",
        "KuraPruneCapacityAdmissionV2::transaction_peak_bytes",
    ),
    "prune_absolute_peak_projection": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura/prune_commit_merge_support.rs",
        "method",
        "KuraPruneCapacityAdmissionV2::required_peak_bytes",
    ),
    "prune_live_capacity_before_intent": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::prune_to_height",
    ),
    "prune_startup_capacity_before_repair": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::new_inner",
    ),
    "prune_roster_authorized_mutation": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura/prune_recovery_capacity.rs",
        "method",
        "Kura::truncate_roster_for_prune",
    ),
    "prune_roster_deterministic_publication": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/commit_roster_journal.rs",
        "method",
        "CommitRosterJournal::persist_durable",
    ),
    "entrypoint_claim_preflight_order": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/lane_consensus.rs",
        "method",
        "LaneBlockSessionCache::insert_proposal",
    ),
    "startup_carrier_envelope_reconstruction_order": (
        "MLStartupRepairAfterCarrierEnvelopes",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::new_inner",
    ),
    "entrypoint_claim_set_peak_projection": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura/hot_path_capacity_preflight.rs",
        "method",
        "Kura::preflight_autonomous_lane_entrypoint_claims_locked",
    ),
    "entrypoint_claim_set_preflight_before_mutation": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::prepare_autonomous_lane_entrypoint_claims_with_limit_locked",
    ),
    "canonical_association_stage_projection": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura/hot_path_capacity_preflight.rs",
        "method",
        "Kura::prepare_canonical_association_stage",
    ),
    "canonical_association_normal_budget_peak": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::check_storage_budget",
    ),
    "canonical_association_replacement_budget_peak": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::check_replace_storage_budget",
    ),
    "canonical_association_normal_preflight_order": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs",
        "method",
        "Kura::store_block_durable",
    ),
    "canonical_association_replacement_preflight_order": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::replace_top_block",
    ),
    "canonical_association_publication_boundary": (
        "MLMutationPeaksAdmittedBeforeFirstWrite",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::write_canonical_association_stage",
    ),
    "debug_append_capacity_preflight_order": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs",
        "method",
        "Kura::append_debug_block_dump",
    ),
    "debug_append_capacity_lock_bridge": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs",
        "method",
        "Kura::store_block_durable",
    ),
    "debug_append_replacement_lock_owner": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::replace_top_block",
    ),
    "debug_append_pending_canonical_snapshot": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura/autonomous_terminal_capacity.rs",
        "method",
        "Kura::pending_canonical_capacity_bytes_under_prune_and_canonical_guards",
    ),
    "debug_append_capacity_locked_bridge": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura/autonomous_terminal_capacity.rs",
        "method",
        "Kura::validate_configured_autonomous_mutation_disk_peak_locked",
    ),
    "debug_append_carrier_reservation_gate": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura/autonomous_terminal_capacity.rs",
        "method",
        "Kura::validate_configured_autonomous_mutation_disk_peak_with_allowed_view_temp_locked",
    ),
    "debug_append_file_accounting": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs",
        "method",
        "Kura::blocks_root_debug_file_bytes",
    ),
    "debug_append_enforced_root_accounting": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::blocks_root_bytes",
    ),
    "debug_append_total_root_accounting": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::blocks_root_total_bytes",
    ),
    "debug_append_enforced_usage_chain": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::kura_disk_usage_bytes",
    ),
    "debug_append_total_usage_chain": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::kura_total_disk_usage_bytes",
    ),
    "debug_append_total_cache_refresh": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::refresh_total_disk_usage_bytes",
    ),
    "debug_append_startup_accounting_publish": (
        "MLDebugAppendReservationAndRestartAccounting",
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::new_inner",
    ),
}

ANCHOR_ONLY_BINDINGS = {
    "durable_reservation_reconciliation_snapshot",
    "certified_merge_stage_consumer",
    "entrypoint_claim_preflight_order",
}

STABLE_BINDING_COVERAGE = {
    binding_id: "anchor_only" if binding_id in ANCHOR_ONLY_BINDINGS else "invariant_chain"
    for binding_id in STABLE_BINDING_IDENTITIES
}

STABLE_BINDING_REQUIRED_ANCHORS = {
    "durable_reservation_reconciliation_snapshot": (
        "self.lane_reservation_journal.lock().is_none()",
        "let ordered_owner_phases = self.lane_reservation_recovery_phases_locked()?",
        "Ok(LaneQueueReservationReconciliationSnapshotV1 {",
    ),
    "exact_incomplete_carrier_history_scan": (
        "identities.len() > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES",
        "for index in 1..=self.total_entries",
        "descriptor.lane_incarnation",
        "descriptor.lane_block_height",
        "descriptor.proposal_height",
        "identities.contains(&identity)",
        "found.insert(identity, frame.entry_hash).is_some()",
    ),
    "exact_incomplete_carrier_reservation_rebuild": (
        ".difference(&inventory.complete_terminal_outcome_identities)",
        "AutonomousLifecycleTerminalOutcomeSourceV1::CanonicalCarrier",
        "read_lane_block_application_receipt_from_paths_durability_attested_locked",
        "receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution",
        "latest_height != identity.0",
        "execution_entries_for_bounded_identities(&historical_execution_identities)",
        "authenticated_carriers.push((entry_hash, carrier))",
        "ensure_post_wsv_lane_artifact_budget_reservation_after_authentication_locked",
    ),
    "autonomous_predecessor_role_dispatch": (
        "session.prepare_qc.payload_availability_qc.is_some()",
        "self.certified_autonomous_lane_block_predecessor_is_globally_applied_cached",
        "self.certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached",
    ),
    "autonomous_predecessor_ordinary_receipt_filter": (
        "let session = crate::lane_consensus::CommittedLaneBlockSession {",
        ".payload_availability_qc",
        ".is_none()",
        ".then_some(session)",
    ),
    "autonomous_predecessor_global_application_gate": (
        "Self::canonical_merged_lane_frontier_from_world",
        "return frontier == (previous_height, Some(previous_descriptor_hash))",
        "self.kura.autonomous_lane_block_predecessor_merge_receipt_revalidates_without_sidecar_repair",
    ),
    "autonomous_predecessor_merge_receipt_revalidator": (
        "receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution",
        "previous.lane_incarnation == descriptor.lane_incarnation",
        "self.lane_block_application_receipt_matches_merge_log_without_sidecar_repair(&receipt)",
    ),
    "autonomous_current_merge_receipt_revalidator": (
        "receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution",
        "receipt.proposal == *proposal",
        "self.lane_block_application_receipt_matches_merge_log_without_sidecar_repair(&receipt)",
    ),
    "certified_merge_stage_consumer": (
        "self.state_ref.validate_certified_merge_entry_for_global_order(entry)?",
        "self.stage_merge_execution_markers(entry.epoch_id, batch)?",
        "self.staged_merge_entry = Some(entry.clone())",
    ),
    "certified_bundle_reservation_sum": (
        "if self.outstanding_components.is_empty()",
        "self.plan.component_bytes.get(component)?",
        "self.plan.component_transient_bytes.get(component)?",
        "stable.checked_add(transient)",
    ),
    "certified_bundle_admission_before_first_write": (
        "let _prune_guard = self.prune_lock.lock()",
        "Self::validate_certified_lane_block_artifact(&artifact)",
        "!authority.authorizes_proposal(&artifact.proposal)",
        "self.ensure_certified_bundle_capacity_reservation_under_prune_guard",
        "self.write_certified_lane_block_artifact_with_authority_under_prune_guard",
        "self.persist_autonomous_lane_merge_bundle_under_prune_guard(&source)?",
        "descriptor.lane_block_height, *network_id, *epoch, None, true",
    ),
    "certified_bundle_exact_pair_capacity_projection": (
        "self.certified_bundle_pair_has_exact_append_recovery_locked",
        "physical_stable.checked_add(recovery.physical_temp_bytes)",
        "self.open_bound_progress_pair(data_path, index_path)?",
        "if existing == payload",
        "let intent = BoundProgressAppendIntentV1",
        "let stable = payload_len.checked_add",
        "let transient = u64::try_from(norito::encode_canonical(&intent)",
        "self.certified_bundle_capacity_pair_read_unchanged(&pair)?",
    ),
    "certified_bundle_complete_plan": (
        "source.bundle.certified != *artifact",
        "source.bundle.encode_framed()? != source.source_bundle",
        "self.certified_bundle_pair_remaining_capacity_locked",
        "CertifiedBundleCapacityComponent::LatestCertifiedFrontier",
        "CertifiedBundleCapacityComponent::CertifiedPair",
        "CertifiedBundleCapacityComponent::AutonomousBundlePair",
        "CertifiedBundleCapacityComponent::AutonomousBundlePair, bundle_component",
        "startup_physical_credit_bytes",
    ),
    "certified_bundle_history_preflight": (
        "self.certified_bundle_pair_has_exact_append_recovery_locked",
        "self.certified_bundle_append_preimage_payloads_locked",
        "self.bound_indexed_sidecar_payload_heights",
        "self.validate_autonomous_lane_merge_bundle_pair_layout_locked",
        "if bundle.certified != *artifact",
        "non-frontier autonomous certificate lacks its durable bundle",
    ),
    "certified_bundle_aggregate_capacity_gate": (
        "another certified/bundle capacity identity is still outstanding for this route",
        "reservation.plan.bundle_bytes_hash != plan.bundle_bytes_hash",
        "let existing_reserved = reservations",
        ".checked_add(pending_block_bytes)",
        ".and_then(|bytes| bytes.checked_add(terminal_reserved_bytes))",
        ".and_then(|bytes| bytes.checked_add(post_wsv_reserved_bytes))",
        "Self::canonical_prune_intent_maintenance_headroom_bytes()",
        ".and_then(|bytes| bytes.checked_add(existing_reserved))",
        ".and_then(|bytes| bytes.checked_add(new_reserved))",
        "reservations.insert(identity, reservation)",
    ),
    "certified_bundle_capacity_snapshot_order": (
        "self.persisted_count_and_unindexed_bytes()?",
        "self.pending_block_bytes(persisted_count, unindexed_bytes)?",
        "self.kura_disk_usage_bytes()?",
        "let _geometry_guard = self.lane_geometry_lock.lock()",
        "let _sidecar_guard = self.sidecar_lock.lock()",
        "self.autonomous_global_terminal_outcome_reserved_bytes_locked()?",
        "self.post_wsv_lane_artifact_budget_reserved_bytes()?",
        "self.certified_bundle_capacity_plan(&entry, artifact, source)?",
        "self.certified_bundle_capacity_consumed_components_locked",
        "self.ensure_certified_bundle_capacity_plan_locked",
    ),
    "certified_bundle_durable_component_consumption": (
        "identity.lane_incarnation == descriptor.lane_incarnation",
        "identity.lane_block_descriptor_hash == descriptor.descriptor_hash",
        "reservation.plan.certified_bytes_hash != certified_hash",
        "if durable_bytes_hash != expected_hash",
        "reservation.outstanding_components.remove(&component)",
        "reservations.remove(&identity)",
    ),
    "certified_bundle_retirement_blocker": (
        "self.certified_bundle_capacity_reservations",
        "identity.lane_id == entry.lane_id",
        "lane retirement is blocked by an outstanding certified/bundle capacity obligation",
    ),
    "certified_bundle_transactional_startup_rebuild": (
        "let mut rebuilt = BTreeMap",
        "read_latest_certified_lane_block_frontier_locked(&active, true)?",
        "self.certified_bundle_capacity_plan(&active, &artifact, &source)?",
        "self.certified_bundle_capacity_consumed_components_locked",
        "self.preflight_certified_bundle_inventory_locked",
        "startup_physical_credit_bytes.min(reserved)",
        ".checked_add(pending_block_bytes)",
        "Self::canonical_prune_intent_maintenance_headroom_bytes()",
        "*self.certified_bundle_capacity_reservations.lock() = rebuilt",
    ),
    "lane_history_compaction_recovery_before_capacity": (
        "let before_recovery = Self::sidecar_tracked_bytes",
        "let recovery_accounting = self.begin_total_disk_usage_mutation()",
        "Self::recover_indexed_sidecar_artifacts",
        "self.update_disk_usage_delta(before_recovery, before)",
        "recovery_accounting.finish()",
        "let temp_peak = before",
        ".checked_add(pending_canonical_bytes)",
        "Self::canonical_prune_intent_maintenance_headroom_bytes()",
        "LaneHistoryCompactionOutcome::CapacityBlocked",
        "Self::prune_indexed_sidecars_through_terminal_frontier",
    ),
    "prune_roster_generation_peak_projection": (
        "self.current_pointer_growth_bytes .checked_add(sidecar_peak_bytes)",
        "self.pointer_temporary_bytes.max(post_pointer)",
        "self.generation_allocation_bytes .checked_add(publication_peak)",
    ),
    "prune_transaction_peak_projection": (
        "self.roster .allocation_peak_with_sidecar(sidecar.sequential_peak_bytes)",
        "self.marker_stable_growth_bytes.checked_add(bytes)",
        "self.marker_temporary_bytes.max(post_marker)",
    ),
    "prune_absolute_peak_projection": (
        "self.source_physical_bytes .checked_add(self.reserved_bytes()?)",
        ".and_then(|bytes| bytes.checked_add(self.intent_bytes))",
        ".and_then(|bytes| bytes.checked_add(self.transaction_peak_bytes(sidecar)?))",
    ),
    "prune_live_capacity_before_intent": (
        "let roster_projection =",
        ".project_truncate_to_height(height)",
        "self.canonical_prune_commit_marker_projection(height)?",
        "self.canonical_prune_capacity_admission_snapshot(",
        "self.seal_and_validate_canonical_prune_capacity_admission(KuraPruneIntentV2 {",
        "self.persist_prune_intent(&intent)?",
        "store.prune_with_failpoint(height, fail_stage)",
    ),
    "prune_startup_capacity_before_repair": (
        "if configured_limit > 0 && intent.capacity.admitted_peak_bytes > configured_limit",
        "Self::apply_prune_intent_to_block_store(&mut block_store, intent)?",
        "kura.preflight_recovered_prune_capacity_before_mutation(intent)?",
        "kura.recover_retained_block_rewrite_stage_on_startup(&blocks_root)?",
        "kura.reconcile_merge_carriers_from_durable_blocks_with_authority(",
        "kura.complete_recovered_prune_intent(intent)?",
    ),
    "prune_roster_authorized_mutation": (
        "let remaining_roster = roster_log",
        "intent.capacity.roster.authorizes(remaining_roster)",
        "self.validate_recovered_prune_capacity(intent, remaining_roster, remaining_sidecar)?",
        "let mut candidate = roster_log.clone();",
        ".truncate_to_height_with_projection(height, intent.capacity.roster)",
        "*roster_log = candidate;",
    ),
    "prune_roster_deterministic_publication": (
        "Self::reconcile_publication_residues(&self.path)?;",
        ".open(&generation_temp_path)",
        ".write_all(&bytes)",
        "fs::rename(&generation_temp_path, &generation_path)",
        "sync_dir(&generations).map_err(|source| {",
        ".open(&pointer_temp_path)",
        ".write_all(pointer_bytes.as_bytes())",
        "fs::rename(&pointer_temp_path, &current_path)",
        "sync_dir(&self.path)",
    ),
    "entrypoint_claim_preflight_order": (
        "self.ensure_entrypoints_available(&proposal, key)?",
        "self.sessions.entry(key).or_default()",
        "self.entrypoint_claims",
    ),
    "startup_carrier_envelope_reconstruction_order": (
        "kura.reconcile_merge_carriers_from_durable_blocks_with_authority(",
        "kura.rebuild_post_wsv_lane_artifact_budget_reservations_on_startup()?",
        "kura.rebuild_certified_bundle_capacity_reservations_on_startup()?",
        "kura.repair_lane_merge_application_frontiers_on_startup()?",
        "kura.repair_autonomous_lane_merge_bundles_on_startup()?",
        "kura.validate_configured_kura_capacity_after_startup_recovery()?",
        "Ok((kura, BlockCount(block_count)))",
    ),
    "entrypoint_claim_set_peak_projection": (
        "self.inspect_autonomous_lane_entrypoint_claim_inventory(max_files)?",
        "for entrypoint_hash in &payload.entrypoint_hashes",
        "capacity.add_physical(u64::try_from(bytes.len())?)",
        "capacity.additional_peak_bytes()",
        "validate_configured_autonomous_mutation_disk_peak_locked",
    ),
    "entrypoint_claim_set_preflight_before_mutation": (
        "self.preflight_autonomous_lane_entrypoint_claims_locked(",
        "pending_canonical_bytes, payload, max_files,",
        "let accounting_mutation = self.begin_total_disk_usage_mutation()",
        "for entrypoint_hash in &payload.entrypoint_hashes",
        ".create_new(true)",
        "temp.write_all(&bytes).and_then(|()| temp.sync_all())",
    ),
    "canonical_association_stage_projection": (
        "let block_wire = block.encode_wire()?",
        "let _ = self.validate_canonical_association_stage(&stage)?",
        "let encoded_len = u64::try_from(bytes.len())?",
        "Some(existing) if existing == stage => 0",
        "None => encoded_len",
    ),
    "canonical_association_normal_budget_peak": (
        "self.canonical_association_stage_additional_bytes(block, merge_entry)?",
        ".saturating_add(association_stage_bytes)",
        "if required > limit",
    ),
    "canonical_association_replacement_budget_peak": (
        "self.canonical_association_stage_additional_bytes(block, None)?",
        "budget_used.max(projected_after)",
        ".saturating_add(association_stage_bytes)",
        "if required > limit",
    ),
    "canonical_association_normal_preflight_order": (
        "self.check_storage_budget(block, merge_entry)?",
        "self.persist_pending_certified_merge_entry(entry)?",
        "self.stage_lane_payload_ownership_artifacts_for_block",
        "self.write_canonical_association_stage(block, merge_entry)?",
    ),
    "canonical_association_replacement_preflight_order": (
        "self.check_replace_storage_budget(block.as_ref())?",
        "self.write_canonical_association_stage(&block, None)?",
    ),
    "canonical_association_publication_boundary": (
        "let publication = self.prepare_canonical_association_stage(block, merge_entry)?",
        "if publication.additional_bytes == 0",
        "let accounting_mutation = self.begin_total_disk_usage_mutation()",
        "self.write_atomic_synced_noclobber(&path, &publication.bytes)?",
    ),
    "debug_append_capacity_preflight_order": (
        "self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()",
        "let _geometry_guard = self.lane_geometry_lock.lock()",
        "let _sidecar_guard = self.sidecar_lock.lock()",
        "self.validate_configured_autonomous_mutation_disk_peak_locked",
        "let accounting_mutation = self.begin_total_disk_usage_mutation()",
        "self.append_bound_debug_block_dump(&path, &bytes)",
    ),
    "debug_append_capacity_lock_bridge": (
        "let _prune_guard = self.prune_lock.lock()",
        "let _canonical_chain_guard = self.canonical_chain_lock.lock()",
        "self.append_debug_block_dump(block)",
    ),
    "debug_append_replacement_lock_owner": (
        "let _prune_guard = self.prune_lock.lock()",
        "let _canonical_chain_guard = self.canonical_chain_lock.lock()",
        "self.append_debug_block_dump(&block)",
    ),
    "debug_append_pending_canonical_snapshot": (
        "self.max_disk_usage_bytes == 0",
        "self.persisted_count_and_unindexed_bytes()?",
        "self.pending_block_bytes(persisted_count, unindexed_bytes)",
    ),
    "debug_append_capacity_locked_bridge": (
        "self.validate_configured_autonomous_mutation_disk_peak_with_allowed_view_temp_locked",
        "pending_canonical_bytes",
        "None",
    ),
    "debug_append_carrier_reservation_gate": (
        "let post_wsv_reservations = self.post_wsv_lane_artifact_budget_reserved_bytes()?",
        "let certified_bundle_reservations = self.certified_bundle_capacity_reserved_bytes()?",
        ".kura_disk_usage_bytes()?",
        ".checked_add(pending_canonical_bytes)",
        ".and_then(|bytes| bytes.checked_add(post_wsv_reservations))",
        ".and_then(|bytes| bytes.checked_add(certified_bundle_reservations))",
        "Self::canonical_prune_intent_maintenance_headroom_bytes()",
        "if required > self.max_disk_usage_bytes",
    ),
    "debug_append_file_accounting": (
        "let path = root.join(\"blocks.jsonl\")",
        "std::fs::symlink_metadata(&path)",
        "!Self::sidecar_is_single_link(&metadata)",
        "Ok(metadata.len())",
    ),
    "debug_append_enforced_root_accounting": (
        "let mut total = Self::blocks_root_debug_file_bytes(root)?",
        "Self::block_store_bytes_with_historical_budget",
        "Ok(total)",
    ),
    "debug_append_total_root_accounting": (
        "let mut total = Self::blocks_root_debug_file_bytes(root)?",
        "Self::block_store_total_bytes_with_historical_budget",
        "Ok(total)",
    ),
    "debug_append_enforced_usage_chain": (
        "Self::blocks_root_bytes(&blocks_root",
        "Self::blocks_root_bytes(&retired_blocks_root",
        "Ok(used)",
    ),
    "debug_append_total_usage_chain": (
        "Self::blocks_root_total_bytes(&blocks_root",
        "Self::blocks_root_total_bytes(&retired_blocks_root",
        "Ok(used)",
    ),
    "debug_append_total_cache_refresh": (
        "let scanned = self.kura_total_disk_usage_bytes()",
        "self.disk_usage_total.store(usage, Ordering::Relaxed)",
        "self.disk_usage_total_initialized",
    ),
    "debug_append_startup_accounting_publish": (
        "match kura.kura_disk_usage_bytes()",
        "kura.disk_usage.store(bytes, Ordering::Relaxed)",
        "match kura.refresh_total_disk_usage_bytes()",
        "Ok((kura, BlockCount(block_count)))",
    ),
}

INCLUDED_BINDING_OWNERS = {
    "crates/iroha_core/src/state/autonomous_predecessor_application.rs": (
        "crates/iroha_core/src/state.rs",
        'include!("state/autonomous_predecessor_application.rs");',
    ),
    "crates/iroha_core/src/kura/autonomous_application_evidence.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/autonomous_application_evidence.rs");',
    ),
    "crates/iroha_core/src/kura/certified_bundle_capacity_reservation_types.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/certified_bundle_capacity_reservation_types.rs");',
    ),
    "crates/iroha_core/src/kura/certified_bundle_capacity.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/certified_bundle_capacity.rs");',
    ),
    "crates/iroha_core/src/kura/lane_history_compaction.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/lane_history_compaction.rs");',
    ),
    "crates/iroha_core/src/kura/autonomous_terminal_capacity.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/autonomous_terminal_capacity.rs");',
    ),
    "crates/iroha_core/src/kura/durable_block_and_atomic_sidecar_io.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/durable_block_and_atomic_sidecar_io.rs");',
    ),
    "crates/iroha_core/src/kura/hot_path_capacity_preflight.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/hot_path_capacity_preflight.rs");',
    ),
    "crates/iroha_core/src/kura/lane_artifact_budget.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/lane_artifact_budget.rs");',
    ),
    "crates/iroha_core/src/kura/merge_ledger_latest_execution_index.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/merge_ledger_latest_execution_index.rs");',
    ),
    "crates/iroha_core/src/kura/prune_commit_merge_support.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/prune_commit_merge_support.rs");',
    ),
    "crates/iroha_core/src/kura/prune_recovery_capacity.rs": (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/prune_recovery_capacity.rs");',
    ),
}

PLACEHOLDERS = {}

MODEL_OPERATOR_TOKENS = {
    "AdvanceRouteSnapshotToNPlusOne": (
        'carrierNStatus = "Incomplete"',
        'IF Mode = "RouteLatestOnlySkip"',
        'THEN "None"',
        "ELSE carrierNSource",
    ),
    "RecoverCarrierNFromExactIncompleteIdentity": (
        'carrierNSource = "IncompleteIdentityN"',
        'carrierNStatus\' = "Recovered"',
        'carrierNSource\' = "RecoveredIdentityN"',
    ),
    "DischargeCarrierNWithTerminalProof": (
        'carrierNStatus\' = "Terminal"',
        'carrierNSource\' = "TerminalProofN"',
    ),
    "DischargeCarrierNWithReceiptProof": (
        'carrierNStatus\' = "Receipted"',
        'carrierNSource\' = "ReceiptProofN"',
    ),
    "ObserveHashOnlyAutonomousPredecessor": (
        'autonomousPredecessorEvidence\' = "HashOnlyOwnership"',
        "autonomousPredecessorAdmitted' = (Mode = \"HashOnlyAutonomousPredecessor\")",
    ),
    "AdmitAutonomousPredecessorFromExactWsvFrontier": (
        'autonomousPredecessorEvidence\' = "ExactWsvFrontier"',
        "autonomousPredecessorAdmitted' = TRUE",
    ),
    "AdmitAutonomousPredecessorFromRevalidatedMergeReceipt": (
        'autonomousPredecessorEvidence\' = "MergeReceiptCarrierRevalidated"',
        "autonomousPredecessorAdmitted' = TRUE",
    ),
    "BeginStartupCapacityRepair": (
        "carrierEnvelopesReconstructed",
        'Mode = "StartupRepairBeforeEnvelope"',
        "startupCapacityMutation' = TRUE",
    ),
    "CertifyFrontier": (
        "frontierPairCapacityObligation' = TRUE",
        'Mode # "FrontierMissingBundleEnvelope"',
        "frontierBundleEnvelope' =",
    ),
    "CrashAfterCertifiedFrontier": (
        "frontierPairCapacityObligation",
        "frontierBundleCapacityObligation",
        "frontierPairEnvelope' = FALSE",
        "frontierBundleEnvelope' = FALSE",
        "frontierStartupClosed' = TRUE",
    ),
    "ReconstructCertifiedFrontierEnvelopes": (
        "frontierPairCapacityObligation",
        "frontierBundleCapacityObligation",
        "frontierPairEnvelope' = TRUE",
        "frontierBundleEnvelope' = TRUE",
    ),
    "BeginEntrypointClaimSetMutation": (
        "claimSetPeakAdmitted",
        'Mode = "ClaimPeakAfterMutation"',
        "claimSetFirstMutation' = TRUE",
    ),
    "BeginCanonicalAssociationStageMutation": (
        "associationStagePeakAdmitted",
        'Mode = "AssociationPeakAfterMutation"',
        "associationStageFirstMutation' = TRUE",
    ),
    "AdmitPruneCapacityPeak": (
        "~pruneCapacityPeakAdmitted",
        "~pruneFirstDurableMutation",
        "pruneCapacityPeakAdmitted' = TRUE",
        'Mode # "PrunePeakDropsRosterGeneration"',
        "pruneRosterGenerationCovered' =",
        'Mode # "PrunePeakDropsReservationEnvelope"',
        "pruneReservationEnvelopeCovered' =",
    ),
    "BeginPruneDurableMutation": (
        "pruneCapacityPeakAdmitted",
        'Mode = "PrunePeakAfterMutation"',
        "pruneFirstDurableMutation' = TRUE",
    ),
    "AppendDebugAfterCarrierReservation": (
        "debugCarrierReservationDurable",
        'Mode = "DebugAppendBeforeCarrierReservation"',
        "debugAppendDurable' = TRUE",
        "debugRuntimeAccounted' = TRUE",
    ),
    "AccountDebugAppendOnRestart": (
        'Mode # "DebugRestartDropsAccounting"',
        "debugRestartAccounted' =",
    ),
    "MLIncompleteCarrierNRecoverable": (
        'carrierNStatus = "Incomplete"',
        'carrierNSource = "IncompleteIdentityN"',
        'carrierNSource = "TerminalProofN"',
        'carrierNSource = "ReceiptProofN"',
    ),
    "MLStartupRepairAfterCarrierEnvelopes": (
        "startupCapacityMutation => carrierEnvelopesReconstructed",
    ),
    "MLAutonomousPredecessorGloballyApplied": (
        "autonomousPredecessorAdmitted =>",
        '"ExactWsvFrontier"',
        '"MergeReceiptCarrierRevalidated"',
        'autonomousPredecessorEvidence = "HashOnlyOwnership"',
        "~autonomousPredecessorAdmitted",
    ),
    "MLCertifiedFrontierCapacityReconstructable": (
        "frontierPairCapacityObligation",
        "frontierBundleCapacityObligation",
        "frontierPairEnvelope",
        "frontierBundleEnvelope",
        "frontierStartupClosed",
    ),
    "MLMutationPeaksAdmittedBeforeFirstWrite": (
        "claimSetFirstMutation => claimSetPeakAdmitted",
        "associationStageFirstMutation => associationStagePeakAdmitted",
        "pruneFirstDurableMutation =>",
        "pruneCapacityPeakAdmitted",
        "pruneRosterGenerationCovered",
        "pruneReservationEnvelopeCovered",
    ),
    "MLDebugAppendReservationAndRestartAccounting": (
        "debugAppendDurable => debugCarrierReservationDurable",
        'debugPhase = "Appended"',
        "debugRuntimeAccounted",
        'debugPhase = "RestartAccounted"',
        "debugRestartAccounted",
    ),
}


def _normalize_space(value: str) -> str:
    return re.sub(r"\s+", "", value)


def _safe_relative_path(value: object) -> Path | None:
    if not isinstance(value, str) or not value:
        return None
    candidate = Path(value)
    if candidate.is_absolute() or ".." in candidate.parts:
        return None
    return candidate


def _read_regular_text(root: Path, relative: Path, errors: list[str]) -> str | None:
    path = root / relative
    try:
        metadata = path.lstat()
    except OSError as exc:
        errors.append(f"{relative}: cannot inspect static contract file: {exc}")
        return None
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        errors.append(f"{relative}: static contract input must be a regular non-symlink file")
        return None
    if metadata.st_size > MAX_STATIC_FILE_BYTES:
        errors.append(
            f"{relative}: static contract input exceeds {MAX_STATIC_FILE_BYTES} bytes"
        )
        return None
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        errors.append(f"{relative}: cannot read UTF-8 static contract file: {exc}")
        return None


def _extract_tla_operator(source: str, name: str) -> str | None:
    start = re.search(rf"(?m)^{re.escape(name)}\s*==", source)
    if start is None:
        return None
    next_operator = re.search(r"(?m)^[A-Za-z][A-Za-z0-9_]*\s*==", source[start.end() :])
    end = len(source) if next_operator is None else start.end() + next_operator.start()
    return source[start.start() : end]


def _extract_rust_method(source: str, symbol: str) -> tuple[str | None, str | None]:
    method = symbol.rsplit("::", 1)[-1]
    pattern = re.compile(
        rf"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?"
        rf"(?:const[ \t]+)?(?:async[ \t]+)?fn[ \t]+{re.escape(method)}\b"
    )
    matches = list(pattern.finditer(source))
    if len(matches) != 1:
        return None, f"expected exactly one method declaration, found {len(matches)}"
    start = matches[0].start()
    opening = source.find("{", matches[0].end())
    if opening < 0:
        return None, "method has no opening brace"
    depth = 0
    for index in range(opening, len(source)):
        byte = source[index]
        if byte == "{":
            depth += 1
        elif byte == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1], None
    return None, "method braces are unbalanced"


def _token_positions(haystack: str, tokens: list[str]) -> tuple[list[int], str | None]:
    normalized = _normalize_space(haystack)
    positions: list[int] = []
    cursor = 0
    for token in tokens:
        needle = _normalize_space(token)
        position = normalized.find(needle, cursor)
        if position < 0:
            return positions, token
        positions.append(position)
        cursor = position + len(needle)
    return positions, None


def _validate_model_source(source: str, errors: list[str]) -> None:
    if not source.startswith(f"---- MODULE {MODEL_NAME} ----"):
        errors.append(f"{MODEL_RELATIVE}: unexpected or missing module header")
    if not source.rstrip().endswith("============================================================================="):
        errors.append(f"{MODEL_RELATIVE}: missing TLA+ module terminator")

    normalized = _normalize_space(source)
    required_global_tokens = (
        "AutonomousRecoveryCapacitySpec == Init /\\ [][Next]_vars",
        "AutonomousRecoveryCapacityProductionRefinementObligation == AutonomousRecoveryCapacitySafetyInvariant",
        "AutonomousRecoveryCapacitySafetyInvariant ==",
    )
    for token in required_global_tokens:
        if _normalize_space(token) not in normalized:
            errors.append(f"{MODEL_RELATIVE}: missing model token {token!r}")

    expected_modes = {"Fixed", *(mode for mode, _, _ in MUTATIONS)}
    for mode in sorted(expected_modes):
        if source.count(f'"{mode}"') < 1:
            errors.append(f"{MODEL_RELATIVE}: missing mode {mode!r}")

    for operator, tokens in MODEL_OPERATOR_TOKENS.items():
        body = _extract_tla_operator(source, operator)
        if body is None:
            errors.append(f"{MODEL_RELATIVE}: missing operator {operator}")
            continue
        normalized_body = _normalize_space(body)
        for token in tokens:
            if _normalize_space(token) not in normalized_body:
                errors.append(
                    f"{MODEL_RELATIVE}: operator {operator} missing token {token!r}"
                )

    next_body = _extract_tla_operator(source, "Next")
    if next_body is None:
        errors.append(f"{MODEL_RELATIVE}: missing Next operator")
    else:
        for action in (
            "AdvanceRouteSnapshotToNPlusOne",
            "RecoverCarrierNFromExactIncompleteIdentity",
            "DischargeCarrierNWithTerminalProof",
            "DischargeCarrierNWithReceiptProof",
            "ObserveHashOnlyAutonomousPredecessor",
            "AdmitAutonomousPredecessorFromExactWsvFrontier",
            "AdmitAutonomousPredecessorFromRevalidatedMergeReceipt",
            "ReconstructCarrierEnvelopesOnStartup",
            "BeginStartupCapacityRepair",
            "CertifyFrontier",
            "CrashAfterCertifiedFrontier",
            "ReconstructCertifiedFrontierEnvelopes",
            "AdmitEntrypointClaimSetPeak",
            "BeginEntrypointClaimSetMutation",
            "AdmitCanonicalAssociationStagePeak",
            "BeginCanonicalAssociationStageMutation",
            "AdmitPruneCapacityPeak",
            "BeginPruneDurableMutation",
            "ReserveDebugCarrierCapacity",
            "AppendDebugAfterCarrierReservation",
            "CrashAfterDebugAppend",
            "AccountDebugAppendOnRestart",
        ):
            if action not in next_body:
                errors.append(f"{MODEL_RELATIVE}: Next omits action {action}")


def _expected_positive_config() -> str:
    invariant_lines = ["INVARIANT AutonomousRecoveryCapacityTypeInvariant"]
    invariant_lines.extend(f"INVARIANT {invariant}" for invariant in INVARIANTS)
    invariant_lines.append("INVARIANT AutonomousRecoveryCapacitySafetyInvariant")
    return "\n".join(
        (
            "INIT Init",
            "NEXT Next",
            'CONSTANT Mode = "Fixed"',
            "CHECK_DEADLOCK FALSE",
            *invariant_lines,
        )
    )


def _expected_mutation_config(mode: str, invariant: str) -> str:
    return "\n".join(
        (
            "INIT Init",
            "NEXT Next",
            f'CONSTANT Mode = "{mode}"',
            "CHECK_DEADLOCK FALSE",
            f"INVARIANT {invariant}",
        )
    )


def _normalized_config(source: str) -> str:
    return "\n".join(line.strip() for line in source.splitlines() if line.strip())


def _validate_configs(root: Path, errors: list[str]) -> None:
    positive_relative = FORMAL_RELATIVE / POSITIVE_CONFIG
    positive = _read_regular_text(root, positive_relative, errors)
    if positive is not None and _normalized_config(positive) != _expected_positive_config():
        errors.append(f"{positive_relative}: positive config differs from the exact contract")

    for mode, config, invariant in MUTATIONS:
        relative = FORMAL_RELATIVE / config
        source = _read_regular_text(root, relative, errors)
        if source is None:
            continue
        expected = _expected_mutation_config(mode, invariant)
        if _normalized_config(source) != expected:
            errors.append(
                f"{relative}: mutation config must check only {invariant} in mode {mode}"
            )


def _validate_stable_bindings(
    root: Path, bindings: object, errors: list[str]
) -> None:
    if not isinstance(bindings, list):
        errors.append("stable_bindings must be a list")
        return
    by_id: dict[str, dict[str, Any]] = {}
    for index, binding in enumerate(bindings):
        if not isinstance(binding, dict):
            errors.append(f"stable_bindings[{index}] must be an object")
            continue
        expected_keys = {
            "id",
            "invariant",
            "coverage",
            "path",
            "kind",
            "symbol",
            "required_tokens",
            "ordered_tokens",
        }
        if set(binding) != expected_keys:
            errors.append(
                f"stable_bindings[{index}] has keys {sorted(binding)}, expected {sorted(expected_keys)}"
            )
            continue
        binding_id = binding.get("id")
        if not isinstance(binding_id, str) or binding_id in by_id:
            errors.append(f"stable_bindings[{index}] has invalid or duplicate id")
            continue
        by_id[binding_id] = binding

    if set(by_id) != set(STABLE_BINDING_IDENTITIES):
        errors.append(
            "stable_bindings ids differ from the exact stable-anchor inventory: "
            f"got {sorted(by_id)}, expected {sorted(STABLE_BINDING_IDENTITIES)}"
        )

    for binding_id, expected_identity in STABLE_BINDING_IDENTITIES.items():
        binding = by_id.get(binding_id)
        if binding is None:
            continue
        invariant, path_text, kind, symbol = expected_identity
        actual_identity = (
            binding.get("invariant"),
            binding.get("path"),
            binding.get("kind"),
            binding.get("symbol"),
        )
        if actual_identity != expected_identity:
            errors.append(
                f"stable binding {binding_id} identity drifted: {actual_identity!r}"
            )
        expected_coverage = STABLE_BINDING_COVERAGE[binding_id]
        if binding.get("coverage") != expected_coverage:
            errors.append(
                f"stable binding {binding_id} must remain coverage={expected_coverage}"
            )
        relative = _safe_relative_path(path_text)
        if relative is None:
            errors.append(f"stable binding {binding_id} has an unsafe source path")
            continue
        source = _read_regular_text(root, relative, errors)
        if source is None:
            continue
        item, extraction_error = _extract_rust_method(source, symbol)
        if item is None:
            errors.append(
                f"stable binding {binding_id} ({symbol}): {extraction_error or 'cannot extract method'}"
            )
            continue
        required_tokens = binding.get("required_tokens")
        ordered_tokens = binding.get("ordered_tokens")
        if not isinstance(required_tokens, list) or not all(
            isinstance(token, str) and token for token in required_tokens
        ):
            errors.append(f"stable binding {binding_id} required_tokens must be non-empty strings")
            continue
        if not isinstance(ordered_tokens, list) or not all(
            isinstance(token, str) and token for token in ordered_tokens
        ):
            errors.append(f"stable binding {binding_id} ordered_tokens must be non-empty strings")
            continue
        normalized_item = _normalize_space(item)
        for token in required_tokens:
            if _normalize_space(token) not in normalized_item:
                errors.append(
                    f"stable binding {binding_id} ({symbol}) missing token {token!r}"
                )
        anchors = STABLE_BINDING_REQUIRED_ANCHORS[binding_id]
        for anchor in anchors:
            if anchor not in required_tokens and anchor not in ordered_tokens:
                errors.append(
                    f"stable binding {binding_id} contract omits required anchor {anchor!r}"
                )
        _, missing = _token_positions(item, ordered_tokens)
        if missing is not None:
            errors.append(
                f"stable binding {binding_id} ({symbol}) missing or reorders token {missing!r}"
            )
        if binding_id in {
            "autonomous_predecessor_role_dispatch",
            "autonomous_predecessor_global_application_gate",
        }:
            for forbidden in (
                "lane_block_artifact_has_hash_only_snapshot_anchor",
                "certified_lane_block_proposal_has_hash_only_snapshot_anchor",
            ):
                if forbidden in item:
                    errors.append(
                        f"stable binding {binding_id} ({symbol}) admits forbidden hash-only predecessor helper {forbidden}"
                    )

    for included_path, (owner_path, include_token) in INCLUDED_BINDING_OWNERS.items():
        owner_relative = Path(owner_path)
        owner_source = _read_regular_text(root, owner_relative, errors)
        if owner_source is not None and include_token not in owner_source:
            errors.append(
                f"{owner_relative}: missing include ownership for stable binding file "
                f"{included_path}: {include_token!r}"
            )


def _validate_placeholders(placeholders: object, errors: list[str]) -> None:
    if not isinstance(placeholders, list):
        errors.append("editor_placeholders must be a list")
        return
    by_id: dict[str, dict[str, Any]] = {}
    for index, placeholder in enumerate(placeholders):
        if not isinstance(placeholder, dict):
            errors.append(f"editor_placeholders[{index}] must be an object")
            continue
        expected_keys = {
            "id",
            "invariant",
            "expected_owner_path",
            "status",
            "required_semantics",
        }
        if set(placeholder) != expected_keys:
            errors.append(
                f"editor_placeholders[{index}] must remain a non-binding placeholder; "
                f"got keys {sorted(placeholder)}"
            )
            continue
        placeholder_id = placeholder.get("id")
        if not isinstance(placeholder_id, str) or placeholder_id in by_id:
            errors.append(f"editor_placeholders[{index}] has invalid or duplicate id")
            continue
        by_id[placeholder_id] = placeholder

    if set(by_id) != set(PLACEHOLDERS):
        errors.append(
            "editor_placeholders ids differ from the exact pending inventory: "
            f"got {sorted(by_id)}, expected {sorted(PLACEHOLDERS)}"
        )

    for placeholder_id, expected in PLACEHOLDERS.items():
        placeholder = by_id.get(placeholder_id)
        if placeholder is None:
            continue
        invariant, owner_path, semantics = expected
        if placeholder.get("invariant") != invariant:
            errors.append(f"placeholder {placeholder_id} changed invariant ownership")
        if placeholder.get("expected_owner_path") != owner_path:
            errors.append(f"placeholder {placeholder_id} changed expected owner path")
        if _safe_relative_path(owner_path) is None:
            errors.append(f"placeholder {placeholder_id} has an unsafe expected owner path")
        if placeholder.get("status") != "editor_in_progress":
            errors.append(
                f"placeholder {placeholder_id} must remain editor_in_progress until replaced by a reviewed stable binding"
            )
        actual_semantics = placeholder.get("required_semantics")
        if actual_semantics != list(semantics):
            errors.append(f"placeholder {placeholder_id} changed required semantics")


def _validate_contract_data(root: Path, contract: object, errors: list[str]) -> None:
    if not isinstance(contract, dict):
        errors.append("source-binding contract root must be an object")
        return
    if set(contract) != EXPECTED_TOP_LEVEL_KEYS:
        errors.append(
            f"source-binding contract keys differ: got {sorted(contract)}, "
            f"expected {sorted(EXPECTED_TOP_LEVEL_KEYS)}"
        )
    expected_scalars = {
        "schema_version": 1,
        "module": MODEL_NAME,
        "positive_config": POSITIVE_CONFIG,
        "evidence_document": EVIDENCE_RELATIVE.name,
        "production_refinement_obligation": "AutonomousRecoveryCapacityProductionRefinementObligation",
        "integration_status": "static_model_and_production_bindings_complete",
        "formal_engine_status": "not_run_by_static_contract",
    }
    for key, expected in expected_scalars.items():
        if contract.get(key) != expected:
            errors.append(f"source-binding contract {key} must equal {expected!r}")
    if contract.get("invariants") != list(INVARIANTS):
        errors.append("source-binding contract invariant inventory or order drifted")
    expected_mutations = [
        {"mode": mode, "config": config, "invariant": invariant}
        for mode, config, invariant in MUTATIONS
    ]
    if contract.get("mutations") != expected_mutations:
        errors.append("source-binding contract mutation inventory or routing drifted")

    _validate_stable_bindings(root, contract.get("stable_bindings"), errors)
    _validate_placeholders(contract.get("editor_placeholders"), errors)

    covered: set[str] = set()
    for collection_name in ("stable_bindings", "editor_placeholders"):
        collection = contract.get(collection_name)
        if isinstance(collection, list):
            covered.update(
                item.get("invariant")
                for item in collection
                if isinstance(item, dict) and isinstance(item.get("invariant"), str)
            )
    if covered != set(INVARIANTS):
        errors.append(
            f"stable anchors plus placeholders cover {sorted(covered)}, expected {sorted(INVARIANTS)}"
        )


def _validate_evidence_document(source: str, errors: list[str]) -> None:
    required = (
        "Static-only evidence",
        "No TLC or Apalache result is claimed",
        "production bindings are complete",
        "RouteLatestOnlySkip",
        "HashOnlyAutonomousPredecessor",
        "StartupRepairBeforeEnvelope",
        "FrontierMissingBundleEnvelope",
        "ClaimPeakAfterMutation",
        "AssociationPeakAfterMutation",
        "PrunePeakAfterMutation",
        "PrunePeakDropsRosterGeneration",
        "PrunePeakDropsReservationEnvelope",
        "DebugAppendBeforeCarrierReservation",
        "DebugRestartDropsAccounting",
        "MergeLedgerLog::execution_entries_for_bounded_identities",
        "Kura::rebuild_post_wsv_lane_artifact_budget_reservations_on_startup",
        "Kura::preflight_autonomous_lane_entrypoint_claims_locked",
        "Kura::prepare_canonical_association_stage",
        "CommitRosterJournalPruneProjectionV2::allocation_peak_with_sidecar",
        "KuraPruneCapacityAdmissionV2::required_peak_bytes",
        "Kura::truncate_roster_for_prune",
        "CommitRosterJournal::persist_durable",
        "Debug append is source-bound",
        "Kura::rebuild_certified_bundle_capacity_reservations_on_startup",
        "CertifiedBundleCapacityReservation::reserved_bytes",
    )
    for token in required:
        if token not in source:
            errors.append(f"{EVIDENCE_RELATIVE}: missing evidence status token {token!r}")


def validate_repository(root: Path = ROOT_DIR) -> list[str]:
    errors: list[str] = []
    contract_source = _read_regular_text(root, CONTRACT_RELATIVE, errors)
    model_source = _read_regular_text(root, MODEL_RELATIVE, errors)
    evidence_source = _read_regular_text(root, EVIDENCE_RELATIVE, errors)

    contract: object = None
    if contract_source is not None:
        try:
            contract = json.loads(contract_source)
        except json.JSONDecodeError as exc:
            errors.append(f"{CONTRACT_RELATIVE}: invalid JSON: {exc}")
    if model_source is not None:
        _validate_model_source(model_source, errors)
    if contract is not None:
        _validate_contract_data(root, contract, errors)
    if evidence_source is not None:
        _validate_evidence_document(evidence_source, errors)
    _validate_configs(root, errors)
    return errors


def _manifest_relatives(contract: dict[str, Any]) -> list[Path]:
    relatives = [
        CONTRACT_RELATIVE,
        MODEL_RELATIVE,
        EVIDENCE_RELATIVE,
        Path("scripts/formal/check_sumeragi_v2_autonomous_recovery_capacity_contract.py"),
        FORMAL_RELATIVE / contract["positive_config"],
    ]
    relatives.extend(FORMAL_RELATIVE / mutation["config"] for mutation in contract["mutations"])
    relatives.extend(Path(binding["path"]) for binding in contract["stable_bindings"])
    relatives.extend(Path(owner) for owner, _ in INCLUDED_BINDING_OWNERS.values())
    return sorted(set(relatives), key=lambda path: path.as_posix())


def source_manifest_sha256(root: Path = ROOT_DIR) -> str:
    contract = json.loads((root / CONTRACT_RELATIVE).read_text(encoding="utf-8"))
    digest = hashlib.sha256()
    for relative in _manifest_relatives(contract):
        payload = (root / relative).read_bytes()
        digest.update(relative.as_posix().encode("utf-8"))
        digest.update(b"\0")
        digest.update(hashlib.sha256(payload).digest())
        digest.update(b"\0")
    return digest.hexdigest()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--print-source-manifest-sha256",
        action="store_true",
        help="print a deterministic digest after the static contract passes",
    )
    args = parser.parse_args(argv)

    errors = validate_repository(ROOT_DIR)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    if args.print_source_manifest_sha256:
        print(source_manifest_sha256(ROOT_DIR))
    else:
        print("autonomous recovery/capacity static contract: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
