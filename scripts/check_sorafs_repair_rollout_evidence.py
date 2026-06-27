#!/usr/bin/env python3
"""Validate SoraFS repair rollout evidence artifacts."""

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
    validate_bound_evidence_digest_references,
    require_2xx_status,
    require_bool_true,
    require_count_equal,
    require_false,
    require_hex,
    require_config_backed_governance_approval,
    validate_standard_evidence_payload,
    require_maximum_number,
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


SUMMARY_SCHEMA = "sorafs.repair.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_EVENT_LAG_SECS = 15 * 60
DEFAULT_MAX_REPAIR_LATENCY_SECS = 2 * 60 * 60
DEFAULT_MIN_AUDITORS = 3
HEX64_LEN = 64

REQUIRED_FAILURE_SOURCES = ("por", "potr")
REQUIRED_AUDITOR_ROUTES = (
    "repair_report",
    "repair_slash",
    "repair_status",
    "repair_status_manifest",
)
REQUIRED_WORKER_ROUTES = (
    "repair_claim",
    "repair_heartbeat",
    "repair_complete",
    "repair_fail",
)
REQUIRED_EVENT_ROUTES = ("repair_events", "repair_events_sse", "repair_events_ws")
REQUIRED_LIFECYCLE_STATUSES = ("queued", "in_progress", "completed", "escalated")
REQUIRED_GOVERNANCE_TARGETS = (
    "governance_dag",
    "repair_slash_proposal",
    "reserve_rent",
    "transparency_ledger",
    "reputation",
)
REQUIRED_METRICS = (
    "torii_sorafs_repair_tasks_total",
    "torii_sorafs_repair_latency_minutes_bucket",
    "torii_sorafs_repair_queue_depth",
    "torii_sorafs_repair_backlog_oldest_age_seconds",
    "torii_sorafs_repair_lease_expired_total",
    "torii_sorafs_slash_proposals_total",
)
ROSTER_BOUND_KINDS = (
    "auditor_api",
    "worker_lifecycle",
    "event_streams",
    "governance_handoff",
    "governance_approval",
)
FAILURE_BOUND_KINDS = (
    "worker_lifecycle",
    "event_streams",
    "governance_handoff",
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "evidence_json",
    "ledger",
    "mnemonic",
    "norito_bytes",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_auditor_request",
    "raw_evidence",
    "raw_ledger",
    "raw_manifest",
    "raw_por",
    "raw_potr",
    "raw_repair_payload",
    "raw_request",
    "raw_response",
    "repair_payload",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signed_auditor_request",
    "signed_transaction",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SF-8b rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("auditor_roster", "sorafs.repair.auditor_roster_canary.v1"),
    EvidenceKind("failure_capture", "sorafs.repair.failure_capture_canary.v1"),
    EvidenceKind("auditor_api", "sorafs.repair.auditor_api_canary.v1"),
    EvidenceKind("worker_lifecycle", "sorafs.repair.worker_lifecycle_canary.v1"),
    EvidenceKind("event_streams", "sorafs.repair.event_streams_canary.v1"),
    EvidenceKind("governance_handoff", "sorafs.repair.governance_handoff_canary.v1"),
    EvidenceKind("observability", "sorafs.repair.observability_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.repair.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SF-8b repair rollout gate."""

    now_unix: int
    max_evidence_age_secs: int
    max_route_latency_ms: int
    max_event_lag_secs: int
    max_repair_latency_secs: int
    min_auditors: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "roster_digest_hex",
    "evidence_bundle_digest_hex",
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
        require_maximum_number(
            record,
            "latency_ms",
            options.max_route_latency_ms,
            errors,
            path=f"routes[{index}].latency_ms",
        )
        for field in ("authz_enforced", "signature_verified"):
            require_bool_true(record, field, errors, path=f"routes[{index}].{field}")


def validate_auditor_roster(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_bool_true(payload, "roster_published", errors)
    require_bool_true(payload, "roster_signature_verified", errors)
    require_bool_true(payload, "sf9_coordinator_bound", errors)
    require_bool_true(payload, "runbook_published", errors)
    require_bool_true(payload, "auditor_notifications_configured", errors)
    require_minimum_int(payload, "auditor_count", options.min_auditors, errors)
    require_hex(payload, "roster_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_roster_included", errors)


def validate_failure_capture(payload: dict[str, Any], errors: list[str]) -> None:
    require_string_coverage(payload, "failure_sources", "", REQUIRED_FAILURE_SOURCES, errors)
    require_bool_true(payload, "por_history_replayed", errors)
    require_bool_true(payload, "potr_receipt_replayed", errors)
    require_bool_true(payload, "coordinator_event_verified", errors)
    require_bool_true(payload, "merkle_or_receipt_inclusion_verified", errors)
    require_bool_true(payload, "object_storage_retention_bound", errors)
    require_positive_int(payload, "failure_event_count", errors)
    require_hex(payload, "evidence_bundle_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_evidence_included", errors)


def validate_auditor_api(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_hex(payload, "roster_digest_hex", HEX64_LEN, errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_AUDITOR_ROUTES, errors)
    require_bool_true(payload, "signed_auditor_envelope_required", errors)
    require_bool_true(payload, "nonce_replay_rejected", errors)
    require_bool_true(payload, "legacy_raw_payload_rejected", errors)
    require_bool_true(payload, "per_auditor_rate_limit_enforced", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_routes(payload, errors, options)


def validate_worker_lifecycle(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_hex(payload, "roster_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "evidence_bundle_digest_hex", HEX64_LEN, errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_WORKER_ROUTES, errors)
    require_string_coverage(payload, "statuses_observed", "", REQUIRED_LIFECYCLE_STATUSES, errors)
    require_bool_true(payload, "worker_permission_enforced", errors)
    require_bool_true(payload, "lease_heartbeat_enforced", errors)
    require_bool_true(payload, "idempotency_enforced", errors)
    require_bool_true(payload, "norito_snapshot_persisted", errors)
    require_bool_true(payload, "gc_protection_verified", errors)
    require_maximum_number(
        payload,
        "repair_latency_seconds",
        options.max_repair_latency_secs,
        errors,
    )
    require_false(payload, "raw_repair_payloads_included", errors)
    validate_routes(payload, errors, options)


def validate_event_streams(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_hex(payload, "roster_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "evidence_bundle_digest_hex", HEX64_LEN, errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_EVENT_ROUTES, errors)
    require_bool_true(payload, "backlog_replay_verified", errors)
    require_bool_true(payload, "sse_delivery_verified", errors)
    require_bool_true(payload, "websocket_delivery_verified", errors)
    require_maximum_number(
        payload,
        "event_lag_seconds",
        options.max_event_lag_secs,
        errors,
    )
    require_false(payload, "response_bodies_included", errors)
    validate_routes(payload, errors, options)


def validate_governance_handoff(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "roster_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "evidence_bundle_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "slash_proposal_generated", errors)
    require_bool_true(payload, "governance_dag_published", errors)
    require_bool_true(payload, "escalation_policy_enforced", errors)
    require_bool_true(payload, "appeal_window_enforced", errors)
    require_bool_true(payload, "reserve_rent_handoff_verified", errors)
    require_bool_true(payload, "transparency_publication_verified", errors)
    require_bool_true(payload, "reputation_handoff_verified", errors)
    require_string_coverage(payload, "handoff_targets", "", REQUIRED_GOVERNANCE_TARGETS, errors)
    require_hex(payload, "handoff_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_ledger_included", errors)


def validate_observability(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_false(payload, "response_bodies_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "repair_policy_bound", errors)
    require_bool_true(payload, "auditor_roster_bound", errors)
    require_hex(payload, "roster_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "slash_policy_bound", errors)
    require_policy_digest(payload, errors)


def validate_kind_specific(
    kind: EvidenceKind,
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_passed_status(payload, errors)
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=options.now_unix,
        max_age_secs=options.max_evidence_age_secs,
    )

    if kind.name == "auditor_roster":
        validate_auditor_roster(payload, errors, options)
    elif kind.name == "failure_capture":
        validate_failure_capture(payload, errors)
    elif kind.name == "auditor_api":
        validate_auditor_api(payload, errors, options)
    elif kind.name == "worker_lifecycle":
        validate_worker_lifecycle(payload, errors, options)
    elif kind.name == "event_streams":
        validate_event_streams(payload, errors, options)
    elif kind.name == "governance_handoff":
        validate_governance_handoff(payload, errors)
    elif kind.name == "observability":
        validate_observability(payload, errors)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS SF-8b rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        lambda kind, checked_payload, errors: validate_kind_specific(
            kind, checked_payload, errors, options
        ),
        require_reviewed_deployment_context=True,
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
    valid_roster_digests: set[str] = set()
    valid_failure_bundle_digests: set[str] = set()
    valid_roster_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_failure_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            roster_digest = fingerprint.get("roster_digest_hex")
            failure_digest = fingerprint.get("evidence_bundle_digest_hex")
            if kind_name == "auditor_roster" and isinstance(roster_digest, str):
                valid_roster_digests.add(roster_digest.lower())
            elif kind_name in ROSTER_BOUND_KINDS:
                valid_roster_bound_artifacts.append((kind_name, artifact))
            if kind_name == "failure_capture" and isinstance(failure_digest, str):
                valid_failure_bundle_digests.add(failure_digest.lower())
            elif kind_name in FAILURE_BOUND_KINDS:
                valid_failure_bound_artifacts.append((kind_name, artifact))
        record_evidence_validation_errors(path, validation_errors, errors)

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("auditor_roster",) + ROSTER_BOUND_KINDS,
        bound_artifacts=valid_roster_bound_artifacts,
        valid_anchor_digests=valid_roster_digests,
        digest_field="roster_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} roster_digest_hex must reference a valid auditor_roster "
            "roster_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} roster_digest_hex requires a valid auditor_roster "
            "roster_digest_hex"
        ),
    )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("failure_capture",) + FAILURE_BOUND_KINDS,
        bound_artifacts=valid_failure_bound_artifacts,
        valid_anchor_digests=valid_failure_bundle_digests,
        digest_field="evidence_bundle_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} evidence_bundle_digest_hex must reference a valid "
            "failure_capture evidence_bundle_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} evidence_bundle_digest_hex requires a valid failure_capture "
            "evidence_bundle_digest_hex"
        ),
    )

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
            "max_evidence_age_secs": options.max_evidence_age_secs,
            "max_route_latency_ms": options.max_route_latency_ms,
            "max_event_lag_secs": options.max_event_lag_secs,
            "max_repair_latency_secs": options.max_repair_latency_secs,
            "min_auditors": options.min_auditors,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_roster_digests": sorted(valid_roster_digests),
        "valid_failure_bundle_digests": sorted(valid_failure_bundle_digests),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SF-8b repair rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SF-8b kinds.",
    )
    parser.add_argument("--summary-out", type=Path, help="Optional summary JSON output path.")
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        default=int(time.time()),
        help="Validator clock used for age checks. Defaults to current Unix time.",
    )
    parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )
    parser.add_argument(
        "--max-route-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_ROUTE_LATENCY_MS,
    )
    parser.add_argument(
        "--max-event-lag-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_EVENT_LAG_SECS,
    )
    parser.add_argument(
        "--max-repair-latency-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_REPAIR_LATENCY_SECS,
    )
    parser.add_argument("--min-auditors", type=positive_int_arg, default=DEFAULT_MIN_AUDITORS)
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
        max_evidence_age_secs=args.max_evidence_age_secs,
        max_route_latency_ms=args.max_route_latency_ms,
        max_event_lag_secs=args.max_event_lag_secs,
        max_repair_latency_secs=args.max_repair_latency_secs,
        min_auditors=args.min_auditors,
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
        emit_checker_error_block("ERROR: SoraFS repair rollout evidence is incomplete:", errors)
        return 1

    emit_checker_notice(
        "SoraFS repair rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
