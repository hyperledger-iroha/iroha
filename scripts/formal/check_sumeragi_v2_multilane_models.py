#!/usr/bin/env python3
"""Validate multilane model/config and static/differential source bindings.

This is a structural gate. It verifies that every finite model, positive
configuration, conceptual mutation mapping, production item, and release-only
check still exists with the reviewed semantic anchors. It does not treat TLC
output as deductive proof.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


DEFAULT_ROOT = Path(__file__).resolve().parents[2]
FORMAL_RELATIVE = Path("formal/sumeragi_v2")
BINDINGS_FILENAME = "multilane_source_bindings.json"
PROOF_COVERAGE_RELATIVE = FORMAL_RELATIVE / "proof_coverage.json"
CLOSURE_LEDGER_RELATIVE = Path(
    "specs/sumeragi_v2_multilane_closure_ledger.md"
)
APALACHE_RUNNER_RELATIVE = Path(
    "scripts/formal/run_sumeragi_v2_multilane_apalache.sh"
)
APALACHE_RUNNER_TEST_RELATIVE = Path(
    "scripts/formal/check_sumeragi_v2_multilane_apalache_runner_contract.py"
)
APALACHE_INSTALLER_RELATIVE = Path("scripts/formal/install_apalache.sh")
TLC_RUNNER_RELATIVE = Path("scripts/formal/run_sumeragi_v2_tlc.sh")
TLC_MUTATION_RUNNER_RELATIVE = Path(
    "scripts/formal/run_sumeragi_v2_multilane_mutations.sh"
)
FORMAL_WORKFLOW_RELATIVES = (
    Path(".github/workflows/pr.yml"),
    Path(".github/workflows/nightly_sumeragi_formal.yml"),
)
README_RELATIVE = FORMAL_RELATIVE / "README.md"
APALACHE_VERSION = "0.52.2"
APALACHE_ARCHIVE_SHA256 = (
    "e0ebea7e45c8f99df8d92f2755101dda84ab71df06d1ec3a21955d3b53a886e2"
)
APALACHE_LAUNCHER_SHA256 = (
    "bda52d2dbdbc7f6e95289a69dfe7ddeb162493ddd3501898d33ea7d1da3a8cd7"
)
APALACHE_JAR_SHA256 = (
    "1ac65e9c16595c19241519b209c8055d1aa79bf718f23df7cde5cf9b3dd88f2a"
)
MODULE_RE = re.compile(r"(?m)^---- MODULE ([A-Za-z_][A-Za-z0-9_]*) ----$")
TLA_DECLARATION_TEMPLATE = (
    r"(?m)^[ \t]*(?:THEOREM[ \t]+)?{symbol}"
    r"\s*(?:\([^)=\n]*\))?\s*=="
)
RUST_DECLARATION_TEMPLATES = {
    "fn": (
        r"(?m)^[ \t]*(?:pub(?:\([^)\n]*\))?[ \t]+)?"
        r"(?:const[ \t]+)?(?:async[ \t]+)?fn[ \t]+{symbol}\b"
    ),
    "struct": (
        r"(?m)^[ \t]*(?:pub(?:\([^)\n]*\))?[ \t]+)?"
        r"struct[ \t]+{symbol}\b"
    ),
    "enum": (
        r"(?m)^[ \t]*(?:pub(?:\([^)\n]*\))?[ \t]+)?"
        r"enum[ \t]+{symbol}\b"
    ),
    "macro": r"(?m)^[ \t]*macro_rules![ \t]+{symbol}\b",
}
RUST_BINDING_KINDS = frozenset((*RUST_DECLARATION_TEMPLATES, "method"))
EXPECTED_CLOSURE_INVARIANTS = {
    "SumeragiV2AutoscaleLifecycle": (
        "MLActivationAfterAtomicCreate",
        "MLDrainImpliesNoOwnedWork",
        "MLDrainCertificateMonotonic",
        "MLRetirementConsumesExactIncarnation",
    ),
    "SumeragiV2NativeApplicationEvidence": (
        "MLSeparateParticipantApplication",
        "MLNativeSourceClaimInjective",
        "MLNativeContiguousActiveRoute",
        "MLNativeGroupExactCover",
        "MLNativeManifestAuthenticates",
        "MLNativeDurabilityPrecedesFrontier",
        "MLNativeLatestIndexExact",
        "MLNativeSharedEvidenceBudget",
        "MLNativeSingleIncomingPairHeadroom",
        "MLNativeTempPromotionAuthenticated",
        "MLNativeRetainedHistoryExact",
        "MLNativePruneOldestPrefix",
        "MLUnifiedStartupEvidenceRepairSafe",
    ),
    "SumeragiV2AutonomousReservationCarrier": (
        "MLReservationSingleOwner",
        "MLReservationIdentityStable",
        "MLCertifiedBundleDurable",
        "MLMergeCandidateExactPrefix",
        "MLCarrierExactlyOnce",
        "MLRestartOwnershipPartition",
        "MLRecoveredCarrierBodyAuthenticated",
        "MLRecoveredCarrierLengthAuthenticated",
        "MLHistoricalRecoveryContextExact",
        "MLHistoricalQueueGateOrder",
        "MLHistoricalAllGroupsPreflight",
        "MLStageEvidenceMonotonic",
    ),
    "SumeragiV2QueuePlanAdmissionRegistry": (
        "MLAdmissionCasUnique",
        "MLCertificateDurable",
        "MLPublic202Exact",
        "MLExecutionRequiresExactBinding",
        "MLQueueEligibilityExact",
        "MLAdmissionAtMostOnceExecution",
        "MLImmutableAdmissionTombstone",
        "MLCancellationStopsExecution",
    ),
}
_FORMAL_SCRIPT_DIR = str(Path(__file__).resolve().parent)
sys.path.insert(0, _FORMAL_SCRIPT_DIR)
try:
    from sumeragi_v2_multilane_kura_retention_contract import (
        KURA_RETENTION_CONTRACT_KEY,
        KURA_RETENTION_HANDOFF_ORDERED_TOKENS,
        KURA_RETENTION_INVARIANTS,
        KURA_RETENTION_MODULE,
        KURA_RETENTION_MUTATIONS,
        KURA_RETENTION_POSITIVE_CONFIG,
        KURA_RETENTION_PRESTAGE_ORDERED_TOKENS,
        KURA_RETENTION_REFINEMENT_OBLIGATION,
        KURA_RETENTION_REFRESH_START_ORDERED_TOKENS,
        KURA_RETENTION_REQUIRED_BINDINGS,
    )
    from sumeragi_v2_multilane_inflight_contract import (
        INFLIGHT_COMPOSED_TLA_ALIGNMENT_TOKENS,
        INFLIGHT_LAYOUT_CLAIM,
        INFLIGHT_LAYOUT_EVIDENCE,
        INFLIGHT_LAYOUT_FORBIDDEN_SOURCE_CHECKS,
        INFLIGHT_LAYOUT_FORBIDDEN_TOKENS,
        INFLIGHT_LAYOUT_MODULE,
        INFLIGHT_LAYOUT_MUTATIONS,
        INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS,
        INFLIGHT_LAYOUT_POSITIVE_CONFIG,
        INFLIGHT_LAYOUT_PRODUCTION_BINDINGS,
        INFLIGHT_LAYOUT_REQUIRED_ACTIONS,
        INFLIGHT_LAYOUT_REQUIRED_INVARIANTS,
        INFLIGHT_LAYOUT_RUNNER,
        INFLIGHT_LAYOUT_SOURCE_CHECKS,
        INFLIGHT_LAYOUT_TEST,
    )
finally:
    sys.path.pop(0)


TLA_COUNTEREXAMPLE = "tla_counterexample"
STATIC_RELEASE = "static_release"
DIFFERENTIAL_RELEASE = "differential_release"
RELEASE_INVARIANT_CLASSIFICATIONS = frozenset(
    (STATIC_RELEASE, DIFFERENTIAL_RELEASE)
)
EXPECTED_CLOSURE_MUTATIONS = {
    "ML-MUT-NAT-01": (
        TLA_COUNTEREXAMPLE,
        "MLSeparateParticipantApplication",
        ("multilane_native_same_route_marker_bug.cfg",),
    ),
    "ML-MUT-NAT-02": (
        TLA_COUNTEREXAMPLE,
        "MLNativeSourceClaimInjective",
        ("multilane_native_source_claim_equivocation_bug.cfg",),
    ),
    "ML-MUT-NAT-03": (
        TLA_COUNTEREXAMPLE,
        "MLNativeContiguousActiveRoute",
        ("multilane_native_noncontiguous_route_bug.cfg",),
    ),
    "ML-MUT-NAT-04": (
        TLA_COUNTEREXAMPLE,
        "MLNativeGroupExactCover",
        ("multilane_native_partial_group_application_bug.cfg",),
    ),
    "ML-MUT-NAT-05": (
        TLA_COUNTEREXAMPLE,
        "MLNativeManifestAuthenticates",
        ("multilane_native_forged_manifest_leaf_bug.cfg",),
    ),
    "ML-MUT-NAT-06": (
        TLA_COUNTEREXAMPLE,
        "MLNativeDurabilityPrecedesFrontier",
        (
            "multilane_native_frontier_before_sidecars_bug.cfg",
            "multilane_native_hash_only_pruning_bug.cfg",
            "multilane_native_dropped_startup_repair_bug.cfg",
            "multilane_native_shared_evidence_budget_bug.cfg",
            "multilane_native_second_incoming_pair_bug.cfg",
            "multilane_native_unauthenticated_temp_promotion_bug.cfg",
            "multilane_native_punctured_retained_history_bug.cfg",
            "multilane_native_nonoldest_prefix_prune_bug.cfg",
            "multilane_native_nonhighest_repair_half_bug.cfg",
            "multilane_native_multiple_repair_halves_bug.cfg",
            "multilane_native_conflicting_retained_pair_bug.cfg",
            "multilane_native_retained_predecessor_drift_bug.cfg",
            "multilane_native_mutating_unified_startup_plan_bug.cfg",
            "multilane_native_uncoalesced_canonical_body_needs_bug.cfg",
            "multilane_native_partial_unified_startup_preflight_bug.cfg",
            "multilane_native_queue_before_evidence_readback_bug.cfg",
            "multilane_native_missing_reverse_merge_carrier_bug.cfg",
            "multilane_native_orphan_merge_carrier_bug.cfg",
            "multilane_native_skip_post_cache_carrier_reconcile_bug.cfg",
        ),
    ),
    "ML-MUT-NAT-07": (
        TLA_COUNTEREXAMPLE,
        "MLNativeLatestIndexExact",
        ("multilane_native_ambiguous_latest_index_bug.cfg",),
    ),
    "ML-MUT-KURA-01": (
        TLA_COUNTEREXAMPLE,
        KURA_RETENTION_REFINEMENT_OBLIGATION,
        tuple(config for config, _ in KURA_RETENTION_MUTATIONS),
    ),
    "ML-MUT-QUEUE-01": (
        TLA_COUNTEREXAMPLE,
        "QueuePlanAdmissionRegistryProductionRefinementObligation",
        (
            "multilane_queue_plan_split_route_public_acceptance_bug.cfg",
            "multilane_queue_plan_execution_before_global_cas_bug.cfg",
            "multilane_queue_plan_conflicting_cas_bug.cfg",
            "multilane_queue_plan_restart_aba_bug.cfg",
            "multilane_queue_plan_local_expiry_clears_tombstone_bug.cfg",
            "multilane_queue_plan_deferred_bypass_bug.cfg",
            "multilane_queue_plan_cancellation_bypass_bug.cfg",
            "multilane_queue_plan_guard_drop_deletes_durable_owner_bug.cfg",
            "multilane_queue_plan_execution_without_exact_binding_bug.cfg",
            "multilane_queue_plan_duplicate_execution_bug.cfg",
        ),
    ),
    "ML-MUT-AUT-01": (
        TLA_COUNTEREXAMPLE,
        "MLReservationSingleOwner",
        ("multilane_autonomous_reserve_before_durable_bug.cfg",),
    ),
    "ML-MUT-AUT-02": (
        TLA_COUNTEREXAMPLE,
        "MLReservationIdentityStable",
        ("multilane_autonomous_carrier_drift_bug.cfg",),
    ),
    "ML-MUT-AUT-03": (
        TLA_COUNTEREXAMPLE,
        "MLCertifiedBundleDurable",
        ("multilane_autonomous_digest_only_authorization_bug.cfg",),
    ),
    "ML-MUT-AUT-04": (
        TLA_COUNTEREXAMPLE,
        "MLMergeCandidateExactPrefix",
        ("multilane_autonomous_noncanonical_merge_prefix_bug.cfg",),
    ),
    "ML-MUT-AUT-05": (
        TLA_COUNTEREXAMPLE,
        "MLCarrierExactlyOnce",
        (
            "multilane_autonomous_duplicate_application_bug.cfg",
            "multilane_autonomous_release_after_apply_bug.cfg",
            "multilane_autonomous_release_before_barrier_bug.cfg",
            "multilane_autonomous_ordinary_anchor_execution_bug.cfg",
            "multilane_autonomous_skip_canonical_reexecution_bug.cfg",
        ),
    ),
    "ML-MUT-AUT-06": (
        TLA_COUNTEREXAMPLE,
        "MLRestartOwnershipPartition",
        (
            "multilane_autonomous_aba_release_bug.cfg",
            "multilane_autonomous_restart_drops_ownership_bug.cfg",
        ),
    ),
    "ML-MUT-AUT-07": (
        TLA_COUNTEREXAMPLE,
        "MLRecoveredCarrierBodyAuthenticated",
        (
            "multilane_autonomous_unauthenticated_recovery_body_bug.cfg",
            "multilane_autonomous_mixed_signer_recovery_body_bug.cfg",
        ),
    ),
    "ML-MUT-AUT-08": (
        TLA_COUNTEREXAMPLE,
        "MLHistoricalRecoveryContextExact",
        ("multilane_autonomous_historical_context_drift_bug.cfg",),
    ),
    "ML-MUT-AUT-09": (
        TLA_COUNTEREXAMPLE,
        "MLHistoricalQueueGateOrder",
        ("multilane_autonomous_open_queue_before_recovery_install_bug.cfg",),
    ),
    "ML-MUT-AUT-10": (
        TLA_COUNTEREXAMPLE,
        "MLHistoricalAllGroupsPreflight",
        (
            "multilane_autonomous_partial_recovery_group_preflight_bug.cfg",
        ),
    ),
    "ML-MUT-AUT-11": (
        TLA_COUNTEREXAMPLE,
        "MLRecoveredCarrierLengthAuthenticated",
        (
            "multilane_autonomous_inflated_recovery_wire_length_bug.cfg",
        ),
    ),
    "ML-MUT-LIFE-01": (
        TLA_COUNTEREXAMPLE,
        "MLActivationAfterAtomicCreate",
        ("multilane_autoscale_activation_before_storage_bug.cfg",),
    ),
    "ML-MUT-LIFE-02": (
        TLA_COUNTEREXAMPLE,
        "MLDrainImpliesNoOwnedWork",
        ("multilane_autoscale_early_drain_bug.cfg",),
    ),
    "ML-MUT-LIFE-03": (
        TLA_COUNTEREXAMPLE,
        "MLDrainCertificateMonotonic",
        ("multilane_autoscale_weak_drain_certificate_bug.cfg",),
    ),
    "ML-MUT-LIFE-04": (
        TLA_COUNTEREXAMPLE,
        "MLRetirementConsumesExactIncarnation",
        (
            "multilane_autoscale_destroy_before_archive_bug.cfg",
            "multilane_autoscale_incarnation_reuse_bug.cfg",
            "multilane_autoscale_cleanup_by_lane_id_bug.cfg",
        ),
    ),
    "ML-MUT-LIFE-05": (
        TLA_COUNTEREXAMPLE,
        "MLStageEvidenceMonotonic",
        ("multilane_autonomous_volatile_stage_diagnostics_bug.cfg",),
    ),
    "ML-MUT-API-01": (
        STATIC_RELEASE,
        "MLDiagnosticsAreDerived",
        (),
    ),
    "ML-MUT-API-02": (
        DIFFERENTIAL_RELEASE,
        "MLApiAuthoritySeparation",
        (),
    ),
    "ML-MUT-API-03": (
        DIFFERENTIAL_RELEASE,
        "MLSdkAcceptSetEqualsRust",
        (),
    ),
    "ML-MUT-API-04": (
        DIFFERENTIAL_RELEASE,
        "MLFixtureHasOneCanonicalOwner",
        (),
    ),
    "ML-MUT-WIRE-01": (
        STATIC_RELEASE,
        "MLConsensusLayoutAgreement",
        (),
    ),
}
EXPECTED_RELEASE_INVARIANT_SOURCE_PATHS = {
    "ML-MUT-API-01": (
        "crates/iroha_core/src/state.rs",
        "crates/iroha_core/src/state/tests.rs",
        "crates/iroha_torii/src/routing.rs",
        "crates/iroha_torii/src/tests/routing.rs",
    ),
    "ML-MUT-API-02": (
        "pytests/scripts/native_amx_v2_grouped_fixture_test.py",
        "python/iroha_torii_client/tests/test_client.py",
        "python/iroha_python/tests/client_sumeragi_v2_status_test.py",
        "IrohaSwift/Tests/IrohaSwiftTests/NativeAmxV2GroupedFixtureTests.swift",
    ),
    "ML-MUT-API-03": (
        "ci/run_native_amx_v2_grouped_sdk_parity.sh",
        "fixtures/sumeragi_v2/native_amx_v2_grouped.json",
    ),
    "ML-MUT-API-04": (
        "crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs",
        "crates/iroha_data_model/src/bin/native_amx_grouped.rs",
        "ci/check_sumeragi_v2_multilane_release_inventory.sh",
    ),
    "ML-MUT-WIRE-01": (
        "scripts/check_no_legacy_codec.sh",
        "ci/check_sumeragi_v2_multilane_release_inventory.sh",
    ),
}
CLOSURE_MUTATION_ID_RE = re.compile(r"`(ML-MUT-[A-Z]+-[0-9]{2})`")
FORBIDDEN_PRODUCTION_TOKENS = {
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "plan_lane_reservation_ownership",
    ): ("merge_ledger_all_entries",),
}
NATIVE_PREPUBLICATION_MODULE = "SumeragiV2NativeApplicationEvidence"
NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_BINDINGS = (
    (
        "crates/iroha_core/src/block.rs",
        "fn",
        "validate_native_amx_participant_groups",
        (
            "for leg in &receipt.legs",
            "native_amx_participant_application_role(receipt, leg)",
            "NativeAmxParticipantApplicationRole::Coordinator",
            "continue;",
            "NativeAmxParticipantApplicationRole::SeparateParticipant",
            "return Err(Self::execution_context_error",
            "native AMX participant application identity is invalid at index",
            "let descriptor = &leg.participant_proposal.descriptor",
            "groups.get_mut(&key)",
            "groups.insert(",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "native_amx_participant_application_diagnostic_rows_from_native_receipt",
        (
            "for leg in &receipt.legs",
            "native_amx_participant_application_role(receipt, leg)",
            "NativeAmxParticipantApplicationRole::Coordinator",
            "continue;",
            "NativeAmxParticipantApplicationRole::SeparateParticipant",
            "return Err(MergeLedgerCommitError::ExecutionBatchInvalid",
            "certified Native AMX participant diagnostics contain an invalid leg",
            "let row = SumeragiNativeAmxParticipantApplication",
            "SumeragiNativeAmxParticipantApplicationState::CertifiedPendingCarrier",
            "row.validate().map_err",
            "rows.push(row)",
        ),
    ),
)
NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_MATCH_RELATIONS = (
    (
        "crates/iroha_core/src/block.rs",
        "fn",
        "validate_native_amx_participant_groups",
        (
            "match crate::native_amx::"
            "native_amx_participant_application_role(receipt, leg) { "
            "Ok(crate::native_amx::"
            "NativeAmxParticipantApplicationRole::Coordinator) => { continue; } "
            "Ok( crate::native_amx::"
            "NativeAmxParticipantApplicationRole::SeparateParticipant, ) => {} "
            "Err(error) => { return Err(Self::execution_context_error(format!( "
            '"native AMX participant application identity is invalid at index '
            '{index}: {error}" ))); } }'
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "native_amx_participant_application_diagnostic_rows_from_native_receipt",
        (
            "match crate::native_amx::"
            "native_amx_participant_application_role(receipt, leg) { "
            "Ok(crate::native_amx::"
            "NativeAmxParticipantApplicationRole::Coordinator) => { continue; } "
            "Ok(crate::native_amx::"
            "NativeAmxParticipantApplicationRole::SeparateParticipant) => { } "
            "Err(reason) => { return Err("
            "MergeLedgerCommitError::ExecutionBatchInvalid(format!( "
            '"certified Native AMX participant diagnostics contain an invalid '
            'leg: {reason}", ))); } }'
        ),
    ),
)
NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/block.rs",
        "fn",
        "validate_native_amx_participant_groups",
        (
            "for leg in &receipt.legs",
            "match crate::native_amx::native_amx_participant_application_role(receipt, leg)",
            "let descriptor = &leg.participant_proposal.descriptor",
            "groups.get_mut(&key)",
            "groups.insert(",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "native_amx_participant_application_diagnostic_rows_from_native_receipt",
        (
            "for leg in &receipt.legs",
            "match crate::native_amx::native_amx_participant_application_role(receipt, leg)",
            "let descriptor = &leg.participant_proposal.descriptor",
            "let row = SumeragiNativeAmxParticipantApplication",
            "SumeragiNativeAmxParticipantApplicationState::CertifiedPendingCarrier",
            "row.validate().map_err",
            "rows.push(row)",
        ),
    ),
)
NATIVE_PREPUBLICATION_BINDINGS = (
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "repair_native_amx_participant_application_evidence",
        (
            "prune_lock.lock",
            "ensure_prune_recovery_not_required",
            "native_amx_participant_application_evidence_for_block_under_publication_guard",
            "block, true",
            "persist_native_amx_participant_application_evidence_under_publication_guard",
            "NativeAmxParticipantApplicationPublicationMode::PostWsvRepair",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "prepublish_native_amx_participant_application_evidence",
        (
            "prune_lock.lock",
            "ensure_prune_recovery_not_required",
            "native_amx_participant_application_evidence_for_block_under_publication_guard",
            "block, false",
            "persist_native_amx_participant_application_evidence_under_publication_guard",
            "NativeAmxParticipantApplicationPublicationMode::PreWsv",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "enum",
        "NativeAmxParticipantApplicationPublicationMode",
        ("PreWsv", "PostWsvRepair"),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "NativeAmxParticipantApplicationPublicationMode::requires_post_apply_metadata",
        ("matches!(self, Self::PostWsvRepair)",),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "NativeAmxParticipantApplicationPublicationMode::permits_retention_cleanup",
        ("matches!(self, Self::PostWsvRepair)",),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "persist_native_amx_participant_application_evidence_under_publication_guard",
        (
            "get_durable_block_hash",
            "plan.application_block_height",
            "plan.application_block_hash",
            "plan.manifest_leaf_count",
            "mode.permits_retention_cleanup()",
            "write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard",
            "write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard",
            "write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard",
            "authenticate_native_amx_participant_application_prepublication_under_publication_guard",
            "mode.requires_post_apply_metadata()",
            "NativeAmxParticipantApplicationPrepublicationToken::from_plan",
            "if permit_cleanup",
            "cleanup_native_amx_participant_application_evidence_under_publication_guard",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard",
        (
            "ensure_prune_recovery_not_required",
            "get_durable_block_hash",
            "require_active_lane_incarnation",
            "native_amx_evidence_namespace_for_entry",
            "permit_retention_cleanup",
            "require_native_amx_evidence_prune_intent_absent_locked",
            "recover_native_amx_evidence_publication_temp_locked",
            "discard_native_amx_latest_index_temp_locked",
            "publish_native_amx_evidence_file_locked",
            "NativeAmxEvidenceKind::Manifest",
            "STRICT_INIT_MAX_BLOCK_BYTES",
            "progress_mutation_namespace_unchanged",
            "native_amx_evidence_tracked_bytes_locked",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard",
        (
            "manifest_artifact_hash",
            "native_amx_participant_receipt_matches_manifest_leaf",
            "require_active_lane_artifact",
            "native_amx_evidence_namespace_for_entry",
            "permit_retention_cleanup",
            "require_native_amx_evidence_prune_intent_absent_locked",
            "recover_native_amx_evidence_publication_temp_locked",
            "discard_native_amx_latest_index_temp_locked",
            "read_native_amx_participant_application_manifest_from_paths_locked",
            "publish_native_amx_evidence_file_locked",
            "NativeAmxEvidenceKind::Receipt",
            "progress_mutation_namespace_unchanged",
            "native_amx_evidence_tracked_bytes_locked",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard",
        (
            "manifest_artifact_hash",
            "finality_artifact_hash",
            "native_amx_participant_receipt_matches_manifest_leaf",
            "preflight.incoming",
            "get_durable_block_hash",
            "require_active_lane_artifact",
            "native_amx_evidence_namespace_for_entry",
            "permit_retention_cleanup",
            "current != preflight.current",
            "validate_native_amx_prepublication_transition_locked",
            "require_native_amx_evidence_prune_intent_absent_locked",
            "recover_native_amx_evidence_publication_temp_locked",
            "read_native_amx_participant_application_manifest_from_paths_locked",
            "read_native_amx_participant_application_receipt_from_paths_locked",
            "persist_native_amx_participant_receipt_latest_index_from_reconstructed_inventory_locked",
            "progress_mutation_namespace_unchanged",
            "native_amx_evidence_tracked_bytes_locked",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "authenticate_native_amx_participant_application_prepublication_under_publication_guard",
        (
            "require_active_lane_artifact",
            "read_native_amx_participant_application_manifest_from_paths_locked",
            "read_native_amx_participant_application_receipt_from_paths_locked",
            "decode_bound_native_amx_participant_receipt_latest_index_locked",
            "require_post_apply_metadata",
            "native_amx_participant_application_receipt_matches_manifest_and_available_evidence_under_prune_canonical_and_sidecar_guards",
            "native_amx_participant_application_manifest_matches_available_finality_under_prune_and_canonical_guards",
            "latest.matches_receipt",
            "latest.matches_manifest",
            "progress_mutation_namespace_unchanged",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "cleanup_native_amx_participant_application_evidence_under_publication_guard",
        (
            "require_active_lane_artifact",
            "decode_bound_native_amx_participant_receipt_latest_index_locked",
            "latest.matches_receipt",
            "prune_native_amx_evidence_pairs_locked",
            "native_amx_evidence_tracked_bytes_locked",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_and_apply",
        (
            "NativeAmxApplicationManifestV1::from_result_bearing_block",
            "execution_commitment_from_validated_block",
            "store_block",
            "store_v2_finality_artifact",
            "prepublish_native_amx_participant_application_evidence",
            "State::native_amx_participant_frontier_markers",
            "token.authenticates_state_frontiers",
            "apply_without_execution_with_verified_v2_finality",
            "state_block.commit",
        ),
    ),
)
NATIVE_PREPUBLICATION_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_and_apply",
        (
            ".store_v2_finality_artifact(artifact)",
            ".prepublish_native_amx_participant_application_evidence(",
            "State::native_amx_participant_frontier_markers(",
            "token.authenticates_state_frontiers(",
            ".apply_without_execution_with_verified_v2_finality(&committed_block, commit_topology)",
            "state_block.commit().map_err",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "persist_native_amx_participant_application_evidence_under_publication_guard",
        (
            "self.write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard(",
            "self.read_back_native_amx_plan_manifests_under_publication_guard(plan)",
            "self.write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard(",
            "self.write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard(",
            "self.authenticate_native_amx_participant_application_prepublication_under_publication_guard(",
            "let token = NativeAmxParticipantApplicationPrepublicationToken::from_plan",
            "if permit_cleanup {",
            "self.cleanup_native_amx_participant_application_evidence_under_publication_guard(",
        ),
    ),
)
NATIVE_PREPUBLICATION_RETENTION_WRITERS = (
    "write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard",
    "write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard",
    "write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard",
)
QUEUE_PLAN_STARTUP_REPLAY_MODULE = "SumeragiV2QueuePlanAdmissionRegistry"
QUEUE_PLAN_STARTUP_REPLAY_BINDINGS = (
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournalReplay::into_verified_records",
        (
            "self.verify_snapshot_content()?",
            "std::mem::take(&mut self.live_positions)",
            "live.ownership_position",
            "self.verify_snapshot_storage()?",
            "record.claim_digest()",
            "record.entrypoint_hash != entrypoint_hash",
            "record.plan_digest() != live.plan_digest",
            "claim_digest != live.claim_digest",
            "verified.push(record)",
            "Ok(verified)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournal::remove_all_live_exact_atomic_strict_durable",
        (
            "remove_many_exact_atomic_strict_durable_inner(removals, true)?",
            "Ok(())",
        ),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournal::remove_many_exact_atomic_strict_durable_inner",
        (
            "self.ensure_healthy()?",
            "removals.len() > self.limits.max_live_records",
            "QueuePlanJournalFrameV4::RemoveBatch(requested.clone())",
            "prepare_replay_with_removed_entrypoints(Some(&entrypoints))",
            "if require_all_live",
            "live_removals.len() != requested.len()",
            "QueuePlanJournalExactRemoveResult::Removed",
            "atomic live-removal batch contains an already-absent target",
            "QueuePlanJournalFrameV4::RemoveBatch(live_removals.clone())",
            "self.compact(true)?",
            "if compacted != (outcomes.clone(), live_removals.clone())",
            "self.append_encoded(&encoded, AppendPhase::OrdinaryRemove)",
            "self.sync_all_raw(SyncPhase::General)?",
            "Ok(outcomes)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::ensure_plan_journal_replay_startup_shape_locked",
        (
            "self.txs.is_empty()",
            "self.materialized_active_len() == 0",
            "self.materialized_retained_bytes() == 0",
            "self.tx_hashes.is_empty()",
            "self.queued_count.load(Ordering::Acquire) == 0",
            "self.routing_decisions.is_empty()",
            "self.routing_plans.is_empty()",
            "self.durable_plan_claims.is_empty()",
            "self.tx_encoded_len.is_empty()",
            "self.tx_gas_cost.is_empty()",
            "self.tx_enqueued_at_ms.is_empty()",
            "self.queued_tx_enqueued_at_ms.is_empty()",
            "self.queued_age_ring.lock().is_empty()",
            "self.removed_hashes.is_empty()",
            "self.txs_per_user.is_empty()",
            "fee_admission_reservations",
            "self.expiry_ring.lock().is_empty()",
            "self.expiry_ring_members.is_empty()",
            "self.tx_gossip.is_empty()",
            "self.tx_teu.is_empty()",
            "lane_teu_pending",
            "dataspace_teu_pending",
            "only exact durable reservation FIFO identities may pre-exist",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::plan_journal_replay_reservation_shape_locked",
        (
            "store.durable_owned_hashes().collect::<HashSet<_>>()",
            ".filter(|hash| !self.txs.contains_key(hash))",
            "expected_missing_payload_hashes != store.missing_payload_hashes",
            "missing_reservation_payload_count",
            "store.missing_payload_hashes.len()",
            "store.live_by_hash.values().chain(",
            "completed_releases",
            "record.validate()",
            ".insert(hash, record.fifo_order)",
            "multiple durable FIFO owners",
            "durable_owned_hashes",
            "durable_fifo_orders",
            "missing_payload_hashes: store.missing_payload_hashes.clone()",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::prepare_plan_journal_replay_locked",
        (
            "self.ensure_plan_journal_replay_startup_shape_locked()?",
            "self.plan_journal_replay_reservation_shape_locked()?",
            "journal_hashes.len() != records.len()",
            ".missing_payload_hashes",
            ".difference(&journal_hashes)",
            "commit_barrier_hashes.contains(hash)",
            "state_view.transactions.get(hash).is_none()",
            "let replay_observed_at = self.time_source.get_unix_time();",
            "AcceptedTransaction::accept_entrypoint_at_time",
            "accepted.hash_as_entrypoint() != entrypoint_hash",
            "queue_plan_replay_reservation_owner",
            "reservation_shape.durable_owned_hashes.contains(&hash)",
            "reservation_owner.is_present()",
            "reservation_owner.fifo_order()",
            "state_view.transactions.get(&hash).is_some()",
            "recorded_global_admission_identity",
            "queue_plan_admission_registry_match",
            "QueuePlanAdmissionRegistryMatch::Conflict",
            "global_registry_match.is_none()",
            "self.is_expired_at_with_enqueue_timestamp(",
            "replay_observed_at",
            "!has_durable_reservation_owner",
            "resolve_routing_plan_for_queue_admission(",
            "durable_plan_claim_context_revalidates_in_view",
            "QueueAdmissionPreparationMode::AtomicJournalReplay",
            "transaction_selection_durability_faulted()",
            "self.active_len()",
            "self.retained_bytes()",
            "projected_active > self.capacity.get()",
            "projected_retained > self.max_retained_bytes.get()",
            "projected > self.capacity_per_user.get()",
            ".reserve(admission.hash, reservation)",
            "orphaned FIFO identity",
            "reservation FIFO anchors disagree with authenticated journal order",
            "anchors.len() != reservation_shape.durable_fifo_orders.len()",
            "final_fifo.len() > self.tx_hashes.capacity()",
            "terminal_removals",
            "Ok(PreparedQueuePlanReplay {",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::apply_plan_journal_replay_locked",
        (
            "terminal_removals: _",
            "*self.fee_admission_reservations.lock() = fee_reservations;",
            "*self.next_fifo_ordinal.lock() = next_fifo_ordinal;",
            "self.fifo_order_by_hash.insert(hash, fifo_order);",
            "self.txs.insert(hash, Arc::clone(&tx_arc));",
            "self.track_active_transaction();",
            "self.routing_decisions.insert(hash, routing_decision);",
            "self.routing_plans.insert(hash, routing_plan.clone());",
            "self.durable_plan_claims.insert(hash, claim.clone());",
            "self.track_expiry_hash(hash);",
            "notifications.push(QueueAdmissionNotification {",
            "self.apply_per_user_tx_count_increments(per_user_increments);",
            "self.reconcile_missing_reservation_payloads_locked(&mut store);",
            "self.replace_fifo_locked(&final_fifo);",
            "(summary, notifications)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::replay_plan_journal",
        (
            "self.plan_journal_install_lock.lock()",
            "self.lane_reservation_transition_lock.lock()",
            "state.lock_lane_lifecycle_work_admission()",
            "state.state_view_generation()",
            "let state_view = state.view();",
            "self.ensure_plan_journal_replay_startup_shape_locked()?",
            "self.sync_nexus_routing_with_view(&state_view);",
            "let mut journal_guard = self.plan_journal.lock();",
            "let queue_guard = self.push_remove_lock.lock();",
            "let records = journal.prepare_replay()?.into_verified_records()?;",
            "let expected_record_claims = records",
            "self.prepare_plan_journal_replay_locked(",
            "let observed_record_claims = journal",
            "if observed_record_claims != expected_record_claims",
            "let terminal_removals = prepared.terminal_removals.clone();",
            "remove_all_live_exact_atomic_strict_durable(&terminal_removals)",
            "self.mark_plan_journal_durability_fault",
            "self.apply_plan_journal_replay_locked(prepared)",
            "self.publish_admission_notifications(&notifications);",
            "self.publish_backpressure_state(self.active_len(), backpressure_telemetry);",
            "status::set_tx_queue_pressure(self.pressure_snapshot());",
            "Ok(summary)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::complete_lane_reservation_startup_reconciliation",
        (
            "self.lane_reservation_transition_lock.lock()",
            "self.push_remove_lock.lock()",
            "self.transaction_selection_durability_faulted()",
            "!store.commit_barriers.is_empty()",
            "!store.release_barriers.is_empty()",
            "!store.completed_releases.is_empty()",
            "!store.missing_payload_hashes.is_empty()",
            "lane_reservation_reconciliation_pending",
            ".store(false, Ordering::Release)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "fn",
        "apply_lane_reservation_reconciliation_plan",
        (
            "historical_autonomous_install_is_durable",
            "lane_reservation_reconciliation_snapshot",
            "lane_reservation_commit_barriers",
            "lane_reservation_release_barriers",
            "commit_lane_reservation",
            "retire_autonomous_lane_slot_and_release_reservations",
            "release_lane_reservations_in_order",
            "complete_lane_reservation_startup_reconciliation",
        ),
    ),
    (
        "crates/irohad/src/main.rs",
        "method",
        "Iroha::start_with_runtime_deps",
        (
            "install_lane_reservation_journal(",
            "install_plan_journal(",
            "replay_plan_journal(&state)",
            "IrohaNetwork::start_with_crypto(",
        ),
    ),
)
QUEUE_PLAN_STARTUP_REPLAY_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournalReplay::into_verified_records",
        (
            "self.verify_snapshot_content()?;",
            "std::mem::take(&mut self.live_positions)",
            "ordered.sort_unstable_by_key",
            "for (entrypoint_hash, live) in ordered {",
            "self.verify_snapshot_storage()?;",
            "let claim_digest = record.claim_digest()",
            "if record.entrypoint_hash != entrypoint_hash",
            "verified.push(record);",
            "self.verify_snapshot_content()?;",
            "Ok(verified)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        "method",
        "QueuePlanJournal::remove_many_exact_atomic_strict_durable_inner",
        (
            "let (outcomes, live_removals) =",
            "if require_all_live",
            "atomic live-removal batch contains an already-absent target",
            "if live_removals.is_empty()",
            "let encoded = encode_frame(",
            "self.ensure_append_capacity(encoded.len())",
            "self.compact(true)?;",
            "let compacted =",
            "if compacted != (outcomes.clone(), live_removals.clone())",
            "self.append_encoded(&encoded, AppendPhase::OrdinaryRemove)",
            "self.sync_all_raw(SyncPhase::General)?;",
            "Ok(outcomes)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::prepare_plan_journal_replay_locked",
        (
            "self.ensure_plan_journal_replay_startup_shape_locked()?;",
            "self.plan_journal_replay_reservation_shape_locked()?;",
            "let journal_hashes = records",
            ".missing_payload_hashes",
            ".difference(&journal_hashes)",
            "let replay_observed_at = self.time_source.get_unix_time();",
            "for record in records {",
            "AcceptedTransaction::accept_entrypoint_at_time(",
            "queue_plan_replay_reservation_owner(",
            "let state_committed = state_view.transactions.get(&hash).is_some();",
            "let global_registry_match =",
            "self.is_expired_at_with_enqueue_timestamp(",
            "resolve_routing_plan_for_queue_admission(",
            "prepare_checked_for_enqueue(",
            "if self.transaction_selection_durability_faulted()",
            "let mut projected_active = self.active_len();",
            "let mut fifo_orders =",
            "let anchors = pending_admissions",
            "Ok(PreparedQueuePlanReplay {",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::apply_plan_journal_replay_locked",
        (
            "*self.fee_admission_reservations.lock() = fee_reservations;",
            "*self.next_fifo_ordinal.lock() = next_fifo_ordinal;",
            "for replayed in admissions {",
            "self.fifo_order_by_hash.insert(hash, fifo_order);",
            "self.txs.insert(hash, Arc::clone(&tx_arc));",
            "self.durable_plan_claims.insert(hash, claim.clone());",
            "notifications.push(QueueAdmissionNotification {",
            "self.apply_per_user_tx_count_increments(per_user_increments);",
            "self.reconcile_missing_reservation_payloads_locked(&mut store);",
            "self.replace_fifo_locked(&final_fifo);",
            "(summary, notifications)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::replay_plan_journal",
        (
            "self.plan_journal_install_lock.lock()",
            "self.lane_reservation_transition_lock.lock()",
            "state.lock_lane_lifecycle_work_admission()",
            "let state_view = state.view();",
            "self.ensure_plan_journal_replay_startup_shape_locked()?;",
            "self.sync_nexus_routing_with_view(&state_view);",
            "let mut journal_guard = self.plan_journal.lock();",
            "let queue_guard = self.push_remove_lock.lock();",
            "let records = journal.prepare_replay()?.into_verified_records()?;",
            "let expected_record_claims = records",
            "let prepared = self.prepare_plan_journal_replay_locked(",
            "let observed_record_claims = journal",
            ".prepare_replay()?",
            ".into_verified_records()?",
            "if observed_record_claims != expected_record_claims",
            "let terminal_removals = prepared.terminal_removals.clone();",
            "remove_all_live_exact_atomic_strict_durable(&terminal_removals)",
            "self.apply_plan_journal_replay_locked(prepared)",
            "self.publish_admission_notifications(&notifications);",
            "Ok(summary)",
        ),
    ),
    (
        "crates/irohad/src/main.rs",
        "method",
        "Iroha::start_with_runtime_deps",
        (
            "install_lane_reservation_journal(",
            "install_plan_journal(",
            "replay_plan_journal(&state)",
            "IrohaNetwork::start_with_crypto(",
        ),
    ),
)
QUEUE_PLAN_STARTUP_REPLAY_FORBIDDEN_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::apply_plan_journal_replay_locked",
        (
            "?",
            "Result<",
            "return Err(",
            "expect(",
            "unwrap(",
            "panic!(",
            "unreachable!(",
        ),
    ),
)
QUEUE_PLAN_STARTUP_REPLAY_POST_APPLY_MARKER = (
    "let (summary, notifications) = self.apply_plan_journal_replay_locked(prepared);"
)
QUEUE_PLAN_STARTUP_REPLAY_POST_APPLY_FORBIDDEN_TOKENS = (
    "?",
    "return Err(",
    ".map_err(",
    "expect(",
    "unwrap(",
    "panic!(",
    "unreachable!(",
)
QUEUE_PLAN_STARTUP_REPLAY_TEST_BINDINGS = (
    (
        "crates/iroha_core/src/queue/journal.rs",
        "exact_atomic_live_tombstone_batch_rejects_retry_before_append",
        (
            "remove_all_live_exact_atomic_strict_durable(",
            "expect_err(",
            "io::ErrorKind::InvalidData",
            "the startup publication form must reject a mixed absent and live batch",
            "the all-live precondition must reject a mixed batch before append",
            "rejecting a mixed batch must retain its still-live member",
            "the all-live precondition must reject before another frame is appended",
        ),
    ),
    (
        "crates/iroha_core/src/queue/plan_journal_replay_tests.rs",
        "materialized_replay_rejects_later_record_corruption_before_any_callback",
        (
            ".get_mut(&second_key)",
            ".for_each_record(",
            "expect_err(",
            "callbacks, 0",
            "a valid earlier record must remain private when a later record is corrupt",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "queue_plan_journal_replay_retains_current_admission_rejection_and_fails_startup",
        (
            "expect_err(\"a current admission failure must abort startup\")",
            "failed current admission",
            "assert_eq!(replay_queue.active_len(), 0);",
            "live_record_count()",
            "without publishing or tombstoning a prefix",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "queue_plan_journal_replay_rejects_aggregate_per_user_overflow_without_prefix",
        (
            "capacity_per_user = nonzero!(1_usize)",
            "aggregate per-user overflow must reject the complete replay",
            "std::io::ErrorKind::PermissionDenied",
            "assert_eq!(replay_queue.active_len(), 0);",
            "live_record_count()",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "queue_plan_journal_replay_rejects_orphaned_startup_fifo_identity",
        (
            "fifo_order_by_hash.insert(orphan, fifo_order)",
            "an unowned startup FIFO identity must fail closed",
            "orphaned FIFO identity",
            "Some(fifo_order)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/lane_reservation_tests.rs",
        "reservation_restart_fits_ordinary_fifo_around_middle_anchor",
        (
            "install_lane_reservation_journal(&reservation_path",
            "replay_plan_journal(&state)",
            "Some(u64::try_from(index)",
            "release_lane_reservations_in_order(&[reserved_key])",
            "restart replay must preserve A/B/C",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_recovery_tests.rs",
        "state_committed_live_reservation_replays_quarantined_until_explicit_proof_commit",
        (
            "authenticate and quarantine the sole payload source",
            "tombstoned_committed: 0",
            "assert!(queue.txs.contains_key(&hash));",
            "assert_eq!(queue.queued_len(), 0);",
            "missing_reservation_payload_count",
            "live_record_count()",
            "commit_lane_reservation_for_test(&key)",
            "lane_reservation_commit_barriers().is_empty()",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_recovery_tests.rs",
        "expired_live_reservation_replays_payload_without_fifo_or_tombstone",
        (
            "transaction_time_to_live: Duration::from_millis(1)",
            "time_handle.advance(Duration::from_millis(2));",
            "materialize expired payload under its durable reservation owner",
            "tombstoned_expired: 0",
            "assert_eq!(queue.queued_len(), 0);",
            "must not tombstone the sole payload source",
        ),
    ),
)


def _regular_file(path: Path, label: str, errors: list[str]) -> bool:
    if not path.is_file() or path.is_symlink():
        errors.append(f"{label} must be a regular non-symlink file: {path}")
        return False
    return True


def _extract_braced_item(source: str, declaration: re.Match[str]) -> str | None:
    """Return one brace-balanced Rust item while skipping comments and literals."""

    start = declaration.start()
    index = source.find("{", declaration.end())
    if index < 0:
        return None
    depth = 0
    state = "code"
    block_comment_depth = 0
    raw_hashes = 0
    while index < len(source):
        char = source[index]
        pair = source[index : index + 2]
        if state == "line-comment":
            if char == "\n":
                state = "code"
            index += 1
            continue
        if state == "block-comment":
            if pair == "/*":
                block_comment_depth += 1
                index += 2
                continue
            if pair == "*/":
                block_comment_depth -= 1
                index += 2
                if block_comment_depth == 0:
                    state = "code"
                continue
            index += 1
            continue
        if state == "string":
            if char == "\\":
                index += 2
                continue
            if char == '"':
                state = "code"
            index += 1
            continue
        if state == "char":
            if char == "\\":
                index += 2
                continue
            if char == "'":
                state = "code"
            index += 1
            continue
        if state == "raw-string":
            terminator = '"' + ("#" * raw_hashes)
            if source.startswith(terminator, index):
                index += len(terminator)
                state = "code"
            else:
                index += 1
            continue

        if pair == "//":
            state = "line-comment"
            index += 2
            continue
        if pair == "/*":
            state = "block-comment"
            block_comment_depth = 1
            index += 2
            continue
        if char == "r":
            raw_end = index + 1
            while raw_end < len(source) and source[raw_end] == "#":
                raw_end += 1
            if raw_end < len(source) and source[raw_end] == '"':
                raw_hashes = raw_end - index - 1
                state = "raw-string"
                index = raw_end + 1
                continue
        if char == '"':
            state = "string"
            index += 1
            continue
        if char == "'" and index + 1 < len(source):
            # Rust lifetimes are followed by an identifier; character literals
            # have a closing quote nearby.
            closing = source.find("'", index + 1, min(index + 8, len(source)))
            if closing >= 0:
                state = "char"
                index += 1
                continue
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
        index += 1
    return None


def _nonempty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _validate_closure_mutation_ledger(
    root: Path,
    formal_dir: Path,
    closure_mutations: Any,
    models: Any,
    kura_retention_contract: Any,
    errors: list[str],
) -> None:
    """Bind every conceptual closure mutation to TLC configs or release checks."""

    if not isinstance(closure_mutations, list):
        errors.append("closure_mutations must be an array")
        return
    expected_ids = tuple(EXPECTED_CLOSURE_MUTATIONS)
    actual_ids = tuple(
        item.get("id") for item in closure_mutations if isinstance(item, dict)
    )
    if actual_ids != expected_ids:
        errors.append(
            "closure_mutations must contain the exact reviewed conceptual IDs "
            "in closure-ledger order"
        )

    seen_ids: set[str] = set()
    mapped_configs: list[str] = []
    release_obligations: list[str] = []
    for item in closure_mutations:
        if not isinstance(item, dict) or set(item) != {
            "id",
            "classification",
            "obligation",
            "mutation_configs",
            "source_checks",
        }:
            errors.append(
                "each closure mutation must contain only id, classification, "
                "obligation, mutation_configs, and source_checks"
            )
            continue
        mutation_id = item.get("id")
        classification = item.get("classification")
        obligation = item.get("obligation")
        mutation_configs = item.get("mutation_configs")
        source_checks = item.get("source_checks")
        if (
            not _nonempty_string(mutation_id)
            or not _nonempty_string(classification)
            or not _nonempty_string(obligation)
            or not isinstance(mutation_configs, list)
            or not all(_nonempty_string(config) for config in mutation_configs)
            or not isinstance(source_checks, list)
        ):
            errors.append(f"malformed closure mutation {item!r}")
            continue
        if mutation_id in seen_ids:
            errors.append(f"duplicate conceptual closure mutation {mutation_id}")
        seen_ids.add(mutation_id)

        expected = EXPECTED_CLOSURE_MUTATIONS.get(mutation_id)
        if expected is None:
            errors.append(f"unreviewed conceptual closure mutation {mutation_id}")
            continue
        expected_classification, expected_obligation, expected_configs = expected
        if (
            classification,
            obligation,
            tuple(mutation_configs),
        ) != (
            expected_classification,
            expected_obligation,
            expected_configs,
        ):
            errors.append(
                f"{mutation_id}: classification, obligation, or exact ordered "
                "mutation-config mapping differs from the reviewed contract"
            )

        if len(set(mutation_configs)) != len(mutation_configs):
            errors.append(f"{mutation_id}: duplicate mutation config")
        mapped_configs.extend(mutation_configs)

        if classification == TLA_COUNTEREXAMPLE:
            if not mutation_configs:
                errors.append(
                    f"{mutation_id}: TLA counterexample mappings must be non-empty"
                )
            if source_checks:
                errors.append(
                    f"{mutation_id}: TLA counterexamples must not masquerade as "
                    "static/differential release checks"
                )
            continue
        if classification not in RELEASE_INVARIANT_CLASSIFICATIONS:
            errors.append(
                f"{mutation_id}: unsupported closure classification {classification!r}"
            )
            continue
        release_obligations.append(obligation)
        if mutation_configs:
            errors.append(
                f"{mutation_id}: release invariant must own zero TLA mutation configs"
            )
        expected_paths = EXPECTED_RELEASE_INVARIANT_SOURCE_PATHS.get(mutation_id)
        if expected_paths is None:
            errors.append(f"{mutation_id}: no reviewed release source-check contract")
            continue
        actual_paths = tuple(
            check.get("path") for check in source_checks if isinstance(check, dict)
        )
        if actual_paths != expected_paths:
            errors.append(
                f"{mutation_id}: source checks differ from the exact reviewed paths"
            )
        seen_paths: set[str] = set()
        for check in source_checks:
            if not isinstance(check, dict) or set(check) != {
                "path",
                "required_tokens",
            }:
                errors.append(
                    f"{mutation_id}: every source check must contain only path "
                    "and required_tokens"
                )
                continue
            relative = check.get("path")
            tokens = check.get("required_tokens")
            if (
                not _nonempty_string(relative)
                or not isinstance(tokens, list)
                or not tokens
                or not all(_nonempty_string(token) for token in tokens)
                or len(set(tokens)) != len(tokens)
            ):
                errors.append(f"{mutation_id}: malformed source check {check!r}")
                continue
            if Path(relative).is_absolute() or ".." in Path(relative).parts:
                errors.append(
                    f"{mutation_id}: source-check path must stay within repo: {relative}"
                )
                continue
            if relative in seen_paths:
                errors.append(f"{mutation_id}: duplicate source-check path {relative}")
            seen_paths.add(relative)
            path = root / relative
            if not _regular_file(path, "release invariant source check", errors):
                continue
            source = path.read_text(encoding="utf-8")
            for token in tokens:
                if token not in source:
                    errors.append(
                        f"{path}: release invariant {obligation} is missing "
                        f"source-binding token {token!r}"
                    )

    model_configs: list[str] = []
    if isinstance(models, list):
        for model in models:
            if not isinstance(model, dict):
                continue
            for mutation in model.get("mutations", ()):
                if isinstance(mutation, dict) and _nonempty_string(
                    mutation.get("config")
                ):
                    model_configs.append(mutation["config"])
    if isinstance(kura_retention_contract, dict):
        for mutation in kura_retention_contract.get("mutations", ()):
            if isinstance(mutation, dict) and _nonempty_string(
                mutation.get("config")
            ):
                model_configs.append(mutation["config"])
    if len(mapped_configs) != len(set(mapped_configs)):
        errors.append("one TLA mutation config maps to multiple conceptual IDs")
    if len(model_configs) != len(set(model_configs)):
        errors.append("model inventory contains duplicate TLA mutation configs")
    if set(mapped_configs) != set(model_configs):
        errors.append(
            "conceptual closure mappings must cover every and only the model "
            "mutation configs"
        )
    if len(model_configs) != 73:
        errors.append(
            f"reviewed multilane mutation inventory must contain 73 configs, "
            f"found {len(model_configs)}"
        )

    closure_path = root / CLOSURE_LEDGER_RELATIVE
    if _regular_file(closure_path, "multilane closure ledger", errors):
        closure_source = closure_path.read_text(encoding="utf-8")
        documented_ids = tuple(CLOSURE_MUTATION_ID_RE.findall(closure_source))
        if documented_ids != expected_ids:
            errors.append(
                f"{closure_path}: conceptual ML-MUT IDs must occur exactly once "
                "in machine-ledger order"
            )
        queue_heading = (
            "### ML-QUEUE-01 — globally unique durable admission before "
            "autonomous ownership"
        )
        if closure_source.count(queue_heading) != 1:
            errors.append(
                f"{closure_path}: must contain the exact QueuePlan closure row"
            )
        for mutation_id, (
            classification,
            obligation,
            _,
        ) in EXPECTED_CLOSURE_MUTATIONS.items():
            if classification not in RELEASE_INVARIANT_CLASSIFICATIONS:
                continue
            label = (
                "Static"
                if classification == STATIC_RELEASE
                else "Differential"
            )
            contract_re = re.compile(
                rf"\*\*{label} release invariant and negative control\.\*\*"
                rf"\s+`{re.escape(obligation)}`"
            )
            if contract_re.search(closure_source) is None:
                errors.append(
                    f"{closure_path}: {mutation_id} must classify {obligation} "
                    f"as a {classification} invariant"
                )

    if release_obligations:
        tla_sources: list[tuple[Path, str]] = []
        if isinstance(models, list):
            for model in models:
                if not isinstance(model, dict):
                    continue
                module = model.get("module")
                if _nonempty_string(module):
                    path = formal_dir / f"{module}.tla"
                    if path.is_file() and not path.is_symlink():
                        tla_sources.append(
                            (path, path.read_text(encoding="utf-8"))
                        )
        for obligation in release_obligations:
            for path, source in tla_sources:
                declaration_re = re.compile(
                    TLA_DECLARATION_TEMPLATE.format(
                        symbol=re.escape(obligation)
                    )
                )
                if declaration_re.search(source) is not None:
                    errors.append(
                        f"{path}: release-only invariant {obligation} must not "
                        "be declared as a TLA+ invariant"
                    )


def _extract_rust_binding_items(
    source: str, kind: str, symbol: str
) -> tuple[str, ...]:
    """Extract exact free items or `Type::method` items from Rust source."""

    if kind != "method":
        declaration_re = re.compile(
            RUST_DECLARATION_TEMPLATES[kind].format(symbol=re.escape(symbol))
        )
        return tuple(
            item
            for declaration in declaration_re.finditer(source)
            if (item := _extract_braced_item(source, declaration)) is not None
        )

    if symbol.count("::") != 1:
        return ()
    owner, method = symbol.split("::", 1)
    if not owner or not method:
        return ()
    # Bind both inherent methods (`impl Owner`) and trait methods
    # (`impl Trait for Owner`). The latter is required for capability-bearing
    # implementations such as `CheckedReplayAuthorizationDomain::clone`;
    # treating that method as an unscoped `fn clone` would allow an unrelated
    # implementation in the same file to satisfy the binding.
    impl_re = re.compile(
        rf"(?m)^[ \t]*impl[ \t]+(?:"
        rf"{re.escape(owner)}|[^{{\n]+[ \t]+for[ \t]+{re.escape(owner)}"
        rf")[ \t]*(?=\{{)"
    )
    method_re = re.compile(
        RUST_DECLARATION_TEMPLATES["fn"].format(symbol=re.escape(method))
    )
    items: list[str] = []
    for impl_declaration in impl_re.finditer(source):
        impl_item = _extract_braced_item(source, impl_declaration)
        if impl_item is None:
            continue
        for method_declaration in method_re.finditer(impl_item):
            item = _extract_braced_item(impl_item, method_declaration)
            if item is not None:
                items.append(item)
    return tuple(items)


def _validate_mutation_runner(
    root: Path,
    models: list[Any],
    kura_retention_contract: Any,
    errors: list[str],
) -> None:
    """Require the deterministic TLC runner to cover the exact ledger corpus."""

    runner = root / TLC_MUTATION_RUNNER_RELATIVE
    if not _regular_file(runner, "multilane TLC mutation runner", errors):
        return
    if runner.stat().st_mode & 0o111 == 0:
        errors.append(f"multilane TLC mutation runner must be executable: {runner}")
    source = runner.read_text(encoding="utf-8")
    normalized = source.replace("\\\n", " ")
    compact = " ".join(normalized.split())
    call_re = re.compile(
        r'run_mutant\s+[a-z0-9-]+\s+"?\$[A-Z_]+"?\s+'
        r"((?:multilane|kura_replica)_[a-z0-9_]+_bug\.cfg)\s+"
        r"([A-Za-z0-9_]+)"
    )
    actual = call_re.findall(normalized)
    expected: list[tuple[str, str]] = []
    for model in models:
        if not isinstance(model, dict):
            continue
        mutations = model.get("mutations")
        if not isinstance(mutations, list):
            continue
        for mutation in mutations:
            if not isinstance(mutation, dict):
                continue
            config = mutation.get("config")
            invariant = mutation.get("invariant")
            if _nonempty_string(config) and _nonempty_string(invariant):
                expected.append((config, invariant))
    if isinstance(kura_retention_contract, dict):
        mutations = kura_retention_contract.get("mutations")
        if isinstance(mutations, list):
            for mutation in mutations:
                if not isinstance(mutation, dict):
                    continue
                config = mutation.get("config")
                invariant = mutation.get("invariant")
                if _nonempty_string(config) and _nonempty_string(invariant):
                    expected.append((config, invariant))
    if actual != expected:
        errors.append(
            f"{runner}: exact ordered mutation calls differ from the "
            "multilane source-binding ledger"
        )
    required_once = (
        'source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"',
        'readonly KURA_RETENTION_MODULE="SumeragiV2KuraReplicaRetention.tla"',
        '[[ "$status" -ne 12 ]]',
        'local invariant_marker="Error: Invariant ${invariant} is violated."',
        'sumeragi_v2_tlc_assert_exact_line "$name" "$log" "$invariant_marker"',
        'sumeragi_v2_tlc_assert_exact_line "$name" "$log" '
        '"Error: The behavior up to this point is:"',
        'sumeragi_v2_tlc_assert_terminal "$name" "$log"',
        'grep -Fq "TLC2 Version 2.19"',
        f"[tlc] all {len(expected)} multilane mutations produced their exact "
        "named counterexamples; no deductive proof status was changed",
    )
    for token in required_once:
        count = compact.count(token)
        if count != 1:
            errors.append(
                f"{runner}: mutation runner contract must contain {token!r} "
                f"exactly once, found {count}"
            )


def _apalache_runner_source_errors(source: str) -> list[str]:
    """Validate the exact pinned multilane Apalache runner contract."""

    errors: list[str] = []
    required_once = (
        f'readonly APALACHE_VERSION="{APALACHE_VERSION}"',
        f'readonly APALACHE_LAUNCHER_SHA256="{APALACHE_LAUNCHER_SHA256}"',
        f'readonly APALACHE_JAR_SHA256="{APALACHE_JAR_SHA256}"',
        'readonly CONTRACT_CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_models.py"',
        'readonly RUNNER_CONTRACT_TEST="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_apalache_runner_contract.py"',
        'readonly EVIDENCE_PATH="${EVIDENCE_DIR}/multilane_apalache_evidence.tsv"',
        'readonly KURA_RETENTION_MODULE="SumeragiV2KuraReplicaRetention"',
        '\npython3 -I -S "$CONTRACT_CHECKER"\n',
        'python3 -I -S "$RUNNER_CONTRACT_TEST"',
        'tool_version="$("$RESOLVED_APALACHE_BIN" version)"',
        'run_typecheck "$KURA_RETENTION_MODULE"',
        '[[ "$tool_version" != "$APALACHE_VERSION" ]]',
        '"$RESOLVED_APALACHE_BIN" --out-dir="$out" typecheck "${module}.tla"',
        '"$RESOLVED_APALACHE_BIN" --out-dir="$out" check',
        "--algo=incremental",
        '--config="$config"',
        '--length="$length"',
        "--no-deadlock",
        'grep -Fc "The outcome is: NoError"',
        'grep -Fc "Checker reports no error up to computation length ${length}"',
        'echo "multilane formal or production sources changed during the Apalache run"',
        "printf 'result_count\\t6\\n'",
        "printf 'result\\tkura-replica-retention\\t%s\\t%s\\t8\\tNoError\\t%s\\t%s\\t%s\\n'",
        'mv -- "$evidence_tmp" "$EVIDENCE_PATH"',
        "[apalache] all 5 source-bound refinement kernels plus the layout-only "
        "in-flight carrier passed pinned",
    )
    for token in required_once:
        count = source.count(token)
        if count != 1:
            errors.append(
                f"multilane Apalache runner must contain {token!r} exactly once, "
                f"found {count}"
            )
    manifest_calls = source.count(
        'python3 -I -S "$CONTRACT_CHECKER" --print-source-manifest-sha256'
    )
    if manifest_calls != 2:
        errors.append(
            "multilane Apalache runner must source-seal before and after its "
            f"bounded checks, found {manifest_calls} manifest calls"
        )
    exit_marker_count = source.count('grep -Fxc "EXITCODE: OK"')
    if exit_marker_count != 2:
        errors.append(
            "multilane Apalache runner must require the exact EXITCODE: OK "
            f"marker in typecheck and bounded-check paths, found {exit_marker_count}"
        )

    expected_calls = (
        """run_positive \\
  autoscale-lifecycle \\
  "$AUTOSCALE_MODULE" \\
  multilane_autoscale_lifecycle_fixed.cfg \\
  8 \\
  "LifecycleTypeInvariant, StorageBeforeActivationInvariant, DrainEvidenceInvariant, ArchiveBeforeDestroyInvariant, NoIncarnationReuseInvariant, MLActivationAfterAtomicCreate, MLDrainImpliesNoOwnedWork, MLDrainCertificateMonotonic, MLRetirementConsumesExactIncarnation\"""",
        """run_positive \\
  native-application-evidence \\
  "$NATIVE_MODULE" \\
  multilane_native_application_evidence_fixed.cfg \\
  5 \\
  "NativeEvidenceTypeInvariant, NativeStandaloneEvidenceInvariant, NativeEvidenceRetentionBoundInvariant, NativeNoClobberPublicationInvariant, NativeLegacyDenseRejectedInvariant, NativePruneJournalInvariant, SidecarsRequireManifestInvariant, FrontierPublicationInvariant, PrunedEvidenceVerifiableInvariant, SameRouteControlOnlyInvariant, MLSeparateParticipantApplication, MLNativeSourceClaimInjective, MLNativeContiguousActiveRoute, MLNativeGroupExactCover, MLNativeManifestAuthenticates, MLUnifiedStartupEvidenceRepairSafe, MLNativeDurabilityPrecedesFrontier, MLNativeLatestIndexExact\"""",
        """run_positive \\
  autonomous-reservation-carrier \\
  "$AUTONOMOUS_MODULE" \\
  multilane_autonomous_reservation_carrier_fixed.cfg \\
  10 \\
  "ReservationCarrierTypeInvariant, SingleOwnershipInvariant, ExactCarrierIdentityInvariant, ControlOnlyAnchorInvariant, CandidateAuthorizationInvariant, ReleaseOrderingInvariant, QueueReleaseCompletionInvariant, AtMostOnceApplicationInvariant, NoReleaseAfterApplicationInvariant, NoStaleIncarnationReleaseInvariant, ForgottenOnlyAfterApplicationInvariant, MLReservationSingleOwner, MLReservationIdentityStable, MLCertifiedBundleDurable, MLMergeCandidateExactPrefix, MLCarrierExactlyOnce, MLRestartOwnershipPartition, MLRecoveredCarrierBodyAuthenticated, MLRecoveredCarrierLengthAuthenticated, MLHistoricalRecoveryContextExact, MLHistoricalQueueGateOrder, MLHistoricalAllGroupsPreflight, MLStageEvidenceMonotonic\"""",
        """run_positive \\
  queue-plan-admission-registry \\
  "$QUEUE_PLAN_ADMISSION_MODULE" \\
  multilane_queue_plan_admission_registry_fixed.cfg \\
  8 \\
  "QueuePlanAdmissionTypeInvariant, MLAdmissionCasUnique, MLCertificateDurable, MLPublic202Exact, MLExecutionRequiresExactBinding, MLQueueEligibilityExact, MLAdmissionAtMostOnceExecution, MLImmutableAdmissionTombstone, MLCancellationStopsExecution\"""",
        """run_positive \\
  kura-replica-retention \\
  "$KURA_RETENTION_MODULE" \\
  kura_replica_retention_fixed.cfg \\
  8 \\
  "KuraReplicaRetentionTypeInvariant, KRAdmittedAdvertsSigned, KRAdmittedAdvertsDirectAuthenticated, KRAdmittedAdvertsBindExactFinality, KRAdmittedAdvertsBindExactWire, KRDeterministicFPlusOneKeepers, KRLocalSelectedKeeperPinsBody, KREvictionRequiresAllSelectedRemoteFresh, KRExpiredAdvertsCannotAuthorize, KRRestartClearsAdvertRegistry, KRRegistryCapacityBounded, KRRefreshWindowBounded, KRRefreshCursorExact, KRFinalPreStageRecheck\"""",
        """run_positive \\
  inflight-first-release-layout \\
  "$INFLIGHT_FIRST_RELEASE_MODULE" \\
  inflight_first_release_fixed.cfg \\
  18 \\
  "FirstReleaseTypeInvariant, MLPayloadSchemaV2CarriesExactAdmissionPreimage, MLValidatorCarrierOwnership, MLSelectedQueuePlanV4ConjunctionBeforeReservationV5, MLReservationV5BeforeKuraActive, MLKuraActiveBeforeExecutionInput, MLExecutionInputBeforeReadyAuthorization, MLReadyAuthorizationBeforeLocalSignature, MLLocalSignaturesBeforeDurableReadyQc, MLCrashDurableFactsRecoverable, MLVolatileSessionLostOnCrash, MLCommitAndReleaseRetainExactScope, MLLaneCommitBeforeAtomicWsvCarrierApplication, MLExactlyOnceCarrierApplication, MLPostCarrierCommitCleanupOrder, MLReleasePrefixesRecoverable, MLReleaseStageOrder, MLQueuePlanV4SelectedConjunctionBound4096\"""",
    )
    for call in expected_calls:
        if source.count(call) != 1:
            label = call.splitlines()[1].strip(" \\")
            errors.append(
                f"multilane Apalache runner must contain the exact {label} "
                "bounded positive contract"
            )

    for forbidden in (
        "APALACHE_LENGTH",
        "multilane_autoscale_early_drain_bug.cfg",
        "multilane_autoscale_destroy_before_archive_bug.cfg",
        "multilane_autoscale_incarnation_reuse_bug.cfg",
        "multilane_autoscale_activation_before_storage_bug.cfg",
        "multilane_autoscale_weak_drain_certificate_bug.cfg",
        "multilane_autoscale_cleanup_by_lane_id_bug.cfg",
        "multilane_native_frontier_before_sidecars_bug.cfg",
        "multilane_native_hash_only_pruning_bug.cfg",
        "multilane_native_same_route_marker_bug.cfg",
        "multilane_native_source_claim_equivocation_bug.cfg",
        "multilane_native_noncontiguous_route_bug.cfg",
        "multilane_native_partial_group_application_bug.cfg",
        "multilane_native_forged_manifest_leaf_bug.cfg",
        "multilane_native_dropped_startup_repair_bug.cfg",
        "multilane_native_ambiguous_latest_index_bug.cfg",
        "multilane_native_mutating_unified_startup_plan_bug.cfg",
        "multilane_native_uncoalesced_canonical_body_needs_bug.cfg",
        "multilane_native_partial_unified_startup_preflight_bug.cfg",
        "multilane_native_queue_before_evidence_readback_bug.cfg",
        "multilane_native_missing_reverse_merge_carrier_bug.cfg",
        "multilane_native_orphan_merge_carrier_bug.cfg",
        "multilane_native_skip_post_cache_carrier_reconcile_bug.cfg",
        "multilane_autonomous_carrier_drift_bug.cfg",
        "multilane_autonomous_duplicate_application_bug.cfg",
        "multilane_autonomous_release_after_apply_bug.cfg",
        "multilane_autonomous_release_before_barrier_bug.cfg",
        "multilane_autonomous_aba_release_bug.cfg",
        "multilane_autonomous_digest_only_authorization_bug.cfg",
        "multilane_autonomous_ordinary_anchor_execution_bug.cfg",
        "multilane_autonomous_reserve_before_durable_bug.cfg",
        "multilane_autonomous_noncanonical_merge_prefix_bug.cfg",
        "multilane_autonomous_skip_canonical_reexecution_bug.cfg",
        "multilane_autonomous_restart_drops_ownership_bug.cfg",
        "multilane_autonomous_unauthenticated_recovery_body_bug.cfg",
        "multilane_autonomous_mixed_signer_recovery_body_bug.cfg",
        "multilane_autonomous_historical_context_drift_bug.cfg",
        "multilane_autonomous_open_queue_before_recovery_install_bug.cfg",
        "multilane_autonomous_partial_recovery_group_preflight_bug.cfg",
        "multilane_autonomous_volatile_stage_diagnostics_bug.cfg",
        "multilane_queue_plan_split_route_public_acceptance_bug.cfg",
        "multilane_queue_plan_execution_before_global_cas_bug.cfg",
        "multilane_queue_plan_conflicting_cas_bug.cfg",
        "multilane_queue_plan_restart_aba_bug.cfg",
        "multilane_queue_plan_local_expiry_clears_tombstone_bug.cfg",
        "multilane_queue_plan_deferred_bypass_bug.cfg",
        "multilane_queue_plan_cancellation_bypass_bug.cfg",
        "multilane_queue_plan_guard_drop_deletes_durable_owner_bug.cfg",
        "multilane_queue_plan_execution_without_exact_binding_bug.cfg",
        "multilane_queue_plan_duplicate_execution_bug.cfg",
        "kura_replica_forged_signature_bug.cfg",
        "kura_replica_relayed_advert_bug.cfg",
        "kura_replica_wrong_finality_identity_bug.cfg",
        "kura_replica_wrong_wire_identity_bug.cfg",
        "kura_replica_keeper_cardinality_bug.cfg",
        "kura_replica_nonsigner_keeper_bug.cfg",
        "kura_replica_local_keeper_evict_bug.cfg",
        "kura_replica_partial_remote_freshness_bug.cfg",
        "kura_replica_ttl_expiry_bug.cfg",
        "kura_replica_restart_registry_reuse_bug.cfg",
        "kura_replica_registry_capacity_overflow_bug.cfg",
        "kura_replica_refresh_window_oversize_bug.cfg",
        "kura_replica_refresh_cursor_skip_bug.cfg",
        "kura_replica_skip_final_prestage_recheck_bug.cfg",
        "inflight_first_release_reservation_before_selected_queue_plan_bug.cfg",
        "inflight_first_release_kura_before_reservation_bug.cfg",
        "inflight_first_release_ready_authorization_before_input_bug.cfg",
        "inflight_first_release_ready_signature_before_authorization_bug.cfg",
        "inflight_first_release_ready_qc_before_signatures_bug.cfg",
        "inflight_first_release_crash_drops_durable_bug.cfg",
        "inflight_first_release_crash_retains_volatile_body_bug.cfg",
        "inflight_first_release_payload_conflict_bug.cfg",
        "inflight_first_release_lane_commit_scope_conflict_bug.cfg",
        "inflight_first_release_release_scope_conflict_bug.cfg",
        "inflight_first_release_duplicate_apply_bug.cfg",
        "inflight_first_release_reservation_commit_before_carrier_bug.cfg",
        "inflight_first_release_plan_tombstone_before_reservation_commit_bug.cfg",
        "inflight_first_release_forget_commit_before_plan_tombstone_bug.cfg",
        "inflight_first_release_release_pending_before_retirement_bug.cfg",
        "inflight_first_release_release_prepare_before_pending_bug.cfg",
        "inflight_first_release_released_claims_before_prepare_bug.cfg",
        "inflight_first_release_release_complete_before_released_bug.cfg",
        "inflight_first_release_forget_release_before_fifo_bug.cfg",
        "inflight_first_release_oversize_selected_queue_plan_bug.cfg",
    ):
        if forbidden in source:
            errors.append(
                f"multilane Apalache runner contains prohibited override or "
                f"TLC-owned mutation {forbidden!r}"
            )
    return errors


def _validate_apalache_gate(root: Path, errors: list[str]) -> None:
    runner = root / APALACHE_RUNNER_RELATIVE
    if _regular_file(runner, "multilane Apalache runner", errors):
        if runner.stat().st_mode & 0o111 == 0:
            errors.append(f"multilane Apalache runner must be executable: {runner}")
        errors.extend(_apalache_runner_source_errors(runner.read_text(encoding="utf-8")))

    runner_test = root / APALACHE_RUNNER_TEST_RELATIVE
    _regular_file(runner_test, "multilane Apalache runner contract test", errors)

    installer = root / APALACHE_INSTALLER_RELATIVE
    if _regular_file(installer, "pinned Apalache installer", errors):
        installer_source = installer.read_text(encoding="utf-8")
        installer_tokens = (
            f'readonly pinned_version="{APALACHE_VERSION}"',
            f'readonly pinned_archive_sha256="{APALACHE_ARCHIVE_SHA256}"',
            f'readonly pinned_launcher_sha256="{APALACHE_LAUNCHER_SHA256}"',
            f'readonly pinned_jar_sha256="{APALACHE_JAR_SHA256}"',
            'if [[ "$version" != "$pinned_version" ]]',
            '[[ "$expected_sha256" != "$pinned_archive_sha256" ]]',
            '[[ "$actual_sha256" != "$pinned_archive_sha256" ]]',
        )
        for token in installer_tokens:
            if installer_source.count(token) != 1:
                errors.append(
                    f"{installer}: pinned installer contract must contain "
                    f"{token!r} exactly once"
                )

    tlc_runner = root / TLC_RUNNER_RELATIVE
    if _regular_file(tlc_runner, "Sumeragi v2 TLC runner", errors):
        tlc_source = tlc_runner.read_text(encoding="utf-8")
        for token in (
            'readonly MULTILANE_APALACHE_RUNNER="${REPO_ROOT}/scripts/formal/run_sumeragi_v2_multilane_apalache.sh"',
            'bash "$MULTILANE_APALACHE_RUNNER"',
        ):
            if tlc_source.count(token) != 1:
                errors.append(
                    f"{tlc_runner}: default TLC release matrix must contain "
                    f"{token!r} exactly once"
                )
        allowed_match = re.search(
            r"(?m)^allowed_configs=\(\n(?P<body>(?:  [a-z0-9_]+\n)+)\)$",
            tlc_source,
        )
        expected_allowed_configs = (
            "quorum_count",
            "quorum_stake",
            "safety_count",
            "safety_stake",
            "chain_epoch",
            "liveness",
            "effective_lock_acquisition",
            "resume_locked_commit_witness",
            "multilane_autoscale_lifecycle_fixed",
            "multilane_native_application_evidence_fixed",
            "multilane_autonomous_reservation_carrier_fixed",
            "multilane_queue_plan_admission_registry_fixed",
            "kura_replica_retention_fixed",
        )
        actual_allowed_configs = ()
        if allowed_match is not None:
            actual_allowed_configs = tuple(
                line.strip()
                for line in allowed_match.group("body").splitlines()
            )
        if actual_allowed_configs != expected_allowed_configs:
            errors.append(
                f"{tlc_runner}: default TLC matrix must contain the exact "
                "thirteen reviewed positive/search configurations"
            )
        kura_tlc_dispatch = """      kura_replica_retention_fixed)
        "${common[@]}" SumeragiV2KuraReplicaRetention.tla
        ;;"""
        if tlc_source.count(kura_tlc_dispatch) != 1:
            errors.append(
                f"{tlc_runner}: Kura retention positive config must dispatch "
                "exactly once to SumeragiV2KuraReplicaRetention.tla"
            )
        if tlc_source.count(
            'sumeragi_v2_tlc_assert_fixed_success \\\n'
            '      "bounded-check-${config}" "$tlc_log" "$tlc_status"'
        ) != 1:
            errors.append(
                f"{tlc_runner}: all fixed positive configs must retain the "
                "exact successful TLC transcript contract"
            )

    workflow_install_block = """      - name: Install pinned formal tools
        run: |
          bash scripts/formal/install_sumeragi_v2_tlapm.sh
          bash scripts/formal/install_sumeragi_v2_tla2tools.sh
          bash scripts/formal/install_apalache.sh 0.52.2
          bash scripts/formal/install_sumeragi_v2_verus.sh
"""
    for workflow_relative in FORMAL_WORKFLOW_RELATIVES:
        workflow = root / workflow_relative
        if _regular_file(workflow, "Sumeragi v2 formal workflow", errors):
            workflow_source = workflow.read_text(encoding="utf-8")
            if workflow_source.count(workflow_install_block) != 1:
                errors.append(
                    f"{workflow}: pinned formal install block must contain the "
                    "Apalache 0.52.2 installer exactly once"
                )

    readme = root / README_RELATIVE
    if _regular_file(readme, "Sumeragi v2 formal README", errors):
        readme_source = readme.read_text(encoding="utf-8")
        for token in (
            "`run_sumeragi_v2_multilane_apalache.sh`",
            "pinned Apalache 0.52.2",
            "| autoscale lifecycle | `multilane_autoscale_lifecycle_fixed.cfg` | 8 |",
            "| Native application evidence | `multilane_native_application_evidence_fixed.cfg` | 5 |",
            "| autonomous reservation/carrier | `multilane_autonomous_reservation_carrier_fixed.cfg` | 10 |",
            "| QueuePlan admission registry | `multilane_queue_plan_admission_registry_fixed.cfg` | 8 |",
            "| Kura replica retention | `kura_replica_retention_fixed.cfg` | 8 |",
            "| in-flight carrier (layout-only) | `inflight_first_release_fixed.cfg` | 18 |",
            "not independent ledger rows, TLAPS evidence",
            "cross-tool proof evidence",
            "changes no proof-ledger status",
        ):
            if token not in readme_source:
                errors.append(
                    f"{readme}: missing multilane Apalache documentation contract "
                    f"{token!r}"
                )


def source_manifest_sha256(root: Path = DEFAULT_ROOT) -> str:
    """Hash every source/config/script and production item owned by this gate."""

    root = root.resolve()
    formal_dir = root / FORMAL_RELATIVE
    ledger_path = formal_dir / BINDINGS_FILENAME
    ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
    relative_paths = {
        FORMAL_RELATIVE / BINDINGS_FILENAME,
        PROOF_COVERAGE_RELATIVE,
        CLOSURE_LEDGER_RELATIVE,
        README_RELATIVE,
        APALACHE_RUNNER_RELATIVE,
        APALACHE_RUNNER_TEST_RELATIVE,
        APALACHE_INSTALLER_RELATIVE,
        TLC_RUNNER_RELATIVE,
        TLC_MUTATION_RUNNER_RELATIVE,
        *FORMAL_WORKFLOW_RELATIVES,
        Path("scripts/formal/check_sumeragi_v2_multilane_models.py"),
        Path(
            "scripts/formal/"
            "sumeragi_v2_multilane_kura_retention_contract.py"
        ),
        INFLIGHT_LAYOUT_TEST,
    }
    for closure_mutation in ledger["closure_mutations"]:
        for source_check in closure_mutation["source_checks"]:
            relative_paths.add(Path(source_check["path"]))
    for model in ledger["models"]:
        relative_paths.add(FORMAL_RELATIVE / f"{model['module']}.tla")
        relative_paths.add(FORMAL_RELATIVE / model["positive_config"])
        for mutation in model["mutations"]:
            relative_paths.add(FORMAL_RELATIVE / mutation["config"])
        for binding in model["production_symbols"]:
            relative_paths.add(Path(binding["path"]))
    kura_retention = ledger[KURA_RETENTION_CONTRACT_KEY]
    relative_paths.add(
        FORMAL_RELATIVE / f"{kura_retention['module']}.tla"
    )
    relative_paths.add(FORMAL_RELATIVE / kura_retention["positive_config"])
    for mutation in kura_retention["mutations"]:
        relative_paths.add(FORMAL_RELATIVE / mutation["config"])
    for binding in kura_retention["production_symbols"]:
        relative_paths.add(Path(binding["path"]))
    inflight = ledger["inflight_first_release_layout_contract"]
    relative_paths.add(FORMAL_RELATIVE / f"{inflight['module']}.tla")
    relative_paths.add(FORMAL_RELATIVE / inflight["positive_config"])
    relative_paths.add(Path(inflight["runner"]))
    relative_paths.add(Path(inflight["evidence"]))
    for mutation in inflight["mutations"]:
        relative_paths.add(FORMAL_RELATIVE / mutation["config"])
    for binding in inflight["production_symbols"]:
        relative_paths.add(Path(binding["path"]))
    for check in inflight["ordered_source_checks"]:
        relative_paths.add(Path(check["path"]))
    for check in inflight["source_checks"]:
        relative_paths.add(Path(check["path"]))

    digest = hashlib.sha256()
    for relative in sorted(relative_paths, key=lambda path: path.as_posix()):
        payload = (root / relative).read_bytes()
        encoded_path = relative.as_posix().encode("utf-8")
        digest.update(len(encoded_path).to_bytes(8, "big"))
        digest.update(encoded_path)
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)
    return digest.hexdigest()


def _validate_model(
    root: Path,
    formal_dir: Path,
    model: Any,
    errors: list[str],
    reviewed_invariants: tuple[str, ...] | None = None,
) -> None:
    if not isinstance(model, dict):
        errors.append("each multilane model binding must be an object")
        return
    expected_keys = {
        "module",
        "positive_config",
        "production_refinement_obligation",
        "mutations",
        "production_symbols",
    }
    if set(model) != expected_keys:
        errors.append(
            f"multilane model fields must equal {sorted(expected_keys)}, "
            f"found {sorted(model)}"
        )
        return
    module = model.get("module")
    positive_config = model.get("positive_config")
    obligation = model.get("production_refinement_obligation")
    if not all(
        _nonempty_string(value)
        for value in (module, positive_config, obligation)
    ):
        errors.append("module, positive_config, and obligation must be non-empty")
        return

    module_path = formal_dir / f"{module}.tla"
    module_source: str | None = None
    if _regular_file(module_path, "multilane TLA+ module", errors):
        module_source = module_path.read_text(encoding="utf-8")
        header = MODULE_RE.search(module_source)
        if header is None or header.group(1) != module:
            errors.append(f"{module_path}: module header must declare {module}")
        if not module_source.rstrip().endswith("===="):
            errors.append(f"{module_path}: module must end with ====")
        obligation_re = re.compile(
            TLA_DECLARATION_TEMPLATE.format(symbol=re.escape(obligation))
        )
        if obligation_re.search(module_source) is None:
            errors.append(
                f"{module_path}: missing production refinement obligation {obligation}"
            )

    positive_path = formal_dir / positive_config
    positive_source: str | None = None
    if _regular_file(positive_path, "positive multilane TLC config", errors):
        positive_source = positive_path.read_text(encoding="utf-8")
        if not positive_source.startswith("INIT Init\nNEXT Next\n"):
            errors.append(
                f"{positive_path}: positive config must use the executable Init/Next kernel"
            )
        if "_fixed.cfg" not in positive_config:
            errors.append(
                f"{positive_path}: positive config name must end in _fixed.cfg"
            )

    mutations = model.get("mutations")
    mutation_invariants: list[str] = []
    if not isinstance(mutations, list) or not mutations:
        errors.append(f"{module}: mutations must be a non-empty array")
    else:
        seen_configs: set[str] = set()
        for mutation in mutations:
            if not isinstance(mutation, dict) or set(mutation) != {
                "config",
                "invariant",
            }:
                errors.append(
                    f"{module}: each mutation must contain only config and invariant"
                )
                continue
            config = mutation.get("config")
            invariant = mutation.get("invariant")
            if not _nonempty_string(config) or not _nonempty_string(invariant):
                errors.append(f"{module}: mutation config/invariant must be non-empty")
                continue
            mutation_invariants.append(invariant)
            if config in seen_configs:
                errors.append(f"{module}: duplicate mutation config {config}")
            seen_configs.add(config)
            config_path = formal_dir / config
            if not _regular_file(
                config_path, "multilane mutation TLC config", errors
            ):
                continue
            config_source = config_path.read_text(encoding="utf-8")
            if f'INVARIANT {invariant}\n' not in config_source:
                errors.append(
                    f"{config_path}: mutation must check named invariant {invariant}"
                )
            if "_bug.cfg" not in config:
                errors.append(f"{config_path}: mutation config must end in _bug.cfg")

    expected_invariants = (
        EXPECTED_CLOSURE_INVARIANTS.get(module)
        if reviewed_invariants is None
        else reviewed_invariants
    )
    if expected_invariants is None:
        errors.append(f"{module}: no reviewed multilane closure-invariant contract")
    else:
        for invariant in expected_invariants:
            declaration_re = re.compile(
                TLA_DECLARATION_TEMPLATE.format(symbol=re.escape(invariant))
            )
            if module_source is None or declaration_re.search(module_source) is None:
                errors.append(f"{module_path}: missing closure invariant {invariant}")
            if (
                positive_source is None
                or positive_source.count(f"INVARIANT {invariant}\n") != 1
            ):
                errors.append(
                    f"{positive_path}: closure invariant {invariant} must be "
                    "checked exactly once"
                )
            if invariant not in mutation_invariants:
                errors.append(
                    f"{module}: closure invariant {invariant} has no exact "
                    "named counterexample mutation"
                )

    symbols = model.get("production_symbols")
    if not isinstance(symbols, list) or not symbols:
        errors.append(f"{module}: production_symbols must be a non-empty array")
        return
    seen_bindings: set[tuple[str, str]] = set()
    for binding in symbols:
        if not isinstance(binding, dict) or set(binding) != {
            "path",
            "kind",
            "symbol",
            "required_tokens",
        }:
            errors.append(
                f"{module}: each production binding must contain path, kind, "
                "symbol, and required_tokens"
            )
            continue
        relative = binding.get("path")
        kind = binding.get("kind")
        symbol = binding.get("symbol")
        tokens = binding.get("required_tokens")
        if (
            not _nonempty_string(relative)
            or kind not in RUST_BINDING_KINDS
            or not _nonempty_string(symbol)
            or not isinstance(tokens, list)
            or not tokens
            or not all(_nonempty_string(token) for token in tokens)
        ):
            errors.append(f"{module}: malformed production binding {binding!r}")
            continue
        if Path(relative).is_absolute() or ".." in Path(relative).parts:
            errors.append(f"{module}: production path must stay within repo: {relative}")
            continue
        key = (relative, symbol)
        if key in seen_bindings:
            errors.append(f"{module}: duplicate production binding {relative}!{symbol}")
        seen_bindings.add(key)
        path = root / relative
        if not _regular_file(path, "production binding source", errors):
            continue
        source = path.read_text(encoding="utf-8")
        if kind == "method":
            items = _extract_rust_binding_items(source, kind, symbol)
            if len(items) != 1:
                errors.append(
                    f"{path}: production symbol {symbol} must have one {kind} "
                    f"declaration, found {len(items)}"
                )
                continue
            item = items[0]
        else:
            declaration_re = re.compile(
                RUST_DECLARATION_TEMPLATES[kind].format(
                    symbol=re.escape(symbol)
                )
            )
            declarations = list(declaration_re.finditer(source))
            if len(declarations) != 1:
                errors.append(
                    f"{path}: production symbol {symbol} must have one {kind} "
                    f"declaration, found {len(declarations)}"
                )
                continue
            item = _extract_braced_item(source, declarations[0])
            if item is None:
                errors.append(f"{path}: cannot extract production item {symbol}")
                continue
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{path}: production item {symbol} is missing source-binding "
                    f"token {token!r}"
                )
        for token in FORBIDDEN_PRODUCTION_TOKENS.get((relative, symbol), ()):
            if token in item:
                errors.append(
                    f"{path}: production item {symbol} contains forbidden "
                    f"unbounded token {token!r}"
                )


def _validate_kura_replica_retention_contract(
    root: Path, formal_dir: Path, contract: Any, errors: list[str]
) -> None:
    """Validate the schema-separated fifth Kura retention refinement kernel."""

    if not isinstance(contract, dict):
        errors.append("Kura replica retention contract must be an object")
        return
    model_keys = {
        "module",
        "positive_config",
        "production_refinement_obligation",
        "mutations",
        "production_symbols",
    }
    expected_keys = model_keys | {
        "ordered_source_checks",
        "pending_source_checks",
    }
    if set(contract) != expected_keys:
        errors.append(
            "Kura replica retention contract fields must equal "
            f"{sorted(expected_keys)}, found {sorted(contract)}"
        )
        return

    if contract.get("module") != KURA_RETENTION_MODULE:
        errors.append(
            "Kura replica retention contract must use module "
            f"{KURA_RETENTION_MODULE}"
        )
    if contract.get("positive_config") != KURA_RETENTION_POSITIVE_CONFIG:
        errors.append(
            "Kura replica retention contract must use positive config "
            f"{KURA_RETENTION_POSITIVE_CONFIG}"
        )
    if (
        contract.get("production_refinement_obligation")
        != KURA_RETENTION_REFINEMENT_OBLIGATION
    ):
        errors.append(
            "Kura replica retention contract has the wrong production "
            "refinement obligation"
        )

    model_projection = {key: contract.get(key) for key in model_keys}
    _validate_model(
        root,
        formal_dir,
        model_projection,
        errors,
        reviewed_invariants=KURA_RETENTION_INVARIANTS,
    )
    module_path = formal_dir / f"{KURA_RETENTION_MODULE}.tla"
    if _regular_file(module_path, "Kura replica retention TLA+ module", errors):
        module_source = module_path.read_text(encoding="utf-8")
        for token in (
            "advertSenders[keeper] = keeper",
            "advertVias[keeper] = keeper",
            '![keeper] = IF Mode = "RelayedAdvert" /\\ keeper = FaultyKeeper',
            "THEN NonSignerKeeper",
            "ELSE Cardinality(registryKeys) < RegistryCapacity",
            "Cardinality(registryKeys) <= RegistryCapacity",
        ):
            if token not in module_source:
                errors.append(
                    f"{module_path}: Kura retention model is missing reviewed "
                    f"transport/capacity token {token!r}"
                )
    refresh_source_path = (
        root
        / "crates/iroha_core/src/sumeragi/v2_worker/"
        / "kura_replica_advert_refresh.rs"
    )
    if _regular_file(
        refresh_source_path, "Kura replica advert refresh owner", errors
    ):
        refresh_source = refresh_source_path.read_text(encoding="utf-8")
        refresh_bound = (
            "pub(crate) const KURA_REPLICA_ADVERT_REFRESH_PROBES_PER_TURN: "
            "usize = 8;"
        )
        if refresh_source.count(refresh_bound) != 1:
            errors.append(
                f"{refresh_source_path}: Kura refresh probe bound must equal "
                "the exact reviewed eight-probe contract"
            )
    policy_source_path = root / "crates/iroha_config/src/parameters/actual.rs"
    if _regular_file(
        policy_source_path, "Kura replica advert policy bounds", errors
    ):
        policy_source = policy_source_path.read_text(encoding="utf-8")
        exact_policy_bounds = (
            (
                "pub const KURA_REPLICA_ADVERT_TTL_MIN: Duration = "
                "Duration::from_millis(2);",
                "two-millisecond TTL floor",
            ),
            (
                "pub const KURA_REPLICA_ADVERT_REFRESH_INTERVAL_MIN: Duration = "
                "Duration::from_millis(1);",
                "one-millisecond refresh floor",
            ),
        )
        for declaration, label in exact_policy_bounds:
            if policy_source.count(declaration) != 1:
                errors.append(
                    f"{policy_source_path}: Kura replica advert policy must "
                    f"retain the exact reviewed {label}"
                )

    mutations = contract.get("mutations")
    actual_mutations = ()
    if isinstance(mutations, list):
        actual_mutations = tuple(
            (mutation.get("config"), mutation.get("invariant"))
            for mutation in mutations
            if isinstance(mutation, dict)
        )
    if actual_mutations != KURA_RETENTION_MUTATIONS:
        errors.append(
            "Kura replica retention mutations differ from the reviewed exact "
            "counterexample inventory"
        )

    symbols = contract.get("production_symbols")
    if isinstance(symbols, list):
        expected_binding_keys = {
            (relative, kind, symbol)
            for relative, kind, symbol, _ in KURA_RETENTION_REQUIRED_BINDINGS
        }
        actual_binding_keys = {
            (binding.get("path"), binding.get("kind"), binding.get("symbol"))
            for binding in symbols
            if isinstance(binding, dict)
        }
        if actual_binding_keys != expected_binding_keys or len(symbols) != len(
            KURA_RETENTION_REQUIRED_BINDINGS
        ):
            errors.append(
                "Kura replica retention production bindings differ from the "
                "reviewed exact symbol inventory"
            )
        for relative, kind, symbol, required_tokens in (
            KURA_RETENTION_REQUIRED_BINDINGS
        ):
            matches = [
                binding
                for binding in symbols
                if isinstance(binding, dict)
                and binding.get("path") == relative
                and binding.get("kind") == kind
                and binding.get("symbol") == symbol
            ]
            if len(matches) != 1:
                continue
            tokens = matches[0].get("required_tokens")
            if not isinstance(tokens, list) or tuple(tokens) != required_tokens:
                errors.append(
                    "Kura replica retention source-binding tokens changed for "
                    f"{relative}!{symbol}"
                )

    expected_ordered_checks = [
        {
            "path": "crates/iroha_core/src/kura.rs",
            "kind": "method",
            "symbol": "Kura::evict_block_bodies_unlocked",
            "required_tokens": list(KURA_RETENTION_PRESTAGE_ORDERED_TOKENS),
        },
        {
            "path": "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "kind": "method",
            "symbol": "ProductionV2Services::handoff_applied_height_output_to_durable_reconstruction",
            "required_tokens": list(KURA_RETENTION_HANDOFF_ORDERED_TOKENS),
        },
        {
            "path": "crates/iroha_core/src/sumeragi/v2_worker/kura_replica_advert_refresh.rs",
            "kind": "method",
            "symbol": "KuraReplicaAdvertRefreshOwner::drive_turn",
            "required_tokens": list(KURA_RETENTION_REFRESH_START_ORDERED_TOKENS),
        },
    ]
    ordered_checks = contract.get("ordered_source_checks")
    if ordered_checks != expected_ordered_checks:
        errors.append(
            "Kura replica retention contract must contain the exact final "
            "pre-stage eviction recheck, durable-handoff scheduling, and "
            "refresh scan-start deadline orders"
        )
    else:
        ordered_contracts = (
            (
                expected_ordered_checks[0],
                KURA_RETENTION_PRESTAGE_ORDERED_TOKENS,
                "final pre-stage recheck",
            ),
            (
                expected_ordered_checks[1],
                KURA_RETENTION_HANDOFF_ORDERED_TOKENS,
                "durable-handoff scheduling",
            ),
            (
                expected_ordered_checks[2],
                KURA_RETENTION_REFRESH_START_ORDERED_TOKENS,
                "refresh scan-start deadline",
            ),
        )
        for expected_ordered_check, required_tokens, label in ordered_contracts:
            item = _rust_binding_item(
                root,
                expected_ordered_check["path"],
                expected_ordered_check["kind"],
                expected_ordered_check["symbol"],
                f"Kura {label} source binding",
                errors,
            )
            if item is None:
                continue
            cursor = -1
            for token in required_tokens:
                count = item.count(token)
                position = item.find(token, cursor + 1)
                if count != 1 or position < 0:
                    errors.append(
                        f"{root / expected_ordered_check['path']}: Kura {label} "
                        "token must occur exactly once and in order: "
                        f"{token!r}; found {count}"
                    )
                    break
                cursor = position

    if contract.get("pending_source_checks") != []:
        errors.append(
            "Kura replica retention contract must have no pending source checks"
        )


def _rust_binding_item(
    root: Path,
    relative: str,
    kind: str,
    symbol: str,
    label: str,
    errors: list[str],
) -> str | None:
    """Load one exact Rust item owned by a source-binding contract."""

    path = root / relative
    if not _regular_file(path, label, errors):
        return None
    source = path.read_text(encoding="utf-8")
    if kind == "method":
        items = _extract_rust_binding_items(source, kind, symbol)
    else:
        declaration_re = re.compile(
            RUST_DECLARATION_TEMPLATES[kind].format(symbol=re.escape(symbol))
        )
        items = tuple(
            item
            for declaration in declaration_re.finditer(source)
            if (item := _extract_braced_item(source, declaration)) is not None
        )
    if len(items) != 1:
        errors.append(
            f"{path}: source-bound symbol {symbol} must have one {kind} "
            f"declaration, found {len(items)}"
        )
        return None
    return items[0]


def _validate_native_participant_application_classifier_contract(
    root: Path, models: Any, errors: list[str]
) -> None:
    """Bind participant grouping and diagnostics to the shared role classifier."""

    if not isinstance(models, list):
        return
    native_models = [
        model
        for model in models
        if isinstance(model, dict)
        and model.get("module") == NATIVE_PREPUBLICATION_MODULE
    ]
    if len(native_models) != 1:
        errors.append(
            "Native participant classifier source contract requires exactly one "
            f"{NATIVE_PREPUBLICATION_MODULE} model"
        )
        return
    production_symbols = native_models[0].get("production_symbols")
    if not isinstance(production_symbols, list):
        return

    binding_items: dict[tuple[str, str, str], str] = {}
    for relative, kind, symbol, expected_tokens in (
        NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_BINDINGS
    ):
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
                f"{NATIVE_PREPUBLICATION_MODULE}: reviewed shared participant "
                f"classifier consumer binding {relative}!{symbol} must occur "
                f"exactly once, found {len(matches)}"
            )
            continue
        actual_tokens = matches[0].get("required_tokens")
        if (
            not isinstance(actual_tokens, list)
            or tuple(actual_tokens) != expected_tokens
        ):
            errors.append(
                f"{NATIVE_PREPUBLICATION_MODULE}: reviewed shared participant "
                f"classifier consumer tokens changed for {relative}!{symbol}"
            )

        item = _rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "Native participant classifier consumer binding",
            errors,
        )
        if item is None:
            continue
        binding_items[(relative, kind, symbol)] = item
        for token in expected_tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: shared participant classifier consumer "
                    f"{symbol} is missing source-bound token {token!r}"
                )

    classifier_match_re = re.compile(
        r"match\s+crate::native_amx::"
        r"native_amx_participant_application_role\s*"
        r"\(\s*receipt\s*,\s*leg\s*\)"
    )
    role_tokens = (
        "NativeAmxParticipantApplicationRole::Coordinator",
        "NativeAmxParticipantApplicationRole::SeparateParticipant",
    )
    for relative, kind, symbol, expected_match in (
        NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_MATCH_RELATIONS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            continue
        matches = list(classifier_match_re.finditer(item))
        if len(matches) != 1:
            errors.append(
                f"{root / relative}: shared participant classifier consumer "
                f"{symbol} must contain exactly one classifier match, found "
                f"{len(matches)}"
            )
            continue
        match_item = _extract_braced_item(item, matches[0])
        if (
            match_item is None
            or " ".join(match_item.split()) != expected_match
        ):
            errors.append(
                f"{root / relative}: shared participant classifier match "
                f"relation drifted in {symbol}"
            )
            continue
        for role_token in role_tokens:
            if item.count(role_token) != 1 or match_item.count(role_token) != 1:
                errors.append(
                    f"{root / relative}: shared participant classifier role "
                    f"{role_token!r} must occur exactly once inside the match "
                    f"in {symbol}"
                )

    for relative, kind, symbol, ordered_tokens in (
        NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_ORDERED_SOURCE_CHECKS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            continue
        cursor = -1
        for token in ordered_tokens:
            count = item.count(token)
            position = item.find(token, cursor + 1)
            if count != 1 or position < 0:
                errors.append(
                    f"{root / relative}: shared participant classifier consumer "
                    f"{symbol} must contain exactly one ordered downstream token "
                    f"{token!r}, found {count}"
                )
                break
            cursor = position


def _validate_native_prepublication_contract(
    root: Path, models: Any, errors: list[str]
) -> None:
    """Bind Native participant frontier publication to its durable phase order."""

    if not isinstance(models, list):
        return
    native_models = [
        model
        for model in models
        if isinstance(model, dict)
        and model.get("module") == NATIVE_PREPUBLICATION_MODULE
    ]
    if len(native_models) != 1:
        errors.append(
            "Native prepublication source contract requires exactly one "
            f"{NATIVE_PREPUBLICATION_MODULE} model"
        )
        return
    production_symbols = native_models[0].get("production_symbols")
    if not isinstance(production_symbols, list):
        return

    for relative, kind, symbol, expected_tokens in NATIVE_PREPUBLICATION_BINDINGS:
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
                f"{NATIVE_PREPUBLICATION_MODULE}: reviewed prepublication "
                f"binding {relative}!{symbol} must occur exactly once, "
                f"found {len(matches)}"
            )
            continue
        actual_tokens = matches[0].get("required_tokens")
        if (
            not isinstance(actual_tokens, list)
            or tuple(actual_tokens) != expected_tokens
        ):
            errors.append(
                f"{NATIVE_PREPUBLICATION_MODULE}: reviewed prepublication "
                f"tokens changed for {relative}!{symbol}"
            )

    binding_items: dict[tuple[str, str, str], str] = {}
    for relative, kind, symbol, tokens in NATIVE_PREPUBLICATION_BINDINGS:
        item = _rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "Native prepublication production binding",
            errors,
        )
        if item is None:
            continue
        binding_items[(relative, kind, symbol)] = item
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: Native prepublication item {symbol} "
                    f"is missing source-bound token {token!r}"
                )

    for relative, kind, symbol, tokens in (
        NATIVE_PREPUBLICATION_ORDERED_SOURCE_CHECKS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = _rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "ordered Native prepublication source binding",
                errors,
            )
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            count = item.count(token)
            position = item.find(token, cursor + 1)
            if count != 1 or position < 0:
                errors.append(
                    f"{root / relative}: ordered Native prepublication item "
                    f"{symbol} must contain exactly one ordered token "
                    f"{token!r}, found {count}"
                )
                break
            cursor = position

    kura_relative = "crates/iroha_core/src/kura.rs"
    persist_key = (
        kura_relative,
        "fn",
        "persist_native_amx_participant_application_evidence_under_publication_guard",
    )
    persist_item = binding_items.get(persist_key)
    if persist_item is not None:
        normalized = " ".join(persist_item.split())
        phase_snippets = (
            "for (manifest, _) in &plan.artifacts { "
            "self.write_native_amx_participant_application_manifest_artifact_"
            "with_retention_policy_under_publication_guard( "
            "manifest, permit_cleanup, )?; }",
            "for (manifest, receipt) in &plan.artifacts { "
            "self.write_native_amx_participant_application_receipt_artifact_"
            "only_with_retention_policy_under_publication_guard( "
            "receipt, manifest, permit_cleanup, )?; }",
            "for ((manifest, receipt), preflight) in "
            "plan.artifacts.iter().zip(route_preflights.iter()) { "
            "self.write_native_amx_participant_receipt_latest_index_"
            "for_prepublication_under_publication_guard( "
            "receipt, manifest, permit_cleanup, preflight, )?; }",
            "if permit_cleanup { for (_, receipt) in &plan.artifacts { "
            "self.cleanup_native_amx_participant_application_evidence_"
            "under_publication_guard( receipt, )?; } }",
        )
        for snippet in phase_snippets:
            if snippet not in normalized:
                errors.append(
                    f"{root / kura_relative}: Native prepublication phase "
                    "loops must remain manifest-all, receipt-all, latest-all, "
                    "read-back-authenticated, then cleanup-only-after-WSV"
                )
                break

    expected_mode_methods = {
        "NativeAmxParticipantApplicationPublicationMode::requires_post_apply_metadata": (
            "const fn requires_post_apply_metadata(self) -> bool { "
            "matches!(self, Self::PostWsvRepair) }"
        ),
        "NativeAmxParticipantApplicationPublicationMode::permits_retention_cleanup": (
            "const fn permits_retention_cleanup(self) -> bool { "
            "matches!(self, Self::PostWsvRepair) }"
        ),
    }
    for symbol, expected in expected_mode_methods.items():
        item = binding_items.get((kura_relative, "method", symbol))
        if item is not None and " ".join(item.split()) != expected:
            errors.append(
                f"{root / kura_relative}: {symbol} must authorize only "
                "PostWsvRepair"
            )

    retention_guard = (
        "if !permit_retention_cleanup { "
        "self.require_native_amx_evidence_prune_intent_absent_locked(&namespace)?; "
        "}"
    )
    for symbol in NATIVE_PREPUBLICATION_RETENTION_WRITERS:
        item = binding_items.get((kura_relative, "fn", symbol))
        if item is None:
            continue
        normalized = " ".join(item.split())
        if normalized.count(retention_guard) != 1:
            errors.append(
                f"{root / kura_relative}: Native prepublication writer {symbol} "
                "must fail closed on retention state before PostWsvRepair"
            )
        for forbidden in (
            "cleanup_native_amx_participant_application_evidence_under_publication_guard(",
            "prune_native_amx_evidence_pairs_locked(",
        ):
            if forbidden in item:
                errors.append(
                    f"{root / kura_relative}: Native prepublication writer "
                    f"{symbol} contains forbidden pre-WSV cleanup {forbidden!r}"
                )

    prepublish_item = binding_items.get(
        (
            kura_relative,
            "fn",
            "prepublish_native_amx_participant_application_evidence",
        )
    )
    if prepublish_item is not None:
        for forbidden in (
            "NativeAmxParticipantApplicationPublicationMode::PostWsvRepair",
            "cleanup_native_amx_participant_application_evidence_under_publication_guard(",
            "prune_native_amx_evidence_pairs_locked(",
        ):
            if forbidden in prepublish_item:
                errors.append(
                    f"{root / kura_relative}: pre-WSV Native publication "
                    f"contains forbidden cleanup/repair token {forbidden!r}"
                )

    repair_item = binding_items.get(
        (
            kura_relative,
            "fn",
            "repair_native_amx_participant_application_evidence",
        )
    )
    if (
        repair_item is not None
        and "NativeAmxParticipantApplicationPublicationMode::PreWsv"
        in repair_item
    ):
        errors.append(
            f"{root / kura_relative}: post-WSV Native repair must not use "
            "PreWsv publication mode"
        )


def _validate_queue_plan_startup_replay_contract(
    root: Path, models: Any, errors: list[str]
) -> None:
    """Bind QueuePlan startup replay to one atomic durable publication seam."""

    if not isinstance(models, list):
        return
    queue_models = [
        model
        for model in models
        if isinstance(model, dict)
        and model.get("module") == QUEUE_PLAN_STARTUP_REPLAY_MODULE
    ]
    if len(queue_models) != 1:
        errors.append(
            "QueuePlan startup replay source contract requires exactly one "
            f"{QUEUE_PLAN_STARTUP_REPLAY_MODULE} model"
        )
        return
    production_symbols = queue_models[0].get("production_symbols")
    if not isinstance(production_symbols, list):
        return

    for relative, kind, symbol, expected_tokens in (
        QUEUE_PLAN_STARTUP_REPLAY_BINDINGS
    ):
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
                f"{QUEUE_PLAN_STARTUP_REPLAY_MODULE}: reviewed startup replay "
                f"binding {relative}!{symbol} must occur exactly once, "
                f"found {len(matches)}"
            )
            continue
        actual_tokens = matches[0].get("required_tokens")
        if not isinstance(actual_tokens, list) or tuple(actual_tokens) != expected_tokens:
            errors.append(
                f"{QUEUE_PLAN_STARTUP_REPLAY_MODULE}: reviewed startup replay "
                f"tokens changed for {relative}!{symbol}"
            )

    binding_items: dict[tuple[str, str, str], str] = {}
    for relative, kind, symbol, tokens in QUEUE_PLAN_STARTUP_REPLAY_BINDINGS:
        item = _rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "QueuePlan startup replay production binding",
            errors,
        )
        if item is None:
            continue
        binding_items[(relative, kind, symbol)] = item
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: QueuePlan startup replay item {symbol} "
                    f"is missing source-bound token {token!r}"
                )

    for relative, kind, symbol, tokens in (
        QUEUE_PLAN_STARTUP_REPLAY_ORDERED_SOURCE_CHECKS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = _rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "ordered QueuePlan startup replay source binding",
                errors,
            )
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: ordered QueuePlan startup replay item "
                    f"{symbol} is missing or reorders token {token!r}"
                )
                break
            cursor = position

    for relative, kind, symbol, tokens in (
        QUEUE_PLAN_STARTUP_REPLAY_FORBIDDEN_SOURCE_CHECKS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = _rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "infallible QueuePlan startup replay source binding",
                errors,
            )
        if item is None:
            continue
        for token in tokens:
            if token in item:
                errors.append(
                    f"{root / relative}: QueuePlan startup replay item {symbol} "
                    f"contains forbidden fallible/panicking token {token!r}"
                )

    replay_key = (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::replay_plan_journal",
    )
    replay_item = binding_items.get(replay_key)
    if replay_item is not None:
        marker_offset = replay_item.find(QUEUE_PLAN_STARTUP_REPLAY_POST_APPLY_MARKER)
        if marker_offset < 0:
            errors.append(
                f"{root / replay_key[0]}: QueuePlan replay is missing its exact "
                "atomic in-memory apply boundary"
            )
        else:
            post_apply = replay_item[
                marker_offset + len(QUEUE_PLAN_STARTUP_REPLAY_POST_APPLY_MARKER) :
            ]
            for token in QUEUE_PLAN_STARTUP_REPLAY_POST_APPLY_FORBIDDEN_TOKENS:
                if token in post_apply:
                    errors.append(
                        f"{root / replay_key[0]}: QueuePlan replay contains "
                        f"fallible/panicking token {token!r} after atomic apply"
                    )

    for relative, symbol, tokens in QUEUE_PLAN_STARTUP_REPLAY_TEST_BINDINGS:
        item = _rust_binding_item(
            root,
            relative,
            "fn",
            symbol,
            "QueuePlan startup replay static negative-control test",
            errors,
        )
        if item is None:
            continue
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: QueuePlan startup replay test {symbol} "
                    f"is missing negative-control token {token!r}"
                )


def _validate_inflight_layout_contract(
    root: Path,
    formal_dir: Path,
    contract: Any,
    errors: list[str],
) -> None:
    """Bind the in-flight corpus and composed relation without trace-theorem inflation."""

    expected_keys = {
        "claim",
        "module",
        "positive_config",
        "runner",
        "evidence",
        "required_actions",
        "required_invariants",
        "mutations",
        "production_symbols",
        "ordered_source_checks",
        "forbidden_source_checks",
        "source_checks",
        "forbidden_tokens",
    }
    if not isinstance(contract, dict) or set(contract) != expected_keys:
        errors.append(
            "in-flight layout contract must contain exactly claim, module, "
            "positive_config, runner, evidence, required_actions, "
            "required_invariants, mutations, production_symbols, "
            "ordered_source_checks, forbidden_source_checks, source_checks, "
            "and forbidden_tokens"
        )
        return
    expected_scalars = {
        "claim": INFLIGHT_LAYOUT_CLAIM,
        "module": INFLIGHT_LAYOUT_MODULE,
        "positive_config": INFLIGHT_LAYOUT_POSITIVE_CONFIG,
        "runner": INFLIGHT_LAYOUT_RUNNER.as_posix(),
        "evidence": INFLIGHT_LAYOUT_EVIDENCE.as_posix(),
    }
    for field, expected in expected_scalars.items():
        if contract.get(field) != expected:
            errors.append(
                f"in-flight layout contract {field} must equal {expected!r}"
            )

    required_actions = contract.get("required_actions")
    if (
        not isinstance(required_actions, list)
        or tuple(required_actions) != INFLIGHT_LAYOUT_REQUIRED_ACTIONS
    ):
        errors.append(
            "in-flight layout contract actions differ from the exact reviewed "
            "current-semantics inventory"
        )

    required_invariants = contract.get("required_invariants")
    if (
        not isinstance(required_invariants, list)
        or tuple(required_invariants) != INFLIGHT_LAYOUT_REQUIRED_INVARIANTS
    ):
        errors.append(
            "in-flight layout contract invariants differ from the exact reviewed "
            "current-layout inventory"
        )

    mutations = contract.get("mutations")
    actual_mutations: list[tuple[str, str]] = []
    if not isinstance(mutations, list):
        errors.append("in-flight layout mutations must be an array")
    else:
        for mutation in mutations:
            if not isinstance(mutation, dict) or set(mutation) != {
                "config",
                "invariant",
            }:
                errors.append(
                    "each in-flight layout mutation must contain only config "
                    "and invariant"
                )
                continue
            config = mutation.get("config")
            invariant = mutation.get("invariant")
            if not _nonempty_string(config) or not _nonempty_string(invariant):
                errors.append(f"malformed in-flight layout mutation {mutation!r}")
                continue
            actual_mutations.append((config, invariant))
    if tuple(actual_mutations) != INFLIGHT_LAYOUT_MUTATIONS:
        errors.append(
            "in-flight layout mutation mapping differs from the exact reviewed "
            "twenty-two-control corpus"
        )

    production_symbols = contract.get("production_symbols")
    actual_bindings: list[tuple[str, str, str, tuple[str, ...]]] = []
    if not isinstance(production_symbols, list):
        errors.append("in-flight production_symbols must be an array")
    else:
        for binding in production_symbols:
            if not isinstance(binding, dict) or set(binding) != {
                "path",
                "kind",
                "symbol",
                "required_tokens",
            }:
                errors.append(
                    "each in-flight production binding must contain only path, "
                    "kind, symbol, and required_tokens"
                )
                continue
            relative = binding.get("path")
            kind = binding.get("kind")
            symbol = binding.get("symbol")
            tokens = binding.get("required_tokens")
            if (
                not _nonempty_string(relative)
                or kind not in RUST_BINDING_KINDS
                or not _nonempty_string(symbol)
                or not isinstance(tokens, list)
                or not tokens
                or not all(_nonempty_string(token) for token in tokens)
                or len(tokens) != len(set(tokens))
            ):
                errors.append(f"malformed in-flight production binding {binding!r}")
                continue
            actual_bindings.append((relative, kind, symbol, tuple(tokens)))
    if tuple(actual_bindings) != INFLIGHT_LAYOUT_PRODUCTION_BINDINGS:
        errors.append(
            "in-flight production bindings differ from the exact reviewed "
            "payload/queue/Kura/replay-state layout contract"
        )

    ordered_source_checks = contract.get("ordered_source_checks")
    actual_ordered: list[tuple[str, str, str, tuple[str, ...]]] = []
    if not isinstance(ordered_source_checks, list):
        errors.append("in-flight ordered_source_checks must be an array")
    else:
        for check in ordered_source_checks:
            if not isinstance(check, dict) or set(check) != {
                "path",
                "kind",
                "symbol",
                "tokens",
            }:
                errors.append(
                    "each ordered in-flight source check must contain only path, "
                    "kind, symbol, and tokens"
                )
                continue
            relative = check.get("path")
            kind = check.get("kind")
            symbol = check.get("symbol")
            tokens = check.get("tokens")
            if (
                not _nonempty_string(relative)
                or kind not in RUST_BINDING_KINDS
                or not _nonempty_string(symbol)
                or not isinstance(tokens, list)
                or not tokens
                or not all(_nonempty_string(token) for token in tokens)
                or len(tokens) != len(set(tokens))
            ):
                errors.append(f"malformed ordered in-flight source check {check!r}")
                continue
            actual_ordered.append((relative, kind, symbol, tuple(tokens)))
    if tuple(actual_ordered) != INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS:
        errors.append(
            "in-flight ordered source checks differ from the exact reviewed "
            "validation/durability/publication order contract"
        )

    forbidden_source_checks = contract.get("forbidden_source_checks")
    actual_forbidden_source_checks: list[
        tuple[str, str, str, tuple[str, ...]]
    ] = []
    if not isinstance(forbidden_source_checks, list):
        errors.append("in-flight forbidden_source_checks must be an array")
    else:
        for check in forbidden_source_checks:
            if not isinstance(check, dict) or set(check) != {
                "path",
                "kind",
                "symbol",
                "forbidden_tokens",
            }:
                errors.append(
                    "each forbidden in-flight source check must contain only "
                    "path, kind, symbol, and forbidden_tokens"
                )
                continue
            relative = check.get("path")
            kind = check.get("kind")
            symbol = check.get("symbol")
            tokens = check.get("forbidden_tokens")
            if (
                not _nonempty_string(relative)
                or kind not in RUST_BINDING_KINDS
                or not _nonempty_string(symbol)
                or not isinstance(tokens, list)
                or not tokens
                or not all(_nonempty_string(token) for token in tokens)
                or len(tokens) != len(set(tokens))
            ):
                errors.append(f"malformed forbidden in-flight source check {check!r}")
                continue
            actual_forbidden_source_checks.append(
                (relative, kind, symbol, tuple(tokens))
            )
    if (
        tuple(actual_forbidden_source_checks)
        != INFLIGHT_LAYOUT_FORBIDDEN_SOURCE_CHECKS
    ):
        errors.append(
            "in-flight forbidden source checks differ from the exact reviewed "
            "bounded-application/capability contract"
        )

    source_checks = contract.get("source_checks")
    actual_source_checks: list[tuple[str, tuple[str, ...]]] = []
    if not isinstance(source_checks, list):
        errors.append("in-flight source_checks must be an array")
    else:
        for check in source_checks:
            if not isinstance(check, dict) or set(check) != {
                "path",
                "required_tokens",
            }:
                errors.append(
                    "each in-flight source check must contain only path and "
                    "required_tokens"
                )
                continue
            relative = check.get("path")
            tokens = check.get("required_tokens")
            if (
                not _nonempty_string(relative)
                or not isinstance(tokens, list)
                or not tokens
                or not all(_nonempty_string(token) for token in tokens)
                or len(tokens) != len(set(tokens))
            ):
                errors.append(f"malformed in-flight source check {check!r}")
                continue
            actual_source_checks.append((relative, tuple(tokens)))
    if tuple(actual_source_checks) != INFLIGHT_LAYOUT_SOURCE_CHECKS:
        errors.append(
            "in-flight whole-file source checks differ from the exact reviewed "
            "version/bound/runner contract"
        )

    forbidden_tokens = contract.get("forbidden_tokens")
    if (
        not isinstance(forbidden_tokens, list)
        or tuple(forbidden_tokens) != INFLIGHT_LAYOUT_FORBIDDEN_TOKENS
    ):
        errors.append(
            "in-flight forbidden tokens differ from the exact reviewed stale-layout "
            "inventory"
        )

    module_path = formal_dir / f"{INFLIGHT_LAYOUT_MODULE}.tla"
    module_source: str | None = None
    if _regular_file(module_path, "in-flight first-release TLA+ module", errors):
        module_source = module_path.read_text(encoding="utf-8")
        header = MODULE_RE.search(module_source)
        if header is None or header.group(1) != INFLIGHT_LAYOUT_MODULE:
            errors.append(
                f"{module_path}: module header must declare {INFLIGHT_LAYOUT_MODULE}"
            )
        if not module_source.rstrip().endswith("===="):
            errors.append(f"{module_path}: module must end with ====")
        if "ProductionRefinementObligation" in module_source:
            errors.append(
                f"{module_path}: bounded kernel without production trace extraction "
                "must not declare a production refinement obligation"
            )
        for action in INFLIGHT_LAYOUT_REQUIRED_ACTIONS:
            declaration_re = re.compile(
                TLA_DECLARATION_TEMPLATE.format(symbol=re.escape(action))
            )
            count = len(tuple(declaration_re.finditer(module_source)))
            if count != 1:
                errors.append(
                    f"{module_path}: current-semantics action {action} must be "
                    f"declared exactly once, found {count}"
                )
        for invariant in INFLIGHT_LAYOUT_REQUIRED_INVARIANTS:
            declaration_re = re.compile(
                TLA_DECLARATION_TEMPLATE.format(symbol=re.escape(invariant))
            )
            count = len(tuple(declaration_re.finditer(module_source)))
            if count != 1:
                errors.append(
                    f"{module_path}: current-layout invariant {invariant} must be "
                    f"declared exactly once, found {count}"
                )
        for token in INFLIGHT_COMPOSED_TLA_ALIGNMENT_TOKENS:
            count = module_source.count(token)
            if count != 1:
                errors.append(
                    f"{module_path}: composed Rust/TLA action-alignment token "
                    f"{token!r} must occur exactly once, found {count}"
                )

    positive_path = formal_dir / INFLIGHT_LAYOUT_POSITIVE_CONFIG
    positive_source: str | None = None
    if _regular_file(
        positive_path, "in-flight first-release positive TLC config", errors
    ):
        positive_source = positive_path.read_text(encoding="utf-8")
        if not positive_source.startswith("INIT Init\nNEXT Next\n"):
            errors.append(
                f"{positive_path}: positive config must use executable Init/Next"
            )
        positive_invariants = tuple(
            re.findall(r"(?m)^INVARIANT ([A-Za-z0-9_]+)$", positive_source)
        )
        if positive_invariants != INFLIGHT_LAYOUT_REQUIRED_INVARIANTS:
            errors.append(
                f"{positive_path}: invariant list differs from the exact reviewed "
                "current-layout contract"
            )

    mutation_sources: list[tuple[Path, str]] = []
    for config, invariant in INFLIGHT_LAYOUT_MUTATIONS:
        config_path = formal_dir / config
        if not _regular_file(
            config_path, "in-flight first-release mutation TLC config", errors
        ):
            continue
        config_source = config_path.read_text(encoding="utf-8")
        mutation_sources.append((config_path, config_source))
        config_invariants = tuple(
            re.findall(r"(?m)^INVARIANT ([A-Za-z0-9_]+)$", config_source)
        )
        if config_invariants != ("FirstReleaseTypeInvariant", invariant):
            errors.append(
                f"{config_path}: mutation must check exactly the type invariant "
                f"and {invariant}"
            )

    runner_path = root / INFLIGHT_LAYOUT_RUNNER
    runner_source: str | None = None
    if _regular_file(
        runner_path, "in-flight first-release TLC mutation runner", errors
    ):
        if runner_path.stat().st_mode & 0o111 == 0:
            errors.append(
                f"in-flight first-release runner must be executable: {runner_path}"
            )
        runner_source = runner_path.read_text(encoding="utf-8")
        runner_calls = tuple(
            re.findall(
                r"(?m)^run_mutant ([a-z0-9_]+_bug\.cfg) ([A-Za-z0-9_]+)$",
                runner_source,
            )
        )
        if runner_calls != INFLIGHT_LAYOUT_MUTATIONS:
            errors.append(
                f"{runner_path}: mutation calls differ from the exact reviewed "
                "twenty-two-control corpus"
            )
        compact_runner_source = " ".join(
            runner_source.replace("\\\n", " ").split()
        )
        for token in (
            'source "${REPO_ROOT}/scripts/formal/sumeragi_v2_tlc_result_contract.sh"',
            '[[ "$status" -ne 12 ]]',
            'local invariant_marker="Error: Invariant ${invariant} is violated."',
            'sumeragi_v2_tlc_assert_exact_line "$config" "$log" '
            '"$invariant_marker"',
            'sumeragi_v2_tlc_assert_exact_line "$config" "$log" '
            '"Error: The behavior up to this point is:"',
            'sumeragi_v2_tlc_assert_terminal "$config" "$log"',
            "bounded abstract evidence only; no production refinement claim",
        ):
            count = compact_runner_source.count(token)
            if count != 1:
                errors.append(
                    f"{runner_path}: fail-closed runner token {token!r} must occur "
                    f"exactly once, found {count}"
                )
        if runner_source.count("\nrun_positive\n") != 1:
            errors.append(
                f"{runner_path}: fail-closed runner must invoke the positive "
                "model exactly once"
            )

    evidence_path = root / INFLIGHT_LAYOUT_EVIDENCE
    evidence_source: str | None = None
    if _regular_file(
        evidence_path, "in-flight first-release evidence boundary", errors
    ):
        evidence_source = evidence_path.read_text(encoding="utf-8")
        for token in (
            "`LaneExecutablePayloadV1`",
            "`LANE_EXECUTABLE_PAYLOAD_VERSION_V2`",
            "QueuePlan journal V4",
            "reservation journal V5",
            "selected-batch conjunction",
            "READY signature",
            "atomic WSV carrier application",
            "four-stage release",
            "twenty-two `_bug.cfg`",
            "`composed_state_action_relation_no_trace_extraction`",
            "fixed-width composed state/action relation",
            "production trace-extraction theorem",
            "reverse terminal-owner projection",
        ):
            if token not in evidence_source:
                errors.append(
                    f"{evidence_path}: missing current-layout evidence token {token!r}"
                )

    closure_path = root / CLOSURE_LEDGER_RELATIVE
    closure_source: str | None = None
    if _regular_file(
        closure_path, "multilane closure ledger for in-flight layout", errors
    ):
        closure_source = closure_path.read_text(encoding="utf-8")
        for token in (
            "Current first-release layouts are source-bound",
            "`LaneExecutablePayloadV1`",
            "QueuePlan journal V4",
            "reservation journal V5",
            "selected-batch",
            "READY authorization/signature/QC",
            "post-carrier",
            "four-stage",
            "`composed_state_action_relation_no_trace_extraction`",
            "fixed-width composed transition relation is implemented",
            "production trace extraction is not implemented",
        ):
            if token not in closure_source:
                errors.append(
                    f"{closure_path}: missing current-layout closure token {token!r}"
                )

    stale_surfaces: list[tuple[Path, str]] = []
    if module_source is not None:
        stale_surfaces.append((module_path, module_source))
    if positive_source is not None:
        stale_surfaces.append((positive_path, positive_source))
    stale_surfaces.extend(mutation_sources)
    if runner_source is not None:
        stale_surfaces.append((runner_path, runner_source))
    if evidence_source is not None:
        stale_surfaces.append((evidence_path, evidence_source))
    if closure_source is not None:
        stale_surfaces.append((closure_path, closure_source))
    for path, source in stale_surfaces:
        for token in INFLIGHT_LAYOUT_FORBIDDEN_TOKENS:
            if token in source:
                errors.append(
                    f"{path}: stale first-release layout token {token!r} is forbidden"
                )

    binding_items: dict[tuple[str, str, str], str] = {}
    for relative, kind, symbol, tokens in INFLIGHT_LAYOUT_PRODUCTION_BINDINGS:
        item = _rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "in-flight production layout binding",
            errors,
        )
        if item is None:
            continue
        binding_items[(relative, kind, symbol)] = item
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: in-flight production item {symbol} is "
                    f"missing current-layout token {token!r}"
                )

    for relative, kind, symbol, tokens in INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS:
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = _rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "ordered in-flight production layout binding",
                errors,
            )
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: ordered in-flight item {symbol} is "
                    f"missing or reorders token {token!r}"
                )
                break
            cursor = position

    for relative, kind, symbol, tokens in INFLIGHT_LAYOUT_FORBIDDEN_SOURCE_CHECKS:
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = _rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "forbidden in-flight production source binding",
                errors,
            )
        if item is None:
            continue
        for token in tokens:
            if token in item:
                errors.append(
                    f"{root / relative}: in-flight production item {symbol} "
                    f"contains forbidden source-bound token {token!r}"
                )

    for relative, tokens in INFLIGHT_LAYOUT_SOURCE_CHECKS:
        path = root / relative
        if not _regular_file(path, "in-flight whole-file source binding", errors):
            continue
        source = path.read_text(encoding="utf-8")
        for token in tokens:
            count = source.count(token)
            if count != 1:
                errors.append(
                    f"{path}: current-layout token {token!r} must occur exactly "
                    f"once, found {count}"
                )

    _regular_file(
        root / INFLIGHT_LAYOUT_TEST,
        "in-flight layout negative-control test",
        errors,
    )


def validate(root: Path = DEFAULT_ROOT) -> tuple[str, ...]:
    """Return structural/source-binding errors for the multilane model slice."""

    errors: list[str] = []
    root = root.resolve()
    formal_dir = root / FORMAL_RELATIVE
    bindings_path = formal_dir / BINDINGS_FILENAME
    if not _regular_file(bindings_path, "multilane source binding ledger", errors):
        return tuple(errors)
    try:
        ledger = json.loads(bindings_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return (f"cannot load {bindings_path}: {error}",)
    if not isinstance(ledger, dict) or set(ledger) != {
        "schema_version",
        "closure_mutations",
        "inflight_first_release_layout_contract",
        KURA_RETENTION_CONTRACT_KEY,
        "models",
    }:
        return (
            "multilane binding ledger must contain exactly schema_version, "
            "closure_mutations, inflight_first_release_layout_contract, "
            f"{KURA_RETENTION_CONTRACT_KEY}, and models",
        )
    if ledger.get("schema_version") != 5:
        errors.append("multilane binding ledger schema_version must equal 5")
    models = ledger.get("models")
    if not isinstance(models, list) or len(models) != 4:
        errors.append("multilane binding ledger must contain exactly four models")
        return tuple(errors)
    modules = [model.get("module") for model in models if isinstance(model, dict)]
    if len(set(modules)) != len(modules):
        errors.append("multilane binding ledger contains duplicate model modules")
    if set(modules) != set(EXPECTED_CLOSURE_INVARIANTS):
        errors.append(
            "multilane binding ledger modules differ from the reviewed "
            "closure-invariant inventory"
        )
    for model in models:
        _validate_model(root, formal_dir, model, errors)
    _validate_kura_replica_retention_contract(
        root,
        formal_dir,
        ledger.get(KURA_RETENTION_CONTRACT_KEY),
        errors,
    )
    _validate_native_participant_application_classifier_contract(
        root, models, errors
    )
    _validate_native_prepublication_contract(root, models, errors)
    _validate_queue_plan_startup_replay_contract(root, models, errors)
    _validate_inflight_layout_contract(
        root,
        formal_dir,
        ledger.get("inflight_first_release_layout_contract"),
        errors,
    )
    _validate_closure_mutation_ledger(
        root,
        formal_dir,
        ledger.get("closure_mutations"),
        models,
        ledger.get(KURA_RETENTION_CONTRACT_KEY),
        errors,
    )
    _validate_mutation_runner(
        root, models, ledger.get(KURA_RETENTION_CONTRACT_KEY), errors
    )
    _validate_apalache_gate(root, errors)
    return tuple(errors)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=DEFAULT_ROOT,
        help="repository root (defaults to the checker-derived root)",
    )
    parser.add_argument(
        "--print-source-manifest-sha256",
        action="store_true",
        help="print the current source-bound multilane gate manifest digest",
    )
    return parser


def main() -> int:
    args = _parser().parse_args()
    errors = validate(args.root)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    if args.print_source_manifest_sha256:
        print(source_manifest_sha256(args.root))
        return 0
    print(
        "Sumeragi v2 multilane models are structurally valid: five refinement "
        "kernels (including authenticated Kura retention) and the composed "
        "in-flight state/action relation are source-bound without a production "
        "trace-extraction claim; no Kura "
        "retention source check remains pending"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
