"""Static source contract for passive diagnostics and bounded recovery retry."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable, Optional


PASSIVE_RECOVERY_CONTRACT_RELATIVE = Path(
    "scripts/formal/sumeragi_v2_multilane_passive_recovery_contract.py"
)
PASSIVE_RECOVERY_TEST_RELATIVE = Path(
    "pytests/scripts/sumeragi_v2_multilane_passive_recovery_contract_test.py"
)

NATIVE_MODULE = "SumeragiV2NativeApplicationEvidence"
AUTONOMOUS_MODULE = "SumeragiV2AutonomousReservationCarrier"

PASSIVE_RECOVERY_MODEL_BINDINGS = (
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/state.rs",
        "method",
        "State::native_amx_participant_applications_diagnostics_once",
        (
            "pending_native_source_hashes",
            "merge_entry_by_hash_without_append_repair",
            "latest_certified_lane_block_artifacts_matching_without_sidecar_repair",
            "durable_autonomous_lane_merge_source",
            "read_native_amx_participant_application_receipt",
            "read_structural_native_amx_participant_application_receipt",
            "HistoricalNativeAmxSourceAuthority::CertifiedCoordinator",
            "authenticated_native_amx_participant_application_rows_from_merge_entry",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "MergeLedgerLog::entry_by_hash_without_append_repair",
        (
            "self.append_recovery_offset.is_some()",
            "passive diagnostics cannot repair it",
            "entry_by_hash_with_append_repair_policy(hash, false)",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::merge_entry_by_hash_without_append_repair",
        (
            "ensure_prune_recovery_not_required",
            "ensure_canonical_storage_not_poisoned",
            "read_pending_merge_entry_path",
            "entry_by_hash_without_append_repair",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura.rs",
        "fn",
        "read_structural_native_amx_participant_application_receipt",
        (
            "prune_recovery_is_required",
            "regular_sidecar_metadata_for",
            "decode_structural_native_amx_receipt_file_locked",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura.rs",
        "fn",
        "read_native_amx_participant_application_receipt",
        (
            "read_native_amx_participant_application_receipt_from_paths_locked",
            "native_amx_participant_application_receipt_matches_available_evidence_under_prune_guard",
            "read_structural()? == artifact",
        ),
    ),
    (
        NATIVE_MODULE,
        "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
        "method",
        "Kura::durable_autonomous_lane_merge_source",
        (
            "prune_lock.lock",
            "durable_autonomous_lane_merge_source_under_prune_guard",
            "None",
            "true",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::read_lane_block_execution_input_without_sidecar_repair",
        (
            "read_lane_block_execution_input_with_repair_policy",
            "false",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::read_lane_block_execution_preflight_without_sidecar_repair",
        (
            "read_lane_block_execution_preflight_with_repair_policy",
            "false",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::read_preflighted_lane_block_execution_input_for_application_without_sidecar_repair",
        (
            "lane_block_predecessor_application_receipt_available_without_sidecar_repair",
            "lane_block_application_receipt_available_without_sidecar_repair",
            "lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair",
            "read_lane_block_execution_preflight_without_sidecar_repair",
            "read_lane_block_execution_input_without_sidecar_repair",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::latest_lane_block_artifact_matching_without_sidecar_repair",
        (
            "ensure_bound_progress_pair_has_no_recovery_artifacts_locked",
            "read_active_lane_block_artifact_from_bound_without_repair_locked",
            "bound_progress_sidecar_unchanged",
            "read_lane_block_artifact_without_sidecar_repair",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/passive_diagnostic_reads.rs",
        "method",
        "Kura::latest_certified_lane_block_artifacts_matching_without_sidecar_repair",
        (
            "PASSIVE_DIAGNOSTIC_CERTIFIED_RESULT_BUDGET",
            "PASSIVE_DIAGNOSTIC_CERTIFIED_SCAN_BUDGET",
            "ensure_bound_progress_pair_has_no_recovery_artifacts_locked",
            "read_active_certified_lane_block_artifact_from_bound_locked",
            "bound_progress_sidecar_unchanged",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura/autonomous_application_evidence.rs",
        "method",
        "Kura::autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair",
        (
            "read_lane_block_application_receipt_without_sidecar_repair",
            "LaneBlockApplicationReceiptArtifactFormat::MergeExecution",
            "lane_block_application_receipt_matches_merge_log_without_sidecar_repair",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/kura.rs",
        "fn",
        "lane_block_payload_is_recoverable",
        (
            "recover_lane_block_payload_with_sidecar_repair(proposal, false)",
            ".is_ok()",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/state/passive_lane_diagnostic_methods.rs",
        "fn",
        "durable_lane_diagnostic_execution_status",
        (
            "autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair",
            "lane_block_application_receipt_available_without_sidecar_repair",
            "ExecutionStatus::StateAppliedByCanonicalBlock",
            "lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair",
            "read_preflighted_lane_block_execution_input_for_application_without_sidecar_repair",
            "lane_block_execution_preflight_has_rejections_without_sidecar_repair",
            "lane_block_execution_input_available_without_sidecar_repair",
            "lane_block_payload_is_recoverable",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/state.rs",
        "method",
        "State::durable_lane_diagnostics",
        (
            "latest_lane_block_artifact_matching_without_sidecar_repair",
            "latest_certified_lane_block_artifacts_matching_without_sidecar_repair",
            "durable_lane_diagnostic_execution_status",
            "DurableLaneDiagnosticsSnapshot",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::committed_lane_block_status_snapshot",
        (
            "lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair",
            "read_lane_block_application_receipt_without_sidecar_repair",
            "autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair",
            "read_lane_block_execution_preflight_without_sidecar_repair",
            "read_lane_block_execution_input_without_sidecar_repair",
            "lane_block_payload_is_recoverable",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/state.rs",
        "method",
        "State::autonomous_lane_execution_diagnostics_once",
        (
            "lane_consensus_lifecycle_snapshot",
            "routes.truncate(SUMERAGI_AUTONOMOUS_LANE_EXECUTIONS_MAX)",
            "latest_autonomous_lane_block_artifacts_snapshot",
            "latest_certified_lane_block_artifacts_matching_without_sidecar_repair",
            "pending_certified_merge_entries_bounded",
            "merge_ledger_latest_snapshot",
            "source_budget",
            "decode_autonomous_lane_merge_bundle",
            "read_lane_block_application_receipt_without_sidecar_repair",
            "LaneBlockApplicationReceiptArtifactFormat::MergeExecution",
            "lane_reservation_diagnostic_groups_bounded",
            "AutonomousLaneDiagnosticEvidence::from_reservation_group",
            "lane_reservation_group_is_finalized_for_diagnostics",
            "AutonomousLaneDiagnosticEvidence::finish",
            "rows.sort_by_key",
            "row.validate()",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_torii/src/routing.rs",
        "fn",
        "handle_v1_sumeragi_diagnostics",
        (
            "native_amx_participant_applications_diagnostics",
            "durable_lane_diagnostics",
            "Option::as_ref(&durable_queue)",
            "autonomous_lane_execution_diagnostics",
            "autonomous_lane_execution_diagnostics_with_queue",
            "validate_autonomous_lane_executions",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "struct",
        "HistoricalRecoveryRequestCadence",
        (
            "reason: HistoricalRecoveryWaitReason",
            "retained_attempts: u32",
            "next_retry_at: Instant",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "HistoricalRecoveryWait::retry_delay",
        (
            "let ceiling = ceiling.max(floor)",
            "consecutive_attempts.saturating_sub(1)",
            "retry_tier_attempts.get()",
            "min(self.max_retry_tier.get())",
            "floor.saturating_mul(1_u32 << tier).min(ceiling)",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "HistoricalRecoveryRequestCadence::after_retained_attempt",
        (
            "self.retained_attempts.saturating_add(1)",
            "consecutive_attempts: retained_attempts",
            "retry_delay(floor, ceiling)",
            "now.checked_add(delay)",
            "reason: observation.reason",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::service_next_historical_recovery_at",
        (
            "persist_historical_recovery_session",
            "historical_recovery_diagnostics.complete(identity)",
            "retire_historical_recovery_request(identity)",
            "historical_recovery_diagnostics",
            ".observe(identity, reason)",
            "schedule_historical_recovery_request",
            "historical_recovery_sessions.push_back(session)",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::schedule_historical_recovery_request",
        (
            "observation: HistoricalRecoveryWait",
            "now: Instant",
            "HistoricalRecoveryRetry::LocalState",
            "retire_historical_recovery_request(identity)",
            "existing.request == request",
            "existing.request_hash == request_hash",
            "existing.cadence.reason == observation.reason",
            "HistoricalRecoveryRequestCadence::immediate(observation.reason, now)",
            "if !cadence.due(now)",
            "after_retained_attempt",
            "historical_recovery_retry_floor",
            "historical_recovery_retry_ceiling",
            "if retained",
            ".cadence = next_cadence",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "lane_work_limits",
        (
            "historical_recovery_retry_floor: Duration",
            "historical_recovery_retry_ceiling: Duration",
            "V2LaneWorkLimits::new",
            "historical_recovery_retry_floor",
            "historical_recovery_retry_ceiling",
            "historical_recovery_retry_tier_attempts",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "run_inner",
        (
            "public_key: genesis_public_key",
            "block_cadence",
            "sumeragi_v2_timing_ms(block_cadence_ms)",
            "let round_timeout = Duration::from_millis(round_timeout_ms)",
            "let retransmit_interval = Duration::from_millis(retransmit_interval_ms)",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
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
            "let lane_work_limits = lane_work_limits(",
            "block_sync_frame_byte_capacity",
            "retransmit_interval",
            "round_timeout",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "run_pending_kura_lifecycle_height",
        (
            "let lane_work_limits = lane_work_limits(",
            "block_sync_frame_byte_capacity",
            "retransmit_interval",
            "round_timeout",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_lifecycle_active_height",
        (
            "let mut next_lane_retransmit = deadline_after(height_started_at, retransmit_interval)",
            "service_historical_recovery_tick(&mut lane_work)?",
            "lane_work.schedule_autonomous_new_view_timeouts(",
            "lane_work.schedule_retransmission()?",
            "next_lane_retransmit = deadline_after(now, retransmit_interval)",
            "dispatch_lane_work_effects(&mut lane_work, services, control_queue_capacity)?",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "run_pending_active_height",
        (
            "let mut next_lane_retransmit = deadline_after(Instant::now(), retransmit_interval)",
            "service_historical_recovery_tick(lane_work)?",
            "lane_work.schedule_autonomous_new_view_timeouts(",
            "lane_work.schedule_retransmission()?",
            "next_lane_retransmit = deadline_after(now, retransmit_interval)",
            "dispatch_lane_work_effects(lane_work, services, control_queue_capacity)?",
        ),
    ),
    (
        AUTONOMOUS_MODULE,
        "crates/iroha_core/src/sumeragi/v2_runner/canonical_recovery_ingress.rs",
        "fn",
        "service_historical_recovery_tick",
        (
            "service_next_historical_recovery()",
            "map_err(V2RunnerError::from)",
        ),
    ),
)

PASSIVE_RECOVERY_ORDERED_CHECKS = (
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::service_next_historical_recovery_at",
        (
            "persist_historical_recovery_session(&session)",
            "historical_recovery_diagnostics.complete(identity)",
            "retire_historical_recovery_request(identity)",
            ".observe(identity, reason)",
            "schedule_historical_recovery_request(",
            "historical_recovery_sessions.push_back(session)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::schedule_historical_recovery_request",
        (
            "let retained_request_matches",
            "existing.request == request",
            "existing.request_hash == request_hash",
            "existing.cadence.reason == observation.reason",
            "self.retire_historical_recovery_request(identity)",
            "HistoricalRecoveryRequestCadence::immediate(observation.reason, now)",
            "if !cadence.due(now)",
            ".after_retained_attempt(",
            "if retained",
            ".cadence = next_cadence",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn",
        "run_inner",
        (
            "public_key: genesis_public_key",
            "block_cadence",
            "sumeragi_v2_timing_ms(block_cadence_ms)",
            "let round_timeout = Duration::from_millis(round_timeout_ms)",
            "let retransmit_interval = Duration::from_millis(retransmit_interval_ms)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_non_pending_lifecycle_loop",
        (
            "let lane_work_limits = lane_work_limits(",
            "block_sync_frame_byte_capacity",
            "retransmit_interval",
            "round_timeout",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "run_pending_kura_lifecycle_height",
        (
            "let lane_work_limits = lane_work_limits(",
            "block_sync_frame_byte_capacity",
            "retransmit_interval",
            "round_timeout",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn",
        "run_lifecycle_active_height",
        (
            "let mut next_lane_retransmit =",
            "deadline_after(height_started_at, retransmit_interval)",
            "if now >= next_lane_retransmit",
            "service_historical_recovery_tick(&mut lane_work)?",
            "lane_work.schedule_autonomous_new_view_timeouts(",
            "lane_work.schedule_retransmission()?",
            "next_lane_retransmit = deadline_after(now, retransmit_interval)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "fn",
        "run_pending_active_height",
        (
            "let mut next_lane_retransmit = deadline_after(Instant::now(), retransmit_interval);",
            "if now >= next_lane_retransmit",
            "service_historical_recovery_tick(lane_work)?",
            "lane_work.schedule_autonomous_new_view_timeouts(",
            "lane_work.schedule_retransmission()?",
            "next_lane_retransmit = deadline_after(now, retransmit_interval)",
        ),
    ),
    (
        "crates/iroha_core/src/state/passive_lane_diagnostic_methods.rs",
        "fn",
        "durable_lane_diagnostic_execution_status",
        (
            "autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair",
            "lane_block_application_receipt_available_without_sidecar_repair",
            "ExecutionStatus::StateAppliedByCanonicalBlock",
            "lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair",
            "read_preflighted_lane_block_execution_input_for_application_without_sidecar_repair",
            "lane_block_execution_preflight_has_rejections_without_sidecar_repair",
            "lane_block_execution_input_available_without_sidecar_repair",
            "lane_block_payload_is_recoverable",
        ),
    ),
)

PASSIVE_RECOVERY_FORBIDDEN_CHECKS = (
    (
        "crates/iroha_core/src/state/passive_lane_diagnostic_methods.rs",
        "fn",
        "durable_lane_diagnostic_execution_status",
        (
            ".recover_lane_block_payload(",
            ".lane_block_payload_availability(",
            ".read_lane_block_execution_input(",
            ".read_lane_block_execution_preflight(",
            ".read_lane_block_application_receipt(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::committed_lane_block_status_snapshot",
        (
            ".recover_lane_block_payload(",
            ".lane_block_payload_availability(",
            ".read_lane_block_execution_input(",
            ".read_lane_block_execution_preflight(",
            ".read_lane_block_application_receipt(",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "method",
        "State::native_amx_participant_applications_diagnostics_once",
        (
            ".merge_entry_by_hash(",
            ".latest_certified_lane_block_artifacts_matching(",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "method",
        "State::autonomous_lane_execution_diagnostics_once",
        (
            ".latest_certified_lane_block_artifacts_matching(",
            ".read_lane_block_application_receipt(",
        ),
    ),
    (
        "crates/iroha_torii/src/routing.rs",
        "fn",
        "handle_v1_sumeragi_diagnostics",
        (
            ".recover_lane_block_payload(",
            ".lane_block_payload_availability(",
        ),
    ),
)

PASSIVE_RECOVERY_INCLUDE_RELATIONS = (
    (
        "crates/iroha_core/src/kura.rs",
        'include!("kura/autonomous_application_evidence.rs");',
    ),
    (
        "crates/iroha_core/src/kura/autonomous_application_evidence.rs",
        'include!("passive_diagnostic_reads.rs");',
    ),
    (
        "crates/iroha_core/src/state.rs",
        'include!("state/passive_lane_diagnostic_methods.rs");',
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        'include!("v2_runner/canonical_recovery_ingress.rs");',
    ),
)

PASSIVE_RECOVERY_RAW_TEST_CHECKS = (
    (
        "crates/iroha_core/src/state/autonomous_merge_and_queue_plan_native_diagnostic_tests.rs",
        "assert_passive_state_diagnostics",
        (
            "std::fs::rename(&ownership_data, &ownership_data_temp)",
            "let passive_revision = kura.committed_lane_status_revision()",
            "for _ in 0..2",
            "state.durable_lane_diagnostics()",
            "native_amx_participant_applications_diagnostics()",
            "autonomous_lane_execution_diagnostics()",
            "assert!(!ownership_data.exists())",
            "kura.committed_lane_status_revision()",
            "kura.recover_lane_block_payload(&session.proposal)",
            "assert!(ownership_data.is_file())",
        ),
    ),
    (
        "crates/iroha_torii/src/tests/routing.rs",
        "permissioned_sumeragi_diagnostics_omit_npos_and_canonical_state",
        (
            "install_passive_diagnostic_lane_artifact",
            "std::fs::rename(&ownership_data, &ownership_data_temp)",
            "for _ in 0..2",
            "handle_v1_sumeragi_diagnostics(",
            "assert!(!ownership_data.exists())",
            "kura.recover_lane_block_payload(&proposal)",
            "assert!(ownership_data.is_file())",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/historical_recovery_and_carrier_tests.rs",
        "historical_missing_canonical_block_schedules_authenticated_retry_then_completes",
        (
            "first_cadence.retained_attempts, 1",
            "service_next_historical_recovery_at(before_deadline)",
            "must not fan out",
            "a full effect queue must not advance the retry deadline",
            "retry must preserve the exact peer order and request bytes",
            "second_cadence.retained_attempts, 2",
            "the next deadline is anchored at the service turn",
            "local completion is never gated by the network deadline",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_runner_upstream_recovery.rs",
        "quiet_retransmission_tick_services_one_retained_historical_session",
        (
            "quiet_historical_recovery_fixture",
            "service_historical_recovery_tick",
            "CanonicalBlockPending",
            "has_pending_historical_recovery",
        ),
    ),
)

PASSIVE_RECOVERY_SOURCE_RELATIVES = tuple(
    Path(relative)
    for relative in sorted(
        {
            *(binding[1] for binding in PASSIVE_RECOVERY_MODEL_BINDINGS),
            *(check[0] for check in PASSIVE_RECOVERY_INCLUDE_RELATIONS),
            *(check[0] for check in PASSIVE_RECOVERY_RAW_TEST_CHECKS),
            PASSIVE_RECOVERY_CONTRACT_RELATIVE.as_posix(),
            PASSIVE_RECOVERY_TEST_RELATIVE.as_posix(),
        }
    )
)

RustBindingItem = Callable[
    [Path, str, str, str, str, list[str]], Optional[str]
]


def _models_by_name(models: Any) -> dict[str, dict[str, Any]]:
    if not isinstance(models, list):
        return {}
    return {
        model["module"]: model
        for model in models
        if isinstance(model, dict) and isinstance(model.get("module"), str)
    }


def _binding_items(
    root: Path,
    models: Any,
    errors: list[str],
    rust_binding_item: RustBindingItem,
) -> dict[tuple[str, str, str], str]:
    by_name = _models_by_name(models)
    items: dict[tuple[str, str, str], str] = {}
    for module, relative, kind, symbol, expected_tokens in (
        PASSIVE_RECOVERY_MODEL_BINDINGS
    ):
        model = by_name.get(module)
        bindings = model.get("production_symbols") if model is not None else None
        matches = [
            binding
            for binding in bindings or ()
            if isinstance(binding, dict)
            and binding.get("path") == relative
            and binding.get("kind") == kind
            and binding.get("symbol") == symbol
        ]
        if len(matches) != 1:
            errors.append(
                f"{module}: passive/recovery binding {relative}!{symbol} must "
                f"occur exactly once, found {len(matches)}"
            )
        elif tuple(matches[0].get("required_tokens", ())) != expected_tokens:
            errors.append(
                f"{module}: passive/recovery tokens changed for {relative}!{symbol}"
            )
        key = (relative, kind, symbol)
        if key in items:
            continue
        item = rust_binding_item(
            root, relative, kind, symbol, "passive/recovery source binding", errors
        )
        if item is None:
            continue
        items[key] = item
        for token in expected_tokens:
            if token not in item:
                errors.append(
                    f"{root / relative}: passive/recovery item {symbol} is "
                    f"missing source-bound token {token!r}"
                )
    return items


def _item_for(
    root: Path,
    items: dict[tuple[str, str, str], str],
    relative: str,
    kind: str,
    symbol: str,
    errors: list[str],
    rust_binding_item: RustBindingItem,
) -> Optional[str]:
    return items.get((relative, kind, symbol)) or rust_binding_item(
        root, relative, kind, symbol, "passive/recovery relation", errors
    )


def _validate_source_relations(
    root: Path,
    items: dict[tuple[str, str, str], str],
    errors: list[str],
    rust_binding_item: RustBindingItem,
) -> None:
    for relative, kind, symbol, tokens in PASSIVE_RECOVERY_ORDERED_CHECKS:
        item = _item_for(
            root, items, relative, kind, symbol, errors, rust_binding_item
        )
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: passive/recovery item {symbol} "
                    f"is missing or reorders token {token!r}"
                )
                break
            cursor = position

    for relative, kind, symbol, tokens in PASSIVE_RECOVERY_FORBIDDEN_CHECKS:
        item = _item_for(
            root, items, relative, kind, symbol, errors, rust_binding_item
        )
        if item is None:
            continue
        for token in tokens:
            if token in item:
                errors.append(
                    f"{root / relative}: passive diagnostic item {symbol} "
                    f"contains repair-capable token {token!r}"
                )

    for relative, symbol, token in (
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "run_lifecycle_active_height",
            "service_historical_recovery_tick(&mut lane_work)?",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "run_pending_active_height",
            "service_historical_recovery_tick(lane_work)?",
        ),
    ):
        item = _item_for(
            root, items, relative, "fn", symbol, errors, rust_binding_item
        )
        if item is not None and item.count(token) != 1:
            errors.append(
                f"{root / relative}: passive/recovery item {symbol} must "
                "service exactly one retained historical owner per quiet "
                "retransmission turn"
            )


def _validate_includes(root: Path, errors: list[str]) -> None:
    for relative, token in PASSIVE_RECOVERY_INCLUDE_RELATIONS:
        path = root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{path}: cannot read passive provider include: {error}")
            continue
        if source.count(token) != 1:
            errors.append(
                f"{path}: passive provider include {token!r} must occur exactly once"
            )


def _validate_raw_tests(
    root: Path, errors: list[str], rust_binding_item: RustBindingItem
) -> None:
    for relative, symbol, tokens in PASSIVE_RECOVERY_RAW_TEST_CHECKS:
        item = rust_binding_item(
            root,
            relative,
            "fn",
            symbol,
            "passive/recovery focused Rust control",
            errors,
        )
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: passive/recovery focused control "
                    f"{symbol} is missing or reorders token {token!r}"
                )
                break
            cursor = position


def validate_passive_recovery_contract(
    root: Path,
    models: Any,
    errors: list[str],
    rust_binding_item: RustBindingItem,
) -> None:
    """Validate passive diagnostics and deadline-driven recovery bindings."""

    items = _binding_items(root, models, errors, rust_binding_item)
    _validate_source_relations(root, items, errors, rust_binding_item)
    _validate_includes(root, errors)
    _validate_raw_tests(root, errors, rust_binding_item)
