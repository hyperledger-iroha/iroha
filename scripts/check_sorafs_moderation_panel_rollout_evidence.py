#!/usr/bin/env python3
"""Validate SoraFS moderation panel rollout evidence artifacts."""

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
from sorafs_evidence_validation import (  # noqa: E402
    build_evidence_artifact,
    count_evidence_artifacts,
    count_evidence_files,
    evidence_gate_status,
    evidence_artifact_is_valid,
    evidence_artifact_fingerprint,
    evidence_schema_by_kind,
    init_evidence_artifact_buckets,
    build_required_evidence_summary,
    record_explicit_evidence_validation_errors,
    record_evidence_artifact,
    record_evidence_validation_errors,
    require_2xx_status,
    require_bool_true,
    require_count_equal,
    require_count_value_equal,
    require_false,
    require_hex,
    require_config_backed_governance_approval,
    validate_standard_evidence_payload,
    require_maximum_int,
    require_maximum_number,
    require_maximum_value,
    require_minimum_int,
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
    require_zero_count,
    validate_bound_evidence_digest_references,
    validate_bound_evidence_tuple_references,
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


SUMMARY_SCHEMA = "sorafs.moderation_panel.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_CANARY_AGE_SECS = 24 * 60 * 60
DEFAULT_MAX_EVENT_LAG_SECS = 15 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_VIEWER_URL_TTL_SECS = 5 * 60
DEFAULT_MIN_PANEL_SIZE = 7
DEFAULT_MIN_PEERS = 4
HEX64_LEN = 64

REQUIRED_INTAKE_ROUTES = (
    "appeal_submit",
    "case_status",
    "deposit_quote",
    "deposit_confirm",
)
REQUIRED_OPERATOR_ROUTES = (
    "operator_panel",
    "bridge_plan",
    "juror_plan",
    "commit_reveal_status",
)
REQUIRED_BALLOT_ROUTES = (
    "ballot_announce",
    "ballot_commit",
    "ballot_reveal",
    "ballot_tally",
    "ballot_events",
)
REQUIRED_DECISION_ROUTES = (
    "decision_publish",
    "decision_status",
    "challenge_status",
)
REQUIRED_OUTCOMES = ("uphold", "overturn", "modify", "escalate")
REQUIRED_PUBLICATION_TARGETS = (
    "governance_dag",
    "transparency_ledger",
    "moderation_cache",
    "appeal_finance",
    "reputation",
)
REQUIRED_VIEWER_EVENT_KINDS = (
    "view",
    "seek",
    "pause",
    "screenshot_attempt",
    "download_attempt",
    "annotation",
)
REQUIRED_VIEWER_ROLES = ("juror", "auditor", "legal_reviewer")
REQUIRED_VIEWER_SECURITY_CONTROLS = (
    "strict_csp",
    "offline_mode_disabled",
    "short_lived_urls",
    "role_scoped_manifest",
    "watermark_overlay",
)
REQUIRED_VIEWER_EXPORT_TARGETS = (
    "governance_dag",
    "transparency_ledger",
)
REQUIRED_COMMIT_REVEAL_SCENARIOS = (
    "happy_path",
    "duplicate_commit",
    "mismatched_reveal",
    "late_commit",
    "late_reveal",
    "missed_quorum",
    "no_show_failover",
    "contested_challenge",
)
REQUIRED_METRICS = (
    "sorafs_moderation_panel_case_total",
    "sorafs_moderation_panel_commit_total",
    "sorafs_moderation_panel_reveal_total",
    "sorafs_moderation_panel_tally_total",
    "sorafs_moderation_panel_decision_lag_seconds",
    "sorafs_moderation_panel_no_show_total",
)
CASE_BOUND_KINDS = (
    "sortition_roster",
    "evidence_viewer",
    "operator_workflow",
    "juror_notifications",
    "commit_reveal",
    "decision_publication",
    "settlement_integration",
    "transparency_reputation",
    "e2e_panel",
    "metrics_alerts",
    "governance_approval",
)
ROSTER_BOUND_KINDS = (
    "evidence_viewer",
    "operator_workflow",
    "juror_notifications",
    "commit_reveal",
    "decision_publication",
    "settlement_integration",
    "transparency_reputation",
    "e2e_panel",
    "metrics_alerts",
    "governance_approval",
)
TALLY_BOUND_KINDS = (
    "decision_publication",
    "settlement_integration",
    "transparency_reputation",
    "e2e_panel",
    "metrics_alerts",
    "governance_approval",
)

SENSITIVE_KEYS = {
    "account_private_key",
    "access_log_body",
    "access_log_entries",
    "audit_log_body",
    "audit_log_entries",
    "authorization",
    "bearer_token",
    "body",
    "commit_payload",
    "commit_payload_b64",
    "evidence_body",
    "evidence_payload",
    "message_body",
    "nonce",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_ballot",
    "private_key",
    "raw_commit",
    "raw_access_log",
    "raw_decision",
    "raw_evidence",
    "raw_legal_hold_receipt",
    "raw_reveal",
    "raw_transparency_report",
    "reveal_payload",
    "reveal_payload_b64",
    "response_body",
    "secret",
    "signature_key",
    "signed_url",
    "signed_urls",
    "signed_transaction",
    "session_token",
    "session_tokens",
    "token",
    "url_signature",
    "watermark_secret",
    "watermark_key",
    "webauthn_assertion",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SFM-4b rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("appeal_intake", "sorafs.moderation_panel.appeal_intake_canary.v1"),
    EvidenceKind("sortition_roster", "sorafs.moderation_panel.sortition_roster_canary.v1"),
    EvidenceKind("evidence_viewer", "sorafs.moderation_panel.evidence_viewer_canary.v1"),
    EvidenceKind("operator_workflow", "sorafs.moderation_panel.operator_workflow_canary.v1"),
    EvidenceKind("juror_notifications", "sorafs.moderation_panel.juror_notifications_canary.v1"),
    EvidenceKind("commit_reveal", "sorafs.moderation_panel.commit_reveal_canary.v1"),
    EvidenceKind("decision_publication", "sorafs.moderation_panel.decision_publication_canary.v1"),
    EvidenceKind(
        "settlement_integration",
        "sorafs.moderation_panel.settlement_integration_canary.v1",
    ),
    EvidenceKind(
        "transparency_reputation",
        "sorafs.moderation_panel.transparency_reputation_canary.v1",
    ),
    EvidenceKind("e2e_panel", "sorafs.moderation_panel.e2e_panel_canary.v1"),
    EvidenceKind("metrics_alerts", "sorafs.moderation_panel.metrics_alert_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.moderation_panel.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SFM-4b moderation panel rollout gate."""

    now_unix: int
    max_canary_age_secs: int
    max_event_lag_secs: int
    max_route_latency_ms: int
    min_panel_size: int
    min_peers: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "case_digest_hex",
    "roster_hash_hex",
    "tally_digest_hex",
    "generated_at_unix",
    "peer_count",
    "validator_count",
    "case_count",
)


def validate_routes(payload: dict[str, Any], errors: list[str], options: ValidationOptions) -> None:
    for index, record in require_object_array(payload, "routes", errors):
        require_bool_true(record, "passed", errors, path=f"routes[{index}].passed")
        require_2xx_status(
            record,
            "status_code",
            errors,
            path=f"routes[{index}].status_code",
        )
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


def validate_fresh(payload: dict[str, Any], errors: list[str], options: ValidationOptions) -> None:
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_canary_age_secs,
    )


def validate_appeal_intake(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_INTAKE_ROUTES, errors)
    require_count_equal(payload, "case_count", "accepted_case_count", errors)
    require_bool_true(payload, "appellant_auth_enforced", errors)
    require_bool_true(payload, "proof_token_verified", errors)
    require_bool_true(payload, "deposit_confirmation_bound", errors)
    require_bool_true(payload, "policy_reference_bound", errors)
    require_bool_true(payload, "duplicate_case_rejected", errors)
    require_bool_true(payload, "invalid_payload_rejected", errors)
    require_false(payload, "payloads_included", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_routes(payload, errors, options)


def validate_sortition_roster(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "pop_snapshot_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_hex(payload, "sortition_seed_hex", HEX64_LEN, errors)
    panel_size = require_minimum_int(
        payload,
        "panel_size",
        options.min_panel_size,
        errors,
    )
    quorum = require_positive_int(payload, "quorum", errors)
    if panel_size:
        require_maximum_value(
            quorum,
            "quorum",
            panel_size,
            errors,
            message="quorum must be <= panel_size",
        )
    require_bool_true(payload, "pop_snapshot_bound", errors)
    require_bool_true(payload, "juror_eligibility_verified", errors)
    require_bool_true(payload, "failover_plan_present", errors)
    require_bool_true(payload, "roster_privacy_preserved", errors)
    require_false(payload, "juror_private_data_included", errors)


def validate_evidence_viewer(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    session_count = require_count_equal(payload, "session_count", "attested_session_count", errors)
    require_bool_true(payload, "attested_viewer_enabled", errors)
    require_bool_true(payload, "role_scoped_manifest_verified", errors)
    require_bool_true(payload, "short_lived_urls_verified", errors)
    require_bool_true(payload, "session_key_workflow_verified", errors)
    require_bool_true(payload, "strict_csp_enforced", errors)
    require_bool_true(payload, "offline_mode_disabled", errors)
    require_bool_true(payload, "per_session_access_logged", errors)
    require_bool_true(payload, "append_only_log_verified", errors)
    require_bool_true(payload, "anomaly_events_recorded", errors)
    require_bool_true(payload, "watermark_overlay_rendered", errors)
    require_bool_true(payload, "watermark_metadata_hashed", errors)
    require_bool_true(payload, "audit_digest_exported", errors)
    require_bool_true(payload, "transparency_report_exported", errors)
    require_bool_true(payload, "daily_digest_published", errors)
    require_bool_true(payload, "payload_redaction_verified", errors)
    require_bool_true(payload, "denylisted_digest_blocked", errors)
    require_bool_true(payload, "unauthorized_access_rejected", errors)
    require_bool_true(payload, "stale_url_rejected", errors)
    require_bool_true(payload, "session_replay_rejected", errors)
    require_bool_true(payload, "legal_hold_policy_bound", errors)
    require_maximum_int(
        payload,
        "max_url_ttl_secs",
        DEFAULT_MAX_VIEWER_URL_TTL_SECS,
        errors,
        minimum=1,
    )
    require_count_value_equal(
        payload,
        "logged_session_count",
        session_count,
        "session_count",
        errors,
    )
    require_string_coverage(payload, "roles_tested", "", REQUIRED_VIEWER_ROLES, errors)
    require_string_coverage(
        payload,
        "viewer_security_controls",
        "",
        REQUIRED_VIEWER_SECURITY_CONTROLS,
        errors,
    )
    require_string_coverage(
        payload,
        "access_event_kinds",
        "",
        REQUIRED_VIEWER_EVENT_KINDS,
        errors,
    )
    require_string_coverage(
        payload,
        "export_targets",
        "",
        REQUIRED_VIEWER_EXPORT_TARGETS,
        errors,
    )
    require_hex(payload, "session_manifest_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "watermark_metadata_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "access_log_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "legal_hold_receipt_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "transparency_report_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "audit_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_evidence_included", errors)
    require_false(payload, "session_tokens_included", errors)
    require_false(payload, "signed_urls_included", errors)
    require_false(payload, "watermark_secrets_included", errors)
    require_false(payload, "response_bodies_included", errors)


def validate_operator_workflow(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_OPERATOR_ROUTES, errors)
    require_bool_true(payload, "operator_role_enforced", errors)
    require_bool_true(payload, "bridge_plan_generated", errors)
    require_bool_true(payload, "juror_plan_generated", errors)
    require_bool_true(payload, "mutation_forwarding_signed", errors)
    require_bool_true(payload, "payload_bytes_rejected", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_routes(payload, errors, options)


def validate_juror_notifications(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_count_equal(payload, "notification_count", "delivered_notification_count", errors)
    require_positive_int(payload, "juror_count", errors)
    require_bool_true(payload, "dedup_keys_verified", errors)
    require_bool_true(payload, "transport_canary_passed", errors)
    require_bool_true(payload, "retry_policy_verified", errors)
    require_bool_true(payload, "private_payloads_rejected", errors)
    require_false(payload, "message_bodies_included", errors)
    require_false(payload, "response_bodies_included", errors)


def validate_commit_reveal(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_BALLOT_ROUTES, errors)
    panel_size = require_minimum_int(
        payload,
        "panel_size",
        options.min_panel_size,
        errors,
    )
    require_positive_int(payload, "commit_count", errors)
    require_positive_int(payload, "reveal_count", errors)
    require_bool_true(payload, "commit_auth_bound_to_juror", errors)
    require_bool_true(payload, "reveal_auth_bound_to_juror", errors)
    require_bool_true(payload, "quorum_satisfied", errors)
    require_bool_true(payload, "challenge_buffer_enforced", errors)
    require_bool_true(payload, "contested_tie_detected", errors)
    require_bool_true(payload, "commit_digest_recomputed", errors)
    require_bool_true(payload, "duplicate_commit_rejected", errors)
    require_bool_true(payload, "mismatched_reveal_rejected", errors)
    require_bool_true(payload, "late_commit_rejected", errors)
    require_bool_true(payload, "late_reveal_rejected", errors)
    require_bool_true(payload, "missed_quorum_detected", errors)
    require_bool_true(payload, "no_show_failover_exercised", errors)
    require_bool_true(payload, "juror_penalty_plan_emitted", errors)
    require_bool_true(payload, "tally_deterministic_replay_verified", errors)
    require_bool_true(payload, "governance_event_digest_bound", errors)
    require_bool_true(payload, "executor_canary_passed", errors)
    require_maximum_number(
        payload,
        "max_event_lag_seconds",
        options.max_event_lag_secs,
        errors,
    )
    require_string_coverage(
        payload,
        "scenarios_exercised",
        "",
        REQUIRED_COMMIT_REVEAL_SCENARIOS,
        errors,
    )
    require_hex(payload, "tally_digest_hex", HEX64_LEN, errors)
    require_false(payload, "commit_payloads_included", errors)
    require_false(payload, "reveal_payloads_included", errors)
    validate_routes(payload, errors, options)


def validate_decision_publication(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_hex(payload, "tally_digest_hex", HEX64_LEN, errors)
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_DECISION_ROUTES, errors)
    require_string_coverage(payload, "outcomes", "", REQUIRED_OUTCOMES, errors)
    require_bool_true(payload, "decision_signature_verified", errors)
    require_bool_true(payload, "governance_dag_event_published", errors)
    require_bool_true(payload, "public_decision_trail_published", errors)
    require_bool_true(payload, "challenge_dag_bound", errors)
    require_false(payload, "raw_decision_included", errors)
    validate_routes(payload, errors, options)


def validate_settlement_integration(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_hex(payload, "tally_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "settlement_count", errors)
    require_bool_true(payload, "appeal_finance_report_published", errors)
    require_bool_true(payload, "settlement_receipt_published", errors)
    require_bool_true(payload, "treasury_reconciliation_passed", errors)
    require_bool_true(payload, "no_show_penalties_applied", errors)
    require_bool_true(payload, "reputation_penalty_handoff_present", errors)
    require_false(payload, "signed_transaction_included", errors)
    require_false(payload, "raw_ledger_included", errors)


def validate_transparency_reputation(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_hex(payload, "tally_digest_hex", HEX64_LEN, errors)
    require_string_coverage(
        payload,
        "publication_targets",
        "",
        REQUIRED_PUBLICATION_TARGETS,
        errors,
    )
    require_bool_true(payload, "moderation_cache_updated", errors)
    require_bool_true(payload, "transparency_source_entry_published", errors)
    require_bool_true(payload, "privacy_aggregate_updated", errors)
    require_bool_true(payload, "reputation_delta_applied", errors)
    require_bool_true(payload, "gateway_compliance_cache_updated", errors)
    require_false(payload, "payloads_included", errors)


def validate_e2e_panel(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    validate_fresh(payload, errors, options)
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_hex(payload, "tally_digest_hex", HEX64_LEN, errors)
    require_minimum_int(payload, "peer_count", options.min_peers, errors)
    require_minimum_int(payload, "validator_count", options.min_peers, errors)
    require_positive_int(payload, "case_count", errors)
    require_bool_true(payload, "appeal_submission_verified", errors)
    require_bool_true(payload, "juror_selection_verified", errors)
    require_bool_true(payload, "evidence_access_verified", errors)
    require_bool_true(payload, "commit_reveal_verified", errors)
    require_bool_true(payload, "decision_publication_verified", errors)
    require_bool_true(payload, "settlement_verified", errors)
    require_bool_true(payload, "all_peers_reconciled", errors)
    require_zero_count(payload, "unexpected_failure_count", errors)
    require_false(payload, "raw_evidence_included", errors)


def validate_metrics_alerts(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_hex(payload, "tally_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_false(payload, "response_bodies_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "case_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "roster_hash_hex", HEX64_LEN, errors)
    require_hex(payload, "tally_digest_hex", HEX64_LEN, errors)
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "appeal_intake_policy_present", errors)
    require_bool_true(payload, "sortition_policy_present", errors)
    require_bool_true(payload, "evidence_access_policy_present", errors)
    require_bool_true(payload, "commit_reveal_policy_present", errors)
    require_bool_true(payload, "settlement_policy_present", errors)
    require_bool_true(payload, "public_decision_policy_present", errors)
    require_bool_true(payload, "e2e_panel_evidence_accepted", errors)
    require_policy_digest(payload, errors)


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_passed_status(payload, errors)

    if kind.name == "appeal_intake":
        validate_appeal_intake(payload, errors, options)
    elif kind.name == "sortition_roster":
        validate_sortition_roster(payload, errors, options)
    elif kind.name == "evidence_viewer":
        validate_evidence_viewer(payload, errors, options)
    elif kind.name == "operator_workflow":
        validate_operator_workflow(payload, errors, options)
    elif kind.name == "juror_notifications":
        validate_juror_notifications(payload, errors, options)
    elif kind.name == "commit_reveal":
        validate_commit_reveal(payload, errors, options)
    elif kind.name == "decision_publication":
        validate_decision_publication(payload, errors, options)
    elif kind.name == "settlement_integration":
        validate_settlement_integration(payload, errors, options)
    elif kind.name == "transparency_reputation":
        validate_transparency_reputation(payload, errors, options)
    elif kind.name == "e2e_panel":
        validate_e2e_panel(payload, errors, options)
    elif kind.name == "metrics_alerts":
        validate_metrics_alerts(payload, errors)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS SFM-4b rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        lambda kind, checked_payload, errors: validate_kind_specific(
            kind, checked_payload, errors, options
        ),
    )



def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    options: ValidationOptions,
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    valid_case_digests: set[str] = set()
    case_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_roster_bindings: set[tuple[str, str]] = set()
    roster_candidate_artifacts: list[dict[str, Any]] = []
    roster_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_tally_bindings: set[tuple[str, str, str]] = set()
    tally_candidate_artifacts: list[dict[str, Any]] = []
    tally_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    e2e_candidate_artifacts: list[dict[str, Any]] = []
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
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            case_digest = fingerprint.get("case_digest_hex")
            roster_hash = fingerprint.get("roster_hash_hex")
            tally_digest = fingerprint.get("tally_digest_hex")
            if kind_name == "appeal_intake" and isinstance(case_digest, str):
                valid_case_digests.add(case_digest.lower())
            elif kind_name in CASE_BOUND_KINDS:
                case_bound_artifacts.append((kind_name, artifact))
            if (
                kind_name == "sortition_roster"
                and isinstance(case_digest, str)
                and isinstance(roster_hash, str)
            ):
                roster_candidate_artifacts.append(artifact)
            elif kind_name in ROSTER_BOUND_KINDS:
                roster_bound_artifacts.append((kind_name, artifact))
            if (
                kind_name == "commit_reveal"
                and isinstance(case_digest, str)
                and isinstance(roster_hash, str)
                and isinstance(tally_digest, str)
            ):
                tally_candidate_artifacts.append(artifact)
            elif kind_name in TALLY_BOUND_KINDS:
                tally_bound_artifacts.append((kind_name, artifact))
            if kind_name == "e2e_panel":
                e2e_candidate_artifacts.append(artifact)
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact)
        record_evidence_validation_errors(path, validation_errors, errors)


    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=tuple(KIND_BY_NAME),
        bound_artifacts=case_bound_artifacts,
        valid_anchor_digests=valid_case_digests,
        digest_field="case_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} case_digest_hex must match a valid "
            "appeal_intake case_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} case_digest_hex must match a valid "
            "appeal_intake case_digest_hex"
        ),
    )

    for artifact in roster_candidate_artifacts:
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            case_digest = fingerprint.get("case_digest_hex")
            roster_hash = fingerprint.get("roster_hash_hex")
            if isinstance(case_digest, str) and isinstance(roster_hash, str):
                valid_roster_bindings.add((case_digest.lower(), roster_hash.lower()))

    validate_bound_evidence_tuple_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=tuple(KIND_BY_NAME),
        bound_artifacts=roster_bound_artifacts,
        valid_anchor_bindings=valid_roster_bindings,
        binding_fields=("case_digest_hex", "roster_hash_hex"),
        errors=errors,
        binding_error_template=(
            "{kind_name} case_digest_hex and roster_hash_hex "
            "must match a valid case-bound sortition_roster artifact"
        ),
        missing_anchor_error_template=(
            "{kind_name} case_digest_hex and roster_hash_hex "
            "must match a valid case-bound sortition_roster artifact"
        ),
    )

    for artifact in tally_candidate_artifacts:
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            case_digest = fingerprint.get("case_digest_hex")
            roster_hash = fingerprint.get("roster_hash_hex")
            tally_digest = fingerprint.get("tally_digest_hex")
            if (
                isinstance(case_digest, str)
                and isinstance(roster_hash, str)
                and isinstance(tally_digest, str)
            ):
                valid_tally_bindings.add(
                    (case_digest.lower(), roster_hash.lower(), tally_digest.lower())
                )

    validate_bound_evidence_tuple_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=tuple(KIND_BY_NAME),
        bound_artifacts=tally_bound_artifacts,
        valid_anchor_bindings=valid_tally_bindings,
        binding_fields=("case_digest_hex", "roster_hash_hex", "tally_digest_hex"),
        errors=errors,
        binding_error_template=(
            "{kind_name} case_digest_hex, roster_hash_hex, and "
            "tally_digest_hex must match a valid roster-bound commit_reveal artifact"
        ),
        missing_anchor_error_template=(
            "{kind_name} case_digest_hex, roster_hash_hex, and "
            "tally_digest_hex must match a valid roster-bound commit_reveal artifact"
        ),
    )

    valid_e2e_runs = [
        evidence_artifact_fingerprint(artifact)
        for artifact in e2e_candidate_artifacts
        if evidence_artifact_is_valid(artifact)
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
            "max_canary_age_secs": options.max_canary_age_secs,
            "max_event_lag_secs": options.max_event_lag_secs,
            "max_route_latency_ms": options.max_route_latency_ms,
            "min_panel_size": options.min_panel_size,
            "min_peers": options.min_peers,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_case_digests": sorted(valid_case_digests),
        "valid_roster_bindings": [
            {
                "case_digest_hex": case_digest,
                "roster_hash_hex": roster_hash,
            }
            for case_digest, roster_hash in sorted(valid_roster_bindings)
        ],
        "valid_tally_bindings": [
            {
                "case_digest_hex": case_digest,
                "roster_hash_hex": roster_hash,
                "tally_digest_hex": tally_digest,
            }
            for case_digest, roster_hash, tally_digest in sorted(valid_tally_bindings)
        ],
        "valid_e2e_runs": valid_e2e_runs,
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SFM-4b moderation panel rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SFM-4b kinds.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        default=int(time.time()),
        help="Validator clock used for age checks. Defaults to current Unix time.",
    )
    parser.add_argument(
        "--max-canary-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_CANARY_AGE_SECS,
    )
    parser.add_argument(
        "--max-event-lag-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVENT_LAG_SECS,
    )
    parser.add_argument(
        "--max-route-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_ROUTE_LATENCY_MS,
    )
    parser.add_argument("--min-panel-size", type=positive_int_arg, default=DEFAULT_MIN_PANEL_SIZE)
    parser.add_argument("--min-peers", type=positive_int_arg, default=DEFAULT_MIN_PEERS)
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_checker_error_lines((str(error),))
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
        emit_checker_error_lines((str(error),))
        return 2

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_canary_age_secs=args.max_canary_age_secs,
        max_event_lag_secs=args.max_event_lag_secs,
        max_route_latency_ms=args.max_route_latency_ms,
        min_panel_size=args.min_panel_size,
        min_peers=args.min_peers,
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
            "ERROR: SoraFS moderation panel rollout evidence is incomplete:",
            errors,
        )
        return 1

    emit_checker_notice(
        "SoraFS moderation panel rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s), including "
        f"{len(summary['valid_e2e_runs'])} end-to-end panel run(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
