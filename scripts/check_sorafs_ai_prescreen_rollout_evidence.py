#!/usr/bin/env python3
"""Validate SoraFS AI pre-screening rollout evidence artifacts."""

from __future__ import annotations

import argparse
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
    require_count_length_match,
    require_false,
    require_false_or_absent,
    require_hex,
    require_iroha_config_binding,
    validate_standard_evidence_payload,
    require_minimum_value,
    require_non_negative_int,
    require_object,
    require_object_array,
    required_evidence_kind_names,
    require_optional_hex,
    require_policy_digest,
    require_positive_int,
    require_score_bps,
    require_status_in,
    require_string,
    require_string_coverage,
    require_string_equal,
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
RUNNER_BOUND_KINDS = ("committee",)
WORKFLOW_BOUND_KINDS = (
    "operator_workflow",
    "notification_transport",
    "commit_reveal_executor",
    "transparency_publication",
    "governance_dag",
)
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



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "manifest_id_hex",
    "runner_hash_hex",
    "subject_digest_hex",
    "workflow_digest_hex",
    "policy_digest_hex",
)


def validate_status(kind: EvidenceKind, payload: dict[str, Any], errors: list[str]) -> None:
    require_status_in(payload, kind.accepted_statuses, errors)


def validate_runner(payload: dict[str, Any], errors: list[str]) -> None:
    require_string(payload, "runner_url", errors)
    require_string(payload, "status_url", errors)
    require_string(payload, "screen_url", errors)
    require_hex(payload, "manifest_id_hex", HEX32_LEN, errors)
    require_hex(payload, "runner_hash_hex", HEX64_LEN, errors)
    require_string(payload, "subject", errors)
    require_hex(payload, "subject_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "screened_at_unix", errors)
    require_positive_int(payload, "checked_at_unix", errors)
    require_score_bps(payload, "combined_score_bps", errors)
    require_string(payload, "verdict", errors)
    require_optional_hex(payload, "evidence_digest_hex", HEX64_LEN, errors)
    require_optional_hex(payload, "policy_digest_hex", HEX64_LEN, errors)


def validate_committee(payload: dict[str, Any], errors: list[str]) -> None:
    require_string(payload, "committee_url", errors)
    require_string(payload, "status_url", errors)
    require_string(payload, "aggregate_url", errors)
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
    require_string_equal(payload, "aggregation", "median_score_bps", errors)
    require_string(payload, "subject", errors)
    require_hex(payload, "subject_digest_hex", HEX64_LEN, errors)
    require_score_bps(payload, "aggregated_score_bps", errors)
    require_string(payload, "verdict", errors)
    require_positive_int(payload, "checked_at_unix", errors)


def validate_routes(
    payload: dict[str, Any],
    errors: list[str],
    required_routes: tuple[str, ...],
) -> None:
    route_records = require_object_array(payload, "routes", errors)
    if not route_records:
        return
    route_count = require_positive_int(payload, "route_count", errors)
    require_count_length_match(
        route_count, route_records, "route_count", "routes", errors
    )
    for index, record in route_records:
        name = require_string(record, "name", errors)
        require_2xx_status(
            record,
            "status_code",
            errors,
            path=f"routes[{index}].status_code",
        )
        require_optional_hex(record, "body_blake3_hex", HEX64_LEN, errors)
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
    require_string(payload, "operator_url", errors)
    require_hex(payload, "quarantine_id_hex", HEX32_LEN, errors)
    require_positive_int(payload, "generated_at_unix", errors)
    require_false(payload, "payload_bytes_included", errors)
    require_false(payload, "private_payloads_included", errors)
    validate_routes(payload, errors, REQUIRED_OPERATOR_ROUTES)


def validate_probe_array(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
    *,
    success_field: str,
    status_field: str,
) -> None:
    probe_records = require_object_array(payload, field, errors)
    if not probe_records:
        return
    probe_count = require_positive_int(payload, "probe_count", errors)
    require_count_length_match(
        probe_count, probe_records, "probe_count", "probes", errors
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
    require_string(payload, "webhook_url", errors)
    require_hex(payload, "manifest_body_blake3", HEX64_LEN, errors)
    require_count_equal(payload, "probe_count", "accepted_count", errors)
    require_false(payload, "payload_bytes_included", errors)
    require_false(payload, "private_payloads_included", errors)
    validate_probe_array(
        payload,
        "probes",
        errors,
        success_field="response_success",
        status_field="response_status",
    )
    for index, probe in enumerate(payload.get("probes", [])):
        record = require_object(probe, f"probes[{index}]", errors)
        require_positive_int(record, "notification_bytes", errors)
        require_hex(record, "notification_body_blake3", HEX64_LEN, errors)
        require_hex(record, "response_body_blake3", HEX64_LEN, errors)
        require_false(record, "payload_bytes_included", errors)
        require_false(record, "private_payloads_included", errors)


def validate_commit_reveal_executor(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "workflow_digest_hex", HEX64_LEN, errors)
    artifact_count = require_count_equal(
        payload, "artifact_count", "passed_artifact_count", errors
    )
    require_bool_true(payload, "execution_summary_present", errors)
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
        for _index, record in artifact_records:
            require_string(record, "name", errors)
            require_string(record, "kind", errors)
            require_bool_true(record, "exists", errors)
            require_bool_true(record, "passed", errors)
            require_hex(record, "body_blake3", HEX64_LEN, errors)
            require_false(record, "payload_bytes_included", errors)
            require_false(record, "private_payloads_included", errors)

    summary = require_object(payload.get("execution_summary"), "execution_summary", errors)
    if not summary:
        return
    require_bool_true(summary, "passed", errors)
    require_hex(summary, "body_blake3", HEX64_LEN, errors)
    action_count = require_positive_int(summary, "action_count", errors)
    require_minimum_value(
        action_count,
        "execution_summary.action_count",
        1,
        errors,
    )
    require_false(summary, "payload_bytes_included", errors)
    require_false(summary, "private_payloads_included", errors)


def validate_transparency_publication(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "workflow_digest_hex", HEX64_LEN, errors)
    require_count_equal(payload, "probe_count", "passed_probe_count", errors)
    require_positive_int(payload, "source_entry_probe_count", errors)
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
    )
    for index, probe in enumerate(payload.get("probes", [])):
        record = require_object(probe, f"probes[{index}]", errors)
        require_hex(record, "request_body_blake3", HEX64_LEN, errors)
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
    require_string(payload, "workflow_id", errors)
    require_bool_true(payload, "deployed_services", errors)
    require_bool_true(payload, "runner_committee_live", errors)
    require_bool_true(payload, "ingest_quarantine_release_path_passed", errors)
    require_bool_true(payload, "appeal_path_passed", errors)
    require_bool_true(payload, "transparency_publication_passed", errors)
    require_bool_true(payload, "role_gate_checks_passed", errors)
    require_bool_true(payload, "encrypted_object_api_checks_passed", errors)
    require_count_equal(payload, "step_count", "passed_step_count", errors)
    require_string_coverage(payload, "steps", "name", REQUIRED_E2E_STEPS, errors)
    for index, step in enumerate(payload.get("steps", [])):
        record = require_object(step, f"steps[{index}]", errors)
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
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS AI pre-screen rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        validate_kind_specific,
    )



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
    workflow_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
            path,
            digest,
            payload,
            validation_errors,
            FINGERPRINT_FIELDS,
        )
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact)
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            if kind_name == "runner":
                manifest_id = fingerprint.get("manifest_id_hex")
                runner_hash = fingerprint.get("runner_hash_hex")
                subject_digest = fingerprint.get("subject_digest_hex")
                if (
                    isinstance(manifest_id, str)
                    and isinstance(runner_hash, str)
                    and isinstance(subject_digest, str)
                ):
                    valid_runner_bindings.add(
                        (manifest_id.lower(), runner_hash.lower(), subject_digest.lower())
                    )
            elif kind_name in RUNNER_BOUND_KINDS:
                runner_bound_artifacts.append((kind_name, artifact))
            elif kind_name == "end_to_end_workflow":
                digest = fingerprint.get("workflow_digest_hex")
                if isinstance(digest, str):
                    valid_workflow_digests.add(digest.lower())
            elif kind_name in WORKFLOW_BOUND_KINDS:
                workflow_bound_artifacts.append((kind_name, artifact))
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
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_runner_bindings": [
            {
                "manifest_id_hex": manifest_id,
                "runner_hash_hex": runner_hash,
                "subject_digest_hex": subject_digest,
            }
            for manifest_id, runner_hash, subject_digest in sorted(valid_runner_bindings)
        ],
        "valid_workflow_digests": sorted(valid_workflow_digests),
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
