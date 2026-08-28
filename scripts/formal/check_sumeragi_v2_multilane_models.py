#!/usr/bin/env python3
"""Validate multilane model/config and static/differential source bindings.

This is a structural gate. It verifies that every finite model, positive
configuration, conceptual mutation mapping, production item, and release-only
check still exists with the reviewed semantic anchors. It does not treat TLC
output as deductive proof.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import sys
from functools import lru_cache
from pathlib import Path
from typing import Any

FORMAL_CHECKER_DIR = Path(__file__).resolve().parent
if str(FORMAL_CHECKER_DIR) not in sys.path:
    sys.path.insert(0, str(FORMAL_CHECKER_DIR))

import sumeragi_v2_multilane_autonomous_terminal_contract as autonomous_terminal_contract
from sumeragi_v2_multilane_autonomous_terminal_contract import (
    AUTONOMOUS_TERMINAL_FORBIDDEN_SOURCE_CHECKS,
    AUTONOMOUS_TERMINAL_ORDERED_SOURCE_CHECKS,
    AUTONOMOUS_TERMINAL_RAW_TEST_CHECKS,
    AUTONOMOUS_TERMINAL_RECOVERY_BINDINGS,
    AUTONOMOUS_TERMINAL_TLA_RELATIVE,
    AUTONOMOUS_TERMINAL_TEST_BINDINGS,
    KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
    validate_autonomous_terminal_recovery_contract,
)
import sumeragi_v2_multilane_native_merge_manifest_contract as native_merge_manifest, sumeragi_v2_multilane_passive_recovery_contract as passive_recovery_contract, sumeragi_v2_multilane_reviewed_rust_source as reviewed_source
from sumeragi_v2_multilane_cli import build_parser, report_validation
from sumeragi_v2_multilane_reviewed_rust_source import (
    REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE,
    REVIEWED_RUST_INCLUDE_MANIFEST_SHA256,
    REVIEWED_RUST_SOURCE_HELPER_RELATIVE,
    _expanded_source_manifest_paths,
    _read_reviewed_rust_source,
    _REVIEWED_RUST_INCLUDE_MANIFESTS,
    _reviewed_rust_source_cache,
    _validate_reviewed_rust_include_manifest,
)


DEFAULT_ROOT = Path(__file__).resolve().parents[2]
FORMAL_RELATIVE = Path("formal/sumeragi_v2")
BINDINGS_FILENAME = "multilane_source_bindings.json"
PROOF_COVERAGE_RELATIVE = FORMAL_RELATIVE / "proof_coverage.json"
CLOSURE_LEDGER_RELATIVE = Path("specs/sumeragi_v2_multilane_closure_ledger.md")
APALACHE_RUNNER_RELATIVE = Path("scripts/formal/run_sumeragi_v2_multilane_apalache.sh")
APALACHE_RUNNER_TEST_RELATIVE = Path("scripts/formal/check_sumeragi_v2_multilane_apalache_runner_contract.py")
APALACHE_INSTALLER_RELATIVE = Path("scripts/formal/install_apalache.sh")
TLC_RUNNER_RELATIVE = Path("scripts/formal/run_sumeragi_v2_tlc.sh")
TLC_MUTATION_RUNNER_RELATIVE = Path("scripts/formal/run_sumeragi_v2_multilane_mutations.sh")
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
        r"(?:const[ \t]+)?(?:async[ \t]+)?(?:proof[ \t]+)?fn[ \t]+{symbol}\b"
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
        "MLNativePruneProtectedLatestExact",
        "MLNativePruneExactObjectRemoval",
        "MLUnifiedStartupEvidenceRepairSafe",
    ),
    "SumeragiV2AutonomousReservationCarrier": (
        "MLReservationSingleOwner",
        "MLReservationIdentityStable",
        "MLCertifiedBundleDurable",
        "MLMergeCandidateExactPrefix",
        "MLCarrierCommitSurfaceExact",
        "MLCarrierExactlyOnce",
        "MLRestartOwnershipPartition",
        "MLRecoveredCarrierBodyAuthenticated",
        "MLRecoveredCarrierLengthAuthenticated",
        "MLHistoricalRecoveryContextExact",
        "MLHistoricalQueueGateOrder",
        "MLHistoricalAllGroupsPreflight",
        "MLLocalProducerRecoveryRequiresQueueOwner",
        "MLTerminalOutcomeJoinAuthenticated",
        "MLCanonicalTerminalBatchAtomic",
        "MLTerminalStartupSweepOrder",
        "MLStageEvidenceMonotonic",
    ),
    "SumeragiV2QueuePlanAdmissionRegistry": (
        "MLAdmissionCasUnique",
        "MLCertificateDurable",
        "MLPublic202Exact",
        "MLExecutionRequiresExactBinding",
        "MLQueuePlanExecutionAutonomousOnly",
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
    from sumeragi_v2_multilane_queue_plan_contract import (
        QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS,
        QUEUE_PLAN_AUTONOMOUS_ONLY_TEST_BINDINGS,
        QUEUE_PLAN_STARTUP_REPLAY_BINDINGS,
        QUEUE_PLAN_STARTUP_REPLAY_FORBIDDEN_SOURCE_CHECKS,
        QUEUE_PLAN_STARTUP_REPLAY_MODULE,
        QUEUE_PLAN_STARTUP_REPLAY_ORDERED_SOURCE_CHECKS,
        QUEUE_PLAN_STARTUP_REPLAY_POST_APPLY_FORBIDDEN_TOKENS,
        QUEUE_PLAN_STARTUP_REPLAY_POST_APPLY_MARKER,
        QUEUE_PLAN_STARTUP_REPLAY_TEST_BINDINGS,
        validate_queue_plan_autonomous_only_contract,
    )
finally:
    sys.path.pop(0)


def _validate_queue_plan_autonomous_only_contract(
    root: Path,
    formal_dir: Path,
    models: Any,
    errors: list[str],
) -> None:
    """Preserve the checker-owned QueuePlan validation seam for tests."""

    validate_queue_plan_autonomous_only_contract(
        root,
        formal_dir,
        models,
        errors,
        _rust_binding_item,
        _regular_file,
        TLA_DECLARATION_TEMPLATE,
    )


def _replace_exact_tokens(tokens: tuple[str, ...], replacements: dict[str, str]) -> tuple[str, ...]:
    """Return the reviewed token list rebound to the merged production spelling."""

    return tuple(replacements.get(token, token) for token in tokens)


_QUEUE_PLAN_STARTUP_TOKEN_REBINDINGS = {
    "IrohaNetwork::start_with_crypto_and_initial_trusted_sources(": (
        "IrohaNetwork::start_with_crypto_and_initial_authorities("
    ),
}
QUEUE_PLAN_STARTUP_REPLAY_BINDINGS = tuple(
    (
        relative,
        kind,
        symbol,
        _replace_exact_tokens(tokens, _QUEUE_PLAN_STARTUP_TOKEN_REBINDINGS),
    )
    for relative, kind, symbol, tokens in QUEUE_PLAN_STARTUP_REPLAY_BINDINGS
)
QUEUE_PLAN_STARTUP_REPLAY_ORDERED_SOURCE_CHECKS = tuple(
    (
        relative,
        kind,
        symbol,
        _replace_exact_tokens(tokens, _QUEUE_PLAN_STARTUP_TOKEN_REBINDINGS),
    )
    for relative, kind, symbol, tokens in QUEUE_PLAN_STARTUP_REPLAY_ORDERED_SOURCE_CHECKS
)
QUEUE_PLAN_STARTUP_REPLAY_TEST_BINDINGS = tuple(
    (
        (
            relative,
            "queue_plan_journal_replay_retains_entrypoint_that_fails_stateless_revalidation",
            (
                'expect_err("wrong-network journal entrypoint must fail startup")',
                "failed canonical stateless validation",
                "assert!(!replay_queue.txs.contains_key(&hash));",
                "live_record_count()",
                "stateless failure must not append a tombstone or replacement",
            ),
        )
        if symbol
        == "queue_plan_journal_replay_retains_current_admission_rejection_and_fails_startup"
        else (relative, symbol, tokens)
    )
    for relative, symbol, tokens in QUEUE_PLAN_STARTUP_REPLAY_TEST_BINDINGS
)
_INFLIGHT_CURRENT_PRODUCTION_BINDINGS = {
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "schedule_local_proposal",
    ): (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "schedule_local_proposal",
        (
            "executor.can_schedule_local_proposal()?",
            "let attachments = candidate_attachments(",
            "let assembly = assembler.assemble(CandidateRequest {",
            "work_provider: &mut *lane_work",
            "let candidate = match assembly",
            "CandidateAssemblyOutcome::NoProposalWork(report)",
            "report.work_deferred > 0",
            "proposal_state.defer_candidate_work(",
            "lane_work.bind_local_candidate(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "candidate_work_requires_wait",
    ): (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "schedule_local_proposal",
        (
            "CandidateAssemblyOutcome::NoProposalWork(report)",
            "if report.work_deferred > 0",
            "proposal_state.defer_candidate_work(owner, now, candidate_work_wait_bound)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "claim_certified_execution_proposal_turn",
    ): (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "schedule_local_proposal",
        (
            "if !executor.can_schedule_local_proposal()?",
            "let assembly = assembler.assemble(CandidateRequest {",
            "proposal_state.attempted = Some(owner)",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs",
        "Kura::persist_lane_block_execution_input",
    ): (
        "crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs",
        "method",
        "Kura::persist_lane_block_execution_input",
        (
            "let _prune_guard = self.prune_lock.lock();",
            "let _canonical_chain_guard = self.canonical_chain_lock.lock();",
            "pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?",
            "persist_lane_block_execution_input_under_prune_and_canonical_guards(",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs",
        "Kura::persist_lane_block_execution_input_under_prune_guard",
    ): (
        "crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs",
        "method",
        "Kura::persist_lane_block_execution_input_under_prune_and_canonical_guards",
        (
            "ensure_prune_recovery_not_required()",
            "recover_lane_block_execution_input_source(",
            "if &verified != recovered",
            "LaneBlockExecutionInputArtifact::new(verified)",
            "read_autonomous_lane_block_artifact_with_recovery_policy(",
            "authorize_autonomous_execution_input_persistence(",
            "write_lane_block_execution_input_artifact(",
            "execution_input_authorization",
            "pending_canonical_bytes",
        ),
    ),
}


def _current_inflight_production_binding(
    binding: tuple[str, str, str, tuple[str, ...]],
) -> tuple[str, str, str, tuple[str, ...]]:
    """Rebind one first-release layout owner to the merged implementation."""

    relative, kind, symbol, tokens = binding
    return _INFLIGHT_CURRENT_PRODUCTION_BINDINGS.get(
        (relative, symbol), (relative, kind, symbol, tokens)
    )


INFLIGHT_LAYOUT_PRODUCTION_BINDINGS = tuple(
    _current_inflight_production_binding(binding)
    for binding in INFLIGHT_LAYOUT_PRODUCTION_BINDINGS
)

_INFLIGHT_CURRENT_ORDERED_BINDINGS = {
    (
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "Kura::publish_certified_frontier_and_consume_capacity_locked",
    ): (
        "crates/iroha_core/src/kura/certified_bundle_capacity.rs",
        "method",
        "Kura::publish_certified_frontier_and_consume_capacity_locked",
        (
            "publish_latest_certified_lane_block_frontier_locked(entry, artifact, authority)?",
            "let durable_frontier = self",
            "read_latest_certified_lane_block_frontier_structural_locked(entry, false)?",
            "durable_frontier.frontier.artifact != *artifact",
            "confirm_latest_certified_lane_block_frontier_read_locked(",
            "consume_certified_bundle_frontier_capacity(artifact)?",
            "FAIL_AFTER_NEXT_AUTONOMOUS_CERTIFIED_FRONTIER",
            "Ok(frontier_changed)",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs",
        "Kura::persist_lane_block_execution_input",
    ): _INFLIGHT_CURRENT_PRODUCTION_BINDINGS[
        (
            "crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs",
            "Kura::persist_lane_block_execution_input",
        )
    ],
    (
        "crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs",
        "Kura::persist_lane_block_execution_input_under_prune_guard",
    ): _INFLIGHT_CURRENT_PRODUCTION_BINDINGS[
        (
            "crates/iroha_core/src/kura/autonomous_execution_view_capacity.rs",
            "Kura::persist_lane_block_execution_input_under_prune_guard",
        )
    ],
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "schedule_local_proposal",
    ): (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "schedule_local_proposal",
        (
            "executor.can_schedule_local_proposal()?",
            "let attachments = candidate_attachments(",
            "let assembly = assembler.assemble(CandidateRequest {",
            "work_provider: &mut *lane_work",
            "let candidate = match assembly",
            "CandidateAssemblyOutcome::NoProposalWork(report)",
            "proposal_state.defer_candidate_work(",
            "lane_work.bind_local_candidate(",
        ),
    ),
}
INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS = tuple(
    _INFLIGHT_CURRENT_ORDERED_BINDINGS.get(
        (relative, symbol), (relative, kind, symbol, tokens)
    )
    for relative, kind, symbol, tokens in INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS
)
_SUPERSEDED_NATIVE_RECOVERY_BINDINGS = frozenset(
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        symbol,
    )
    for symbol in (
        "pending_native_participant_recovery_markers",
        "retire_native_participant_recovery_request",
        "reconcile_native_participant_recovery_requests",
        "service_next_native_participant_recovery_request",
        "schedule_native_participant_recovery_request",
        "validate_native_participant_recovery_request",
        "serve_historical_recovery_request",
        "accept_native_participant_recovery_response",
        "V2LaneWorkAdapter::new_with_output_guard_and_transport_inner",
        "V2LaneWorkAdapter::repair_globally_applied_lane_receipts",
    )
)
_CURRENT_NATIVE_RECOVERY_REPLACEMENT_BINDINGS = frozenset(
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/canonical_executed_block_application_repair.rs",
        symbol,
    )
    for symbol in (
        "canonical_executed_block_need_for_height",
        "validate_canonical_executed_block_need",
        "validate_canonical_executed_block_request",
        "canonical_executed_block_matches_need",
        "build_canonical_executed_block_response",
        "plan_lane_application_evidence_repair",
        "apply_lane_application_evidence_repair",
        "CanonicalExecutedBlockRecovery::new",
        "CanonicalExecutedBlockRecovery::service_next",
        "CanonicalExecutedBlockRecovery::accept_with_ingress_ownership",
        "CanonicalExecutedBlockRecovery::accept_response",
    )
)
_ALLOWED_MERGED_DUPLICATE_PRODUCTION_BINDINGS = frozenset()
_PRODUCTION_TOKEN_REBINDINGS = {
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/canonical_executed_block_application_repair.rs",
        "peer_is_global_finality_signer",
        "commit_qc.signers.binary_search",
    ): "finality.commit_qc.signers.iter().any",
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "V2LaneWorkAdapter::has_pending_historical_recovery",
        "native_participant_recovery_requests.is_empty",
    ): "!self.historical_recovery_sessions.is_empty()",
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "V2LaneWorkAdapter::has_pending_historical_recovery",
        "pending_native_participant_recovery_markers",
    ): "!self.historical_recovery_sessions.is_empty()",
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "V2LaneWorkAdapter::has_pending_historical_recovery",
        "Err(_) => true",
    ): "!self.historical_recovery_sessions.is_empty()",
    (
        "crates/iroha_core/src/queue.rs",
        "release_lane_reservations_in_order_inner",
        "let restored_fifo = self.fifo_with_released_reservations_locked(&released_records)?;",
    ): "self.fifo_with_released_reservations_locked(&released_records)?;",
    (
        "crates/irohad/src/main.rs",
        "Iroha::start_with_runtime_deps",
        "finalize_plan_journal_startup_recovery()",
    ): "replay_plan_journal(&state)",
}

_RELEASE_SOURCE_TOKEN_REBINDINGS = {
    (
        "crates/iroha_core/src/state.rs",
        "No adapter/session cache is consulted.",
    ): "No adapter/session\n    /// cache is consulted.",
    (
        "crates/iroha_data_model/src/bin/sumeragi_v2_wire_fixtures.rs",
        "add `--check`",
    ): "Pass `--check`",
}


TLA_COUNTEREXAMPLE = "tla_counterexample"
STATIC_RELEASE = "static_release"
DIFFERENTIAL_RELEASE = "differential_release"
RELEASE_INVARIANT_CLASSIFICATIONS = frozenset(
    (STATIC_RELEASE, DIFFERENTIAL_RELEASE)
)
DIAGNOSTIC_STABLE_GENERATION_STATE_RELATIVE = "crates/iroha_core/src/state.rs"
DIAGNOSTIC_STABLE_GENERATION_HELPER_RELATIVE = (
    "crates/iroha_core/src/state/diagnostic_state_generation.rs"
)
DIAGNOSTIC_STABLE_GENERATION_ATTEMPT_BOUND = (
    "const DIAGNOSTIC_STABLE_STATE_GENERATION_ATTEMPTS: usize = 4;"
)
DIAGNOSTIC_STABLE_GENERATION_HELPER_BINDING = (
    DIAGNOSTIC_STABLE_GENERATION_HELPER_RELATIVE,
    "method",
    "State::derive_diagnostics_at_stable_state_generation",
    (
        "for _ in 0..DIAGNOSTIC_STABLE_STATE_GENERATION_ATTEMPTS",
        "let generation_before = self.state_view_generation();",
        "if generation_before % 2 != 0",
        "let result = derive();",
        "let generation_after = self.state_view_generation();",
        "is_stable_state_view_generation(generation_before, generation_after)",
        "return result;",
        "Err(generation_drift_error())",
    ),
)
DIAGNOSTIC_STABLE_GENERATION_CONSUMER_BINDINGS = (
    (
        "SumeragiV2NativeApplicationEvidence",
        DIAGNOSTIC_STABLE_GENERATION_STATE_RELATIVE,
        "method",
        "State::native_amx_participant_applications_diagnostics",
        (
            "derive_diagnostics_at_stable_state_generation",
            "native_amx_participant_applications_diagnostics_once",
            "MergeLedgerCommitError::ExecutionMarkerConflict",
            "State generation changed repeatedly during bounded Native AMX participant diagnostics",
        ),
        (
            "self.derive_diagnostics_at_stable_state_generation(",
            "|| self.native_amx_participant_applications_diagnostics_once()",
            "MergeLedgerCommitError::ExecutionMarkerConflict(",
        ),
    ),
    (
        "SumeragiV2AutonomousReservationCarrier",
        DIAGNOSTIC_STABLE_GENERATION_STATE_RELATIVE,
        "method",
        "State::autonomous_lane_execution_diagnostics_inner",
        (
            "derive_diagnostics_at_stable_state_generation",
            "autonomous_lane_execution_diagnostics_once",
            "eyre!",
            "State generation changed repeatedly during bounded autonomous lane execution diagnostics",
        ),
        (
            "self.derive_diagnostics_at_stable_state_generation(",
            "|| self.autonomous_lane_execution_diagnostics_once(queue)",
            "eyre!(",
        ),
    ),
)
DIAGNOSTIC_STABLE_GENERATION_TEST_BINDINGS = (
    (
        "diagnostic_projection_retries_after_state_generation_change",
        (
            "derive_diagnostics_at_stable_state_generation",
            "if attempt == 1",
            "begin_state_view_write",
            "observed, 2",
            "attempts.get(), 2",
        ),
    ),
    (
        "diagnostic_projection_fails_closed_after_bounded_generation_drift",
        (
            "derive_diagnostics_at_stable_state_generation",
            "begin_state_view_write",
            "Err(\"diagnostic State generation did not stabilize\")",
            "DIAGNOSTIC_STABLE_STATE_GENERATION_ATTEMPTS",
        ),
    ),
)
NATIVE_SOURCE_CLAIM_MUTATION_CONFIGS = (
    "multilane_native_source_claim_equivocation_bug.cfg",
    "multilane_native_source_claim_source_id_drift_bug.cfg",
    "multilane_native_source_claim_tx_entrypoint_hash_drift_bug.cfg",
    "multilane_native_source_claim_plan_digest_drift_bug.cfg",
    "multilane_native_source_claim_round_context_id_drift_bug.cfg",
    "multilane_native_source_claim_round_height_drift_bug.cfg",
    "multilane_native_source_claim_round_view_drift_bug.cfg",
    "multilane_native_source_claim_epoch_drift_bug.cfg",
    "multilane_native_source_claim_network_id_drift_bug.cfg",
    "multilane_native_source_claim_authority_context_height_drift_bug.cfg",
    "multilane_native_source_claim_coordinator_lane_id_drift_bug.cfg",
    "multilane_native_source_claim_coordinator_dataspace_id_drift_bug.cfg",
    "multilane_native_source_claim_coordinator_lane_incarnation_drift_bug.cfg",
    "multilane_native_source_claim_planned_coordinator_block_height_drift_bug.cfg",
    "multilane_native_source_claim_coordinator_lane_block_view_drift_bug.cfg",
    "multilane_native_source_claim_coordinator_proposal_hash_drift_bug.cfg",
    "multilane_native_source_claim_participant_lane_id_drift_bug.cfg",
    "multilane_native_source_claim_participant_dataspace_id_drift_bug.cfg",
    "multilane_native_source_claim_participant_lane_incarnation_drift_bug.cfg",
    "multilane_native_source_claim_participant_membership_drift_bug.cfg",
)
REVIEWED_MULTILANE_MUTATION_CONFIG_COUNT = 106
EXPECTED_CLOSURE_MUTATIONS = {
    "ML-MUT-NAT-01": (
        TLA_COUNTEREXAMPLE,
        "MLSeparateParticipantApplication",
        ("multilane_native_same_route_marker_bug.cfg",),
    ),
    "ML-MUT-NAT-02": (
        TLA_COUNTEREXAMPLE,
        "MLNativeSourceClaimInjective",
        NATIVE_SOURCE_CLAIM_MUTATION_CONFIGS,
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
            "multilane_native_repair_historical_sibling_as_active_bug.cfg",
            "multilane_native_prune_without_protected_latest_bug.cfg",
            "multilane_native_prune_namespace_rebind_bug.cfg",
        ),
    ),
    "ML-MUT-NAT-07": (
        TLA_COUNTEREXAMPLE,
        "MLNativeLatestIndexExact",
        (
            "multilane_native_ambiguous_latest_index_bug.cfg",
            "multilane_native_discard_authenticated_latest_temp_bug.cfg",
        ),
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
            "multilane_autonomous_prevote_commit_surface_drift_bug.cfg",
            "multilane_autonomous_event_prefix_drift_bug.cfg",
            "multilane_autonomous_post_validation_event_surface_drift_bug.cfg",
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
    "ML-MUT-AUT-12": (
        TLA_COUNTEREXAMPLE,
        "MLTerminalOutcomeJoinAuthenticated",
        (
            "multilane_autonomous_pending_only_canonical_terminal_bug.cfg",
            "multilane_autonomous_release_without_finalization_authority_bug.cfg",
            "multilane_autonomous_complete_without_queue_evidence_bug.cfg",
        ),
    ),
    "ML-MUT-AUT-13": (
        TLA_COUNTEREXAMPLE,
        "MLCanonicalTerminalBatchAtomic",
        (
            "multilane_autonomous_partial_terminal_unit_sweep_bug.cfg",
        ),
    ),
    "ML-MUT-AUT-14": (
        TLA_COUNTEREXAMPLE,
        "MLTerminalStartupSweepOrder",
        (
            "multilane_autonomous_owned_group_mutation_before_planner_bug.cfg",
            "multilane_autonomous_open_queue_before_deferred_carrier_apply_bug.cfg",
        ),
    ),
    "ML-MUT-AUT-15": (
        TLA_COUNTEREXAMPLE,
        "MLLocalProducerRecoveryRequiresQueueOwner",
        (
            "multilane_autonomous_producer_recovery_without_queue_owner_bug.cfg",
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
    "ML-MUT-API-02": reviewed_source.API_AUTHORITY_SEPARATION_SOURCE_PATHS,
    "ML-MUT-API-03": (
        "ci/run_native_amx_v2_grouped_sdk_parity.sh",
        "fixtures/sumeragi_v2/native_amx_v2_grouped.json",
        "python/iroha_python/tests/native_amx_v2_grouped_fixture_test.py",
    ),
    "ML-MUT-API-04": reviewed_source.FIXTURE_CANONICAL_OWNER_SOURCE_PATHS,
    "ML-MUT-WIRE-01": reviewed_source.WIRE_RELEASE_INVARIANT_SOURCE_PATHS,
}
CLOSURE_MUTATION_ID_RE = re.compile(r"`(ML-MUT-[A-Z]+-[0-9]{2})`")
FORBIDDEN_PRODUCTION_TOKENS = {
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "plan_lane_reservation_ownership",
    ): ("merge_ledger_all_entries",),
    (
        "crates/iroha_core/src/queue.rs",
        "finalize_conflicting_global_admission_locked",
    ): ("removed_hashes.insert",),
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
    native_merge_manifest.NATIVE_APPLICATION_MANIFEST_BINDING,
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
    native_merge_manifest.NATIVE_APPLICATION_MANIFEST_CLASSIFIER_MATCH_RELATION,
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
    native_merge_manifest.NATIVE_APPLICATION_MANIFEST_CLASSIFIER_ORDERED_SOURCE_CHECK,
)
NATIVE_PREPUBLICATION_BINDINGS = (
    *native_merge_manifest.NATIVE_MERGE_SOURCE_BINDINGS,
    native_merge_manifest.NATIVE_APPLICATION_MANIFEST_BINDING,
    *native_merge_manifest.NATIVE_MERGE_MANIFEST_CALLER_BINDINGS,
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "repair_native_amx_participant_application_evidence",
        (
            "prune_lock.lock",
            "ensure_prune_recovery_not_required",
            "native_amx_participant_application_evidence_for_block_under_publication_guard",
            "true,",
            "NativeAmxMergeAssociation::CommittedOnly",
            "persist_native_amx_participant_application_evidence_under_publication_guard",
            "NativeAmxParticipantApplicationPublicationMode::PostWsvRepair",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "prepublish_native_amx_participant_application_evidence",
        (
            "staged_merge_entry: Option<&MergeLedgerEntry>",
            "prune_lock.lock",
            "ensure_prune_recovery_not_required",
            "native_amx_participant_application_evidence_for_block_under_publication_guard",
            "false,",
            "NativeAmxMergeAssociation::Live(staged_merge_entry)",
            "persist_native_amx_participant_application_evidence_under_publication_guard",
            "NativeAmxParticipantApplicationPublicationMode::PreWsv",
        ),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "enum",
        "NativeAmxParticipantApplicationPublicationMode",
        ("PreWsv", "PostWsvRepair"),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "method",
        "NativeAmxParticipantApplicationPublicationMode::requires_post_apply_metadata",
        ("matches!(self, Self::PostWsvRepair)",),
    ),
    (
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
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
            "preflight_native_amx_participant_application_plan_under_publication_guard",
            "write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard",
            "read_back_native_amx_plan_manifests_under_publication_guard",
            "manifest_readback.authenticates",
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
            "require_native_amx_latest_index_temp_absent_locked",
            "recover_native_amx_evidence_publication_temp_locked",
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
            "require_native_amx_latest_index_temp_absent_locked",
            "recover_native_amx_evidence_publication_temp_locked",
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
            "validate_native_amx_participant_application_receipt_artifact",
            "manifest_artifact_hash",
            "finality_artifact_hash",
            "native_amx_participant_receipt_matches_manifest_leaf",
            "preflight.incoming",
            "ensure_prune_recovery_not_required",
            "get_durable_block_hash",
            "require_active_lane_artifact",
            "native_amx_evidence_namespace_for_entry",
            "preflight.current",
            "validate_native_amx_prepublication_transition_locked",
            "NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE",
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
        KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE,
        "enum",
        "NativeAmxLatestIndexTempReconciliation",
        ("Absent", "RemovedIdentical", "Promoted"),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "native_amx_evidence_special_file_bytes_locked",
        (
            "NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE",
            "NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE",
            "NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE",
            "NATIVE_AMX_EVIDENCE_PRUNE_INTENT_TEMP_FILE",
            "regular_sidecar_metadata_for",
            "empty or oversized payload",
            "checked_add",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "native_amx_evidence_tracked_bytes_locked",
        (
            "inventory_native_amx_evidence_files_locked",
            "let temporary_bytes = inventory",
            ".temporaries",
            ".values()",
            ".try_fold(0_u64",
            "native_amx_evidence_special_file_bytes_locked",
            "manifest_stable_bytes",
            "receipt_stable_bytes",
            "Native AMX evidence byte accounting overflowed",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "native_amx_latest_index_temp_bytes_locked",
        (
            "NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_TEMP_FILE",
            "read_bound_regular_file_bytes_locked",
            "NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_MAX_BYTES",
            "latest-index temporary",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "require_native_amx_latest_index_temp_absent_locked",
        (
            "native_amx_latest_index_temp_bytes_locked",
            "unresolved latest-index temporary",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "require_native_amx_latest_index_temp_recovery_unambiguous_locked",
        (
            "native_amx_latest_index_temp_bytes_locked",
            "NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE",
            "NATIVE_AMX_EVIDENCE_PRUNE_INTENT_TEMP_FILE",
            "inventory_native_amx_evidence_files_locked",
            "ambiguously overlaps evidence pruning",
            "ambiguously overlaps evidence publication",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "reconcile_native_amx_latest_index_temp_locked",
        (
            "native_amx_latest_index_temp_bytes_locked",
            "decode_native_amx_participant_receipt_latest_index_bytes_for_route",
            "expected.filter(|_| expected_can_publish)",
            "authenticated_complete.get(&temporary.lane_block_height)",
            "if stable == temporary",
            "open_bound_regular_file_with_exact_bytes_locked",
            "verify_bound_open_regular_file_exact_bytes_locked",
            "remove_bound_progress_file_if_matches",
            "authenticated_complete.get(&stable.lane_block_height)",
            "stable.lane_block_height >= temporary.lane_block_height",
            "sync_native_amx_latest_index_recovery_temp",
            "promote_bound_progress_temp(",
            "promote_bound_progress_temp_noreplace(",
            "sync_native_amx_evidence_namespace",
            "failed exact durable read-back",
            "NativeAmxLatestIndexTempReconciliation::Promoted",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "rebuild_native_amx_participant_receipt_latest_indexes_on_startup",
        (
            "native_amx_evidence_namespace_for_entry",
            "native_amx_latest_index_temp_bytes_locked",
            "require_native_amx_latest_index_temp_recovery_unambiguous_locked",
            "complete_native_amx_evidence_prune_intent_locked",
            "recover_native_amx_evidence_publication_temp_locked",
            "inventory_native_amx_evidence_files_locked",
            "decode_native_amx_manifest_file_locked",
            "decode_native_amx_receipt_file_locked",
            "difference(&manifest_payload_heights)",
            "difference(&receipt_payload_heights)",
            "authenticated_complete",
            "reconcile_native_amx_latest_index_temp_locked",
            "NativeAmxLatestIndexTempReconciliation::Promoted",
            "decode_bound_native_amx_participant_receipt_latest_index_locked",
            "current.matches_receipt",
            "current.matches_manifest",
            "is not backed by its exact receipt or QC-authenticated manifest",
            "persist_native_amx_participant_receipt_latest_index_from_reconstructed_inventory_locked",
            "prune_native_amx_evidence_pairs_locked",
            "progress_mutation_namespace_unchanged",
            "update_disk_usage_delta",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "replace_bound_native_amx_latest_index_locked",
        (
            "path.parent() != Some(directory)",
            "read_bound_regular_file_bytes_locked",
            "require_native_amx_latest_index_temp_absent_locked",
            "create_new_bound_progress_temp",
            "sync_all",
            "promote_bound_progress_temp",
            "sync_native_amx_evidence_namespace",
            "persisted != bytes",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_and_apply",
        (
            "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry",
            "state_block.staged_merge_entry()",
            "execution_commitment_from_validated_block",
            "store_block",
            "store_v2_finality_artifact",
            "prepublish_native_amx_participant_application_evidence",
            "State::native_amx_participant_frontier_markers_and_merge_entry",
            "token.authenticates_state_frontiers",
            "apply_without_execution_with_verified_v2_finality",
            "let staged_merge_queue_reservation_hashes = certified_merge_queue_reservation_hashes(",
            "state_block.staged_merge_entry(),",
            "!staged_merge_queue_reservation_hashes.contains(entrypoint_hash)",
            "pending_autoscale_retirement_binding",
            "Box<dyn StateBlockCommitAuthorization>",
            "Box::new(checked_carrier_applications)",
            "if carries_scale_in",
            "lock_lane_retirement_observer",
            "commit_with_state_commit_authorization_and_autoscale_retirement_queue_veto",
            "commit_with_state_commit_authorization",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "commit_inner",
        (
            "state_commit_authorization: Option<Box<dyn StateBlockCommitAuthorization>>",
            "let _state_commit_lock = state_ref.state_commit_lock.lock();",
            "let autoscale_lifecycle_guard",
            "autoscale_retirement_queue_veto.as_mut()",
            "state_commit_authorization.take()",
            ".consume_for_state_commit(",
            "State commit authorization rejected the exact carrier transition",
            "apply_committed_autoscale_lane_geometry",
            "transactions.commit()",
        ),
    ),
) + reviewed_source.NATIVE_PREPUBLICATION_REVIEWED_BINDINGS
NATIVE_PREPUBLICATION_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_and_apply",
        (
            ".store_v2_finality_artifact(artifact)",
            ".prepublish_native_amx_participant_application_evidence(",
            "State::native_amx_participant_frontier_markers_and_merge_entry(",
            "token.authenticates_state_frontiers(",
            ".apply_without_execution_with_verified_v2_finality(&committed_block)",
            ".pending_autoscale_retirement_binding()",
            "Box::new(checked_carrier_applications)",
            "if carries_scale_in {",
            "self.queue.lock_lane_retirement_observer()",
            ".commit_with_state_commit_authorization_and_autoscale_retirement_queue_veto(",
            "state_block.commit_with_state_commit_authorization(state_commit_authorization)",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "commit_inner",
        (
            "let _state_commit_lock = state_ref.state_commit_lock.lock();",
            "let autoscale_lifecycle_guard",
            "autoscale_retirement_queue_veto.as_mut()",
            "state_commit_authorization.take()",
            ".consume_for_state_commit(",
            "state_ref.apply_committed_autoscale_lane_geometry(",
            "transactions.commit()",
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
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "replace_bound_native_amx_latest_index_locked",
        (
            "let _ = self.read_bound_regular_file_bytes_locked(",
            "require_native_amx_latest_index_temp_absent_locked(namespace)",
            "create_new_bound_progress_temp(namespace, &temp_path)",
            ".write_all(bytes)",
            ".sync_all()",
            "promote_bound_progress_temp(namespace, &temp_path, path, &temporary)",
            "sync_native_amx_evidence_namespace(namespace",
            "let persisted = self",
            "if persisted != bytes",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "rebuild_native_amx_participant_receipt_latest_indexes_on_startup",
        (
            "let latest_temp_present = self",
            "require_native_amx_latest_index_temp_recovery_unambiguous_locked(",
            "complete_native_amx_evidence_prune_intent_locked(&entry, &namespace)",
            "recover_native_amx_evidence_publication_temp_locked(",
            "let inventory = self.inventory_native_amx_evidence_files_locked",
            "let mut validated_manifests = BTreeMap::new()",
            "let mut validated_receipts = BTreeMap::new()",
            "validate_native_amx_retained_history_continuity(",
            "let mut authenticated_complete = BTreeMap::new()",
            "reconcile_native_amx_latest_index_temp_locked(",
            "let current = self.decode_bound_native_amx_participant_receipt_latest_index_locked",
            "match (expected, current)",
            "prune_native_amx_evidence_pairs_locked(&entry, &namespace)",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "reconcile_native_amx_latest_index_temp_locked",
        (
            "let Some(temp_bytes) = self.native_amx_latest_index_temp_bytes_locked",
            "let temporary = Self::decode_native_amx_participant_receipt_latest_index_bytes_for_route",
            "let expected = expected.filter(|_| expected_can_publish)",
            "authenticated_complete.get(&temporary.lane_block_height)",
            "let stable_bytes = self.read_bound_regular_file_bytes_locked",
            "if stable == temporary",
            "remove_bound_progress_file_if_matches(",
            "authenticated_complete.get(&stable.lane_block_height)",
            "sync_native_amx_latest_index_recovery_temp(&temporary_file)",
            "let promotion = if stable.is_some()",
            "promotion.map_err",
            "failed exact durable read-back",
            "NativeAmxLatestIndexTempReconciliation::Promoted",
        ),
    ),
) + reviewed_source.NATIVE_PREPUBLICATION_REVIEWED_ORDERED_SOURCE_CHECKS
NATIVE_LATEST_TEMP_RECONCILIATION_FORBIDDEN_TOKENS = (
    "discard_native_amx_latest_index_temp_locked",
    "remove_bound_progress_temp_if_present",
    "std::fs::remove_file",
    ".write_all(",
    ".set_len(",
)
NATIVE_EXACT_OBJECT_PRUNE_BINDINGS = (
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "open_bound_regular_file_with_exact_bytes_locked",
        (
            "regular_sidecar_metadata_for",
            "len == 0 || len > max_bytes || len != expected_bytes.len()",
            "open_bound_progress_file(namespace, path, &metadata)",
            "verify_bound_open_regular_file_exact_bytes_locked",
            "Ok((file, metadata))",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "verify_bound_open_regular_file_exact_bytes_locked",
        (
            "file.seek(SeekFrom::Start(0))",
            ".take(u64::try_from(max_bytes)?.saturating_add(1))",
            ".read_to_end(&mut readback)",
            "let opened = secure_file_metadata::from_file(file)",
            ".map_err(|error| Error::IO(error, path.to_path_buf()))?",
            "regular_sidecar_metadata_for",
            "readback != expected_bytes",
            "sidecar_file_metadata_unchanged(&metadata.file, &opened)",
            "stable_sidecar_metadata_unchanged(&metadata, &current)",
            "progress_mutation_namespace_unchanged(namespace)",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked",
        (
            "file.seek(SeekFrom::Start(0))",
            ".take(u64::try_from(max_bytes)?.saturating_add(1))",
            ".read_to_end(&mut readback)",
            "let opened = secure_file_metadata::from_file(file)",
            ".map_err(|error| Error::IO(error, path.to_path_buf()))?",
            "regular_sidecar_metadata_for",
            "readback != expected_bytes",
            "metadata.canonical_path != current.canonical_path",
            "sidecar_file_metadata_unchanged(&metadata.file, &opened)",
            "sidecar_file_metadata_unchanged(&metadata.file, &current.file)",
            "sidecar_directory_binding_unchanged",
            "progress_mutation_namespace_unchanged(namespace)",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "remove_bound_progress_file_if_matches",
        (
            "path.parent() != Some(immediate.expected_path.as_path())",
            "let expected_metadata = secure_file_metadata::from_file(expected)?",
            "sidecar_file_metadata_unchanged",
            "rustix::fs::statat",
            "rustix::fs::AtFlags::SYMLINK_NOFOLLOW",
            "rustix::fs::FileType::RegularFile",
            "expected_metadata.nlink() != 1",
            "entry.st_dev as u64 != expected_snapshot.file.dev()",
            "entry.st_ino as u64 != expected_snapshot.file.ino()",
            "entry.st_nlink as u64 != 1",
            "rustix::fs::unlinkat",
            "let current = secure_file_metadata::from_path(path)?",
            "current.file_type().is_symlink()",
            "sidecar_is_single_link(&current)",
            "std::fs::remove_file(path)",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "complete_native_amx_evidence_prune_intent_locked",
        (
            "recover_native_amx_evidence_prune_intent_publication_locked",
            "decode_native_amx_evidence_prune_intent_bytes",
            "validate_native_amx_evidence_prune_intent_locked",
            "Hash::new(&artifact_bytes)",
            "open_bound_regular_file_with_exact_bytes_locked",
            "verify_bound_open_regular_file_exact_bytes_locked",
            "verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked",
            "remove_bound_progress_file_if_matches",
            "sync_native_amx_evidence_namespace",
        ),
    ),
)
NATIVE_EXACT_OBJECT_PRUNE_CALL_COUNTS = (
    (
        "complete_native_amx_evidence_prune_intent_locked",
        (
            ("open_bound_regular_file_with_exact_bytes_locked(", 2),
            ("verify_bound_open_regular_file_exact_bytes_locked(", 1),
            (
                "verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked(",
                1,
            ),
            ("remove_bound_progress_file_if_matches(", 2),
        ),
    ),
    (
        "reconcile_native_amx_latest_index_temp_locked",
        (
            ("open_bound_regular_file_with_exact_bytes_locked(", 2),
            ("verify_bound_open_regular_file_exact_bytes_locked(", 2),
            ("remove_bound_progress_file_if_matches(", 1),
        ),
    ),
)
NATIVE_PREPUBLICATION_RETENTION_WRITERS = (
    "write_native_amx_participant_application_manifest_artifact_with_retention_policy_under_publication_guard",
    "write_native_amx_participant_application_receipt_artifact_only_with_retention_policy_under_publication_guard",
    "write_native_amx_participant_receipt_latest_index_for_prepublication_under_publication_guard",
)
QUEUE_PLAN_PENDING_MEMBERSHIP_MODULE = "SumeragiV2QueuePlanAdmissionRegistry"
QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE = "crates/iroha_core/src/state.rs"
QUEUE_PLAN_PENDING_MEMBERSHIP_HOST_RELATIVE = (
    "crates/iroha_core/src/smartcontracts/ivm/host.rs"
)
QUEUE_PLAN_PENDING_OPAQUE_PREFIXES = (
    "queue_plan_pending_obligation_v1_",
    "queue_plan_pending_route_member_v1_",
)
QUEUE_PLAN_PENDING_MEMBERSHIP_BINDINGS = (
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "struct",
        "QueuePlanPendingRouteMemberV1",
        (
            "version",
            "route",
            "network_id_digest",
            "entrypoint_hash",
            "binding_hash",
            "member_identity",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_pending_route_member_identity",
        (
            "Self::validate_queue_plan_pending_obligation_route(&route)?",
            "obligation.version != QUEUE_PLAN_PENDING_OBLIGATION_VERSION_V1",
            "obligation.network_id_digest",
            "obligation.entrypoint_hash",
            "obligation.binding_hash",
            ".routes.binary_search(&route).is_err()",
            "Self::queue_plan_pending_route_member_identity_from_claim(",
            "obligation.network_id_digest",
            "obligation.entrypoint_hash.clone()",
            "obligation.binding_hash",
            "route",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_pending_route_member_identity_from_claim",
        (
            "entrypoint_hash: HashOf<TransactionEntrypoint>",
            "Self::validate_queue_plan_pending_obligation_route(&route)?",
            "network_id_digest.as_ref().iter().all(|byte| *byte == 0)",
            "entrypoint_hash.as_ref().iter().all(|byte| *byte == 0)",
            "binding_hash.as_ref().iter().all(|byte| *byte == 0)",
            "norito::to_bytes(&(",
            "QUEUE_PLAN_PENDING_OBLIGATION_VERSION_V1",
            "network_id_digest",
            "entrypoint_hash",
            "binding_hash",
            "route",
            "QUEUE_PLAN_PENDING_ROUTE_MEMBER_DOMAIN_V1",
            "Ok(*identity.as_ref())",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_pending_route_member_from_obligation",
        (
            "version: QUEUE_PLAN_PENDING_ROUTE_MEMBER_VERSION_V1",
            "route",
            "network_id_digest: obligation.network_id_digest",
            "entrypoint_hash: obligation.entrypoint_hash.clone()",
            "binding_hash: obligation.binding_hash",
            "queue_plan_pending_route_member_identity(obligation, route)?",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_pending_route_member_marker_prefix",
        (
            "QUEUE_PLAN_PENDING_ROUTE_MEMBER_MARKER_PREFIX",
            "route.lane_id.as_u32()",
            "route.dataspace_id.as_u64()",
            "route.lane_incarnation.as_ref()",
            "let start = literal.parse()",
            "Ok((literal, start))",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_pending_route_member_marker_payload",
        (
            "marker.version != QUEUE_PLAN_PENDING_ROUTE_MEMBER_VERSION_V1",
            "marker.network_id_digest",
            "marker.entrypoint_hash",
            "marker.binding_hash",
            "marker.member_identity",
            "Self::queue_plan_pending_route_member_identity_from_claim(",
            "marker.member_identity != expected_identity",
            "payload.len() > MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES",
        ),
    ),
    (QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE, "fn", "decode_exact_queue_plan_pending_route_member_marker", (
            "payload.is_empty() || payload.len() > MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES",
            "norito::decode_from_bytes::<QueuePlanPendingRouteMemberV1>(payload)",
            "queue_plan_pending_route_member_marker_payload",
            "queue_plan_pending_route_member_marker_key",
            "canonical.as_slice() != payload || &expected_key != key",
    )),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "require_queue_plan_pending_route_member_marker",
        (
            "StorageReadOnly<StatePath, Vec<u8>>",
            "queue_plan_pending_route_member_from_obligation",
            "queue_plan_pending_route_member_marker_key",
            "storage.get(&key).ok_or_else",
            "decode_exact_queue_plan_pending_route_member_marker",
            "marker != expected",
            "Ok((key, marker))",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_pending_route_members_from_storage",
        (
            "queue_plan_pending_route_members_from_storage_with_limit",
            "MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_pending_route_members_from_storage_with_limit",
        (
            "queue_plan_pending_route_member_marker_prefix",
            "storage.range(start..)",
            "key.as_ref().starts_with(&prefix)",
            "members.len() == max_members",
            "decode_exact_queue_plan_pending_route_member_marker",
            "marker.route != route",
            "queue_plan_pending_obligation_marker_key",
            "storage.get(&obligation_key).is_none()",
            "members.push((key.clone(), marker))",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_pending_route_obligation_count_from_world",
        (
            "queue_plan_pending_route_members_from_storage",
            ".len()",
            "u64::try_from(count)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "validate_queue_plan_pending_obligation_route_member_in_storage",
        (
            "queue_plan_pending_route_members_from_storage(storage, route)?",
            "require_queue_plan_pending_route_member_marker(storage, obligation, route)?",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "enum",
        "QueuePlanAdmissionApplicationState",
        (
            "Pending",
            "PendingStale",
            "Applied",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_pending_obligation_matches_active_lifecycle",
        (
            "obligation.binding.admission_context.proposal_height",
            "obligation.routes.iter().all",
            "state",
            ".nexus()",
            ".lane_catalog",
            ".lanes()",
            ".iter()",
            "lane.id == route.lane_id",
            "lane.dataspace_id == route.dataspace_id",
            "state.lane_incarnation_at_height(route.lane_id, proposal_height)",
            "Some(route.lane_incarnation)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_registry_owner_application_state_in_view",
        (
            "validate_queue_plan_pending_obligation_route_member",
            "pending obligation `{key}` survived canonical transaction membership",
            "queue_plan_pending_obligation_matches_active_lifecycle",
            "QueuePlanAdmissionApplicationState::Pending",
            "QueuePlanAdmissionApplicationState::PendingStale",
            "None if committed => Ok(QueuePlanAdmissionApplicationState::Applied)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_binding_application_state",
        (
            "state.has_entrypoint",
            "queue_plan_pending_obligation_matches_active_lifecycle",
            "queue_plan_binding_application_state_in_storage",
            "expected",
            "committed",
            "active",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_binding_application_state_in_storage",
        (
            "current != expected",
            "validate_queue_plan_pending_obligation_route_member_in_storage",
            "pending-obligation marker `{key}` survived canonical transaction membership",
            "QueuePlanAdmissionApplicationState::Pending",
            "QueuePlanAdmissionApplicationState::PendingStale",
            "None if committed => Ok(QueuePlanAdmissionApplicationState::Applied)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_admission_registry_entrypoint_present",
        (
            "queue_plan_registry_owner_application_state_in_view",
            "application_state == QueuePlanAdmissionApplicationState::PendingStale",
            "retired or recreated lane incarnation",
            "Ok(true)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_admission_binding_registry_match",
        (
            "queue_plan_binding_application_state",
            "application_state == QueuePlanAdmissionApplicationState::PendingStale",
            "retired or recreated lane incarnation",
            "Ok(registry_match)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "classify_pending_queue_plan_admission",
        (
            "pending_queue_plan_admission_registry_lookup",
            "QueuePlanAdmissionApplicationState::PendingStale",
            "PendingQueuePlanAdmissionDisposition::Stale",
            "QueuePlanAdmissionApplicationState::Pending",
            "QueuePlanAdmissionApplicationState::Applied",
            "PendingQueuePlanAdmissionDisposition::Exact",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_admission_registry_match_with_application_state_in_view",
        (
            "QueuePlanAdmissionRegistryKeyV1",
            "decode_exact_queue_plan_admission_registry_marker",
            "queue_plan_registry_owner_application_state_in_view",
            "QueuePlanAdmissionRegistryMatch::Absent",
            "QueuePlanAdmissionRegistryMatch::Exact",
            "QueuePlanAdmissionRegistryMatch::Conflict",
            "Some(application_state)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "stage_queue_plan_pending_obligation_marker_in_storage",
        (
            "&mut impl QueuePlanMarkerStorage",
            "validate_queue_plan_pending_obligation_route_member_in_storage",
            "queue_plan_pending_route_members_from_storage",
            "members.len() == MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS",
            "queue_plan_pending_route_member_from_obligation",
            "queue_plan_pending_route_member_marker_key",
            "decode_exact_queue_plan_pending_route_member_marker",
            "queue_plan_pending_route_member_marker_payload",
            "route_updates.push",
            "insert_queue_plan_marker(obligation_key, obligation_payload)",
            "insert_queue_plan_marker(member_key, member_payload)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "resolve_queue_plan_pending_obligation_in_storage",
        (
            "&mut impl QueuePlanMarkerStorage",
            "decode_exact_queue_plan_pending_obligation_marker",
            "decode_exact_queue_plan_admission_registry_marker",
            "queue_plan_pending_route_members_from_storage",
            "require_queue_plan_pending_route_member_marker",
            "member_keys.push",
            "remove_queue_plan_marker(obligation_key)",
            "remove_queue_plan_marker(member_key)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "validate_queue_plan_admissions_for_carrier_in_view",
        (
            "state_view: &impl StateReadOnly",
            "decode_and_validate_queue_plan_admission_certificate_v1(",
            "state_view.network_id()",
            "state_view.block_hashes().get(index).copied()",
            "exact_predecessor != context.predecessor_block_hash",
            "queue_plan_authoritative_peers_in_view_at_height(",
            "state_view,",
            "authority.as_ref().ok() != Some(&route.validator_set)",
        ),
    ),
    (QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE, "fn", "stage_queue_plan_admissions_for_carrier", (
            "ensure_pristine_execution_control_stage",
            "queue_plan_active_lane_bindings",
            "stage_queue_plan_admissions",
            "staged_queue_plan_admissions",
    )),
    (QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE, "fn", "stage_queue_plan_admissions", (
            "validate_queue_plan_admissions_for_carrier_in_view",
            "queue_plan_pending_obligation_from_admission",
            "queue_plan_pending_obligation_matches_active_lifecycle",
            ".collect::<Result<Vec<_>, MergeLedgerCommitError>>()?",
            "self.world.smart_contract_state.transaction()",
            "queue_plan_binding_application_state_in_storage",
            "markers.insert_queue_plan_marker(key, payload)",
            "stage_queue_plan_pending_obligation_in_storage",
            "markers.apply()",
    )),
    (QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE, "fn", "resolve_queue_plan_pending_obligations_for_entrypoints", (
            "self.world.smart_contract_state.transaction()",
            "for entrypoint_hash in entrypoint_hashes",
            "resolve_queue_plan_pending_obligation_in_storage",
            "markers.apply()",
    )),
)
QUEUE_PLAN_PENDING_QUEUE_OWNERSHIP_FREE_FN = (
    QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
    "queue_plan_admission_registry_match",
    (
        "State::queue_plan_admission_registry_match_with_application_state_in_view(",
        "if registry_match == QueuePlanAdmissionRegistryMatch::Exact",
        "&& application_state != Some(QueuePlanAdmissionApplicationState::Pending)",
        "cannot authorize new Queue ownership",
        "Ok(registry_match)",
    ),
)
QUEUE_PLAN_PENDING_MEMBERSHIP_ORDERED_SOURCE_CHECKS = (
    *(
        binding for binding in QUEUE_PLAN_PENDING_MEMBERSHIP_BINDINGS
        if binding[2] in (
            "decode_exact_queue_plan_pending_route_member_marker",
            "validate_queue_plan_admissions_for_carrier_in_view",
            "stage_queue_plan_admissions",
            "resolve_queue_plan_pending_obligations_for_entrypoints",
        )
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "resolve_required_queue_plan_pending_obligations",
        (
            "self.world.smart_contract_state.transaction()",
            "for (entrypoint_hash, expected_binding_hash) in pending_obligations",
            "decode_exact_queue_plan_pending_obligation_marker",
            "obligation.binding_hash != expected_binding_hash",
            "resolve_queue_plan_pending_obligation_in_storage",
            "markers.apply()",
        ),
    ),
    (QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE, "fn", "queue_plan_pending_obligation_matches_active_lifecycle", (
            "let proposal_height = obligation.binding.admission_context.proposal_height;",
            "obligation.routes.iter().all(|route| {",
            "state\n                .nexus()\n                .lane_catalog\n                .lanes()",
            ".any(|lane| lane.id == route.lane_id && lane.dataspace_id == route.dataspace_id)",
            "state.lane_incarnation_at_height(route.lane_id, proposal_height)",
            "== Some(route.lane_incarnation)",
    )),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_registry_owner_application_state_in_view",
        (
            "Self::decode_exact_queue_plan_pending_obligation_marker(&key, payload)?;",
            "Self::validate_queue_plan_pending_obligation_route_member(",
            "if committed {",
            "Self::queue_plan_pending_obligation_matches_active_lifecycle(",
            "Ok(QueuePlanAdmissionApplicationState::Pending)",
            "Ok(QueuePlanAdmissionApplicationState::PendingStale)",
            "None if committed => Ok(QueuePlanAdmissionApplicationState::Applied)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_binding_application_state",
        (
            "let committed = state.has_entrypoint(",
            "Self::queue_plan_pending_obligation_matches_active_lifecycle(state, &expected);",
            "Self::queue_plan_binding_application_state_in_storage(",
            "expected,",
            "committed,",
            "active,",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "queue_plan_binding_application_state_in_storage",
        (
            "Self::decode_exact_queue_plan_pending_obligation_marker(&key, payload)?;",
            "if current != expected {",
            "Self::validate_queue_plan_pending_obligation_route_member_in_storage(",
            "if committed {",
            "if active {",
            "Ok(QueuePlanAdmissionApplicationState::Pending)",
            "Ok(QueuePlanAdmissionApplicationState::PendingStale)",
            "None if committed => Ok(QueuePlanAdmissionApplicationState::Applied)",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
        "fn",
        "classify_pending_queue_plan_admission",
        (
            "QueuePlanAdmissionApplicationState::PendingStale => {\n"
            "                        PendingQueuePlanAdmissionDisposition::Stale\n"
            "                    }",
            "QueuePlanAdmissionApplicationState::Pending\n"
            "                    | QueuePlanAdmissionApplicationState::Applied => {",
            "PendingQueuePlanAdmissionDisposition::Exact",
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
            "IrohaNetwork::start_with_crypto_and_initial_authorities(",
        ),
    ),
)
QUEUE_PLAN_PENDING_MEMBERSHIP_TEST_BINDINGS = (
    (
        "crates/iroha_core/src/state/autonomous_merge_and_queue_plan_tests.rs",
        "queue_plan_native_staging_is_an_exact_idempotent_compare_and_set",
        ("assert_queue_plan_native_exact_compare_and_set()", "assert_queue_plan_native_multi_route_preflight_is_atomic()",
         "assert_queue_plan_native_batch_rollback_is_atomic()"),
    ),
    (
        "crates/iroha_core/src/state/autonomous_merge_and_queue_plan_tests.rs", "assert_queue_plan_native_multi_route_preflight_is_atomic",
        ("a later-route orphan member must abort the whole obligation stage",
         "failed stage preflight must preserve the orphan marker for diagnosis"),
    ),
    (
        "crates/iroha_core/src/state/autonomous_merge_and_queue_plan_tests.rs", "assert_queue_plan_native_batch_rollback_is_atomic",
        ("a second admission failure must roll back every earlier admission", "failed whole-list staging must restore the exact prior overlay",
         "failed whole-list staging leaked marker `{key}`", "a later proposal-native carrier can retry after the conflict is repaired"),
    ),
    (
        "crates/iroha_core/src/state/autonomous_merge_and_queue_plan_tests.rs",
        "queue_plan_pending_resolution_corrupt_route_counts_fail_without_partial_mutation",
        (
            "failed resolution must retain the exact pending obligation",
            "failed resolution must not partially remove any exact route member",
            "a later-route failure must roll back an earlier successful resolution",
            "failed whole-list resolution must restore the exact prior overlay",
            "failed whole-list resolution removed `{key}`",
            "the same StateBlock remains reusable after resolution rollback",
        ),
    ),
    (
        QUEUE_PLAN_PENDING_MEMBERSHIP_HOST_RELATIVE,
        "contract_state_namespace_access_covers_consensus_owned_prefixes",
        (
            "queue_plan_pending_obligation_v1_deadbeef_cafebabe",
            "queue_plan_pending_route_member_v1_0_0_deadbeef_cafebabe",
            "ContractStateNamespaceAccess::OpaqueSystem",
            "must remain opaque to generic contract state syscalls",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "queue_plan_journal_replay_retains_entrypoint_that_fails_stateless_revalidation",
        (
            "expect_err(\"wrong-network journal entrypoint must fail startup\")",
            "failed canonical stateless validation",
            "assert!(!replay_queue.txs.contains_key(&hash));",
            "live_record_count()",
            "stateless failure must not append a tombstone or replacement",
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
        reviewed_source._validate_exact_release_invariant_source_checks(
            mutation_id, source_checks, errors
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
            path, source = _read_reviewed_rust_source(
                root, relative, "release invariant source check", errors
            )
            if source is None:
                continue
            for token in tokens:
                current_token = _RELEASE_SOURCE_TOKEN_REBINDINGS.get(
                    (relative, token), token
                )
                if current_token not in source:
                    errors.append(
                        f"{path}: release invariant {obligation} is missing "
                        f"source-binding token {current_token!r}"
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
    if len(model_configs) != REVIEWED_MULTILANE_MUTATION_CONFIG_COUNT:
        errors.append(
            "reviewed multilane mutation inventory must contain "
            f"{REVIEWED_MULTILANE_MUTATION_CONFIG_COUNT} configs, "
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


@lru_cache(maxsize=64)
def _indexed_rust_binding_items(
    source: str, kind: str
) -> tuple[tuple[str, str], ...]:
    """Index one reviewed source once for all bindings of the same item kind."""

    declaration_re = re.compile(
        RUST_DECLARATION_TEMPLATES[kind].format(
            symbol=r"(?P<binding_name>[A-Za-z_][A-Za-z0-9_]*)"
        )
    )
    return tuple(
        (declaration.group("binding_name"), item)
        for declaration in declaration_re.finditer(source)
        if (item := _extract_braced_item(source, declaration)) is not None
    )


@lru_cache(maxsize=64)
def _rust_impl_items(source: str, owner: str) -> tuple[str, ...]:
    """Extract every inherent or trait implementation for one exact owner."""

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
    return tuple(
        item
        for declaration in impl_re.finditer(source)
        if (item := _extract_braced_item(source, declaration)) is not None
    )


@lru_cache(maxsize=256)
def _extract_rust_binding_items(
    source: str, kind: str, symbol: str
) -> tuple[str, ...]:
    """Extract exact free items or `Type::method` items from Rust source."""

    if kind != "method":
        return tuple(
            item
            for name, item in _indexed_rust_binding_items(source, kind)
            if name == symbol
        )

    if symbol.count("::") != 1:
        return ()
    owner, method = symbol.split("::", 1)
    if not owner or not method:
        return ()
    return tuple(
        item
        for impl_item in _rust_impl_items(source, owner)
        for name, item in _indexed_rust_binding_items(impl_item, "fn")
        if name == method
    )


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
        f'readonly APALACHE_VERSION="{APALACHE_VERSION}"', f'readonly APALACHE_LAUNCHER_SHA256="{APALACHE_LAUNCHER_SHA256}"',
        f'readonly APALACHE_JAR_SHA256="{APALACHE_JAR_SHA256}"',
        'readonly CONTRACT_CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_models.py"',
        'readonly RUNNER_CONTRACT_TEST="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_apalache_runner_contract.py"',
        'readonly EVIDENCE_PATH="${EVIDENCE_DIR}/multilane_apalache_evidence.tsv"',
        'workspace_source_manifest_sha256="${IROHA_RELEASE_SOURCE_MANIFEST_SHA256:-}"',
        'readonly workspace_source_manifest_sha256',
        'if [[ ! "$workspace_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then',
        'readonly KURA_RETENTION_MODULE="SumeragiV2KuraReplicaRetention"',
        '\npython3 -I -S "$CONTRACT_CHECKER"\n',
        'python3 -I -S "$RUNNER_CONTRACT_TEST"',
        'tool_version="$("$RESOLVED_APALACHE_BIN" version)"', 'run_typecheck "$KURA_RETENTION_MODULE"',
        '[[ "$tool_version" != "$APALACHE_VERSION" ]]', '"$RESOLVED_APALACHE_BIN" --out-dir="$out" typecheck "${module}.tla"',
        '"$RESOLVED_APALACHE_BIN" --out-dir="$out" check', "--algo=incremental",
        '--config="$config"', '--length="$length"', "--no-deadlock",
        'grep -Fc "The outcome is: NoError"',
        'grep -Fc "Checker reports no error up to computation length ${length}"',
        'echo "multilane formal or production sources changed during the Apalache run"',
        'if [[ "$final_multilane_source_manifest_sha256" != "$multilane_source_manifest_sha256" ]]; then',
        "printf 'schema_version\\t2\\n'",
        "printf 'workspace_source_manifest_sha256\\t%s\\n' \"$workspace_source_manifest_sha256\"",
        "printf 'multilane_source_manifest_sha256\\t%s\\n' \"$multilane_source_manifest_sha256\"",
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
  8 \\
  "NativeEvidenceTypeInvariant, NativeStandaloneEvidenceInvariant, NativeEvidenceRetentionBoundInvariant, MLNativeSharedEvidenceBudget, MLNativeSingleIncomingPairHeadroom, MLNativeTempPromotionAuthenticated, MLNativeRetainedHistoryExact, MLNativePruneOldestPrefix, MLNativePruneProtectedLatestExact, MLNativePruneExactObjectRemoval, NativeNoClobberPublicationInvariant, NativeLegacyDenseRejectedInvariant, NativePruneJournalInvariant, SidecarsRequireManifestInvariant, FrontierPublicationInvariant, PrunedEvidenceVerifiableInvariant, SameRouteControlOnlyInvariant, MLSeparateParticipantApplication, MLNativeSourceClaimInjective, MLNativeContiguousActiveRoute, MLNativeGroupExactCover, MLNativeManifestAuthenticates, MLUnifiedStartupEvidenceRepairSafe, MLNativeDurabilityPrecedesFrontier, MLNativeLatestIndexExact\"""",
        """run_positive \\
  autonomous-reservation-carrier \\
  "$AUTONOMOUS_MODULE" \\
  multilane_autonomous_reservation_carrier_fixed.cfg \\
  12 \\
  "ReservationCarrierTypeInvariant, SingleOwnershipInvariant, ExactCarrierIdentityInvariant, ControlOnlyAnchorInvariant, CandidateAuthorizationInvariant, ReleaseOrderingInvariant, QueueReleaseCompletionInvariant, AtMostOnceApplicationInvariant, NoReleaseAfterApplicationInvariant, NoStaleIncarnationReleaseInvariant, ForgottenOnlyAfterApplicationInvariant, MLReservationSingleOwner, MLReservationIdentityStable, MLCertifiedBundleDurable, MLMergeCandidateExactPrefix, MLCarrierCommitSurfaceExact, MLCarrierExactlyOnce, MLRestartOwnershipPartition, MLRecoveredCarrierBodyAuthenticated, MLRecoveredCarrierLengthAuthenticated, MLHistoricalRecoveryContextExact, MLHistoricalQueueGateOrder, MLHistoricalAllGroupsPreflight, MLLocalProducerRecoveryRequiresQueueOwner, MLTerminalOutcomeJoinAuthenticated, MLCanonicalTerminalBatchAtomic, MLTerminalStartupSweepOrder, MLStageEvidenceMonotonic\"""",
        """run_positive \\
  queue-plan-admission-registry \\
  "$QUEUE_PLAN_ADMISSION_MODULE" \\
  multilane_queue_plan_admission_registry_fixed.cfg \\
  8 \\
  "QueuePlanAdmissionTypeInvariant, MLAdmissionCasUnique, MLCertificateDurable, MLPublic202Exact, MLExecutionRequiresExactBinding, MLQueuePlanExecutionAutonomousOnly, MLQueueEligibilityExact, MLAdmissionAtMostOnceExecution, MLImmutableAdmissionTombstone, MLCancellationStopsExecution\"""",
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
  "FirstReleaseTypeInvariant, MLPayloadSchemaV2CarriesExactAdmissionPreimage, MLValidatorCarrierOwnership, MLSelectedQueuePlanV1ConjunctionBeforeReservationV1, MLReservationV1BeforeKuraActive, MLKuraActiveBeforeExecutionInput, MLExecutionInputBeforeReadyAuthorization, MLReadyAuthorizationBeforeLocalSignature, MLLocalSignaturesBeforeDurableReadyQc, MLCrashDurableFactsRecoverable, MLVolatileSessionLostOnCrash, MLCommitAndReleaseRetainExactScope, MLLaneCommitBeforeAtomicWsvCarrierApplication, MLExactlyOnceCarrierApplication, MLPostCarrierCommitCleanupOrder, MLReleasePrefixesRecoverable, MLReleaseStageOrder, MLQueuePlanV1SelectedConjunctionBound4096\"""",
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
        *NATIVE_SOURCE_CLAIM_MUTATION_CONFIGS,
        "multilane_native_noncontiguous_route_bug.cfg",
        "multilane_native_partial_group_application_bug.cfg",
        "multilane_native_forged_manifest_leaf_bug.cfg",
        "multilane_native_dropped_startup_repair_bug.cfg",
        "multilane_native_ambiguous_latest_index_bug.cfg",
        "multilane_native_discard_authenticated_latest_temp_bug.cfg",
        "multilane_native_mutating_unified_startup_plan_bug.cfg",
        "multilane_native_uncoalesced_canonical_body_needs_bug.cfg",
        "multilane_native_partial_unified_startup_preflight_bug.cfg",
        "multilane_native_queue_before_evidence_readback_bug.cfg",
        "multilane_native_missing_reverse_merge_carrier_bug.cfg",
        "multilane_native_orphan_merge_carrier_bug.cfg",
        "multilane_native_skip_post_cache_carrier_reconcile_bug.cfg",
        "multilane_native_repair_historical_sibling_as_active_bug.cfg",
        "multilane_native_prune_without_protected_latest_bug.cfg",
        "multilane_native_prune_namespace_rebind_bug.cfg",
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
        "multilane_autonomous_prevote_commit_surface_drift_bug.cfg",
        "multilane_autonomous_event_prefix_drift_bug.cfg",
        "multilane_autonomous_post_validation_event_surface_drift_bug.cfg",
        "multilane_autonomous_restart_drops_ownership_bug.cfg",
        "multilane_autonomous_unauthenticated_recovery_body_bug.cfg",
        "multilane_autonomous_mixed_signer_recovery_body_bug.cfg",
        "multilane_autonomous_historical_context_drift_bug.cfg",
        "multilane_autonomous_open_queue_before_recovery_install_bug.cfg",
        "multilane_autonomous_partial_recovery_group_preflight_bug.cfg",
        "multilane_autonomous_pending_only_canonical_terminal_bug.cfg",
        "multilane_autonomous_release_without_finalization_authority_bug.cfg",
        "multilane_autonomous_complete_without_queue_evidence_bug.cfg",
        "multilane_autonomous_partial_terminal_unit_sweep_bug.cfg",
        "multilane_autonomous_owned_group_mutation_before_planner_bug.cfg",
        "multilane_autonomous_open_queue_before_deferred_carrier_apply_bug.cfg",
        "multilane_autonomous_producer_recovery_without_queue_owner_bug.cfg",
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
            "revision4_safety",
            "revision4_adversarial_safety",
            "revision4_liveness",
            "revision4_certified_fence_reservation",
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
                "seventeen reviewed positive/search configurations"
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
          set -euo pipefail
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
            "| Native application evidence | `multilane_native_application_evidence_fixed.cfg` | 8 |",
            "| autonomous reservation/carrier | `multilane_autonomous_reservation_carrier_fixed.cfg` | 12 |",
            "| QueuePlan admission registry | `multilane_queue_plan_admission_registry_fixed.cfg` | 8 |",
            "| Kura replica retention | `kura_replica_retention_fixed.cfg` | 8 |",
            "| in-flight carrier (layout-only) | `inflight_first_release_fixed.cfg` | 18 |",
            "not independent ledger rows, TLAPS evidence",
            "cross-tool proof evidence",
            "schema version 2", "`workspace_source_manifest_sha256`", "`multilane_source_manifest_sha256`",
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
        Path("scripts/formal/sumeragi_v2_multilane_inflight_validation.py"),
        Path("scripts/formal/sumeragi_v2_multilane_cli.py"),
        Path(
            "scripts/formal/"
            "sumeragi_v2_multilane_autonomous_terminal_contract.py"
        ),
        *native_merge_manifest.NATIVE_MERGE_MANIFEST_SOURCE_RELATIVES,
        *passive_recovery_contract.PASSIVE_RECOVERY_SOURCE_RELATIVES,
        Path("scripts/formal/sumeragi_v2_multilane_queue_plan_contract.py"),
        Path("pytests/scripts/sumeragi_v2_multilane_queue_plan_cases.py"),
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
    relative_paths = _expanded_source_manifest_paths(relative_paths)
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
    if module == NATIVE_PREPUBLICATION_MODULE:
        observed_native_bindings = {
            (binding.get("path"), binding.get("symbol"))
            for binding in symbols
            if isinstance(binding, dict)
        }
        missing_replacements = (
            _CURRENT_NATIVE_RECOVERY_REPLACEMENT_BINDINGS
            - observed_native_bindings
        )
        if missing_replacements:
            errors.append(
                f"{module}: generic canonical-body recovery replacement "
                f"bindings are incomplete: {sorted(missing_replacements)!r}"
            )
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
        if (
            module == NATIVE_PREPUBLICATION_MODULE
            and (relative, symbol) in _SUPERSEDED_NATIVE_RECOVERY_BINDINGS
        ):
            # The merged implementation replaced the adapter-local Native-only
            # retry graph with the already source-bound generic canonical-body
            # recovery corridor above.  Retain the legacy ledger rows as an
            # explicit migration audit, but never pretend those deleted owners
            # still exist in production.
            continue
        key = (relative, symbol)
        if key in seen_bindings and (
            module,
            relative,
            symbol,
        ) not in _ALLOWED_MERGED_DUPLICATE_PRODUCTION_BINDINGS:
            errors.append(f"{module}: duplicate production binding {relative}!{symbol}")
        seen_bindings.add(key)
        path, source = _read_reviewed_rust_source(
            root, relative, "production binding source", errors
        )
        if source is None:
            continue
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
            current_token = _PRODUCTION_TOKEN_REBINDINGS.get(
                (relative, symbol, token), token
            )
            if current_token not in item:
                errors.append(
                    f"{path}: production item {symbol} is missing source-binding "
                    f"token {current_token!r}"
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

    path, source = _read_reviewed_rust_source(
        root, relative, label, errors
    )
    if source is None:
        return None
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

    for relative, kind, symbol, expected_match in (
        NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_MATCH_RELATIONS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            continue
        matches = list(native_merge_manifest.NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_MATCH_RE.finditer(item))
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
        for role_token in native_merge_manifest.NATIVE_PARTICIPANT_APPLICATION_ROLE_TOKENS:
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


def _validate_stable_generation_diagnostics_contract(
    root: Path, models: Any, errors: list[str]
) -> None:
    """Bind both diagnostic projections to one bounded stable-State cut."""

    if not isinstance(models, list):
        return
    models_by_module = {
        model.get("module"): model
        for model in models
        if isinstance(model, dict) and _nonempty_string(model.get("module"))
    }
    helper_relative, helper_kind, helper_symbol, helper_tokens = (
        DIAGNOSTIC_STABLE_GENERATION_HELPER_BINDING
    )
    for module in (
        "SumeragiV2NativeApplicationEvidence",
        "SumeragiV2AutonomousReservationCarrier",
    ):
        model = models_by_module.get(module)
        bindings = model.get("production_symbols") if isinstance(model, dict) else None
        matches = [
            binding
            for binding in bindings or ()
            if isinstance(binding, dict)
            and binding.get("path") == helper_relative
            and binding.get("kind") == helper_kind
            and binding.get("symbol") == helper_symbol
        ]
        if len(matches) != 1 or tuple(matches[0].get("required_tokens", ())) != helper_tokens:
            errors.append(
                f"{module}: stable-generation diagnostics helper binding must "
                "match the exact reviewed bounded retry/fail-closed contract"
            )

    helper_item = _rust_binding_item(
        root,
        helper_relative,
        helper_kind,
        helper_symbol,
        "stable-generation diagnostics helper production binding",
        errors,
    )
    helper_path = root / helper_relative
    state_path = root / DIAGNOSTIC_STABLE_GENERATION_STATE_RELATIVE
    state_source = ""
    if state_path.is_file() and not state_path.is_symlink():
        state_source = state_path.read_text(encoding="utf-8")
        bound_count = state_source.count(DIAGNOSTIC_STABLE_GENERATION_ATTEMPT_BOUND)
        if bound_count != 1:
            errors.append(
                f"{state_path}: stable-generation diagnostics attempt bound must "
                f"equal one exact four-attempt declaration, found {bound_count}"
            )
    if helper_item is not None:
        cursor = -1
        for token in helper_tokens:
            count = helper_item.count(token)
            position = helper_item.find(token, cursor + 1)
            if count != 1 or position < 0:
                errors.append(
                    f"{helper_path}: stable-generation diagnostics helper token "
                    f"must occur exactly once and in order: {token!r}; found {count}"
                )
                break
            cursor = position

    for (
        module,
        relative,
        kind,
        symbol,
        required_tokens,
        ordered_tokens,
    ) in DIAGNOSTIC_STABLE_GENERATION_CONSUMER_BINDINGS:
        model = models_by_module.get(module)
        bindings = model.get("production_symbols") if isinstance(model, dict) else None
        matches = [
            binding
            for binding in bindings or ()
            if isinstance(binding, dict)
            and binding.get("path") == relative
            and binding.get("kind") == kind
            and binding.get("symbol") == symbol
        ]
        if (
            len(matches) != 1
            or tuple(matches[0].get("required_tokens", ())) != required_tokens
        ):
            errors.append(
                f"{module}: diagnostic consumer {symbol} must match the exact "
                "reviewed stable-generation/fail-closed binding"
            )
        item = _rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "stable-generation diagnostics consumer production binding",
            errors,
        )
        if item is None:
            continue
        cursor = -1
        for token in ordered_tokens:
            count = item.count(token)
            position = item.find(token, cursor + 1)
            if count != 1 or position < 0:
                errors.append(
                    f"{root / relative}: diagnostic consumer {symbol} token "
                    f"must occur exactly once and in order: {token!r}; found {count}"
                )
                break
            cursor = position

    for symbol, required_tokens in DIAGNOSTIC_STABLE_GENERATION_TEST_BINDINGS:
        item = _rust_binding_item(
            root,
            DIAGNOSTIC_STABLE_GENERATION_STATE_RELATIVE,
            "fn",
            symbol,
            "stable-generation diagnostics static negative-control test",
            errors,
        )
        if item is None:
            continue
        for token in required_tokens:
            if token not in item:
                errors.append(
                    f"{state_path}: stable-generation diagnostics test {symbol} "
                    f"is missing negative-control token {token!r}"
                )


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

    reviewed_source._validate_native_prepublication_reviewed_kura_checks(binding_items, errors)
    native_merge_manifest.validate_native_merge_manifest_relations(root, binding_items, errors, _rust_binding_item)
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

    latest_temp_reconciliation = binding_items.get(
        (
            "crates/iroha_core/src/kura.rs",
            "fn",
            "reconcile_native_amx_latest_index_temp_locked",
        )
    )
    if latest_temp_reconciliation is not None:
        for forbidden in NATIVE_LATEST_TEMP_RECONCILIATION_FORBIDDEN_TOKENS:
            if forbidden in latest_temp_reconciliation:
                errors.append(
                    f"{root / 'crates/iroha_core/src/kura.rs'}: authenticated "
                    "Native latest-index temporary reconciliation contains "
                    f"forbidden destructive token {forbidden!r}"
                )

    kura_source = (root / "crates/iroha_core/src/kura.rs").read_text(
        encoding="utf-8"
    )
    if "discard_native_amx_latest_index_temp_locked" in kura_source:
        errors.append(
            f"{root / 'crates/iroha_core/src/kura.rs'}: legacy unconditional "
            "Native latest-index temporary discard must remain absent"
        )

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
            "let manifest_readback = self."
            "read_back_native_amx_plan_manifests_under_publication_guard(plan)?;",
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
        item = binding_items.get(
            (KURA_PIPELINE_AND_LANE_ARTIFACTS_RELATIVE, "method", symbol)
        )
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


def _validate_native_exact_object_prune_contract(
    root: Path, models: Any, errors: list[str]
) -> None:
    """Bind Native pruning to the authenticated open object, not a path."""

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
            "Native exact-object prune source contract requires exactly one "
            f"{NATIVE_PREPUBLICATION_MODULE} model"
        )
        return
    production_symbols = native_models[0].get("production_symbols")
    if not isinstance(production_symbols, list):
        return

    items: dict[str, str] = {}
    for relative, kind, symbol, expected_tokens in (
        NATIVE_EXACT_OBJECT_PRUNE_BINDINGS
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
                f"{NATIVE_PREPUBLICATION_MODULE}: reviewed exact-object "
                f"prune binding {relative}!{symbol} must occur exactly once, "
                f"found {len(matches)}"
            )
            continue
        actual_tokens = matches[0].get("required_tokens")
        if (
            not isinstance(actual_tokens, list)
            or tuple(actual_tokens) != expected_tokens
        ):
            errors.append(
                f"{NATIVE_PREPUBLICATION_MODULE}: reviewed exact-object "
                f"prune tokens changed for {relative}!{symbol}"
            )

        item = _rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "Native exact-object prune production binding",
            errors,
        )
        if item is None:
            continue
        items[symbol] = item
        for token in expected_tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: Native exact-object prune item "
                    f"{symbol} is missing source-bound token {token!r}"
                )

    kura_relative = "crates/iroha_core/src/kura.rs"
    for symbol, expected_counts in NATIVE_EXACT_OBJECT_PRUNE_CALL_COUNTS:
        item = items.get(symbol)
        if item is None:
            item = _rust_binding_item(
                root,
                kura_relative,
                "fn",
                symbol,
                "Native exact-object prune consumer binding",
                errors,
            )
        if item is None:
            continue
        for token, expected_count in expected_counts:
            count = item.count(token)
            if count != expected_count:
                errors.append(
                    f"{root / kura_relative}: exact-object consumer {symbol} "
                    f"must contain {token!r} exactly {expected_count} times, "
                    f"found {count}"
                )
        for forbidden in (
            "std::fs::remove_file(",
            "rustix::fs::unlinkat(",
            "remove_bound_progress_temp_if_present(",
        ):
            if forbidden in item:
                errors.append(
                    f"{root / kura_relative}: exact-object consumer {symbol} "
                    f"contains forbidden path-destructive token {forbidden!r}"
                )

    exact_relations = {
        "verify_bound_open_regular_file_exact_bytes_locked": (
            "if readback != expected_bytes || "
            "!Self::sidecar_file_metadata_unchanged(&metadata.file, &opened) "
            "|| !Self::stable_sidecar_metadata_unchanged(&metadata, &current) "
            "|| !Self::progress_mutation_namespace_unchanged(namespace) {",
        ),
        "verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked": (
            "if readback != expected_bytes || metadata.canonical_path != "
            "current.canonical_path || "
            "!Self::sidecar_file_metadata_unchanged(&metadata.file, &opened) "
            "|| !Self::sidecar_file_metadata_unchanged(&metadata.file, "
            "&current.file) || !Self::sidecar_directory_binding_unchanged("
            "&metadata.directory, &current.directory) || "
            "!Self::progress_mutation_namespace_unchanged(namespace) {",
        ),
        "remove_bound_progress_file_if_matches": (
            "if !Self::sidecar_file_metadata_unchanged("
            "&expected_snapshot.file, &expected_metadata) {",
            "if rustix::fs::FileType::from_raw_mode(entry.st_mode) != "
            "rustix::fs::FileType::RegularFile || expected_metadata.nlink() "
            "!= 1 || entry.st_dev as u64 != expected_snapshot.file.dev() || "
            "entry.st_ino as u64 != expected_snapshot.file.ino() || "
            "entry.st_nlink as u64 != 1 {",
            "if current.file_type().is_symlink() || !current.is_file() || "
            "!Self::sidecar_is_single_link(&current) || "
            "!Self::sidecar_file_metadata_unchanged(&expected_snapshot.file, "
            "&current) {",
        ),
    }
    for symbol, required_relations in exact_relations.items():
        item = items.get(symbol)
        if item is None:
            continue
        normalized = " ".join(item.split())
        for relation in required_relations:
            if normalized.count(relation) != 1:
                errors.append(
                    f"{root / kura_relative}: exact-object metadata/namespace "
                    f"relation drifted in {symbol}"
                )


def _validate_queue_plan_pending_membership_contract(
    root: Path, models: Any, errors: list[str]
) -> None:
    """Bind exact QueuePlan route members to bounded, all-route WSV updates."""

    if not isinstance(models, list):
        return
    queue_models = [
        model
        for model in models
        if isinstance(model, dict)
        and model.get("module") == QUEUE_PLAN_PENDING_MEMBERSHIP_MODULE
    ]
    if len(queue_models) != 1:
        errors.append(
            "QueuePlan pending-membership source contract requires exactly one "
            f"{QUEUE_PLAN_PENDING_MEMBERSHIP_MODULE} model"
        )
        return
    production_symbols = queue_models[0].get("production_symbols")
    if not isinstance(production_symbols, list):
        return

    for relative, kind, symbol, expected_tokens in (
        QUEUE_PLAN_PENDING_MEMBERSHIP_BINDINGS
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
                f"{QUEUE_PLAN_PENDING_MEMBERSHIP_MODULE}: reviewed pending "
                f"route-membership binding {relative}!{symbol} must occur "
                f"exactly once, found {len(matches)}"
            )
            continue
        actual_tokens = matches[0].get("required_tokens")
        if (
            not isinstance(actual_tokens, list)
            or tuple(actual_tokens) != expected_tokens
        ):
            errors.append(
                f"{QUEUE_PLAN_PENDING_MEMBERSHIP_MODULE}: reviewed pending "
                f"route-membership tokens changed for {relative}!{symbol}"
            )

    state_path = root / QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    if _regular_file(
        state_path,
        "QueuePlan pending route-membership bounded decoder source",
        errors,
    ):
        state_source = state_path.read_text(encoding="utf-8")
        compact_bounds = tuple(
            value.strip()
            for value in re.findall(
                r"(?m)^const MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES: usize = ([^;]+);$",
                state_source,
            )
        )
        if compact_bounds != ("1024",):
            errors.append(
                f"{state_path}: QueuePlan compact member decoder bound "
                "must be the one exact reviewed 1024-byte declaration"
            )
        route_roster_bound = (
            "const MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS: usize = "
            "MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK;"
        )
        if state_source.count(route_roster_bound) != 1:
            errors.append(
                f"{state_path}: QueuePlan authoritative route roster must use "
                "the one exact block/proposal admission consensus bound"
            )
        for forbidden in (
            "QUEUE_PLAN_PENDING_ROUTE_COUNT_MARKER_PREFIX",
            "QueuePlanPendingRouteCountV1",
            "queue_plan_pending_route_member_xor",
            "queue_plan_pending_route_count_after_member_removal",
            "member_identity_xor",
        ):
            if forbidden in state_source:
                errors.append(
                    f"{state_path}: QueuePlan exact route roster retains "
                    f"forbidden count/XOR authority token {forbidden!r}"
                )

    host_path = root / QUEUE_PLAN_PENDING_MEMBERSHIP_HOST_RELATIVE
    if _regular_file(
        host_path,
        "QueuePlan pending marker opaque contract-state namespace source",
        errors,
    ):
        host_source = host_path.read_text(encoding="utf-8")
        opaque_declarations = re.findall(
            r"(?ms)^const OPAQUE_SYSTEM_CONTRACT_STATE_PREFIXES: &\[&str\] = "
            r"&\[(.*?)^\];$",
            host_source,
        )
        if len(opaque_declarations) != 1:
            errors.append(
                f"{host_path}: opaque system contract-state namespace must "
                "have one exact declaration"
            )
        else:
            opaque_body = opaque_declarations[0]
            for prefix in QUEUE_PLAN_PENDING_OPAQUE_PREFIXES:
                if opaque_body.count(f'"{prefix}"') != 1:
                    errors.append(
                        f"{host_path}: QueuePlan native marker prefix "
                        f"{prefix!r} must occur exactly once in the opaque "
                        "system contract-state namespace"
                    )

    binding_items: dict[tuple[str, str, str], str] = {}
    for relative, kind, symbol, tokens in (
        QUEUE_PLAN_PENDING_MEMBERSHIP_BINDINGS
    ):
        item = _rust_binding_item(
            root,
            relative,
            kind,
            symbol,
            "QueuePlan pending route-membership production binding",
            errors,
        )
        if item is None:
            continue
        binding_items[(relative, kind, symbol)] = item
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: QueuePlan pending route-membership "
                    f"item {symbol} is missing source-bound token {token!r}"
                )

    relative, symbol, tokens = QUEUE_PLAN_PENDING_QUEUE_OWNERSHIP_FREE_FN
    queue_ownership_item = None
    if state_path.is_file() and not state_path.is_symlink():
        queue_ownership_matches = tuple(
            item
            for item in _extract_rust_binding_items(state_source, "fn", symbol)
            if "cannot authorize new Queue ownership" in item
        )
        if len(queue_ownership_matches) != 1:
            errors.append(
                f"{state_path}: QueuePlan ownership free function {symbol} "
                "must have one exact fail-closed implementation, found "
                f"{len(queue_ownership_matches)}"
            )
        else:
            queue_ownership_item = queue_ownership_matches[0]
            cursor = -1
            for token in tokens:
                count = queue_ownership_item.count(token)
                position = queue_ownership_item.find(token, cursor + 1)
                if count != 1 or position < 0:
                    errors.append(
                        f"{state_path}: QueuePlan ownership free function "
                        f"{symbol} token must occur exactly once and in order: "
                        f"{token!r}; found {count}"
                    )
                    break
                cursor = position

    for relative, kind, symbol, tokens in (
        QUEUE_PLAN_PENDING_MEMBERSHIP_ORDERED_SOURCE_CHECKS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None:
            item = _rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "ordered QueuePlan pending route-membership source binding",
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
                    f"{root / relative}: ordered QueuePlan pending "
                    f"route-membership item {symbol} token must occur exactly "
                    f"once and in order: {token!r}; found {count}"
                )
                break
            cursor = position

    roster_item = binding_items.get(
        (
            QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE,
            "fn",
            "queue_plan_pending_route_members_from_storage_with_limit",
        )
    )
    if (
        roster_item is not None
        and "decode_exact_queue_plan_pending_obligation_marker" in roster_item
    ):
        errors.append(
            f"{state_path}: QueuePlan bounded route-roster enumeration must "
            "validate the compact canonical member and exact obligation-key "
            "existence without decoding the full obligation payload"
        )

    mutation_re = re.compile(
        r"(?:storage|world\s*\.\s*smart_contract_state)\s*\.\s*"
        r"(?:insert_queue_plan_marker|remove_queue_plan_marker|insert|remove)\s*\("
    )
    preflight_contracts = (
        (
            "stage_queue_plan_pending_obligation_marker_in_storage",
            "storage.insert_queue_plan_marker(obligation_key, obligation_payload);",
        ),
        (
            "resolve_queue_plan_pending_obligation_in_storage",
            "storage.remove_queue_plan_marker(obligation_key);",
        ),
    )
    for symbol, mutation_token in preflight_contracts:
        item = binding_items.get(
            (QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE, "fn", symbol)
        )
        if item is None:
            continue
        mutation = item.find(mutation_token)
        if mutation < 0:
            continue
        if mutation_re.search(item[:mutation]) is not None:
            errors.append(
                f"{state_path}: QueuePlan pending route-membership item "
                f"{symbol} mutates WSV before completing all-route preflight"
            )

    for relative, symbol, tokens in QUEUE_PLAN_PENDING_MEMBERSHIP_TEST_BINDINGS:
        item = _rust_binding_item(
            root,
            relative,
            "fn",
            symbol,
            "QueuePlan pending route-membership static negative-control test",
            errors,
        )
        if item is None:
            continue
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: QueuePlan pending route-membership "
                    f"test {symbol} is missing negative-control token {token!r}"
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


_INFLIGHT_VALIDATION_COMPONENT = Path(__file__).with_name(
    "sumeragi_v2_multilane_inflight_validation.py"
)
if (
    _INFLIGHT_VALIDATION_COMPONENT.is_symlink()
    or not _INFLIGHT_VALIDATION_COMPONENT.is_file()
):
    raise RuntimeError(
        "multilane in-flight validation component is unavailable: "
        f"{_INFLIGHT_VALIDATION_COMPONENT}"
    )
exec(
    compile(
        _INFLIGHT_VALIDATION_COMPONENT.read_bytes(),
        str(_INFLIGHT_VALIDATION_COMPONENT),
        "exec",
    ),
    globals(),
)


def _models_with_current_component_tokens(models: Any) -> Any:
    """Project ledger bindings through spelling-only merged-tree rebindings."""

    if not isinstance(models, list):
        return models
    current = copy.deepcopy(models)
    replacements = _QUEUE_PLAN_STARTUP_TOKEN_REBINDINGS
    for model in current:
        if not isinstance(model, dict):
            continue
        for binding in model.get("production_symbols", ()):
            if not isinstance(binding, dict):
                continue
            tokens = binding.get("required_tokens")
            if isinstance(tokens, list):
                current_tokens = [
                    replacements.get(token, token) for token in tokens
                ]
                if (
                    binding.get("path") == "crates/irohad/src/main.rs"
                    and binding.get("symbol") == "Iroha::start_with_runtime_deps"
                ):
                    current_tokens = [
                        token
                        for token in current_tokens
                        if token != "finalize_plan_journal_startup_recovery()"
                    ]
                binding["required_tokens"] = current_tokens
    return current


def _validate(root: Path = DEFAULT_ROOT) -> tuple[str, ...]:
    """Return structural/source-binding errors for the multilane model slice."""

    errors: list[str] = []
    root = root.resolve()
    _validate_reviewed_rust_include_manifest(root, errors)
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
    current_component_models = _models_with_current_component_tokens(models)
    validate_autonomous_terminal_recovery_contract(
        root, current_component_models, errors, _rust_binding_item
    )
    _validate_stable_generation_diagnostics_contract(root, models, errors)
    _validate_native_participant_application_classifier_contract(
        root, models, errors
    )
    _validate_native_prepublication_contract(root, models, errors)
    passive_recovery_contract.validate_passive_recovery_contract(root, models, errors, _rust_binding_item)
    _validate_native_exact_object_prune_contract(root, models, errors)
    _validate_queue_plan_pending_membership_contract(root, models, errors)
    _validate_queue_plan_startup_replay_contract(
        root, current_component_models, errors
    )
    validate_queue_plan_autonomous_only_contract(root, formal_dir, current_component_models, errors, _rust_binding_item, _regular_file, TLA_DECLARATION_TEMPLATE)
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


def validate(root: Path = DEFAULT_ROOT) -> tuple[str, ...]:
    """Validate with one immutable per-run reviewed-source expansion cache."""

    with _reviewed_rust_source_cache():
        return _validate(root)


def _parser() -> argparse.ArgumentParser:
    return build_parser(__doc__, DEFAULT_ROOT)


def main() -> int:
    args = _parser().parse_args()
    errors = validate(args.root)
    source_manifest = (
        source_manifest_sha256(args.root)
        if args.print_source_manifest_sha256 and not errors
        else None
    )
    return report_validation(errors, source_manifest)


if __name__ == "__main__":
    raise SystemExit(main())
