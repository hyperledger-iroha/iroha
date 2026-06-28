#!/usr/bin/env python3
"""Validate SoraFS reserve/rent rollout evidence artifacts."""

from __future__ import annotations

import argparse
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    emit_checker_notice,
    render_and_write_checker_summary,
    validate_checker_preflight,
)
from sorafs_evidence_paths import (  # noqa: E402
    discover_evidence_files,
    evidence_path_identities,
)
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_fingerprint import artifact_fingerprint  # noqa: E402
from sorafs_evidence_validation import (  # noqa: E402
    build_evidence_artifact,
    count_evidence_artifacts,
    count_evidence_files,
    evidence_gate_status,
    evidence_artifact_detail,
    evidence_artifact_is_valid,
    evidence_artifact_fingerprint,
    evidence_artifact_schema,
    evidence_schema_by_kind,
    init_evidence_artifact_buckets,
    build_required_evidence_summary,
    record_explicit_evidence_validation_errors,
    record_evidence_artifact,
    record_evidence_validation_errors,
    validate_bound_evidence_digest_references,
    validate_bound_evidence_tuple_references,
    require_2xx_status,
    require_bool_true,
    require_count_equal,
    require_false,
    require_false_or_absent,
    require_hex,
    require_config_backed_governance_approval,
    require_iroha_config_binding,
    validate_standard_evidence_payload,
    require_maximum_number,
    require_minimum_int,
    require_minimum_value,
    require_non_negative_int,
    require_object,
    require_object_array,
    required_evidence_kind_names,
    require_passed_status,
    require_policy_digest,
    require_positive_int,
    require_recent_timestamp,
    require_string,
    require_string_coverage,
    require_string_equal,
    require_sum_equal,
    require_zero_count,
)
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_evidence_kinds,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


SUMMARY_SCHEMA = "sorafs.reserve_rent.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_LEDGER_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_LIFECYCLE_LAG_SECS = 15 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_BAKE_AGE_SECS = 14 * 24 * 60 * 60
HEX64_LEN = 64

REQUIRED_STORAGE_CLASSES = ("hot", "warm", "archive")
REQUIRED_TIERS = ("tier-a", "tier-b", "tier-c")
REQUIRED_DURATIONS = ("monthly", "quarterly", "annual")
REQUIRED_LIFECYCLE_ROUTES = (
    "provider_summary",
    "lifecycle_status",
    "event_history",
    "policy_readback",
)
REQUIRED_SIGNED_ROUTES = (
    "top_up",
    "withdraw",
    "appeal_submit",
    "policy_update",
    "provider_status",
)
REQUIRED_METRICS = (
    "sorafs_reserve_ledger_rent_due_xor",
    "sorafs_reserve_ledger_reserve_shortfall_xor",
    "sorafs_reserve_ledger_top_up_shortfall_xor",
    "sorafs_reserve_ledger_requires_top_up",
    "sorafs_reserve_ledger_meets_underwriting",
    "sorafs_reserve_ledger_instruction_total",
    "sorafs_reserve_ledger_transfer_xor",
    "torii_sorafs_reserve_lifecycle_stage_providers",
    "torii_sorafs_reserve_credit_draw_micro_xor",
    "torii_sorafs_reserve_credit_shortfall_micro_xor",
    "torii_sorafs_reserve_accrued_interest_micro_xor",
    "torii_sorafs_reserve_defaulted_providers",
    "torii_sorafs_reserve_appeal_backlog",
    "torii_sorafs_reserve_custody_movements",
    "torii_sorafs_reserve_chain_reconciled_movements",
    "torii_sorafs_reserve_service_requests_total",
    "torii_sorafs_reserve_service_rate_limit_total",
)

SENSITIVE_KEYS = {
    "account_private_key",
    "authorization",
    "bearer_token",
    "body",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_instruction",
    "raw_ledger",
    "raw_quote",
    "raw_transfer",
    "response_body",
    "secret",
    "signature_key",
    "signed_transaction",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SFM-6 rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("policy_config", "sorafs.reserve.policy_config_canary.v1"),
    EvidenceKind("quote_matrix", "sorafs.reserve.quote_matrix_canary.v1"),
    EvidenceKind("ledger_digest", "sorafs.reserve.ledger_digest_canary.v1"),
    EvidenceKind("lifecycle_service", "sorafs.reserve.lifecycle_service_canary.v1"),
    EvidenceKind("signed_routes", "sorafs.reserve.signed_route_canary.v1"),
    EvidenceKind("reserve_movement", "sorafs.reserve.reserve_movement_canary.v1"),
    EvidenceKind("credit_line", "sorafs.reserve.credit_line_canary.v1"),
    EvidenceKind("appeal_policy", "sorafs.reserve.appeal_policy_canary.v1"),
    EvidenceKind("metrics_alerts", "sorafs.reserve.metrics_alert_canary.v1"),
    EvidenceKind("provider_bake", "sorafs.reserve.provider_bake.v1"),
    EvidenceKind("governance_approval", "sorafs.reserve.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)
POLICY_BOUND_KINDS = ("quote_matrix", "ledger_digest")
LEDGER_BOUND_KINDS = (
    "lifecycle_service",
    "signed_routes",
    "reserve_movement",
    "credit_line",
    "appeal_policy",
    "metrics_alerts",
    "provider_bake",
    "governance_approval",
)
COMMON_EVIDENCE_REQUIRED_FIELDS: tuple[str, ...] = (
    "schema",
    "status",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
)
EVIDENCE_REQUIRED_FIELDS: dict[str, tuple[str, ...]] = {
    "policy_config": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "policy_version",
        "config_source",
        "governance_approved",
        "tier_count",
        "storage_class_count",
        "duration_count",
        "credit_line_caps_present",
        "apr_policy_present",
        "policy_payload_included",
    ),
    "quote_matrix": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "matrix_digest_hex",
        "policy_digest_hex",
        "scenario_count",
        "passed_scenario_count",
        "storage_classes",
        "tiers",
        "durations",
        "quote_payloads_included",
    ),
    "ledger_digest": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
        "generated_at_unix",
        "ledger_count",
        "instruction_count",
        "rent_transfer_present",
        "reserve_top_up_transfer_present",
        "instruction_hashes_verified",
        "ledger_projection_verified",
        "raw_ledger_included",
        "raw_transfer_instructions_included",
    ),
    "lifecycle_service": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
        "route_count",
        "passed_route_count",
        "routes",
        "max_lifecycle_lag_seconds",
        "persisted_stage_count",
        "stage_transition_replay_passed",
        "governance_event_emitted",
        "manual_override_audited",
        "response_bodies_included",
    ),
    "signed_routes": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
        "route_count",
        "passed_route_count",
        "routes",
        "max_route_latency_ms",
        "replay_attack_rejected",
        "unsigned_request_rejected",
        "wrong_account_rejected",
        "response_bodies_included",
    ),
    "reserve_movement": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
        "movement_count",
        "accepted_movement_count",
        "failed_movement_count",
        "unexpected_failure_count",
        "rent_settlement_present",
        "reserve_top_up_present",
        "withdrawal_limits_enforced",
        "treasury_reconciliation_passed",
        "double_spend_rejected",
        "chain_submission_count",
        "finality_poll_attempt_count",
        "live_chain_submission_verified",
        "submitted_transaction_hash_readback_verified",
        "automatic_finality_polling_verified",
        "finality_poll_confirmed_status_verified",
        "finality_poll_timeout_rejected",
        "custody_status_route_present",
        "submitted_custody_evidence_present",
        "confirmed_custody_evidence_present",
        "rejected_custody_reconciliation_passed",
        "confirmed_balance_projection_verified",
        "confirmed_withdrawal_underflow_rejected",
        "chain_reconciled_readback_verified",
        "raw_transfer_included",
        "raw_instruction_included",
    ),
    "credit_line": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
        "credit_line_mutation_count",
        "accrual_cycle_count",
        "credit_draw_cap_enforced",
        "apr_accrual_verified",
        "manual_approval_tier_blocked",
        "credit_shortfall_reported",
        "live_account_mutation_verified",
        "credit_line_account_state_readback_verified",
        "credit_accrual_posted_to_account_state",
        "manual_approval_tier_did_not_mutate_account",
        "account_state_reconciliation_verified",
        "no_negative_balance",
        "unexpected_failure_count",
        "raw_ledger_included",
    ),
    "appeal_policy": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
        "appeal_probe_count",
        "approved_appeal_count",
        "rejected_appeal_count",
        "appeal_route_present",
        "policy_update_route_present",
        "governance_recorded",
        "operator_role_enforced",
        "unauthorized_appeal_rejected",
        "policy_digest_bound",
        "appeal_payloads_included",
    ),
    "metrics_alerts": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "critical_alerts_firing",
        "metrics",
        "response_bodies_included",
    ),
    "provider_bake": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
        "bake_id",
        "started_at_unix",
        "completed_at_unix",
        "provider_count",
        "completed_provider_count",
        "failure_count",
        "rent_cycle_count",
        "top_up_cycle_count",
        "appeal_cycle_count",
        "scheduler_config_bound",
        "scheduled_lifecycle_canary_passed",
        "scheduled_lifecycle_canary_last_tick_unix",
        "scheduled_lifecycle_canary_tick_count",
        "scheduled_lifecycle_canary_defaulted_provider_count",
        "scheduled_lifecycle_canary_gateway_sync_verified",
        "scheduled_lifecycle_canary_orderbook_rejection_verified",
        "governance_packet_attached",
        "ledger_digest_attached",
        "dashboard_snapshot_attached",
        "payloads_included",
    ),
    "governance_approval": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "policy_digest_hex",
        "matrix_digest_hex",
        "ledger_digest_hex",
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "reserve_movement_policy_present",
        "credit_line_policy_present",
        "appeal_policy_present",
        "manual_override_policy_present",
        "provider_bake_accepted",
        "governance_source_entries_published",
        "downstream_compliance_policy_applied",
        "downstream_compliance_consumer_count",
        "non_reserve_compliance_entries_preserved",
        "governance_source_entry_handoff_verified",
        "denylist_and_policy_consumers_consistent",
        "config_source",
    ),
}


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SFM-6 rollout gate."""

    now_unix: int
    max_ledger_age_secs: int
    max_lifecycle_lag_secs: int
    max_route_latency_ms: int
    max_bake_age_secs: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "policy_digest_hex",
    "matrix_digest_hex",
    "ledger_digest_hex",
)
PROVIDER_BAKE_FIELDS: tuple[str, ...] = (
    "bake_id",
    "started_at_unix",
    "completed_at_unix",
    "provider_count",
)


def require_policy_matrix_binding(
    payload: dict[str, Any], errors: list[str]
) -> tuple[str, str]:
    """Require the policy/matrix tuple that identifies the staged reserve policy."""

    policy_digest = require_policy_digest(payload, errors)
    matrix_digest = require_hex(payload, "matrix_digest_hex", HEX64_LEN, errors)
    return policy_digest, matrix_digest


def require_policy_matrix_ledger_binding(
    payload: dict[str, Any],
    errors: list[str],
) -> tuple[str, str, str]:
    """Require the policy/matrix/ledger tuple that binds a reserve rollout run."""

    policy_digest, matrix_digest = require_policy_matrix_binding(payload, errors)
    ledger_digest = require_hex(payload, "ledger_digest_hex", HEX64_LEN, errors)
    return policy_digest, matrix_digest, ledger_digest


def validate_route_records(
    payload: dict[str, Any],
    errors: list[str],
    *,
    require_authz: bool,
    options: ValidationOptions,
) -> None:
    for index, record in require_object_array(payload, "routes", errors):
        require_bool_true(record, "passed", errors, path=f"routes[{index}].passed")
        require_2xx_status(
            record,
            "status_code",
            errors,
            path=f"routes[{index}].status_code",
        )
        if require_authz:
            require_bool_true(
                record,
                "authz_enforced",
                errors,
                path=f"routes[{index}].authz_enforced",
            )
        require_bool_true(
            record,
            "signature_verified",
            errors,
            path=f"routes[{index}].signature_verified",
        )
        if record.get("latency_ms") is not None:
            require_maximum_number(
                record,
                "latency_ms",
                options.max_route_latency_ms,
                errors,
                path=f"routes[{index}].latency_ms",
            )


def validate_policy_config(payload: dict[str, Any], errors: list[str]) -> None:
    require_policy_digest(payload, errors)
    require_positive_int(payload, "policy_version", errors)
    require_iroha_config_binding(payload, errors, bound_field=None)
    require_bool_true(payload, "governance_approved", errors)
    require_minimum_int(payload, "tier_count", 3, errors)
    require_minimum_int(payload, "storage_class_count", 3, errors)
    require_minimum_int(payload, "duration_count", 3, errors)
    require_bool_true(payload, "credit_line_caps_present", errors)
    require_bool_true(payload, "apr_policy_present", errors)
    require_false(payload, "policy_payload_included", errors)


def validate_quote_matrix(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "matrix_digest_hex", HEX64_LEN, errors)
    require_policy_digest(payload, errors)
    scenario_count = require_count_equal(payload, "scenario_count", "passed_scenario_count", errors)
    require_minimum_value(
        scenario_count,
        "scenario_count",
        27,
        errors,
        message="scenario_count must cover at least the 3x3x3 policy matrix",
    )
    require_string_coverage(payload, "storage_classes", "", REQUIRED_STORAGE_CLASSES, errors)
    require_string_coverage(payload, "tiers", "", REQUIRED_TIERS, errors)
    require_string_coverage(payload, "durations", "", REQUIRED_DURATIONS, errors)
    require_false(payload, "quote_payloads_included", errors)


def validate_ledger_digest(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_policy_matrix_ledger_binding(payload, errors)
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_ledger_age_secs,
    )
    require_positive_int(payload, "ledger_count", errors)
    require_positive_int(payload, "instruction_count", errors)
    require_bool_true(payload, "rent_transfer_present", errors)
    require_bool_true(payload, "reserve_top_up_transfer_present", errors)
    require_bool_true(payload, "instruction_hashes_verified", errors)
    require_bool_true(payload, "ledger_projection_verified", errors)
    require_false(payload, "raw_ledger_included", errors)
    require_false(payload, "raw_transfer_instructions_included", errors)


def validate_lifecycle_service(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_policy_matrix_ledger_binding(payload, errors)
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_LIFECYCLE_ROUTES, errors)
    require_maximum_number(
        payload,
        "max_lifecycle_lag_seconds",
        options.max_lifecycle_lag_secs,
        errors,
    )
    require_positive_int(payload, "persisted_stage_count", errors)
    require_bool_true(payload, "stage_transition_replay_passed", errors)
    require_bool_true(payload, "governance_event_emitted", errors)
    require_bool_true(payload, "manual_override_audited", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_route_records(payload, errors, require_authz=True, options=options)


def validate_signed_routes(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_policy_matrix_ledger_binding(payload, errors)
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_SIGNED_ROUTES, errors)
    require_maximum_number(
        payload,
        "max_route_latency_ms",
        options.max_route_latency_ms,
        errors,
    )
    require_bool_true(payload, "replay_attack_rejected", errors)
    require_bool_true(payload, "unsigned_request_rejected", errors)
    require_bool_true(payload, "wrong_account_rejected", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_route_records(payload, errors, require_authz=True, options=options)


def validate_reserve_movement(payload: dict[str, Any], errors: list[str]) -> None:
    require_policy_matrix_ledger_binding(payload, errors)
    accepted_movement_count = require_count_equal(
        payload, "movement_count", "accepted_movement_count", errors
    )
    require_zero_count(payload, "failed_movement_count", errors)
    require_zero_count(payload, "unexpected_failure_count", errors)
    require_bool_true(payload, "rent_settlement_present", errors)
    require_bool_true(payload, "reserve_top_up_present", errors)
    require_bool_true(payload, "withdrawal_limits_enforced", errors)
    require_bool_true(payload, "treasury_reconciliation_passed", errors)
    require_bool_true(payload, "double_spend_rejected", errors)
    chain_submission_count = require_positive_int(
        payload, "chain_submission_count", errors
    )
    finality_poll_attempt_count = require_positive_int(
        payload, "finality_poll_attempt_count", errors
    )
    require_minimum_value(
        chain_submission_count,
        "chain_submission_count",
        accepted_movement_count,
        errors,
        message="chain_submission_count must cover every accepted_movement_count",
    )
    require_minimum_value(
        finality_poll_attempt_count,
        "finality_poll_attempt_count",
        accepted_movement_count,
        errors,
        message=(
            "finality_poll_attempt_count must cover every "
            "accepted_movement_count"
        ),
    )
    require_bool_true(payload, "live_chain_submission_verified", errors)
    require_bool_true(payload, "submitted_transaction_hash_readback_verified", errors)
    require_bool_true(payload, "automatic_finality_polling_verified", errors)
    require_bool_true(payload, "finality_poll_confirmed_status_verified", errors)
    require_bool_true(payload, "finality_poll_timeout_rejected", errors)
    require_bool_true(payload, "custody_status_route_present", errors)
    require_bool_true(payload, "submitted_custody_evidence_present", errors)
    require_bool_true(payload, "confirmed_custody_evidence_present", errors)
    require_bool_true(payload, "rejected_custody_reconciliation_passed", errors)
    require_bool_true(payload, "confirmed_balance_projection_verified", errors)
    require_bool_true(payload, "confirmed_withdrawal_underflow_rejected", errors)
    require_bool_true(payload, "chain_reconciled_readback_verified", errors)
    require_false(payload, "raw_transfer_included", errors)
    require_false(payload, "raw_instruction_included", errors)


def validate_credit_line(payload: dict[str, Any], errors: list[str]) -> None:
    require_policy_matrix_ledger_binding(payload, errors)
    require_positive_int(payload, "credit_line_mutation_count", errors)
    require_positive_int(payload, "accrual_cycle_count", errors)
    require_bool_true(payload, "credit_draw_cap_enforced", errors)
    require_bool_true(payload, "apr_accrual_verified", errors)
    require_bool_true(payload, "manual_approval_tier_blocked", errors)
    require_bool_true(payload, "credit_shortfall_reported", errors)
    require_bool_true(payload, "live_account_mutation_verified", errors)
    require_bool_true(payload, "credit_line_account_state_readback_verified", errors)
    require_bool_true(payload, "credit_accrual_posted_to_account_state", errors)
    require_bool_true(payload, "manual_approval_tier_did_not_mutate_account", errors)
    require_bool_true(payload, "account_state_reconciliation_verified", errors)
    require_bool_true(payload, "no_negative_balance", errors)
    require_zero_count(payload, "unexpected_failure_count", errors)
    require_false(payload, "raw_ledger_included", errors)


def validate_appeal_policy(payload: dict[str, Any], errors: list[str]) -> None:
    require_policy_matrix_ledger_binding(payload, errors)
    probe_count = require_positive_int(payload, "appeal_probe_count", errors)
    approved = require_non_negative_int(payload, "approved_appeal_count", errors)
    rejected = require_non_negative_int(payload, "rejected_appeal_count", errors)
    require_sum_equal(
        probe_count,
        (
            ("approved_appeal_count", approved),
            ("rejected_appeal_count", rejected),
        ),
        "appeal_probe_count",
        errors,
    )
    require_bool_true(payload, "appeal_route_present", errors)
    require_bool_true(payload, "policy_update_route_present", errors)
    require_bool_true(payload, "governance_recorded", errors)
    require_bool_true(payload, "operator_role_enforced", errors)
    require_bool_true(payload, "unauthorized_appeal_rejected", errors)
    require_bool_true(payload, "policy_digest_bound", errors)
    require_false(payload, "appeal_payloads_included", errors)


def validate_metrics_alerts(payload: dict[str, Any], errors: list[str]) -> None:
    require_policy_matrix_ledger_binding(payload, errors)
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_false(payload, "response_bodies_included", errors)


def validate_provider_bake(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_policy_matrix_ledger_binding(payload, errors)
    require_string(payload, "bake_id", errors)
    started_at = require_recent_timestamp(
        payload,
        "started_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_bake_age_secs,
    )
    completed_at = require_recent_timestamp(
        payload,
        "completed_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_bake_age_secs,
    )
    if started_at and completed_at:
        require_minimum_value(
            completed_at,
            "completed_at_unix",
            started_at,
            errors,
            message="completed_at_unix must be >= started_at_unix",
        )
    scheduled_tick_at = require_recent_timestamp(
        payload,
        "scheduled_lifecycle_canary_last_tick_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_bake_age_secs,
    )
    if completed_at and scheduled_tick_at:
        require_minimum_value(
            completed_at,
            "completed_at_unix",
            scheduled_tick_at,
            errors,
            message=(
                "completed_at_unix must be >= "
                "scheduled_lifecycle_canary_last_tick_unix"
            ),
        )
        scheduler_lag_secs = completed_at - scheduled_tick_at
        if scheduler_lag_secs > options.max_lifecycle_lag_secs:
            errors.append(
                "scheduled_lifecycle_canary_last_tick_unix must be within "
                f"{options.max_lifecycle_lag_secs} seconds of completed_at_unix"
            )
    require_count_equal(payload, "provider_count", "completed_provider_count", errors)
    require_zero_count(payload, "failure_count", errors)
    require_positive_int(payload, "rent_cycle_count", errors)
    require_positive_int(payload, "top_up_cycle_count", errors)
    require_positive_int(payload, "appeal_cycle_count", errors)
    require_bool_true(payload, "scheduler_config_bound", errors)
    require_bool_true(payload, "scheduled_lifecycle_canary_passed", errors)
    require_positive_int(payload, "scheduled_lifecycle_canary_tick_count", errors)
    require_positive_int(payload, "scheduled_lifecycle_canary_defaulted_provider_count", errors)
    require_bool_true(payload, "scheduled_lifecycle_canary_gateway_sync_verified", errors)
    require_bool_true(payload, "scheduled_lifecycle_canary_orderbook_rejection_verified", errors)
    require_bool_true(payload, "governance_packet_attached", errors)
    require_bool_true(payload, "ledger_digest_attached", errors)
    require_bool_true(payload, "dashboard_snapshot_attached", errors)
    require_false(payload, "payloads_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_policy_matrix_ledger_binding(payload, errors)
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "reserve_movement_policy_present", errors)
    require_bool_true(payload, "credit_line_policy_present", errors)
    require_bool_true(payload, "appeal_policy_present", errors)
    require_bool_true(payload, "manual_override_policy_present", errors)
    require_bool_true(payload, "provider_bake_accepted", errors)
    require_bool_true(payload, "governance_source_entries_published", errors)
    require_bool_true(payload, "downstream_compliance_policy_applied", errors)
    require_positive_int(payload, "downstream_compliance_consumer_count", errors)
    require_bool_true(payload, "non_reserve_compliance_entries_preserved", errors)
    require_bool_true(payload, "governance_source_entry_handoff_verified", errors)
    require_bool_true(payload, "denylist_and_policy_consumers_consistent", errors)
    require_policy_digest(payload, errors)


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_passed_status(payload, errors)

    if kind.name == "policy_config":
        validate_policy_config(payload, errors)
    elif kind.name == "quote_matrix":
        validate_quote_matrix(payload, errors)
    elif kind.name == "ledger_digest":
        validate_ledger_digest(payload, errors, options)
    elif kind.name == "lifecycle_service":
        validate_lifecycle_service(payload, errors, options)
    elif kind.name == "signed_routes":
        validate_signed_routes(payload, errors, options)
    elif kind.name == "reserve_movement":
        validate_reserve_movement(payload, errors)
    elif kind.name == "credit_line":
        validate_credit_line(payload, errors)
    elif kind.name == "appeal_policy":
        validate_appeal_policy(payload, errors)
    elif kind.name == "metrics_alerts":
        validate_metrics_alerts(payload, errors)
    elif kind.name == "provider_bake":
        validate_provider_bake(payload, errors, options)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS SFM-6 rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        lambda kind, checked_payload, errors: validate_kind_specific(
            kind, checked_payload, errors, options
        ),
        require_reviewed_deployment_context=True,
    )


def digest_binding(
    fingerprint: dict[str, Any],
    fields: tuple[str, ...],
) -> tuple[str, ...] | None:
    """Return a normalized digest tuple if all fields are present strings."""

    values: list[str] = []
    for field in fields:
        value = fingerprint.get(field)
        if not isinstance(value, str):
            return None
        values.append(value.lower())
    return tuple(values)


def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    options: ValidationOptions,
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []


    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    valid_provider_bakes: list[dict[str, Any]] = []
    valid_policy_digests: set[str] = set()
    valid_policy_matrix_bindings: set[tuple[str, str]] = set()
    valid_policy_matrix_ledger_bindings: set[tuple[str, str, str]] = set()
    policy_bound_artifacts: list[dict[str, Any]] = []
    ledger_bound_artifacts: list[dict[str, Any]] = []
    files = discover_evidence_files(
        evidence_dirs,
        evidence_files,
        errors,
        reserved_output_paths=() if summary_out is None else (summary_out,),
    )
    explicit = evidence_path_identities(evidence_files, errors)

    for path in files:
        loaded = load_evidence_json_with_sha256_or_record_error(
            path, MAX_EVIDENCE_BYTES, errors
        )
        if loaded is None:
            continue
        payload, digest = loaded
        kind_name, validation_errors = validate_evidence_payload(payload, options)
        if kind_name is None:
            record_explicit_evidence_validation_errors(
                path, explicit, validation_errors, errors
            )
            continue
        artifact = build_evidence_artifact(
            path,
            digest,
            payload,
            validation_errors,
            FINGERPRINT_FIELDS,
        )
        if kind_name == "provider_bake":
            bake = artifact_fingerprint(payload, PROVIDER_BAKE_FIELDS)
            artifact["bake"] = bake
            if evidence_artifact_is_valid(artifact):
                valid_provider_bakes.append(bake)
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            policy_digest = fingerprint.get("policy_digest_hex")
            matrix_digest = fingerprint.get("matrix_digest_hex")
            ledger_digest = fingerprint.get("ledger_digest_hex")
            if kind_name == "policy_config" and isinstance(policy_digest, str):
                valid_policy_digests.add(policy_digest.lower())
            if kind_name in POLICY_BOUND_KINDS:
                policy_bound_artifacts.append(artifact)
            if kind_name in LEDGER_BOUND_KINDS:
                ledger_bound_artifacts.append(artifact)
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        record_evidence_validation_errors(path, validation_errors, errors)

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=POLICY_BOUND_KINDS + LEDGER_BOUND_KINDS,
        bound_artifacts=[
            (evidence_artifact_schema(artifact), artifact)
            for artifact in policy_bound_artifacts
        ],
        missing_anchor_artifacts=[
            (evidence_artifact_schema(artifact), artifact)
            for artifact in policy_bound_artifacts + ledger_bound_artifacts
        ],
        valid_anchor_digests=valid_policy_digests,
        digest_field="policy_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex must match a valid "
            "policy_config policy_digest_hex"
        ),
        missing_anchor_error_template=(
            "reserve rollout evidence requires a valid policy_config "
            "policy_digest_hex"
        ),
        missing_anchor_summary_error=(
            "reserve rollout evidence requires a valid policy_config "
            "policy_digest_hex"
        ),
    )

    valid_policy_matrix_bindings = {
        binding
        for artifact in artifacts_by_kind["quote_matrix"]
        if evidence_artifact_is_valid(artifact)
        for binding in [
            digest_binding(
                evidence_artifact_fingerprint(artifact),
                ("policy_digest_hex", "matrix_digest_hex"),
            )
        ]
        if binding is not None
    }

    validate_bound_evidence_tuple_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("ledger_digest",) + LEDGER_BOUND_KINDS,
        bound_artifacts=[
            (evidence_artifact_schema(artifact), artifact)
            for artifact in ledger_bound_artifacts
            + [
                artifact
                for artifact in artifacts_by_kind["ledger_digest"]
                if evidence_artifact_is_valid(artifact)
            ]
        ],
        missing_anchor_artifacts=[
            (evidence_artifact_schema(artifact), artifact)
            for artifact in ledger_bound_artifacts + artifacts_by_kind["ledger_digest"]
        ],
        valid_anchor_bindings=valid_policy_matrix_bindings,
        binding_fields=("policy_digest_hex", "matrix_digest_hex"),
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex and "
            "matrix_digest_hex must match a valid quote_matrix artifact"
        ),
        missing_anchor_error_template=(
            "ledger-bound reserve evidence requires a valid quote_matrix "
            "policy_digest_hex/matrix_digest_hex tuple"
        ),
        missing_anchor_summary_error=(
            "ledger-bound reserve evidence requires a valid quote_matrix "
            "policy_digest_hex/matrix_digest_hex tuple"
        ),
    )

    valid_policy_matrix_ledger_bindings = {
        binding
        for artifact in artifacts_by_kind["ledger_digest"]
        if evidence_artifact_is_valid(artifact)
        for binding in [
            digest_binding(
                evidence_artifact_fingerprint(artifact),
                ("policy_digest_hex", "matrix_digest_hex", "ledger_digest_hex"),
            )
        ]
        if binding is not None
    }

    validate_bound_evidence_tuple_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=LEDGER_BOUND_KINDS,
        bound_artifacts=[
            (evidence_artifact_schema(artifact), artifact)
            for artifact in ledger_bound_artifacts
        ],
        valid_anchor_bindings=valid_policy_matrix_ledger_bindings,
        binding_fields=(
            "policy_digest_hex",
            "matrix_digest_hex",
            "ledger_digest_hex",
        ),
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex, matrix_digest_hex, and "
            "ledger_digest_hex must match a valid ledger_digest artifact"
        ),
        missing_anchor_error_template=(
            "ledger-bound reserve evidence requires a valid ledger_digest "
            "policy_digest_hex/matrix_digest_hex/ledger_digest_hex tuple"
        ),
        missing_anchor_summary_error=(
            "ledger-bound reserve evidence requires a valid ledger_digest "
            "policy_digest_hex/matrix_digest_hex/ledger_digest_hex tuple"
        ),
    )

    valid_provider_bakes = [
        bake
        for artifact in artifacts_by_kind["provider_bake"]
        for bake in [evidence_artifact_detail(artifact, "bake")]
        if evidence_artifact_is_valid(artifact) and bake
    ]

    required = build_required_evidence_summary(
        required_kinds,
        artifacts_by_kind,
        evidence_schema_by_kind(KIND_BY_NAME),
        errors,
        evidence_label="rollout",
    )

    summary = {
        "schema": SUMMARY_SCHEMA,
        "status": evidence_gate_status(errors),
        "required_kinds": required_evidence_kind_names(required_kinds),
        "thresholds": {
            "max_ledger_age_secs": options.max_ledger_age_secs,
            "max_lifecycle_lag_secs": options.max_lifecycle_lag_secs,
            "max_route_latency_ms": options.max_route_latency_ms,
            "max_bake_age_secs": options.max_bake_age_secs,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_policy_digests": sorted(valid_policy_digests),
        "valid_policy_matrix_bindings": [
            {
                "policy_digest_hex": policy_digest,
                "matrix_digest_hex": matrix_digest,
            }
            for policy_digest, matrix_digest in sorted(valid_policy_matrix_bindings)
        ],
        "valid_policy_matrix_ledger_bindings": [
            {
                "policy_digest_hex": policy_digest,
                "matrix_digest_hex": matrix_digest,
                "ledger_digest_hex": ledger_digest,
            }
            for policy_digest, matrix_digest, ledger_digest in sorted(
                valid_policy_matrix_ledger_bindings
            )
        ],
        "valid_provider_bakes": valid_provider_bakes,
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SFM-6 reserve/rent rollout evidence artifacts."
    )
    parser.add_argument(
        "--evidence-dir",
        action="append",
        type=Path,
        default=[],
        help="Directory containing rollout evidence JSON artifacts.",
    )
    parser.add_argument(
        "--evidence",
        action="append",
        type=Path,
        default=[],
        help="Explicit rollout evidence JSON artifact.",
    )
    parser.add_argument(
        "--require-kind",
        action="append",
        default=[],
        help="Required evidence kind, or comma-separated kinds. Defaults to all SFM-6 kinds.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        default=int(time.time()),
        help="Validator clock used for age checks. Defaults to current Unix time.",
    )
    parser.add_argument(
        "--max-ledger-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_LEDGER_AGE_SECS,
    )
    parser.add_argument(
        "--max-lifecycle-lag-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_LIFECYCLE_LAG_SECS,
    )
    parser.add_argument(
        "--max-route-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_ROUTE_LATENCY_MS,
    )
    parser.add_argument(
        "--max-bake-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_BAKE_AGE_SECS,
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_exception(error)
        return 2
    try:
        args = parser.parse_args(expanded_args)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    try:
        required_kinds = parse_required_evidence_kinds(
            args.require_kind,
            allowed_kinds=KIND_BY_NAME,
            default_required=DEFAULT_REQUIRED_KINDS,
        )
    except ValueError as error:
        emit_checker_exception(error)
        return 2

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_ledger_age_secs=args.max_ledger_age_secs,
        max_lifecycle_lag_secs=args.max_lifecycle_lag_secs,
        max_route_latency_ms=args.max_route_latency_ms,
        max_bake_age_secs=args.max_bake_age_secs,
    )
    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    summary, errors = build_summary(
        args.evidence_dir, args.evidence, required_kinds, options, args.summary_out
    )
    rendered_summary, summary_errors = render_and_write_checker_summary(
        args.summary_out, summary
    )
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2

    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS reserve/rent rollout evidence is incomplete:",
            errors,
        )
        return 1

    emit_checker_notice(
        "SoraFS reserve/rent rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s), including "
        f"{len(summary['valid_provider_bakes'])} provider bake(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
