#!/usr/bin/env python3
"""Validate SoraFS AI pre-screening rollout evidence artifacts."""

from __future__ import annotations

import argparse
import re
import sys
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
from sorafs_evidence_validation import (  # noqa: E402
    EVIDENCE_PATH_FIELD_ERROR,
    EVIDENCE_URL_FIELD_ERROR,
    archive_artifact_path_label,
    build_evidence_artifact,
    count_evidence_artifacts,
    recognized_evidence_artifacts,
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
    require_count_length_match,
    require_count_value_equal,
    require_false,
    require_hex,
    require_iroha_config_binding,
    require_minimum_int,
    validate_standard_evidence_payload,
    require_minimum_value,
    require_non_negative_int,
    require_object,
    require_object_array,
    required_evidence_kind_names,
    require_policy_digest,
    require_positive_int,
    require_archive_portable_path,
    require_recent_timestamp,
    require_score_bps,
    require_safe_url,
    require_status_in,
    require_string,
    require_string_in,
    require_string_type,
    require_string_coverage,
    require_string_equal,
    require_string_inventory_count_match,
    validate_bound_evidence_digest_references,
    validate_bound_evidence_tuple_references,
)
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_evidence_kinds,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
)


SUMMARY_SCHEMA = "sorafs.moderation.ai_prescreen.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
HEX32_LEN = 32
HEX64_LEN = 64
WORKFLOW_ID_PATTERN = re.compile(r"^sfm-4a-[a-z0-9]+(?:-[a-z0-9]+)*\Z")
WORKFLOW_ID_ERROR = "workflow_id must match canonical lowercase `sfm-4a-*`"
SUBJECT_REFERENCE_PATTERN = re.compile(r"^cid:[a-z0-9]+(?:-[a-z0-9]+)*\Z")
SUBJECT_REFERENCE_ERROR = "subject must match canonical lowercase `cid:name`"
COMMITTEE_RESULT_LABEL_PATTERN = re.compile(
    r"^ai-prescreen-committee-result-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
COMMITTEE_RESULT_LABEL_ERROR = (
    "results[].name must match canonical lowercase "
    "`ai-prescreen-committee-result-name`"
)
NOTIFICATION_DELIVERY_LABEL_PATTERN = re.compile(
    r"^ai-prescreen-notification-delivery-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
NOTIFICATION_DELIVERY_LABEL_ERROR = (
    "probes[].delivery_id must match canonical lowercase "
    "`ai-prescreen-notification-delivery-name`"
)
NOTIFICATION_DEDUP_PREFIX = "sorafs-moderation-juror:"
ALLOWED_NOTIFICATION_ACTIONS = ("commit", "reveal")
GOVERNANCE_EDGE_LABEL_PATTERN = re.compile(
    r"^ai-prescreen-governance-edge-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
GOVERNANCE_EDGE_LABEL_ERROR = (
    "edges[].name must match canonical lowercase "
    "`ai-prescreen-governance-edge-name`"
)
FORBIDDEN_WORKFLOW_ID_MARKERS = frozenset(
    (
        "debug",
        "dev",
        "draft",
        "example",
        "fake",
        "latest",
        "placeholder",
        "sample",
        "secret",
        "test",
        "todo",
    )
)
FORBIDDEN_INVENTORY_LABEL_MARKERS = frozenset(
    (
        "debug",
        "dev",
        "draft",
        "example",
        "fake",
        "latest",
        "placeholder",
        "private",
        "sample",
        "secret",
        "test",
        "todo",
    )
)
FORBIDDEN_SUBJECT_REFERENCE_MARKERS = frozenset(
    (
        "debug",
        "dev",
        "draft",
        "example",
        "fake",
        "latest",
        "placeholder",
        "private",
        "sample",
        "secret",
        "test",
        "todo",
    )
)

REQUIRED_OPERATOR_ROUTES = (
    "healthz",
    "status",
    "browser_ui",
    "operator_panel",
    "bridge_plan",
    "juror_plan",
    "juror_notifications",
    "commit_reveal_status",
)
REQUIRED_OPERATOR_SCHEMAS = {
    "healthz": "sorafs.moderation.quarantine.operator_service.status.v1",
    "status": "sorafs.moderation.quarantine.operator_service.status.v1",
    "operator_panel": "sorafs.moderation.quarantine.operator_panel.v1",
    "bridge_plan": "sorafs.moderation.quarantine.bridge_plan.v1",
    "juror_plan": "sorafs.moderation.quarantine.juror_plan.v1",
    "juror_notifications": "sorafs.moderation.quarantine.juror_notifications.v1",
    "commit_reveal_status": "sorafs.moderation.quarantine.commit_reveal_status.v1",
}
REQUIRED_OPERATOR_CONTENT_TYPES = {
    "healthz": "application/json",
    "status": "application/json",
    "browser_ui": "text/html; charset=utf-8",
    "operator_panel": "application/json",
    "bridge_plan": "application/json",
    "juror_plan": "application/json",
    "juror_notifications": "application/json",
    "commit_reveal_status": "application/json",
}


def operator_route_paths(quarantine_id_hex: str) -> dict[str, str]:
    """Return reviewed operator workflow route paths for a quarantine id."""

    workflow = (
        lambda suffix: f"/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/{suffix}"
    )
    return {
        "healthz": "/healthz",
        "status": "/v1/sorafs/moderation/operator-panel/status",
        "browser_ui": "/v1/sorafs/moderation/operator-panel/ui",
        "operator_panel": workflow("operator-panel"),
        "bridge_plan": workflow("bridge-plan"),
        "juror_plan": workflow("juror-plan"),
        "juror_notifications": workflow("juror-notifications"),
        "commit_reveal_status": workflow("commit-reveal-status"),
    }


def expected_operator_route_url(operator_url: str, route_path: str) -> str:
    """Return the reviewed operator route URL for a base URL and route path."""

    return f"{operator_url.rstrip('/')}{route_path}"


REQUIRED_TRANSPARENCY_SOURCE_KINDS = (
    "moderation-reviewed-quarantine",
    "moderation-appeal-handoff",
    "moderation-appeal-ballot",
    "moderation-juror-plan",
    "moderation-juror-notifications-delivery",
    "moderation-juror-notifications-canary",
    "moderation-commit-reveal-status",
    "moderation-ballots-executor",
)
REQUIRED_EXECUTOR_ARTIFACTS = (
    "executor.env",
    "run.sh",
)
REQUIRED_EXECUTOR_ARTIFACT_KINDS = {
    "executor.env": "env",
    "run.sh": "script",
}
REQUIRED_GOVERNANCE_PRODUCERS = (
    "screening_ingest",
    "quarantine_escalation",
    "operator_review",
    "appeal_handoff",
    "appeal_ballot",
    "juror_notifications",
    "commit_reveal_executor",
    "transparency_publication",
)
REQUIRED_GOVERNANCE_EDGE_COUNT = len(REQUIRED_GOVERNANCE_PRODUCERS)
REQUIRED_E2E_STEPS = (
    "ingest",
    "quarantine",
    "operator_review",
    "release",
    "appeal_handoff",
    "appeal_ballot",
    "juror_notifications",
    "commit_reveal_executor",
    "transparency_publication",
)
ALLOWED_PRESCREEN_VERDICTS = (
    "pass",
    "warn",
    "quarantine",
    "escalate",
    "block",
)
RUNNER_BOUND_KINDS = ("committee",)
WORKFLOW_BOUND_KINDS = (
    "operator_workflow",
    "notification_transport",
    "commit_reveal_executor",
    "transparency_publication",
    "governance_dag",
)
POLICY_BOUND_KINDS = ("governance_dag",)
SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "message_body",
    "notification_body",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "private_payload",
    "raw_payload",
    "response_body",
    "secret",
    "token",
}


def require_only_required_values(
    payload: dict[str, Any],
    array_field: str,
    field: str,
    required_values: tuple[str, ...],
    errors: list[str],
) -> None:
    """Reject reviewed inventory rows outside a required closed string set."""

    values = payload.get(array_field)
    if not isinstance(values, list):
        return
    allowed = frozenset(required_values)
    for item in values:
        if field:
            if not isinstance(item, dict):
                continue
            value = item.get(field)
        else:
            value = item
        if not isinstance(value, str) or value.strip() not in allowed:
            errors.append(f"{array_field} must not include unknown values")
            return


def require_workflow_id(payload: dict[str, Any], errors: list[str]) -> str:
    """Require a reviewed lowercase SFM-4a workflow identifier."""

    workflow_id = require_string(payload, "workflow_id", errors)
    if not workflow_id:
        return ""
    if WORKFLOW_ID_PATTERN.fullmatch(workflow_id) is None:
        errors.append(WORKFLOW_ID_ERROR)
        return ""
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_WORKFLOW_ID_MARKERS
        if marker in workflow_id.split("-")
    )
    if forbidden:
        errors.append(
            f"workflow_id must not contain non-production markers {forbidden}"
        )
        return ""
    return workflow_id


def require_subject_reference(
    payload: dict[str, Any], errors: list[str], *, path: str = "subject"
) -> str:
    """Require a reviewed lowercase payload-free content subject reference."""

    subject = require_string(payload, "subject", errors)
    if not subject:
        return ""
    if SUBJECT_REFERENCE_PATTERN.fullmatch(subject) is None:
        errors.append(SUBJECT_REFERENCE_ERROR.replace("subject", path))
        return ""
    subject_tokens = frozenset(
        token for token in re.split(r"[^a-z0-9]+", subject) if token
    )
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_SUBJECT_REFERENCE_MARKERS
        if marker in subject_tokens
    )
    if forbidden:
        errors.append(f"{path} must not contain non-production markers {forbidden}")
        return ""
    return subject


def require_inventory_label(
    record: dict[str, Any],
    field: str,
    errors: list[str],
    *,
    pattern: re.Pattern[str],
    label_error: str,
    path: str,
) -> str:
    """Require a reviewed production inventory label without placeholder markers."""

    label = require_string(record, field, errors)
    if not label:
        return ""
    if pattern.fullmatch(label) is None:
        errors.append(label_error)
        return ""
    label_tokens = frozenset(
        token for token in re.split(r"[^a-z0-9]+", label) if token
    )
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_INVENTORY_LABEL_MARKERS
        if marker in label_tokens
    )
    if forbidden:
        errors.append(f"{path} must not contain non-production markers {forbidden}")
        return ""
    return label


@dataclass(frozen=True)
class EvidenceKind:
    """One SFM-4a AI pre-screening rollout evidence class."""

    name: str
    schema: str
    accepted_statuses: tuple[str, ...]


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind(
        "runner",
        "sorafs.moderation.runner.rollout_evidence.v1",
        ("verified",),
    ),
    EvidenceKind(
        "committee",
        "sorafs.moderation.committee.rollout_evidence.v1",
        ("verified",),
    ),
    EvidenceKind(
        "operator_workflow",
        "sorafs.moderation.quarantine.operator_canary.v1",
        ("passed",),
    ),
    EvidenceKind(
        "notification_transport",
        "sorafs.moderation.juror_notifications.transport_canary.v1",
        ("passed",),
    ),
    EvidenceKind(
        "commit_reveal_executor",
        "sorafs.moderation.ballots.executor_canary.v1",
        ("passed",),
    ),
    EvidenceKind(
        "transparency_publication",
        "sorafs.transparency.source_entry.canary.v1",
        ("passed",),
    ),
    EvidenceKind(
        "governance_dag",
        "sorafs.moderation.governance_dag_rollout.v1",
        ("passed",),
    ),
    EvidenceKind(
        "end_to_end_workflow",
        "sorafs.moderation.end_to_end_rollout.v1",
        ("passed",),
    ),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)
COMMON_EVIDENCE_REQUIRED_FIELDS: tuple[str, ...] = (
    "schema",
    "status",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
)
EVIDENCE_REQUIRED_FIELDS: dict[str, tuple[str, ...]] = {
    "runner": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "runner_url",
        "status_url",
        "screen_url",
        "manifest_id_hex",
        "runner_hash_hex",
        "subject",
        "subject_digest_hex",
        "screened_at_unix",
        "checked_at_unix",
        "combined_score_bps",
        "verdict",
        "policy_digest_hex",
    ),
    "committee": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "committee_url",
        "status_url",
        "aggregate_url",
        "manifest_id_hex",
        "runner_hash_hex",
        "quorum",
        "result_count",
        "results",
        "aggregation",
        "subject",
        "subject_digest_hex",
        "aggregated_score_bps",
        "verdict",
        "checked_at_unix",
    ),
    "operator_workflow": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "workflow_digest_hex",
        "operator_url",
        "quarantine_id_hex",
        "payload_bytes_included",
        "private_payloads_included",
        "route_count",
        "passed_route_count",
        "routes",
    ),
    "notification_transport": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "workflow_digest_hex",
        "manifest_path",
        "webhook_url",
        "manifest_body_blake3",
        "probe_count",
        "accepted_count",
        "payload_bytes_included",
        "private_payloads_included",
        "probes",
    ),
    "commit_reveal_executor": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "workflow_digest_hex",
        "bundle_dir",
        "bundle_metadata_bytes",
        "bundle_metadata_blake3",
        "service_name",
        "interval_secs",
        "artifact_count",
        "passed_artifact_count",
        "execution_summary_present",
        "execution_summary_digest_hex",
        "payload_bytes_included",
        "private_payloads_included",
        "private_payload_files_copied",
        "artifacts",
        "execution_summary",
    ),
    "transparency_publication": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "workflow_digest_hex",
        "probe_count",
        "passed_probe_count",
        "source_entry_probe_count",
        "payload_bytes_included",
        "private_payloads_included",
        "response_bodies_included",
        "probes",
    ),
    "governance_dag": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "workflow_digest_hex",
        "governance_dag_bound",
        "live_producers_bound",
        "transparency_source_entries_bound",
        "screening_ingest_bound",
        "quarantine_escalation_bound",
        "role_provisioning_recorded",
        "config_source",
        "policy_digest_hex",
        "producer_count",
        "edge_count",
        "producers",
        "edges",
        "payload_bytes_included",
        "private_payloads_included",
    ),
    "end_to_end_workflow": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "workflow_digest_hex",
        "workflow_id",
        "deployed_services",
        "runner_committee_live",
        "ingest_quarantine_release_path_passed",
        "appeal_path_passed",
        "transparency_publication_passed",
        "role_gate_checks_passed",
        "encrypted_object_api_checks_passed",
        "step_count",
        "passed_step_count",
        "steps",
        "payload_bytes_included",
        "private_payloads_included",
    ),
}



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
    "manifest_id_hex",
    "runner_hash_hex",
    "subject_digest_hex",
    "workflow_digest_hex",
    "manifest_body_blake3",
    "execution_summary_digest_hex",
    "policy_digest_hex",
)


def validate_status(kind: EvidenceKind, payload: dict[str, Any], errors: list[str]) -> None:
    require_status_in(payload, kind.accepted_statuses, errors)


def validate_runner(payload: dict[str, Any], errors: list[str]) -> None:
    require_safe_url(payload, "runner_url", errors)
    require_safe_url(payload, "status_url", errors)
    require_safe_url(payload, "screen_url", errors)
    require_hex(payload, "manifest_id_hex", HEX32_LEN, errors)
    require_hex(payload, "runner_hash_hex", HEX64_LEN, errors)
    require_subject_reference(payload, errors)
    require_hex(payload, "subject_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "screened_at_unix", errors)
    require_positive_int(payload, "checked_at_unix", errors)
    require_score_bps(payload, "combined_score_bps", errors)
    require_string_in(payload, "verdict", ALLOWED_PRESCREEN_VERDICTS, errors)
    require_hex(payload, "evidence_digest_hex", HEX64_LEN, errors)
    require_policy_digest(payload, errors)


def validate_committee(payload: dict[str, Any], errors: list[str]) -> None:
    require_safe_url(payload, "committee_url", errors)
    require_safe_url(payload, "status_url", errors)
    require_safe_url(payload, "aggregate_url", errors)
    require_hex(payload, "manifest_id_hex", HEX32_LEN, errors)
    require_hex(payload, "runner_hash_hex", HEX64_LEN, errors)
    quorum = require_positive_int(payload, "quorum", errors)
    result_count = require_positive_int(payload, "result_count", errors)
    require_minimum_value(
        result_count,
        "result_count",
        quorum,
        errors,
        message="result_count must be at least quorum",
    )
    require_string_inventory_count_match(
        payload,
        "results",
        "result_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for index, record in require_object_array(payload, "results", errors):
        require_inventory_label(
            record,
            "name",
            errors,
            pattern=COMMITTEE_RESULT_LABEL_PATTERN,
            label_error=COMMITTEE_RESULT_LABEL_ERROR,
            path=f"results[{index}].name",
        )
    require_string_equal(payload, "aggregation", "median_score_bps", errors)
    require_subject_reference(payload, errors)
    require_hex(payload, "subject_digest_hex", HEX64_LEN, errors)
    require_score_bps(payload, "aggregated_score_bps", errors)
    require_string_in(payload, "verdict", ALLOWED_PRESCREEN_VERDICTS, errors)
    require_positive_int(payload, "checked_at_unix", errors)


def validate_routes(
    payload: dict[str, Any],
    errors: list[str],
    required_routes: tuple[str, ...],
    *,
    operator_url: str = "",
    expected_paths: dict[str, str] | None = None,
    expected_content_types: dict[str, str] | None = None,
) -> None:
    route_records = require_object_array(payload, "routes", errors)
    if not route_records:
        return
    route_count = require_positive_int(payload, "route_count", errors)
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_count_length_match(
        route_count, route_records, "route_count", "routes", errors
    )
    require_string_inventory_count_match(
        payload,
        "routes",
        "route_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    require_only_required_values(payload, "routes", "name", required_routes, errors)
    for index, record in route_records:
        name = require_string(record, "name", errors)
        require_string_equal(
            record,
            "method",
            "GET",
            errors,
            path=f"routes[{index}].method",
        )
        expected_path = (expected_paths or {}).get(name)
        if expected_path is not None:
            require_string_equal(
                record,
                "path",
                expected_path,
                errors,
                path=f"routes[{index}].path",
            )
        else:
            require_string_type(record, "path", errors, path=f"routes[{index}].path")
        route_url = require_safe_url(
            record,
            "url",
            errors,
            path=f"routes[{index}].url",
        )
        if operator_url and route_url and expected_path is not None:
            expected_url = expected_operator_route_url(operator_url, expected_path)
            if route_url != expected_url:
                errors.append(
                    f"routes[{index}].url must match operator_url and reviewed route path"
                )
        require_2xx_status(
            record,
            "status_code",
            errors,
            path=f"routes[{index}].status_code",
        )
        require_hex(
            record,
            "body_blake3_hex",
            HEX64_LEN,
            errors,
            path=f"routes[{index}].body_blake3_hex",
        )
        require_positive_int(record, "body_bytes", errors)
        expected_content_type = (expected_content_types or {}).get(name)
        if expected_content_type is not None:
            require_string_equal(
                record,
                "content_type",
                expected_content_type,
                errors,
                path=f"routes[{index}].content_type",
            )
        else:
            require_string_type(
                record,
                "content_type",
                errors,
                path=f"routes[{index}].content_type",
            )
        require_false(record, "payload_bytes_included", errors)
        require_false(record, "private_payloads_included", errors)
        expected_schema = REQUIRED_OPERATOR_SCHEMAS.get(name)
        if expected_schema is not None:
            require_string_equal(
                record,
                "schema",
                expected_schema,
                errors,
                path=f"routes[{index}].schema",
            )
    require_string_coverage(payload, "routes", "name", required_routes, errors)


def validate_operator_workflow(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "workflow_digest_hex", HEX64_LEN, errors)
    operator_url = require_safe_url(payload, "operator_url", errors)
    quarantine_id_hex = require_hex(payload, "quarantine_id_hex", HEX32_LEN, errors)
    require_positive_int(payload, "generated_at_unix", errors)
    require_false(payload, "payload_bytes_included", errors)
    require_false(payload, "private_payloads_included", errors)
    validate_routes(
        payload,
        errors,
        REQUIRED_OPERATOR_ROUTES,
        operator_url=operator_url,
        expected_paths=operator_route_paths(quarantine_id_hex)
        if quarantine_id_hex
        else None,
        expected_content_types=REQUIRED_OPERATOR_CONTENT_TYPES,
    )


def validate_probe_array(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
    *,
    success_field: str,
    status_field: str,
    identity_field: str | None = None,
) -> None:
    probe_records = require_object_array(payload, field, errors)
    if not probe_records:
        return
    probe_count = require_positive_int(payload, "probe_count", errors)
    require_count_length_match(
        probe_count, probe_records, "probe_count", "probes", errors
    )
    if identity_field is not None:
        require_string_inventory_count_match(
            payload,
            field,
            "probe_count",
            errors,
            field=identity_field,
            allow_scalar_items=False,
        )
    for index, record in probe_records:
        require_bool_true(
            record,
            success_field,
            errors,
            path=f"{field}[{index}].{success_field}",
        )
        require_2xx_status(
            record,
            status_field,
            errors,
            path=f"{field}[{index}].{status_field}",
        )


def validate_notification_transport(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "workflow_digest_hex", HEX64_LEN, errors)
    require_archive_portable_path(payload, "manifest_path", errors)
    require_safe_url(payload, "webhook_url", errors)
    require_hex(payload, "manifest_body_blake3", HEX64_LEN, errors)
    require_count_equal(payload, "probe_count", "accepted_count", errors)
    require_minimum_int(payload, "accepted_count", 1, errors)
    require_string_inventory_count_match(
        payload,
        "probes",
        "accepted_count",
        errors,
        field="delivery_id",
        allow_scalar_items=False,
    )
    require_string_inventory_count_match(
        payload,
        "probes",
        "probe_count",
        errors,
        field="dedup_key",
        allow_scalar_items=False,
    )
    require_false(payload, "payload_bytes_included", errors)
    require_false(payload, "private_payloads_included", errors)
    validate_probe_array(
        payload,
        "probes",
        errors,
        success_field="response_success",
        status_field="response_status",
        identity_field="delivery_id",
    )
    for index, probe in enumerate(payload.get("probes", [])):
        record = require_object(probe, f"probes[{index}]", errors)
        delivery_id = require_inventory_label(
            record,
            "delivery_id",
            errors,
            pattern=NOTIFICATION_DELIVERY_LABEL_PATTERN,
            label_error=NOTIFICATION_DELIVERY_LABEL_ERROR,
            path=f"probes[{index}].delivery_id",
        )
        dedup_key = require_string_type(
            record,
            "dedup_key",
            errors,
            path=f"probes[{index}].dedup_key",
        )
        if (
            delivery_id
            and dedup_key
            and dedup_key != f"{NOTIFICATION_DEDUP_PREFIX}{delivery_id}"
        ):
            errors.append(
                "probes[{}].dedup_key must equal "
                "`{}<delivery_id>`".format(index, NOTIFICATION_DEDUP_PREFIX)
            )
        require_string_in(
            record,
            "action",
            ALLOWED_NOTIFICATION_ACTIONS,
            errors,
            path=f"probes[{index}].action",
        )
        require_string_type(
            record,
            "case_id",
            errors,
            path=f"probes[{index}].case_id",
        )
        require_string_type(
            record,
            "round_id",
            errors,
            path=f"probes[{index}].round_id",
        )
        require_string_type(
            record,
            "juror_id",
            errors,
            path=f"probes[{index}].juror_id",
        )
        require_positive_int(record, "notification_bytes", errors)
        require_hex(record, "notification_body_blake3", HEX64_LEN, errors)
        require_non_negative_int(record, "response_bytes", errors)
        require_hex(record, "response_body_blake3", HEX64_LEN, errors)
        require_false(record, "payload_bytes_included", errors)
        require_false(record, "private_payloads_included", errors)


def validate_commit_reveal_executor(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "workflow_digest_hex", HEX64_LEN, errors)
    require_string(payload, "bundle_dir", errors)
    require_positive_int(payload, "bundle_metadata_bytes", errors)
    require_hex(payload, "bundle_metadata_blake3", HEX64_LEN, errors)
    require_string(payload, "service_name", errors)
    require_positive_int(payload, "interval_secs", errors)
    artifact_count = require_count_equal(
        payload, "artifact_count", "passed_artifact_count", errors
    )
    require_minimum_value(
        artifact_count,
        "artifact_count",
        len(REQUIRED_EXECUTOR_ARTIFACTS),
        errors,
    )
    require_bool_true(payload, "execution_summary_present", errors)
    execution_summary_digest = require_hex(
        payload, "execution_summary_digest_hex", HEX64_LEN, errors
    )
    require_false(payload, "payload_bytes_included", errors)
    require_false(payload, "private_payloads_included", errors)
    require_false(payload, "private_payload_files_copied", errors)
    artifact_records = require_object_array(payload, "artifacts", errors)
    if artifact_records:
        require_count_length_match(
            artifact_count,
            artifact_records,
            "artifact_count",
            "artifacts",
            errors,
        )
        require_string_inventory_count_match(
            payload,
            "artifacts",
            "artifact_count",
            errors,
            field="name",
            allow_scalar_items=False,
        )
        require_only_required_values(
            payload,
            "artifacts",
            "name",
            REQUIRED_EXECUTOR_ARTIFACTS,
            errors,
        )
        require_string_coverage(
            payload,
            "artifacts",
            "name",
            REQUIRED_EXECUTOR_ARTIFACTS,
            errors,
            allow_scalar_items=False,
        )
        for index, record in artifact_records:
            name = require_string(record, "name", errors)
            expected_kind = REQUIRED_EXECUTOR_ARTIFACT_KINDS.get(name)
            if expected_kind is None:
                require_string(record, "kind", errors)
            else:
                require_string_equal(
                    record,
                    "kind",
                    expected_kind,
                    errors,
                    path=f"artifacts[{index}].kind",
                )
            artifact_path = require_archive_portable_path(
                record,
                "path",
                errors,
                path=f"artifacts[{index}].path",
            )
            if (
                name in REQUIRED_EXECUTOR_ARTIFACT_KINDS
                and artifact_path
                and artifact_path != name
            ):
                errors.append(f"artifacts[{index}].path must be `{name}`")
            require_bool_true(record, "exists", errors)
            require_bool_true(record, "passed", errors)
            require_positive_int(record, "bytes", errors)
            require_hex(record, "body_blake3", HEX64_LEN, errors)
            require_false(record, "payload_bytes_included", errors)
            require_false(record, "private_payloads_included", errors)

    summary = require_object(payload.get("execution_summary"), "execution_summary", errors)
    if not summary:
        return
    require_bool_true(summary, "passed", errors)
    require_archive_portable_path(summary, "path", errors, path="execution_summary.path")
    require_positive_int(summary, "bytes", errors)
    summary_digest = require_hex(summary, "body_blake3", HEX64_LEN, errors)
    if (
        execution_summary_digest
        and summary_digest
        and execution_summary_digest != summary_digest
    ):
        errors.append(
            "execution_summary.body_blake3 must match execution_summary_digest_hex"
        )
    action_count = require_positive_int(summary, "action_count", errors)
    require_minimum_value(
        action_count,
        "execution_summary.action_count",
        1,
        errors,
    )
    commit_action_count = require_non_negative_int(
        summary, "commit_action_count", errors
    )
    reveal_action_count = require_non_negative_int(
        summary, "reveal_action_count", errors
    )
    tally_action_count = require_non_negative_int(summary, "tally_action_count", errors)
    if (
        action_count
        and action_count
        != commit_action_count + reveal_action_count + tally_action_count
    ):
        errors.append(
            "execution_summary commit/reveal/tally counts must sum to action_count"
        )
    require_false(summary, "payload_bytes_included", errors)
    require_false(summary, "private_payloads_included", errors)


def validate_transparency_publication(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "workflow_digest_hex", HEX64_LEN, errors)
    require_count_equal(payload, "probe_count", "passed_probe_count", errors)
    require_count_equal(payload, "probe_count", "source_entry_probe_count", errors)
    require_minimum_int(
        payload,
        "source_entry_probe_count",
        len(REQUIRED_TRANSPARENCY_SOURCE_KINDS),
        errors,
    )
    require_string_inventory_count_match(
        payload,
        "probes",
        "source_entry_probe_count",
        errors,
        field="source_kind",
        allow_scalar_items=False,
    )
    require_only_required_values(
        payload,
        "probes",
        "source_kind",
        REQUIRED_TRANSPARENCY_SOURCE_KINDS,
        errors,
    )
    require_false(payload, "payload_bytes_included", errors)
    require_false(payload, "private_payloads_included", errors)
    require_false(payload, "response_bodies_included", errors)
    require_string_coverage(
        payload,
        "probes",
        "source_kind",
        REQUIRED_TRANSPARENCY_SOURCE_KINDS,
        errors,
    )
    validate_probe_array(
        payload,
        "probes",
        errors,
        success_field="response_success",
        status_field="response_status",
        identity_field="source_kind",
    )
    for index, probe in enumerate(payload.get("probes", [])):
        record = require_object(probe, f"probes[{index}]", errors)
        require_archive_portable_path(
            record,
            "payload_path",
            errors,
            path=f"probes[{index}].payload_path",
        )
        require_positive_int(record, "request_bytes", errors)
        require_hex(record, "request_body_blake3", HEX64_LEN, errors)
        require_non_negative_int(record, "response_bytes", errors)
        require_hex(record, "response_body_blake3", HEX64_LEN, errors)
        require_false(record, "payload_bytes_included", errors)
        require_false(record, "private_payloads_included", errors)
        require_false(record, "response_body_included", errors)


def validate_governance_dag(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "workflow_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "governance_dag_bound", errors)
    require_bool_true(payload, "live_producers_bound", errors)
    require_bool_true(payload, "transparency_source_entries_bound", errors)
    require_bool_true(payload, "screening_ingest_bound", errors)
    require_bool_true(payload, "quarantine_escalation_bound", errors)
    require_bool_true(payload, "role_provisioning_recorded", errors)
    require_iroha_config_binding(payload, errors, bound_field=None)
    require_policy_digest(payload, errors)
    producer_count = require_positive_int(payload, "producer_count", errors)
    require_minimum_value(
        producer_count,
        "producer_count",
        len(REQUIRED_GOVERNANCE_PRODUCERS),
        errors,
    )
    require_positive_int(payload, "edge_count", errors)
    require_count_value_equal(
        payload,
        "edge_count",
        REQUIRED_GOVERNANCE_EDGE_COUNT,
        "required governance producer inventory",
        errors,
    )
    producer_records = require_object_array(payload, "producers", errors)
    edge_records = require_object_array(payload, "edges", errors)
    if producer_records:
        require_count_length_match(
            producer_count,
            producer_records,
            "producer_count",
            "producers",
            errors,
        )
        require_string_inventory_count_match(
            payload,
            "producers",
            "producer_count",
            errors,
            field="name",
            allow_scalar_items=False,
        )
        require_only_required_values(
            payload,
            "producers",
            "name",
            REQUIRED_GOVERNANCE_PRODUCERS,
            errors,
        )
    if edge_records:
        require_string_inventory_count_match(
            payload,
            "edges",
            "edge_count",
            errors,
            field="name",
            allow_scalar_items=False,
        )
        require_string_coverage(
            payload,
            "edges",
            "producer",
            REQUIRED_GOVERNANCE_PRODUCERS,
            errors,
            allow_scalar_items=False,
        )
        for index, record in edge_records:
            require_inventory_label(
                record,
                "name",
                errors,
                pattern=GOVERNANCE_EDGE_LABEL_PATTERN,
                label_error=GOVERNANCE_EDGE_LABEL_ERROR,
                path=f"edges[{index}].name",
            )
            producer = require_string(record, "producer", errors)
            if producer and producer not in REQUIRED_GOVERNANCE_PRODUCERS:
                errors.append("edges producer must be one of required producers")
    require_string_coverage(
        payload,
        "producers",
        "name",
        REQUIRED_GOVERNANCE_PRODUCERS,
        errors,
    )
    require_false(payload, "payload_bytes_included", errors)
    require_false(payload, "private_payloads_included", errors)


def validate_end_to_end_workflow(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "workflow_digest_hex", HEX64_LEN, errors)
    require_workflow_id(payload, errors)
    require_bool_true(payload, "deployed_services", errors)
    require_bool_true(payload, "runner_committee_live", errors)
    require_bool_true(payload, "ingest_quarantine_release_path_passed", errors)
    require_bool_true(payload, "appeal_path_passed", errors)
    require_bool_true(payload, "transparency_publication_passed", errors)
    require_bool_true(payload, "role_gate_checks_passed", errors)
    require_bool_true(payload, "encrypted_object_api_checks_passed", errors)
    step_count = require_count_equal(payload, "step_count", "passed_step_count", errors)
    require_minimum_value(
        step_count,
        "step_count",
        len(REQUIRED_E2E_STEPS),
        errors,
    )
    step_records = require_object_array(payload, "steps", errors)
    if step_records:
        require_count_length_match(
            step_count,
            step_records,
            "step_count",
            "steps",
            errors,
        )
        require_string_inventory_count_match(
            payload,
            "steps",
            "step_count",
            errors,
            field="name",
            allow_scalar_items=False,
        )
        require_only_required_values(
            payload,
            "steps",
            "name",
            REQUIRED_E2E_STEPS,
            errors,
        )
    require_string_coverage(payload, "steps", "name", REQUIRED_E2E_STEPS, errors)
    for index, record in step_records:
        require_string(record, "name", errors)
        require_bool_true(record, "passed", errors, path=f"steps[{index}].passed")
    require_false(payload, "payload_bytes_included", errors)
    require_false(payload, "private_payloads_included", errors)


def validate_kind_specific(kind: EvidenceKind, payload: dict[str, Any], errors: list[str]) -> None:
    validate_status(kind, payload, errors)
    if kind.name == "runner":
        validate_runner(payload, errors)
    elif kind.name == "committee":
        validate_committee(payload, errors)
    elif kind.name == "operator_workflow":
        validate_operator_workflow(payload, errors)
    elif kind.name == "notification_transport":
        validate_notification_transport(payload, errors)
    elif kind.name == "commit_reveal_executor":
        validate_commit_reveal_executor(payload, errors)
    elif kind.name == "transparency_publication":
        validate_transparency_publication(payload, errors)
    elif kind.name == "governance_dag":
        validate_governance_dag(payload, errors)
    elif kind.name == "end_to_end_workflow":
        validate_end_to_end_workflow(payload, errors)


def validate_evidence_payload(payload: dict[str, Any]) -> tuple[str | None, list[str]]:
    kind_name, errors = validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS AI pre-screen rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        validate_kind_specific,
        require_reviewed_deployment_context=True,
    )
    if kind_name is not None and kind_name != "operator_workflow":
        require_positive_int(payload, "generated_at_unix", errors)
    return kind_name, errors



def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    valid_runner_bindings: set[tuple[str, str, str]] = set()
    runner_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_workflow_digests: set[str] = set()
    valid_notification_manifest_digests: set[str] = set()
    valid_executor_summary_digests: set[str] = set()
    workflow_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_policy_digests: set[str] = set()
    policy_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
        kind_name, validation_errors = validate_evidence_payload(payload)
        if kind_name is None:
            record_explicit_evidence_validation_errors(
                path, explicit, validation_errors, errors
            )
            continue
        artifact = build_evidence_artifact(
            archive_artifact_path_label(path, evidence_dirs),
            digest,
            payload,
            validation_errors,
            FINGERPRINT_FIELDS,
        )
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            if kind_name == "runner":
                manifest_id = fingerprint.get("manifest_id_hex")
                runner_hash = fingerprint.get("runner_hash_hex")
                subject_digest = fingerprint.get("subject_digest_hex")
                policy_digest = fingerprint.get("policy_digest_hex")
                if (
                    isinstance(manifest_id, str)
                    and isinstance(runner_hash, str)
                    and isinstance(subject_digest, str)
                ):
                    valid_runner_bindings.add(
                        (manifest_id.lower(), runner_hash.lower(), subject_digest.lower())
                    )
                if isinstance(policy_digest, str):
                    valid_policy_digests.add(policy_digest.lower())
            if kind_name in RUNNER_BOUND_KINDS:
                runner_bound_artifacts.append((kind_name, artifact))
            if kind_name == "end_to_end_workflow":
                digest = fingerprint.get("workflow_digest_hex")
                if isinstance(digest, str):
                    valid_workflow_digests.add(digest.lower())
            if kind_name == "notification_transport":
                digest = fingerprint.get("manifest_body_blake3")
                if isinstance(digest, str):
                    valid_notification_manifest_digests.add(digest.lower())
            if kind_name == "commit_reveal_executor":
                digest = fingerprint.get("execution_summary_digest_hex")
                if isinstance(digest, str):
                    valid_executor_summary_digests.add(digest.lower())
            if kind_name in WORKFLOW_BOUND_KINDS:
                workflow_bound_artifacts.append((kind_name, artifact))
            if kind_name in POLICY_BOUND_KINDS:
                policy_bound_artifacts.append((kind_name, artifact))
        record_evidence_validation_errors(path, validation_errors, errors)


    validate_bound_evidence_tuple_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=tuple(KIND_BY_NAME),
        bound_artifacts=runner_bound_artifacts,
        valid_anchor_bindings=valid_runner_bindings,
        binding_fields=("manifest_id_hex", "runner_hash_hex", "subject_digest_hex"),
        errors=errors,
        binding_error_template=(
            "{kind_name} manifest_id_hex, runner_hash_hex, and "
            "subject_digest_hex must match a valid runner artifact"
        ),
        missing_anchor_error_template=(
            "{kind_name} manifest_id_hex, runner_hash_hex, and "
            "subject_digest_hex must match a valid runner artifact"
        ),
    )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=tuple(KIND_BY_NAME),
        bound_artifacts=workflow_bound_artifacts,
        valid_anchor_digests=valid_workflow_digests,
        digest_field="workflow_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} workflow_digest_hex must match a valid "
            "end_to_end_workflow workflow_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} workflow_digest_hex must match a valid "
            "end_to_end_workflow workflow_digest_hex"
        ),
    )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("runner",),
        bound_artifacts=policy_bound_artifacts,
        valid_anchor_digests=valid_policy_digests,
        digest_field="policy_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex must match a valid "
            "runner policy_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} policy_digest_hex must match a valid "
            "runner policy_digest_hex"
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
            "max_evidence_bytes": MAX_EVIDENCE_BYTES,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "recognized_artifacts": recognized_evidence_artifacts(artifacts_by_kind),
        "valid_runner_bindings": [
            {
                "manifest_id_hex": manifest_id,
                "runner_hash_hex": runner_hash,
                "subject_digest_hex": subject_digest,
            }
            for manifest_id, runner_hash, subject_digest in sorted(valid_runner_bindings)
        ],
        "valid_workflow_digests": sorted(valid_workflow_digests),
        "valid_notification_manifest_digests": sorted(
            valid_notification_manifest_digests
        ),
        "valid_executor_summary_digests": sorted(valid_executor_summary_digests),
        "valid_policy_digests": sorted(valid_policy_digests),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS AI pre-screening rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SFM-4a kinds.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional summary JSON output path.",
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

    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    summary, errors = build_summary(
        args.evidence_dir, args.evidence, required_kinds, args.summary_out
    )
    rendered_summary, summary_errors = render_and_write_checker_summary(
        args.summary_out, summary
    )
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2

    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS AI pre-screening rollout evidence is incomplete:",
            errors,
        )
        return 1

    emit_checker_notice(
        "SoraFS AI pre-screening rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
