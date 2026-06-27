#!/usr/bin/env python3
"""Validate SoraFS transparency rollout evidence artifacts."""

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
    require_count_match,
    require_false,
    require_hex,
    validate_standard_evidence_payload,
    require_minimum_value,
    require_object,
    require_object_array,
    required_evidence_kind_names,
    require_passed_status,
    require_positive_int,
    require_recent_timestamp,
    require_string,
    require_string_coverage,
    validate_bound_evidence_digest_references,
)
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_evidence_kinds,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
)


SUMMARY_SCHEMA = "sorafs.transparency.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
HEX64_LEN = 64


@dataclass(frozen=True)
class EvidenceKind:
    name: str
    schema: str
    required_false_flags: tuple[str, ...]


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind(
        "source_entry",
        "sorafs.transparency.source_entry.canary.v1",
        ("payload_bytes_included", "private_payloads_included", "response_bodies_included"),
    ),
    EvidenceKind(
        "publication",
        "sorafs.transparency.publication_canary.v1",
        ("payload_bytes_included", "publication_bodies_included", "private_payloads_included"),
    ),
    EvidenceKind(
        "privacy_aggregate",
        "sorafs.transparency.privacy_aggregate.canary.v1",
        ("payload_bytes_included", "raw_metric_values_included", "private_payloads_included"),
    ),
    EvidenceKind(
        "proof_token_issuance",
        "sorafs.transparency.proof_token_issuance.canary.v1",
        (
            "payload_bytes_included",
            "proof_token_frames_included",
            "private_digest_keys_included",
            "response_bodies_included",
        ),
    ),
    EvidenceKind(
        "explorer",
        "sorafs.transparency.explorer_canary.v1",
        ("payload_bytes_included", "private_digest_keys_included"),
    ),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)
DEFAULT_REQUIRED_SOURCE_KINDS = (
    "gar-enforcement-receipt",
    "moderation-ballot-governance-event",
    "appeal-finance-report",
    "appeal-finance-settlement-receipt",
    "legal-hold-notice",
    "redaction-notice",
    "evidence-access-summary",
)
REQUIRED_PUBLICATION_ROUTES = ("cycles_list", "cycle_publication")
REQUIRED_EXPLORER_ROUTES = (
    "explorer_snapshot",
    "browser_ui",
    "proof_token_issuance_index",
)
REQUIRED_PRIVACY_AGGREGATE_ACTIONS = ("source_event", "publish_due")
SOURCE_BOUND_KINDS = ("publication",)
CYCLE_BOUND_KINDS = (
    "privacy_aggregate",
    "proof_token_issuance",
    "explorer",
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_digest_key",
    "private_key",
    "private_payload",
    "proof_token_digest_key",
    "proof_token_frame",
    "raw_payload",
    "request_body",
    "response_body",
    "secret",
    "signed_transaction",
    "token",
    "token_b64",
}



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "deployment_id",
    "environment",
    "source_batch_digest_hex",
    "cycle_digest_hex",
)


def validate_probe_array(
    payload: dict[str, Any],
    field: str,
    errors: list[str],
    *,
    success_field: str,
    status_field: str,
) -> None:
    for index, record in require_object_array(payload, field, errors):
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
        require_hex(
            record,
            "request_body_blake3",
            HEX64_LEN,
            errors,
            path=f"{field}[{index}].request_body_blake3",
        )
        require_hex(
            record,
            "response_body_blake3",
            HEX64_LEN,
            errors,
            path=f"{field}[{index}].response_body_blake3",
        )


def validate_routes(
    payload: dict[str, Any],
    errors: list[str],
    *,
    publication: bool = False,
) -> None:
    for index, record in require_object_array(payload, "routes", errors):
        if publication:
            require_bool_true(record, "passed", errors, path=f"routes[{index}].passed")
        if "http_success" in record:
            require_bool_true(
                record,
                "http_success",
                errors,
                path=f"routes[{index}].http_success",
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
        if publication:
            for field in (
                "anchor_metadata_present",
                "publisher_identity_present",
                "verification_valid",
            ):
                require_bool_true(
                    record,
                    field,
                    errors,
                    path=f"routes[{index}].{field}",
                )


def validate_kind_specific(kind: EvidenceKind, payload: dict[str, Any], errors: list[str]) -> None:
    require_passed_status(payload, errors)
    for field in kind.required_false_flags:
        require_false(payload, field, errors)

    if kind.name == "source_entry":
        require_hex(payload, "source_batch_digest_hex", HEX64_LEN, errors)
        require_count_match(payload, "probe_count", "passed_probe_count", errors)
        require_positive_int(payload, "source_entry_probe_count", errors)
        require_string_coverage(
            payload,
            "probes",
            "source_kind",
            DEFAULT_REQUIRED_SOURCE_KINDS,
            errors,
            allow_scalar_items=False,
            trim_values=False,
        )
        validate_probe_array(
            payload,
            "probes",
            errors,
            success_field="response_success",
            status_field="response_status",
        )
    elif kind.name == "publication":
        require_hex(payload, "source_batch_digest_hex", HEX64_LEN, errors)
        require_hex(payload, "cycle_digest_hex", HEX64_LEN, errors)
        require_count_match(payload, "route_count", "passed_route_count", errors)
        require_positive_int(payload, "cycle_detail_probe_count", errors)
        require_string_coverage(
            payload,
            "routes",
            "name",
            REQUIRED_PUBLICATION_ROUTES,
            errors,
            allow_scalar_items=False,
            trim_values=False,
        )
        require_bool_true(payload, "publisher_identity_required", errors)
        validate_routes(payload, errors, publication=True)
    elif kind.name == "privacy_aggregate":
        require_hex(payload, "cycle_digest_hex", HEX64_LEN, errors)
        require_count_match(payload, "probe_count", "passed_probe_count", errors)
        require_positive_int(payload, "source_event_probe_count", errors)
        require_positive_int(payload, "publish_due_probe_count", errors)
        require_string_coverage(
            payload,
            "probes",
            "action",
            REQUIRED_PRIVACY_AGGREGATE_ACTIONS,
            errors,
            allow_scalar_items=False,
            trim_values=False,
        )
        validate_probe_array(
            payload,
            "probes",
            errors,
            success_field="response_success",
            status_field="response_status",
        )
    elif kind.name == "proof_token_issuance":
        require_hex(payload, "cycle_digest_hex", HEX64_LEN, errors)
        require_count_match(payload, "probe_count", "passed_probe_count", errors)
        require_positive_int(payload, "issuance_probe_count", errors)
        validate_probe_array(
            payload,
            "probes",
            errors,
            success_field="response_success",
            status_field="response_status",
        )
    elif kind.name == "explorer":
        require_hex(payload, "cycle_digest_hex", HEX64_LEN, errors)
        route_count = require_positive_int(payload, "route_count", errors)
        require_minimum_value(route_count, "route_count", 3, errors)
        require_string_coverage(
            payload,
            "routes",
            "name",
            REQUIRED_EXPLORER_ROUTES,
            errors,
            allow_scalar_items=False,
            trim_values=False,
        )
        validate_routes(payload, errors)


def validate_evidence_payload(payload: dict[str, Any]) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS transparency rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        validate_kind_specific,
        require_reviewed_deployment_context=True,
    )



def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    valid_source_batch_digests: set[str] = set()
    source_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_cycle_digests: set[str] = set()
    publication_cycle_artifacts: list[dict[str, Any]] = []
    cycle_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            source_batch = fingerprint.get("source_batch_digest_hex")
            cycle_digest = fingerprint.get("cycle_digest_hex")
            if kind_name == "source_entry" and isinstance(source_batch, str):
                valid_source_batch_digests.add(source_batch.lower())
            elif kind_name in SOURCE_BOUND_KINDS:
                source_bound_artifacts.append((kind_name, artifact))
            if kind_name == "publication" and isinstance(cycle_digest, str):
                publication_cycle_artifacts.append(artifact)
            elif kind_name in CYCLE_BOUND_KINDS:
                cycle_bound_artifacts.append((kind_name, artifact))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        record_evidence_validation_errors(path, validation_errors, errors)


    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=tuple(KIND_BY_NAME),
        bound_artifacts=source_bound_artifacts,
        valid_anchor_digests=valid_source_batch_digests,
        digest_field="source_batch_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} source_batch_digest_hex must match "
            "a valid source_entry artifact"
        ),
        missing_anchor_error_template=(
            "{kind_name} source_batch_digest_hex must match "
            "a valid source_entry artifact"
        ),
    )

    for artifact in publication_cycle_artifacts:
        if evidence_artifact_is_valid(artifact):
            cycle_digest = evidence_artifact_fingerprint(artifact).get("cycle_digest_hex")
            if isinstance(cycle_digest, str):
                valid_cycle_digests.add(cycle_digest.lower())

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=tuple(KIND_BY_NAME),
        bound_artifacts=cycle_bound_artifacts,
        valid_anchor_digests=valid_cycle_digests,
        digest_field="cycle_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} cycle_digest_hex must match "
            "a valid source-bound publication artifact"
        ),
        missing_anchor_error_template=(
            "{kind_name} cycle_digest_hex must match "
            "a valid source-bound publication artifact"
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
        "valid_source_batch_digests": sorted(valid_source_batch_digests),
        "valid_cycle_digests": sorted(valid_cycle_digests),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS transparency rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SFM-4c kinds.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional summary JSON output path.",
    )
    try:
        expanded = expand_response_args(sys.argv[1:] if argv is None else argv, parser)
    except ValueError as error:
        emit_checker_error_lines((str(error),))
        return 2
    try:
        args = parser.parse_args(expanded)
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
        emit_checker_error_block("ERROR: SoraFS transparency rollout evidence is incomplete:", errors)
        return 1

    emit_checker_notice(
        "SoraFS transparency rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
