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
    ),
    "SumeragiV2AutonomousReservationCarrier": (
        "MLReservationSingleOwner",
        "MLReservationIdentityStable",
        "MLCertifiedBundleDurable",
        "MLMergeCandidateExactPrefix",
        "MLCarrierExactlyOnce",
        "MLRestartOwnershipPartition",
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
INFLIGHT_LAYOUT_CLAIM = "layout_only_no_transition_refinement"
INFLIGHT_LAYOUT_MODULE = "SumeragiV2InFlightFirstRelease"
INFLIGHT_LAYOUT_POSITIVE_CONFIG = "inflight_first_release_fixed.cfg"
INFLIGHT_LAYOUT_RUNNER = Path(
    "scripts/formal/run_sumeragi_v2_inflight_first_release.sh"
)
INFLIGHT_LAYOUT_TEST = Path(
    "pytests/scripts/sumeragi_v2_multilane_models_test.py"
)
INFLIGHT_LAYOUT_EVIDENCE = Path(
    "formal/sumeragi_v2/INFLIGHT_FIRST_RELEASE_EVIDENCE.md"
)
INFLIGHT_LAYOUT_REQUIRED_INVARIANTS = (
    "FirstReleaseTypeInvariant",
    "MLPayloadSchemaV2CarriesExactAdmissionPreimage",
    "MLValidatorCarrierOwnership",
    "MLPutBatchV4BeforeReservationV5",
    "MLReservationV5BeforeKuraActive",
    "MLKuraActiveBeforeExecutionInput",
    "MLExecutionInputBeforeReady",
    "MLCrashPrefixLossFree",
    "MLCommitAndReleaseRetainExactScope",
    "MLExactlyOnceCarrierApplication",
    "MLQueuePlanV4PutBatchBound4096",
)
INFLIGHT_LAYOUT_MUTATIONS = (
    (
        "inflight_first_release_reservation_before_put_batch_bug.cfg",
        "MLPutBatchV4BeforeReservationV5",
    ),
    (
        "inflight_first_release_kura_before_reservation_bug.cfg",
        "MLReservationV5BeforeKuraActive",
    ),
    (
        "inflight_first_release_ready_before_input_bug.cfg",
        "MLExecutionInputBeforeReady",
    ),
    (
        "inflight_first_release_crash_drops_durable_bug.cfg",
        "MLCrashPrefixLossFree",
    ),
    (
        "inflight_first_release_payload_conflict_bug.cfg",
        "MLPayloadSchemaV2CarriesExactAdmissionPreimage",
    ),
    (
        "inflight_first_release_commit_scope_conflict_bug.cfg",
        "MLCommitAndReleaseRetainExactScope",
    ),
    (
        "inflight_first_release_release_scope_conflict_bug.cfg",
        "MLCommitAndReleaseRetainExactScope",
    ),
    (
        "inflight_first_release_duplicate_apply_bug.cfg",
        "MLExactlyOnceCarrierApplication",
    ),
    (
        "inflight_first_release_oversize_put_batch_bug.cfg",
        "MLQueuePlanV4PutBatchBound4096",
    ),
)
INFLIGHT_LAYOUT_FORBIDDEN_TOKENS = (
    "LaneExecutablePayloadV3",
    "QueuePlanV5",
    "QueuePlan V5",
    "queuePlanV5",
    "everQueuePlanV5",
    "PutBatchV5",
    "reservationV9",
    "everReservationV9",
    "FsyncReservationV9",
    "reservation V9",
    "MLPayloadV3CarriesExactAdmissionPreimage",
    "MLPutBatchV5BeforeReservationV9",
    "MLReservationV9BeforeKuraActive",
)
INFLIGHT_LAYOUT_PRODUCTION_BINDINGS = (
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "struct",
        "LaneExecutablePayloadV1",
        (
            "pub version: u8",
            "pub entrypoint_hashes: Vec<Hash>",
            "pub entrypoints: Vec<TransactionEntrypoint>",
            "pub reservation_keys: Vec<LaneQueueReservationKeyV2>",
            "pub routing_plans: Vec<RoutingPlan>",
            "pub native_amx_receipts: Vec<Option<NativeAmxReceipt>>",
            "pub payload_hash: Hash",
            "pub producer_signature: Vec<u8>",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "method",
        "LaneExecutablePayloadV1::new_signed_with_reservations",
        (
            "version: LANE_EXECUTABLE_PAYLOAD_VERSION_V2",
            "reservation_keys",
            "routing_plans",
            "native_amx_receipts",
            "payload.computed_payload_hash()?",
            "Signature::try_new",
            "payload.validate(chain_id_hash, epoch)?",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "fn",
        "validate_lane_executable_payload_body",
        (
            "version != LANE_EXECUTABLE_PAYLOAD_VERSION_V2",
            "entrypoints.is_empty()",
            "entrypoints.len() > MAX_LANE_EXECUTABLE_ENTRYPOINTS",
            "reservation_keys.len() != entrypoints.len()",
            "routing_plans.len() != entrypoints.len()",
            "native_amx_receipts.len() != entrypoints.len()",
            "key.validate().is_err()",
            "key.lane_incarnation != descriptor.lane_incarnation",
            "routing_plan.digest() != key.routing_plan_digest",
            "compute_lane_executable_payload_hash(",
        ),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        "struct",
        "QueuePlanJournalRecordV4",
        (
            "pub version: u16",
            "pub entrypoint: TransactionEntrypoint",
            "pub entrypoint_hash: HashOf<TransactionEntrypoint>",
            "pub routing_plan: RoutingPlan",
            "pub admission_context: QueuePlanAdmissionContextV2",
            "pub global_admission_identity: Option<QueuePlanGlobalAdmissionIdentityV2>",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "struct",
        "LaneQueueReservationKeyV2",
        (
            "pub version: u16",
            "pub entrypoint_hash: HashOf<TransactionEntrypoint>",
            "pub queue_plan_admission_binding_hash: Hash",
            "pub routing_plan_digest: Hash",
            "pub coordinator_leg: RouteLeg",
            "pub lane_incarnation: Hash",
            "pub proposal_height: u64",
            "pub lane_block_height: u64",
            "pub lane_block_view: u64",
            "pub reservation_owner_hash: Hash",
            "pub proposal_identity_hash: Hash",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "struct",
        "LaneQueueReservationRecordV5",
        (
            "version: u16",
            "key: LaneQueueReservationKeyV2",
            "enqueue_timestamp_ms: u64",
            "fifo_order: LaneQueueFifoOrderV5",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "fn",
        "reserve_transactions_for_lane_bounded",
        (
            "durable_claim.global_admission_binding()",
            "QueuePlanAdmissionRegistryMatch::Exact",
            "version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION",
            "journal.put_batch(",
            "restore_popped_hash_locked",
            "live_by_hash",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "struct",
        "LaneBlockExecutionInputArtifact",
        (
            "pub format: LaneBlockExecutionInputArtifactFormat",
            "pub proposal: LaneBlockProposalV1",
            "pub autonomous_chain_id_hash: Option<Hash>",
            "pub autonomous_epoch: Option<u64>",
            "pub autonomous_payload_hash: Option<Hash>",
            "pub entrypoint_hashes: Vec<Hash>",
            "pub entrypoints: Vec<TransactionEntrypoint>",
            "pub reservation_keys: Vec<LaneQueueReservationKeyV2>",
            "pub routing_plans: Vec<RoutingPlan>",
            "pub native_amx_receipts: Vec<Option<NativeAmxReceipt>>",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::validate_lane_block_execution_input_artifact",
        (
            "artifact.reservation_keys.len() != artifact.entrypoints.len()",
            "artifact.routing_plans.len() != artifact.entrypoints.len()",
            "artifact.native_amx_receipts.len() != artifact.entrypoints.len()",
            "key.validate().is_err()",
            "artifact.entrypoint_hashes != descriptor.accepted_transaction_hashes",
            "Hash::from(entrypoint.hash()) != expected_hash",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::recover_lane_block_execution_input_source",
        (
            "recover_autonomous_lane_block_payload_with_sidecar_repair",
            "recover_lane_block_payload_with_sidecar_repair",
            "LaneBlockPayloadAvailability::DescriptorMismatch",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::persist_lane_block_execution_input",
        (
            "recover_lane_block_execution_input_source(",
            "if &verified != recovered",
            "LaneBlockExecutionInputArtifact::new(verified)",
            "write_lane_block_execution_input_artifact(&artifact)",
        ),
    ),
)
INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/queue.rs",
        "fn",
        "reserve_transactions_for_lane_bounded",
        (
            "durable_claim.global_admission_binding()",
            "QueuePlanAdmissionRegistryMatch::Exact",
            "journal.put_batch(",
            "live_by_hash",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::persist_lane_block_execution_input",
        (
            "recover_lane_block_execution_input_source(",
            "if &verified != recovered",
            "LaneBlockExecutionInputArtifact::new(verified)",
            "write_lane_block_execution_input_artifact(&artifact)",
        ),
    ),
)
INFLIGHT_LAYOUT_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/lane_consensus.rs",
        (
            "pub(crate) const MAX_LANE_EXECUTABLE_ENTRYPOINTS: usize = "
            "MAX_MERGE_EXECUTION_ENTRYPOINTS;",
            "pub(crate) const LANE_EXECUTABLE_PAYLOAD_VERSION_V2: u8 = 2;",
            "unknown.version = LANE_EXECUTABLE_PAYLOAD_VERSION_V2 + 1;",
        ),
    ),
    (
        "crates/iroha_data_model/src/merge.rs",
        ("pub const MAX_MERGE_EXECUTION_ENTRYPOINTS: usize = 4_096;",),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        (
            "pub const QUEUE_PLAN_JOURNAL_VERSION: u16 = 4;",
            "unsupported.version = QUEUE_PLAN_JOURNAL_VERSION + 1;",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        (
            "pub const LANE_QUEUE_RESERVATION_JOURNAL_VERSION: u16 = 5;",
            "legacy.fifo_order.version = "
            "LANE_QUEUE_RESERVATION_JOURNAL_VERSION - 1;",
        ),
    ),
    (
        "scripts/formal/run_sumeragi_v2_tlc.sh",
        (
            'readonly INFLIGHT_FIRST_RELEASE_RUNNER="${REPO_ROOT}/scripts/formal/'
            'run_sumeragi_v2_inflight_first_release.sh"',
            'bash "$INFLIGHT_FIRST_RELEASE_RUNNER"',
        ),
    ),
    (
        "scripts/write_sumeragi_v2_release_receipt.py",
        (
            "_APALACHE_LAYOUT_ONLY_RESULTS = (",
            '"inflight-first-release-layout"',
            '"SumeragiV2InFlightFirstRelease"',
            '"inflight_first_release_fixed.cfg"',
            "*_APALACHE_REFINEMENT_RESULTS",
            "*_APALACHE_LAYOUT_ONLY_RESULTS",
        ),
    ),
    (
        "pytests/scripts/sumeragi_v2_release_receipt_test.py",
        (
            'canonical.replace("result_count\\t5", "result_count\\t4", 1)',
            '"result\\tinflight-first-release-refinement\\t"',
            '"is not exact source-bound NoError evidence"',
        ),
    ),
)
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
        ),
    ),
    "ML-MUT-NAT-07": (
        TLA_COUNTEREXAMPLE,
        "MLNativeLatestIndexExact",
        ("multilane_native_ambiguous_latest_index_bug.cfg",),
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
        "crates/iroha_torii/src/routing.rs",
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
        "reconcile_lane_reservation_ownership",
    ): ("merge_ledger_all_entries",),
}


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
    if len(mapped_configs) != len(set(mapped_configs)):
        errors.append("one TLA mutation config maps to multiple conceptual IDs")
    if len(model_configs) != len(set(model_configs)):
        errors.append("model inventory contains duplicate TLA mutation configs")
    if set(mapped_configs) != set(model_configs):
        errors.append(
            "conceptual closure mappings must cover every and only the model "
            "mutation configs"
        )
    if len(model_configs) != 37:
        errors.append(
            f"reviewed multilane mutation inventory must contain 37 configs, "
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
    impl_re = re.compile(
        rf"(?m)^[ \t]*impl[ \t]+{re.escape(owner)}[ \t]*(?=\{{)"
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
    root: Path, models: list[Any], errors: list[str]
) -> None:
    """Require the deterministic TLC runner to cover the exact ledger corpus."""

    runner = root / TLC_MUTATION_RUNNER_RELATIVE
    if not _regular_file(runner, "multilane TLC mutation runner", errors):
        return
    if runner.stat().st_mode & 0o111 == 0:
        errors.append(f"multilane TLC mutation runner must be executable: {runner}")
    source = runner.read_text(encoding="utf-8")
    normalized = source.replace("\\\n", " ")
    call_re = re.compile(
        r'run_mutant\s+[a-z0-9-]+\s+"?\$[A-Z_]+"?\s+'
        r"(multilane_[a-z0-9_]+_bug\.cfg)\s+([A-Za-z0-9_]+)"
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
    if actual != expected:
        errors.append(
            f"{runner}: exact ordered mutation calls differ from the "
            "multilane source-binding ledger"
        )
    required_once = (
        '[[ "$status" -ne 12 ]]',
        'grep -Fq "Invariant ${invariant} is violated."',
        'grep -Fq "TLC2 Version 2.19"',
        f"[tlc] all {len(expected)} multilane mutations produced their exact "
        "named counterexamples; no deductive proof status was changed",
    )
    for token in required_once:
        count = source.count(token)
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
        '\npython3 -I -S "$CONTRACT_CHECKER"\n',
        'python3 -I -S "$RUNNER_CONTRACT_TEST"',
        'tool_version="$("$RESOLVED_APALACHE_BIN" version)"',
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
        "printf 'result_count\\t5\\n'",
        'mv -- "$evidence_tmp" "$EVIDENCE_PATH"',
        "[apalache] all 4 source-bound refinement kernels plus the layout-only "
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
  "NativeEvidenceTypeInvariant, NativeStandaloneEvidenceInvariant, NativeEvidenceRetentionBoundInvariant, NativeNoClobberPublicationInvariant, NativeLegacyDenseRejectedInvariant, NativePruneJournalInvariant, SidecarsRequireManifestInvariant, FrontierPublicationInvariant, PrunedEvidenceVerifiableInvariant, SameRouteControlOnlyInvariant, MLSeparateParticipantApplication, MLNativeSourceClaimInjective, MLNativeContiguousActiveRoute, MLNativeGroupExactCover, MLNativeManifestAuthenticates, MLNativeDurabilityPrecedesFrontier, MLNativeLatestIndexExact\"""",
        """run_positive \\
  autonomous-reservation-carrier \\
  "$AUTONOMOUS_MODULE" \\
  multilane_autonomous_reservation_carrier_fixed.cfg \\
  10 \\
  "ReservationCarrierTypeInvariant, SingleOwnershipInvariant, ExactCarrierIdentityInvariant, ControlOnlyAnchorInvariant, CandidateAuthorizationInvariant, ReleaseOrderingInvariant, QueueReleaseCompletionInvariant, AtMostOnceApplicationInvariant, NoReleaseAfterApplicationInvariant, NoStaleIncarnationReleaseInvariant, ForgottenOnlyAfterApplicationInvariant, MLReservationSingleOwner, MLReservationIdentityStable, MLCertifiedBundleDurable, MLMergeCandidateExactPrefix, MLCarrierExactlyOnce, MLRestartOwnershipPartition, MLStageEvidenceMonotonic\"""",
        """run_positive \\
  queue-plan-admission-registry \\
  "$QUEUE_PLAN_ADMISSION_MODULE" \\
  multilane_queue_plan_admission_registry_fixed.cfg \\
  8 \\
  "QueuePlanAdmissionTypeInvariant, MLAdmissionCasUnique, MLCertificateDurable, MLPublic202Exact, MLExecutionRequiresExactBinding, MLQueueEligibilityExact, MLAdmissionAtMostOnceExecution, MLImmutableAdmissionTombstone, MLCancellationStopsExecution\"""",
        """run_positive \\
  inflight-first-release-layout \\
  "$INFLIGHT_FIRST_RELEASE_MODULE" \\
  inflight_first_release_fixed.cfg \\
  10 \\
  "FirstReleaseTypeInvariant, MLPayloadSchemaV2CarriesExactAdmissionPreimage, MLValidatorCarrierOwnership, MLPutBatchV4BeforeReservationV5, MLReservationV5BeforeKuraActive, MLKuraActiveBeforeExecutionInput, MLExecutionInputBeforeReady, MLCrashPrefixLossFree, MLCommitAndReleaseRetainExactScope, MLExactlyOnceCarrierApplication, MLQueuePlanV4PutBatchBound4096\"""",
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
        "inflight_first_release_reservation_before_put_batch_bug.cfg",
        "inflight_first_release_kura_before_reservation_bug.cfg",
        "inflight_first_release_ready_before_input_bug.cfg",
        "inflight_first_release_crash_drops_durable_bug.cfg",
        "inflight_first_release_payload_conflict_bug.cfg",
        "inflight_first_release_commit_scope_conflict_bug.cfg",
        "inflight_first_release_release_scope_conflict_bug.cfg",
        "inflight_first_release_duplicate_apply_bug.cfg",
        "inflight_first_release_oversize_put_batch_bug.cfg",
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
            "| in-flight carrier (layout-only) | `inflight_first_release_fixed.cfg` | 10 |",
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
    root: Path, formal_dir: Path, model: Any, errors: list[str]
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

    expected_invariants = EXPECTED_CLOSURE_INVARIANTS.get(module)
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


def _validate_inflight_layout_contract(
    root: Path,
    formal_dir: Path,
    contract: Any,
    errors: list[str],
) -> None:
    """Bind the abstract in-flight corpus to current layouts without theorem inflation."""

    expected_keys = {
        "claim",
        "module",
        "positive_config",
        "runner",
        "evidence",
        "required_invariants",
        "mutations",
        "production_symbols",
        "ordered_source_checks",
        "source_checks",
        "forbidden_tokens",
    }
    if not isinstance(contract, dict) or set(contract) != expected_keys:
        errors.append(
            "in-flight layout contract must contain exactly claim, module, "
            "positive_config, runner, evidence, required_invariants, mutations, "
            "production_symbols, ordered_source_checks, source_checks, and "
            "forbidden_tokens"
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
            "nine-control corpus"
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
            "payload/queue/Kura layout contract"
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
            "durability/publication order contract"
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
                f"{module_path}: layout-only kernel must not declare a production "
                "refinement obligation"
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
                "nine-control corpus"
            )
        for token in (
            "\nrun_positive\n",
            '[[ "$status" -ne 12 ]]',
            'grep -Fq "Invariant ${invariant} is violated."',
            "bounded abstract evidence only; no production refinement claim",
        ):
            count = runner_source.count(token)
            if count != 1:
                errors.append(
                    f"{runner_path}: fail-closed runner token {token!r} must occur "
                    f"exactly once, found {count}"
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
            "`layout_only_no_transition_refinement`",
            "production-refinement theorem",
            "total projection theorem",
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
            "`layout_only_no_transition_refinement`",
            "total transition projection is not implemented",
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
        "models",
    }:
        return (
            "multilane binding ledger must contain exactly schema_version, "
            "closure_mutations, inflight_first_release_layout_contract, and models",
        )
    if ledger.get("schema_version") != 3:
        errors.append("multilane binding ledger schema_version must equal 3")
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
        errors,
    )
    _validate_mutation_runner(root, models, errors)
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
        "Sumeragi v2 multilane models are structurally valid: four refinement "
        "kernels and the layout-only in-flight carrier are bound to current "
        "production symbols"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
