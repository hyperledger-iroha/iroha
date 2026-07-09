#!/usr/bin/env python3
"""Build payload-free SoraFS moderation panel rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import sys
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_moderation_panel_rollout_evidence import (  # noqa: E402
    APPEAL_CASE_LABEL_ERROR,
    APPEAL_CASE_LABEL_PATTERN,
    COMMIT_LABEL_ERROR,
    COMMIT_LABEL_PATTERN,
    DEFAULT_MAX_CANARY_AGE_SECS,
    DEFAULT_MAX_EVENT_LAG_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MAX_VIEWER_URL_TTL_SECS,
    DEFAULT_MIN_PANEL_SIZE,
    DEFAULT_MIN_PEERS,
    E2E_CASE_LABEL_ERROR,
    E2E_CASE_LABEL_PATTERN,
    E2E_PEER_LABEL_ERROR,
    E2E_PEER_LABEL_PATTERN,
    E2E_VALIDATOR_LABEL_ERROR,
    E2E_VALIDATOR_LABEL_PATTERN,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    JUROR_LABEL_ERROR,
    JUROR_LABEL_PATTERN,
    KIND_BY_NAME,
    NOTIFICATION_LABEL_ERROR,
    NOTIFICATION_LABEL_PATTERN,
    REVEAL_LABEL_ERROR,
    REVEAL_LABEL_PATTERN,
    ROSTER_JUROR_LABEL_ERROR,
    ROSTER_JUROR_LABEL_PATTERN,
    SETTLEMENT_LABEL_ERROR,
    SETTLEMENT_LABEL_PATTERN,
    VIEWER_SESSION_LABEL_ERROR,
    VIEWER_SESSION_LABEL_PATTERN,
    REQUIRED_BALLOT_ROUTES,
    REQUIRED_COMMIT_REVEAL_SCENARIOS,
    REQUIRED_DECISION_ROUTES,
    REQUIRED_INTAKE_ROUTES,
    REQUIRED_METRICS,
    REQUIRED_OPERATOR_ROUTES,
    REQUIRED_OUTCOMES,
    REQUIRED_PUBLICATION_TARGETS,
    REQUIRED_VIEWER_EVENT_KINDS,
    REQUIRED_VIEWER_EXPORT_TARGETS,
    REQUIRED_VIEWER_ROLES,
    REQUIRED_VIEWER_SECURITY_CONTROLS,
    ValidationOptions,
    validate_evidence_payload,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    fsync_checker_output_parent,
    write_all_checker_summary_bytes,
    validate_checker_output_parent,
)
from sorafs_path_identity import (  # noqa: E402
    diagnostic_text_is_canonical,
    error_diagnostic_label,
    path_diagnostic_label,
)
from sorafs_evidence_validation import (  # noqa: E402
    forbidden_non_production_markers,
    require_rollout_deployment_id,
    require_rollout_environment,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


CANARY_KINDS = tuple(KIND_BY_NAME)
ROSTER_DIGEST_KINDS = tuple(kind for kind in CANARY_KINDS if kind != "appeal_intake")
TALLY_DIGEST_KINDS = (
    "commit_reveal",
    "decision_publication",
    "settlement_integration",
    "transparency_reputation",
    "e2e_panel",
    "metrics_alerts",
    "governance_approval",
)
ROUTE_BODY_DIGEST_KINDS = (
    "appeal_intake",
    "operator_workflow",
    "commit_reveal",
    "decision_publication",
)
HEX64_LEN = 64
TRUE_CLAIMS: dict[str, tuple[str, ...]] = {
    "appeal_intake": (
        "appellant_auth_enforced",
        "proof_token_verified",
        "deposit_confirmation_bound",
        "policy_reference_bound",
        "duplicate_case_rejected",
        "invalid_payload_rejected",
    ),
    "sortition_roster": (
        "pop_snapshot_bound",
        "juror_eligibility_verified",
        "failover_plan_present",
        "roster_privacy_preserved",
    ),
    "evidence_viewer": (
        "attested_viewer_enabled",
        "role_scoped_manifest_verified",
        "short_lived_urls_verified",
        "session_key_workflow_verified",
        "strict_csp_enforced",
        "offline_mode_disabled",
        "per_session_access_logged",
        "append_only_log_verified",
        "audit_log_tamper_rejected",
        "anomaly_events_recorded",
        "watermark_overlay_rendered",
        "watermark_metadata_hashed",
        "watermark_metadata_mismatch_rejected",
        "audit_digest_exported",
        "transparency_report_exported",
        "daily_digest_published",
        "payload_redaction_verified",
        "denylisted_digest_blocked",
        "unauthorized_access_rejected",
        "stale_url_rejected",
        "session_replay_rejected",
        "legal_hold_policy_bound",
    ),
    "operator_workflow": (
        "operator_role_enforced",
        "bridge_plan_generated",
        "juror_plan_generated",
        "mutation_forwarding_signed",
        "payload_bytes_rejected",
    ),
    "juror_notifications": (
        "dedup_keys_verified",
        "transport_canary_passed",
        "retry_policy_verified",
        "private_payloads_rejected",
    ),
    "commit_reveal": (
        "commit_auth_bound_to_juror",
        "reveal_auth_bound_to_juror",
        "quorum_satisfied",
        "challenge_buffer_enforced",
        "contested_tie_detected",
        "commit_digest_recomputed",
        "duplicate_commit_rejected",
        "mismatched_reveal_rejected",
        "late_commit_rejected",
        "late_reveal_rejected",
        "missed_quorum_detected",
        "no_show_failover_exercised",
        "juror_penalty_plan_emitted",
        "tally_deterministic_replay_verified",
        "governance_event_digest_bound",
        "executor_canary_passed",
    ),
    "decision_publication": (
        "decision_signature_verified",
        "governance_dag_event_published",
        "public_decision_trail_published",
        "challenge_dag_bound",
    ),
    "settlement_integration": (
        "appeal_finance_report_published",
        "settlement_receipt_published",
        "treasury_reconciliation_passed",
        "no_show_penalties_applied",
        "reputation_penalty_handoff_present",
    ),
    "transparency_reputation": (
        "moderation_cache_updated",
        "transparency_source_entry_published",
        "privacy_aggregate_updated",
        "reputation_delta_applied",
        "gateway_compliance_cache_updated",
    ),
    "e2e_panel": (
        "appeal_submission_verified",
        "juror_selection_verified",
        "evidence_access_verified",
        "commit_reveal_verified",
        "decision_publication_verified",
        "settlement_verified",
        "all_peers_reconciled",
    ),
    "metrics_alerts": (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
    ),
    "governance_approval": (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "appeal_intake_policy_present",
        "sortition_policy_present",
        "evidence_access_policy_present",
        "commit_reveal_policy_present",
        "settlement_policy_present",
        "public_decision_policy_present",
        "e2e_panel_evidence_accepted",
    ),
}
FORCED_FALSE_FIELDS: dict[str, tuple[str, ...]] = {
    "appeal_intake": ("payloads_included", "response_bodies_included"),
    "sortition_roster": ("juror_private_data_included",),
    "evidence_viewer": (
        "raw_evidence_included",
        "session_tokens_included",
        "signed_urls_included",
        "watermark_secrets_included",
        "response_bodies_included",
    ),
    "operator_workflow": ("response_bodies_included",),
    "juror_notifications": ("message_bodies_included", "response_bodies_included"),
    "commit_reveal": ("commit_payloads_included", "reveal_payloads_included"),
    "decision_publication": ("raw_decision_included",),
    "settlement_integration": ("signed_transaction_included", "raw_ledger_included"),
    "transparency_reputation": ("payloads_included",),
    "e2e_panel": ("raw_evidence_included",),
    "metrics_alerts": ("critical_alerts_firing", "response_bodies_included"),
    "governance_approval": (),
}


def split_csv_values(values: Sequence[str]) -> list[str]:
    """Split repeated comma-separated CLI values into exact strings."""

    items: list[str] = []
    for value in values:
        items.extend(value.split(","))
    return items


def validate_name_set(
    values: Iterable[str],
    *,
    allowed: Sequence[str],
    option: str,
    errors: list[str],
) -> list[str]:
    """Return allowed-order values, requiring complete known non-duplicate coverage."""

    values = tuple(values)
    allowed_set = frozenset(allowed)
    value_set = frozenset(values)
    if len(value_set) != len(values):
        errors.append(f"{option} must not contain duplicates")
    if any(name not in allowed_set for name in value_set):
        errors.append(f"{option} contains an unknown value")
    missing = [name for name in allowed if name not in value_set]
    if missing:
        errors.append(f"{option} must include every required value")
    return [name for name in allowed if name in value_set]


def render_inventory_label_error(label_error: str, option: str) -> str:
    """Render checker inventory label diagnostics as CLI option diagnostics."""

    return (
        label_error.replace("jurors[].name", option)
        .replace("cases[].name", option)
        .replace("sessions[].name", option)
        .replace("notifications[].name", option)
        .replace("commits[].name", option)
        .replace("reveals[].name", option)
        .replace("settlements[].name", option)
        .replace("peers[].name", option)
        .replace("validators[].name", option)
        .replace("cases[].name", option)
    )


def validate_reviewed_inventory(
    values: Iterable[str],
    *,
    expected_count: int,
    option: str,
    kind: str,
    count_option: str,
    errors: list[str],
    pattern: re.Pattern[str] | None = None,
    label_error: str | None = None,
) -> list[str]:
    """Return reviewed unique inventory labels whose count matches a CLI count."""

    items = list(values)
    if not items:
        errors.append(f"{option} is required for {kind}")
    for index, item in enumerate(items):
        validate_canonical_string(item, label=f"{option}[{index}]", errors=errors)
        if pattern is None or not isinstance(item, str):
            continue
        if pattern.fullmatch(item) is None:
            errors.append(
                render_inventory_label_error(
                    label_error or f"{option} must use the expected label family",
                    option,
                )
            )
            continue
        forbidden = forbidden_non_production_markers(item, FORBIDDEN_INVENTORY_LABEL_MARKERS)
        if forbidden:
            errors.append(
                f"{option}[{index}] must not contain non-production markers {forbidden}"
            )
    unique_items = set(items)
    if len(unique_items) != len(items):
        errors.append(f"{option} must not contain duplicates")
    if len(unique_items) != expected_count:
        errors.append(f"{option} unique values must match {count_option}")
    return items


def validate_output_path(path: Path, errors: list[str]) -> None:
    """Reject unsafe output targets before writing a canary artifact."""

    if not isinstance(path, Path):
        errors.append(f"--out `{path_diagnostic_label(path)}` must be a path")
        return
    try:
        if path.is_symlink():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a symlink")
            return
        if path.exists() and path.is_dir():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a directory")
            return
    except (OSError, RuntimeError) as error:
        del error
        errors.append(f"--out `{path_diagnostic_label(path)}` cannot be inspected")
        return
    validate_checker_output_parent(path, errors, label="--out")


def validate_hex64(value: str | None, *, option: str, errors: list[str]) -> None:
    """Validate an exact lowercase 32-byte digest hex string."""

    if (
        not isinstance(value, str)
        or len(value) != HEX64_LEN
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{option} must be exact lowercase 32-byte hex")


def validate_canonical_string(value: str | None, *, label: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control/format text."""

    if not diagnostic_text_is_canonical(value):
        errors.append(f"{label} must be a non-empty canonical string")


def require_kind_options(
    args: argparse.Namespace,
    errors: list[str],
    required: Sequence[tuple[str, Any]],
) -> None:
    """Require kind-specific options by stable CLI flag."""

    for option, value in required:
        if value is None:
            errors.append(f"{option} is required for {args.kind}")


def build_common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by moderation panel canary payloads."""

    payload: dict[str, Any] = {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
        "case_digest_hex": args.case_digest_hex,
    }
    if args.kind in ROSTER_DIGEST_KINDS:
        payload["roster_hash_hex"] = args.roster_hash_hex
    if args.kind in TALLY_DIGEST_KINDS:
        payload["tally_digest_hex"] = args.tally_digest_hex
    return payload


def apply_verified_claims(payload: dict[str, Any], args: argparse.Namespace) -> None:
    """Populate explicitly verified true claims and forced payload-free false flags."""

    for claim in TRUE_CLAIMS[args.kind]:
        payload[claim] = claim in args.verified_claims
    for field in FORCED_FALSE_FIELDS[args.kind]:
        payload[field] = False


def build_route_records(args: argparse.Namespace, routes: Sequence[str]) -> list[dict[str, Any]]:
    """Build payload-free moderation route probe records."""

    return [
        {
            "name": route,
            "passed": True,
            "status_code": args.route_status_code,
            "body_blake3_hex": args.route_body_blake3_hex,
            "authz_enforced": True,
            "signature_verified": True,
            "latency_ms": args.route_latency_ms,
        }
        for route in routes
    ]


def build_inventory_records(names: Sequence[str]) -> list[dict[str, str]]:
    """Build reviewed payload-free inventory records."""

    return [{"name": name} for name in names]


def build_case_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build reviewed payload-free appeal-intake case records."""

    return [{"name": name, "accepted": True} for name in names]


def build_roster_juror_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build reviewed payload-free sortition roster juror records."""

    return [{"name": name, "eligible": True} for name in names]


def build_e2e_case_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build reviewed payload-free end-to-end panel case records."""

    return [{"name": name, "passed": True} for name in names]


def build_viewer_session_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build reviewed payload-free evidence-viewer session records."""

    return [{"name": name, "attested": True, "logged": True} for name in names]


def build_notification_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build reviewed payload-free juror notification records."""

    return [{"name": name, "delivered": True} for name in names]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free moderation panel rollout canary payload."""

    payload = build_common_payload(args)
    apply_verified_claims(payload, args)
    if args.kind == "appeal_intake":
        routes = build_route_records(args, args.intake_routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "case_count": args.case_count,
                "accepted_case_count": args.case_count,
                "cases": build_case_records(args.cases),
            }
        )
    elif args.kind == "sortition_roster":
        payload.update(
            {
                "pop_snapshot_digest_hex": args.pop_snapshot_digest_hex,
                "sortition_seed_hex": args.sortition_seed_hex,
                "panel_size": args.panel_size,
                "jurors": build_roster_juror_records(args.roster_jurors),
                "quorum": args.quorum,
            }
        )
    elif args.kind == "evidence_viewer":
        payload.update(
            {
                "session_count": args.session_count,
                "attested_session_count": args.session_count,
                "logged_session_count": args.session_count,
                "sessions": build_viewer_session_records(args.viewer_sessions),
                "max_url_ttl_secs": args.max_url_ttl_secs,
                "role_count": len(args.viewer_roles),
                "roles_tested": args.viewer_roles,
                "security_control_count": len(args.viewer_security_controls),
                "viewer_security_controls": args.viewer_security_controls,
                "access_event_kind_count": len(args.viewer_event_kinds),
                "access_event_kinds": args.viewer_event_kinds,
                "export_target_count": len(args.viewer_export_targets),
                "export_targets": args.viewer_export_targets,
                "session_manifest_digest_hex": args.session_manifest_digest_hex,
                "watermark_metadata_digest_hex": args.watermark_metadata_digest_hex,
                "access_log_digest_hex": args.access_log_digest_hex,
                "legal_hold_receipt_digest_hex": args.legal_hold_receipt_digest_hex,
                "transparency_report_digest_hex": args.transparency_report_digest_hex,
                "audit_digest_hex": args.audit_digest_hex,
            }
        )
    elif args.kind == "operator_workflow":
        routes = build_route_records(args, args.operator_routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
            }
        )
    elif args.kind == "juror_notifications":
        payload.update(
            {
                "notification_count": args.notification_count,
                "delivered_notification_count": args.notification_count,
                "notifications": build_notification_records(args.notifications),
                "juror_count": args.juror_count,
                "jurors": build_inventory_records(args.jurors),
            }
        )
    elif args.kind == "commit_reveal":
        routes = build_route_records(args, args.ballot_routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "panel_size": args.panel_size,
                "commit_count": args.commit_count,
                "commits": build_inventory_records(args.commits),
                "reveal_count": args.reveal_count,
                "reveals": build_inventory_records(args.reveals),
                "max_event_lag_seconds": args.max_event_lag_seconds,
                "scenario_count": len(args.scenarios_exercised),
                "scenarios_exercised": args.scenarios_exercised,
            }
        )
    elif args.kind == "decision_publication":
        routes = build_route_records(args, args.decision_routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "outcome_count": len(args.outcomes),
                "outcomes": args.outcomes,
            }
        )
    elif args.kind == "settlement_integration":
        payload.update(
            {
                "settlement_count": args.settlement_count,
                "settlements": build_inventory_records(args.settlements),
            }
        )
    elif args.kind == "transparency_reputation":
        payload["publication_target_count"] = len(args.publication_targets)
        payload["publication_targets"] = args.publication_targets
    elif args.kind == "e2e_panel":
        payload.update(
            {
                "policy_digest_hex": args.policy_digest_hex,
                "peer_count": args.peer_count,
                "peers": build_inventory_records(args.peers),
                "validator_count": args.validator_count,
                "validators": build_inventory_records(args.validators),
                "case_count": args.case_count,
                "cases": build_e2e_case_records(args.panel_cases),
                "unexpected_failure_count": 0,
            }
        )
    elif args.kind == "metrics_alerts":
        payload.update(
            {
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "config_source": "iroha_config",
                "policy_digest_hex": args.policy_digest_hex,
            }
        )
    return payload


def validate_thresholds(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate shared thresholds before payload construction."""

    if args.route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS:
        errors.append(f"--route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if (
        args.max_event_lag_seconds is not None
        and args.max_event_lag_seconds > DEFAULT_MAX_EVENT_LAG_SECS
    ):
        errors.append(f"--max-event-lag-seconds must be <= {DEFAULT_MAX_EVENT_LAG_SECS}")
    if args.max_url_ttl_secs is not None and args.max_url_ttl_secs > DEFAULT_MAX_VIEWER_URL_TTL_SECS:
        errors.append(f"--max-url-ttl-secs must be <= {DEFAULT_MAX_VIEWER_URL_TTL_SECS}")


def validate_common_digests(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate binding digest arguments for the selected kind."""

    validate_hex64(args.case_digest_hex, option="--case-digest-hex", errors=errors)
    if args.kind in ROSTER_DIGEST_KINDS:
        validate_hex64(args.roster_hash_hex, option="--roster-hash-hex", errors=errors)
    if args.kind in TALLY_DIGEST_KINDS:
        validate_hex64(args.tally_digest_hex, option="--tally-digest-hex", errors=errors)
    if args.kind in ROUTE_BODY_DIGEST_KINDS:
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
        )


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    args.verified_claims = validate_name_set(
        split_csv_values(args.verified_claim),
        allowed=TRUE_CLAIMS[args.kind],
        option="--verified-claim",
        errors=errors,
    )
    if args.kind == "appeal_intake":
        require_kind_options(args, errors, (("--case-count", args.case_count),))
        args.cases = validate_reviewed_inventory(
            split_csv_values(args.case),
            expected_count=args.case_count or 0,
            option="--case",
            kind="appeal_intake",
            count_option="--case-count",
            errors=errors,
            pattern=APPEAL_CASE_LABEL_PATTERN,
            label_error=APPEAL_CASE_LABEL_ERROR,
        )
        args.intake_routes = validate_name_set(
            split_csv_values(args.intake_route),
            allowed=REQUIRED_INTAKE_ROUTES,
            option="--intake-route",
            errors=errors,
        )
    elif args.kind == "sortition_roster":
        require_kind_options(
            args,
            errors,
            (
                ("--pop-snapshot-digest-hex", args.pop_snapshot_digest_hex),
                ("--sortition-seed-hex", args.sortition_seed_hex),
                ("--panel-size", args.panel_size),
                ("--quorum", args.quorum),
            ),
        )
        validate_hex64(
            args.pop_snapshot_digest_hex,
            option="--pop-snapshot-digest-hex",
            errors=errors,
        )
        validate_hex64(
            args.sortition_seed_hex,
            option="--sortition-seed-hex",
            errors=errors,
        )
        args.roster_jurors = validate_reviewed_inventory(
            split_csv_values(args.roster_juror),
            expected_count=args.panel_size or 0,
            option="--roster-juror",
            kind="sortition_roster",
            count_option="--panel-size",
            errors=errors,
            pattern=ROSTER_JUROR_LABEL_PATTERN,
            label_error=ROSTER_JUROR_LABEL_ERROR,
        )
    elif args.kind == "evidence_viewer":
        require_kind_options(
            args,
            errors,
            (
                ("--session-count", args.session_count),
                ("--max-url-ttl-secs", args.max_url_ttl_secs),
                ("--session-manifest-digest-hex", args.session_manifest_digest_hex),
                ("--watermark-metadata-digest-hex", args.watermark_metadata_digest_hex),
                ("--access-log-digest-hex", args.access_log_digest_hex),
                ("--legal-hold-receipt-digest-hex", args.legal_hold_receipt_digest_hex),
                ("--transparency-report-digest-hex", args.transparency_report_digest_hex),
                ("--audit-digest-hex", args.audit_digest_hex),
            ),
        )
        args.viewer_roles = validate_name_set(
            split_csv_values(args.viewer_role),
            allowed=REQUIRED_VIEWER_ROLES,
            option="--viewer-role",
            errors=errors,
        )
        args.viewer_security_controls = validate_name_set(
            split_csv_values(args.viewer_security_control),
            allowed=REQUIRED_VIEWER_SECURITY_CONTROLS,
            option="--viewer-security-control",
            errors=errors,
        )
        args.viewer_event_kinds = validate_name_set(
            split_csv_values(args.viewer_event_kind),
            allowed=REQUIRED_VIEWER_EVENT_KINDS,
            option="--viewer-event-kind",
            errors=errors,
        )
        args.viewer_export_targets = validate_name_set(
            split_csv_values(args.viewer_export_target),
            allowed=REQUIRED_VIEWER_EXPORT_TARGETS,
            option="--viewer-export-target",
            errors=errors,
        )
        args.viewer_sessions = validate_reviewed_inventory(
            split_csv_values(args.viewer_session),
            expected_count=args.session_count or 0,
            option="--viewer-session",
            kind="evidence_viewer",
            count_option="--session-count",
            errors=errors,
            pattern=VIEWER_SESSION_LABEL_PATTERN,
            label_error=VIEWER_SESSION_LABEL_ERROR,
        )
        for option, value in (
            ("--session-manifest-digest-hex", args.session_manifest_digest_hex),
            ("--watermark-metadata-digest-hex", args.watermark_metadata_digest_hex),
            ("--access-log-digest-hex", args.access_log_digest_hex),
            ("--legal-hold-receipt-digest-hex", args.legal_hold_receipt_digest_hex),
            ("--transparency-report-digest-hex", args.transparency_report_digest_hex),
            ("--audit-digest-hex", args.audit_digest_hex),
        ):
            validate_hex64(value, option=option, errors=errors)
    elif args.kind == "operator_workflow":
        args.operator_routes = validate_name_set(
            split_csv_values(args.operator_route),
            allowed=REQUIRED_OPERATOR_ROUTES,
            option="--operator-route",
            errors=errors,
        )
    elif args.kind == "juror_notifications":
        require_kind_options(
            args,
            errors,
            (
                ("--notification-count", args.notification_count),
                ("--juror-count", args.juror_count),
            ),
        )
        args.notifications = validate_reviewed_inventory(
            split_csv_values(args.notification),
            expected_count=args.notification_count or 0,
            option="--notification",
            kind="juror_notifications",
            count_option="--notification-count",
            errors=errors,
            pattern=NOTIFICATION_LABEL_PATTERN,
            label_error=NOTIFICATION_LABEL_ERROR,
        )
        args.jurors = validate_reviewed_inventory(
            split_csv_values(args.juror),
            expected_count=args.juror_count or 0,
            option="--juror",
            kind="juror_notifications",
            count_option="--juror-count",
            errors=errors,
            pattern=JUROR_LABEL_PATTERN,
            label_error=JUROR_LABEL_ERROR,
        )
        if (
            args.notification_count is not None
            and args.juror_count is not None
            and args.juror_count > args.notification_count
        ):
            errors.append("--juror-count must be <= --notification-count")
    elif args.kind == "commit_reveal":
        require_kind_options(
            args,
            errors,
            (
                ("--panel-size", args.panel_size),
                ("--commit-count", args.commit_count),
                ("--reveal-count", args.reveal_count),
                ("--max-event-lag-seconds", args.max_event_lag_seconds),
            ),
        )
        args.commits = validate_reviewed_inventory(
            split_csv_values(args.commit),
            expected_count=args.commit_count or 0,
            option="--commit",
            kind="commit_reveal",
            count_option="--commit-count",
            errors=errors,
            pattern=COMMIT_LABEL_PATTERN,
            label_error=COMMIT_LABEL_ERROR,
        )
        args.reveals = validate_reviewed_inventory(
            split_csv_values(args.reveal),
            expected_count=args.reveal_count or 0,
            option="--reveal",
            kind="commit_reveal",
            count_option="--reveal-count",
            errors=errors,
            pattern=REVEAL_LABEL_PATTERN,
            label_error=REVEAL_LABEL_ERROR,
        )
        if (
            args.commit_count is not None
            and args.reveal_count is not None
            and args.reveal_count > args.commit_count
        ):
            errors.append("--reveal-count must be <= --commit-count")
        args.ballot_routes = validate_name_set(
            split_csv_values(args.ballot_route),
            allowed=REQUIRED_BALLOT_ROUTES,
            option="--ballot-route",
            errors=errors,
        )
        args.scenarios_exercised = validate_name_set(
            split_csv_values(args.scenario),
            allowed=REQUIRED_COMMIT_REVEAL_SCENARIOS,
            option="--scenario",
            errors=errors,
        )
    elif args.kind == "decision_publication":
        args.decision_routes = validate_name_set(
            split_csv_values(args.decision_route),
            allowed=REQUIRED_DECISION_ROUTES,
            option="--decision-route",
            errors=errors,
        )
        args.outcomes = validate_name_set(
            split_csv_values(args.outcome),
            allowed=REQUIRED_OUTCOMES,
            option="--outcome",
            errors=errors,
        )
    elif args.kind == "settlement_integration":
        require_kind_options(
            args,
            errors,
            (("--settlement-count", args.settlement_count),),
        )
        args.settlements = validate_reviewed_inventory(
            split_csv_values(args.settlement),
            expected_count=args.settlement_count or 0,
            option="--settlement",
            kind="settlement_integration",
            count_option="--settlement-count",
            errors=errors,
            pattern=SETTLEMENT_LABEL_PATTERN,
            label_error=SETTLEMENT_LABEL_ERROR,
        )
    elif args.kind == "transparency_reputation":
        args.publication_targets = validate_name_set(
            split_csv_values(args.publication_target),
            allowed=REQUIRED_PUBLICATION_TARGETS,
            option="--publication-target",
            errors=errors,
        )
    elif args.kind == "e2e_panel":
        require_kind_options(
            args,
            errors,
            (
                ("--policy-digest-hex", args.policy_digest_hex),
                ("--peer-count", args.peer_count),
                ("--validator-count", args.validator_count),
                ("--case-count", args.case_count),
            ),
        )
        validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)
        if args.peer_count is not None and args.peer_count < DEFAULT_MIN_PEERS:
            errors.append(f"--peer-count must be >= {DEFAULT_MIN_PEERS}")
        if args.validator_count is not None and args.validator_count < DEFAULT_MIN_PEERS:
            errors.append(f"--validator-count must be >= {DEFAULT_MIN_PEERS}")
        args.peers = validate_reviewed_inventory(
            split_csv_values(args.peer),
            expected_count=args.peer_count or 0,
            option="--peer",
            kind="e2e_panel",
            count_option="--peer-count",
            errors=errors,
            pattern=E2E_PEER_LABEL_PATTERN,
            label_error=E2E_PEER_LABEL_ERROR,
        )
        args.validators = validate_reviewed_inventory(
            split_csv_values(args.validator),
            expected_count=args.validator_count or 0,
            option="--validator",
            kind="e2e_panel",
            count_option="--validator-count",
            errors=errors,
            pattern=E2E_VALIDATOR_LABEL_PATTERN,
            label_error=E2E_VALIDATOR_LABEL_ERROR,
        )
        args.panel_cases = validate_reviewed_inventory(
            split_csv_values(args.panel_case),
            expected_count=args.case_count or 0,
            option="--panel-case",
            kind="e2e_panel",
            count_option="--case-count",
            errors=errors,
            pattern=E2E_CASE_LABEL_PATTERN,
            label_error=E2E_CASE_LABEL_ERROR,
        )
    elif args.kind == "metrics_alerts":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    elif args.kind == "governance_approval":
        require_kind_options(
            args,
            errors,
            (("--policy-digest-hex", args.policy_digest_hex),),
        )
        validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)
    if args.panel_size is not None and args.panel_size < DEFAULT_MIN_PANEL_SIZE:
        errors.append(f"--panel-size must be >= {DEFAULT_MIN_PANEL_SIZE}")


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    require_rollout_deployment_id(
        {"--deployment-id": args.deployment_id},
        errors,
        field="--deployment-id",
    )
    require_rollout_environment(
        {"--environment": args.environment},
        errors,
        field="--environment",
    )
    validate_common_digests(args, errors)
    validate_thresholds(args, errors)
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix,
        max_canary_age_secs=DEFAULT_MAX_CANARY_AGE_SECS,
        max_event_lag_secs=DEFAULT_MAX_EVENT_LAG_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        min_panel_size=DEFAULT_MIN_PANEL_SIZE,
        min_peers=DEFAULT_MIN_PEERS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the moderation panel gate contract."""

    kind, errors = validate_evidence_payload(payload, validation_options(args))
    if kind != args.kind:
        errors.append(f"generated canary must validate as {args.kind}")
    return errors


def write_payload_atomic(path: Path, payload: dict[str, Any]) -> list[str]:
    """Write the canary JSON atomically without following output symlinks."""

    text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    parent = path.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        parent_label = path_diagnostic_label(parent)
        return [
            f"--out parent `{parent_label}` cannot be created: "
            f"{error_diagnostic_label(error, path_label=parent_label)}"
        ]
    tmp_name = f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    tmp_path = parent / tmp_name
    fd = -1
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        nofollow = getattr(os, "O_NOFOLLOW", 0)
        if nofollow:
            flags |= nofollow
        fd = os.open(tmp_path, flags, 0o600)
        write_all_checker_summary_bytes(fd, text.encode("utf-8"))
        os.fsync(fd)
        os.close(fd)
        fd = -1
        os.replace(tmp_path, path)
        parent_sync_errors = fsync_checker_output_parent(path, label="--out")
        if parent_sync_errors:
            return parent_sync_errors
    except (OSError, RuntimeError) as error:
        path_label = path_diagnostic_label(path)
        try:
            if fd >= 0:
                os.close(fd)
        finally:
            try:
                tmp_path.unlink()
            except FileNotFoundError:
                pass
            except (OSError, RuntimeError):
                pass
        return [
            f"--out `{path_label}` cannot be written: "
            f"{error_diagnostic_label(error, path_label=path_label)}"
        ]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Build payload-free SoraFS SFM-4b moderation panel canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg, required=True)
    parser.add_argument("--case-digest-hex", required=True)
    parser.add_argument("--roster-hash-hex")
    parser.add_argument("--tally-digest-hex")
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=40)
    parser.add_argument("--route-body-blake3-hex")
    parser.add_argument("--intake-route", action="append", default=[])
    parser.add_argument("--case-count", type=positive_int_arg)
    parser.add_argument("--case", action="append", default=[])
    parser.add_argument("--pop-snapshot-digest-hex")
    parser.add_argument("--sortition-seed-hex")
    parser.add_argument("--panel-size", type=positive_int_arg)
    parser.add_argument("--roster-juror", action="append", default=[])
    parser.add_argument("--quorum", type=positive_int_arg)
    parser.add_argument("--session-count", type=positive_int_arg)
    parser.add_argument("--viewer-session", action="append", default=[])
    parser.add_argument("--max-url-ttl-secs", type=positive_int_arg)
    parser.add_argument("--viewer-role", action="append", default=[])
    parser.add_argument("--viewer-security-control", action="append", default=[])
    parser.add_argument("--viewer-event-kind", action="append", default=[])
    parser.add_argument("--viewer-export-target", action="append", default=[])
    parser.add_argument("--session-manifest-digest-hex")
    parser.add_argument("--watermark-metadata-digest-hex")
    parser.add_argument("--access-log-digest-hex")
    parser.add_argument("--legal-hold-receipt-digest-hex")
    parser.add_argument("--transparency-report-digest-hex")
    parser.add_argument("--audit-digest-hex")
    parser.add_argument("--operator-route", action="append", default=[])
    parser.add_argument("--notification-count", type=positive_int_arg)
    parser.add_argument("--notification", action="append", default=[])
    parser.add_argument("--juror-count", type=positive_int_arg)
    parser.add_argument("--juror", action="append", default=[])
    parser.add_argument("--ballot-route", action="append", default=[])
    parser.add_argument("--commit-count", type=positive_int_arg)
    parser.add_argument("--commit", action="append", default=[])
    parser.add_argument("--reveal-count", type=positive_int_arg)
    parser.add_argument("--reveal", action="append", default=[])
    parser.add_argument("--max-event-lag-seconds", type=non_negative_int_arg)
    parser.add_argument("--scenario", action="append", default=[])
    parser.add_argument("--decision-route", action="append", default=[])
    parser.add_argument("--outcome", action="append", default=[])
    parser.add_argument("--settlement-count", type=positive_int_arg)
    parser.add_argument("--settlement", action="append", default=[])
    parser.add_argument("--publication-target", action="append", default=[])
    parser.add_argument("--peer-count", type=positive_int_arg)
    parser.add_argument("--peer", action="append", default=[])
    parser.add_argument("--validator-count", type=positive_int_arg)
    parser.add_argument("--validator", action="append", default=[])
    parser.add_argument("--panel-case", action="append", default=[])
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--policy-digest-hex")
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
        return parser.parse_args(expanded_args)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    errors = validate_inputs(args)
    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS moderation panel canary inputs are incomplete:",
            errors,
        )
        return 2

    payload = build_payload(args)
    payload_errors = validate_generated_payload(payload, args)
    if payload_errors:
        emit_checker_error_lines(payload_errors)
        return 2

    write_errors = write_payload_atomic(args.out, payload)
    if write_errors:
        emit_checker_error_lines(write_errors)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
