#!/usr/bin/env python3
"""Validate SoraFS transparency rollout evidence artifacts."""

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
    archive_artifact_path_label,
    forbidden_non_production_markers,
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
    require_count_length_match,
    require_count_match,
    require_count_value_equal,
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
    require_string_inventory_count_match,
    require_sum_equal,
    validate_bound_evidence_digest_references,
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


from sorafs_topology_qualification import (  # noqa: E402
    add_topology_qualification_argument,
    bind_lane_summary_to_topology,
)

SUMMARY_SCHEMA = "sorafs.transparency.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 7 * 24 * 60 * 60
HEX64_LEN = 64
CYCLE_DETAIL_PROBE_LABEL_PATTERN = re.compile(
    r"^transparency-cycle-detail-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
CYCLE_DETAIL_PROBE_LABEL_ERROR = (
    "cycle_detail_probes[].name must match canonical lowercase "
    "`transparency-cycle-detail-name`"
)
FORBIDDEN_INVENTORY_LABEL_MARKERS = frozenset(
    (
        "debug",
        "dev",
        "draft",
        "example",
        "fake",
        "latest",
        "local",
        "mock",
        "placeholder",
        "private",
        "sample",
        "sandbox",
        "secret",
        "test",
        "todo",
    )
)


@dataclass(frozen=True)
class EvidenceKind:
    name: str
    schema: str
    required_false_flags: tuple[str, ...]


@dataclass(frozen=True)
class ValidationOptions:
    """Reviewer-controlled freshness options for transparency evidence."""

    now_unix: int
    max_evidence_age_secs: int


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind(
        "source_entry",
        "sorafs.transparency.source_entry.producer_evidence.v1",
        ("payload_bytes_included", "private_payloads_included"),
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
COMMON_EVIDENCE_REQUIRED_FIELDS: tuple[str, ...] = (
    "schema",
    "status",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
)
EVIDENCE_REQUIRED_FIELDS: dict[str, tuple[str, ...]] = {
    "source_entry": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "source_batch_digest_hex",
        "producer_count",
        "generic_public_ingress_absent",
        "payload_bytes_included",
        "private_payloads_included",
        "producers",
    ),
    "publication": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "source_batch_digest_hex",
        "cycle_digest_hex",
        "route_count",
        "passed_route_count",
        "cycle_detail_probe_count",
        "cycle_detail_probes",
        "publisher_identity_required",
        "payload_bytes_included",
        "publication_bodies_included",
        "private_payloads_included",
        "routes",
    ),
    "privacy_aggregate": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "cycle_digest_hex",
        "probe_count",
        "passed_probe_count",
        "source_event_probe_count",
        "publish_due_probe_count",
        "payload_bytes_included",
        "raw_metric_values_included",
        "private_payloads_included",
        "probes",
    ),
    "proof_token_issuance": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "cycle_digest_hex",
        "probe_count",
        "passed_probe_count",
        "issuance_probe_count",
        "payload_bytes_included",
        "proof_token_frames_included",
        "private_digest_keys_included",
        "response_bodies_included",
        "probes",
    ),
    "explorer": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "cycle_digest_hex",
        "route_count",
        "payload_bytes_included",
        "private_digest_keys_included",
        "routes",
    ),
}
DEFAULT_REQUIRED_SOURCE_KINDS = (
    "gar-enforcement-receipt",
    "moderation-ballot-governance-event",
    "appeal-finance-report",
    "appeal-finance-settlement-receipt",
    "legal-hold-notice",
    "redaction-notice",
    "evidence-access-summary",
)
TRUSTED_SOURCE_PRODUCER_ID_PATTERN = re.compile(
    r"^[a-z0-9]+(?:[._-][a-z0-9]+)*\Z"
)
TRUSTED_SOURCE_PRODUCER_ROUTE_PATTERN = re.compile(
    r"^internal:[a-z0-9]+(?:[._/-][a-z0-9]+)*\Z"
)
TRUSTED_SOURCE_PRODUCER_ID_ERROR = (
    "producers[].producer_id must be a canonical production service id"
)
TRUSTED_SOURCE_PRODUCER_ROUTE_ERROR = (
    "producers[].producer_route must identify a trusted internal producer route"
)
REQUIRED_PUBLICATION_ROUTES = ("cycles_list", "cycle_publication")
REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES = ("transparency-cycle-detail-readback",)
REQUIRED_EXPLORER_ROUTES = (
    "explorer_snapshot",
    "browser_ui",
    "proof_token_issuance_index",
)
REQUIRED_PRIVACY_AGGREGATE_ACTIONS = ("source_event", "publish_due")
REQUIRED_PROOF_TOKEN_ISSUANCE_ACTIONS = ("proof_token_issuance",)
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
        if not isinstance(value, str) or value not in allowed:
            errors.append(f"{array_field} must not include unknown values")
            return


def require_inventory_label(
    record: dict[str, Any],
    field: str,
    errors: list[str],
    *,
    pattern: re.Pattern[str],
    label_error: str,
    path: str,
) -> str:
    """Require a reviewed production inventory label with the expected family."""

    label = require_string(record, field, errors)
    if not label:
        return ""
    if pattern.fullmatch(label) is None:
        errors.append(label_error)
        return ""
    label_tokens = frozenset(
        token for token in re.split(r"[^a-z0-9]+", label) if token
    )
    forbidden = forbidden_non_production_markers(label_tokens, FORBIDDEN_INVENTORY_LABEL_MARKERS)
    if forbidden:
        errors.append(f"{path} must not contain non-production markers {forbidden}")
        return ""
    return label



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
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
) -> list[tuple[int, dict[str, Any]]]:
    probe_records = require_object_array(payload, field, errors)
    if not probe_records:
        return []
    probe_count = require_positive_int(payload, "probe_count", errors)
    require_count_length_match(
        probe_count,
        probe_records,
        "probe_count",
        field,
        errors,
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
    return probe_records


def count_probe_records_with_value(
    probe_records: list[tuple[int, dict[str, Any]]],
    field: str,
    value: str,
) -> int:
    """Count validated probe records carrying a specific field value."""

    return sum(1 for _index, record in probe_records if record.get(field) == value)


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


def validate_route_inventory(
    payload: dict[str, Any],
    required_routes: tuple[str, ...],
    errors: list[str],
    *,
    require_passed_count: bool,
) -> int:
    if require_passed_count:
        require_count_match(payload, "route_count", "passed_route_count", errors)
        route_count = payload.get("route_count")
    else:
        route_count = require_positive_int(payload, "route_count", errors)
    require_string_coverage(
        payload,
        "routes",
        "name",
        required_routes,
        errors,
        allow_scalar_items=False,
    )
    require_only_required_values(payload, "routes", "name", required_routes, errors)
    require_string_inventory_count_match(
        payload,
        "routes",
        "route_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    return route_count if isinstance(route_count, int) and not isinstance(route_count, bool) else 0


def validate_kind_specific(kind: EvidenceKind, payload: dict[str, Any], errors: list[str]) -> None:
    require_passed_status(payload, errors)
    for field in kind.required_false_flags:
        require_false(payload, field, errors)

    if kind.name == "source_entry":
        require_hex(payload, "source_batch_digest_hex", HEX64_LEN, errors)
        producer_count = require_positive_int(payload, "producer_count", errors)
        require_bool_true(payload, "generic_public_ingress_absent", errors)
        require_string_coverage(
            payload,
            "producers",
            "source_kind",
            DEFAULT_REQUIRED_SOURCE_KINDS,
            errors,
            allow_scalar_items=False,
        )
        require_only_required_values(
            payload,
            "producers",
            "source_kind",
            DEFAULT_REQUIRED_SOURCE_KINDS,
            errors,
        )
        require_string_inventory_count_match(
            payload,
            "producers",
            "producer_count",
            errors,
            field="source_kind",
            allow_scalar_items=False,
        )
        producer_records = require_object_array(payload, "producers", errors)
        require_count_length_match(
            producer_count,
            producer_records,
            "producer_count",
            "producers",
            errors,
        )
        for index, record in producer_records:
            producer_id = require_string(record, "producer_id", errors)
            if (
                producer_id
                and TRUSTED_SOURCE_PRODUCER_ID_PATTERN.fullmatch(producer_id) is None
            ):
                errors.append(TRUSTED_SOURCE_PRODUCER_ID_ERROR)
            producer_route = require_string(record, "producer_route", errors)
            if (
                producer_route
                and TRUSTED_SOURCE_PRODUCER_ROUTE_PATTERN.fullmatch(producer_route) is None
            ):
                errors.append(TRUSTED_SOURCE_PRODUCER_ROUTE_ERROR)
            require_hex(
                record,
                "provenance_digest_hex",
                HEX64_LEN,
                errors,
                path=f"producers[{index}].provenance_digest_hex",
            )
            require_bool_true(
                record,
                "durable_checkpoint_verified",
                errors,
                path=f"producers[{index}].durable_checkpoint_verified",
            )
    elif kind.name == "publication":
        require_hex(payload, "source_batch_digest_hex", HEX64_LEN, errors)
        require_hex(payload, "cycle_digest_hex", HEX64_LEN, errors)
        require_positive_int(payload, "cycle_detail_probe_count", errors)
        require_string_coverage(
            payload,
            "cycle_detail_probes",
            "name",
            REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES,
            errors,
            allow_scalar_items=False,
        )
        require_only_required_values(
            payload,
            "cycle_detail_probes",
            "name",
            REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES,
            errors,
        )
        require_string_inventory_count_match(
            payload,
            "cycle_detail_probes",
            "cycle_detail_probe_count",
            errors,
            field="name",
            allow_scalar_items=False,
        )
        for index, record in require_object_array(
            payload,
            "cycle_detail_probes",
            errors,
        ):
            require_inventory_label(
                record,
                "name",
                errors,
                pattern=CYCLE_DETAIL_PROBE_LABEL_PATTERN,
                label_error=CYCLE_DETAIL_PROBE_LABEL_ERROR,
                path=f"cycle_detail_probes[{index}].name",
            )
            require_2xx_status(
                record,
                "status_code",
                errors,
                path=f"cycle_detail_probes[{index}].status_code",
            )
            require_hex(
                record,
                "body_blake3_hex",
                HEX64_LEN,
                errors,
                path=f"cycle_detail_probes[{index}].body_blake3_hex",
            )
            require_bool_true(
                record,
                "anchor_metadata_present",
                errors,
                path=f"cycle_detail_probes[{index}].anchor_metadata_present",
            )
            require_bool_true(
                record,
                "publisher_identity_present",
                errors,
                path=f"cycle_detail_probes[{index}].publisher_identity_present",
            )
            require_bool_true(
                record,
                "verification_valid",
                errors,
                path=f"cycle_detail_probes[{index}].verification_valid",
            )
        validate_route_inventory(
            payload,
            REQUIRED_PUBLICATION_ROUTES,
            errors,
            require_passed_count=True,
        )
        require_bool_true(payload, "publisher_identity_required", errors)
        validate_routes(payload, errors, publication=True)
    elif kind.name == "privacy_aggregate":
        require_hex(payload, "cycle_digest_hex", HEX64_LEN, errors)
        require_count_match(payload, "probe_count", "passed_probe_count", errors)
        source_event_probe_count = require_positive_int(
            payload, "source_event_probe_count", errors
        )
        publish_due_probe_count = require_positive_int(
            payload, "publish_due_probe_count", errors
        )
        require_string_coverage(
            payload,
            "probes",
            "action",
            REQUIRED_PRIVACY_AGGREGATE_ACTIONS,
            errors,
            allow_scalar_items=False,
        )
        require_only_required_values(
            payload,
            "probes",
            "action",
            REQUIRED_PRIVACY_AGGREGATE_ACTIONS,
            errors,
        )
        require_string_inventory_count_match(
            payload,
            "probes",
            "probe_count",
            errors,
            field="action",
            allow_scalar_items=False,
        )
        probe_records = validate_probe_array(
            payload,
            "probes",
            errors,
            success_field="response_success",
            status_field="response_status",
        )
        require_count_value_equal(
            payload,
            "source_event_probe_count",
            count_probe_records_with_value(probe_records, "action", "source_event"),
            "source_event probes count",
            errors,
        )
        require_count_value_equal(
            payload,
            "publish_due_probe_count",
            count_probe_records_with_value(probe_records, "action", "publish_due"),
            "publish_due probes count",
            errors,
        )
        probe_count = payload.get("probe_count")
        if isinstance(probe_count, int) and not isinstance(probe_count, bool):
            require_sum_equal(
                probe_count,
                (
                    ("source_event_probe_count", source_event_probe_count),
                    ("publish_due_probe_count", publish_due_probe_count),
                ),
                "probe_count",
                errors,
            )
    elif kind.name == "proof_token_issuance":
        require_hex(payload, "cycle_digest_hex", HEX64_LEN, errors)
        require_count_match(payload, "probe_count", "passed_probe_count", errors)
        require_positive_int(payload, "issuance_probe_count", errors)
        require_string_coverage(
            payload,
            "probes",
            "action",
            REQUIRED_PROOF_TOKEN_ISSUANCE_ACTIONS,
            errors,
            allow_scalar_items=False,
        )
        require_only_required_values(
            payload,
            "probes",
            "action",
            REQUIRED_PROOF_TOKEN_ISSUANCE_ACTIONS,
            errors,
        )
        require_string_inventory_count_match(
            payload,
            "probes",
            "probe_count",
            errors,
            field="action",
            allow_scalar_items=False,
        )
        require_string_inventory_count_match(
            payload,
            "probes",
            "issuance_probe_count",
            errors,
            field="action",
            allow_scalar_items=False,
        )
        probe_records = validate_probe_array(
            payload,
            "probes",
            errors,
            success_field="response_success",
            status_field="response_status",
        )
        require_count_value_equal(
            payload,
            "issuance_probe_count",
            len(probe_records),
            "issuance probes count",
            errors,
        )
    elif kind.name == "explorer":
        require_hex(payload, "cycle_digest_hex", HEX64_LEN, errors)
        route_count = validate_route_inventory(
            payload,
            REQUIRED_EXPLORER_ROUTES,
            errors,
            require_passed_count=False,
        )
        require_minimum_value(route_count, "route_count", 3, errors)
        validate_routes(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    kind_name, errors = validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS transparency rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        validate_kind_specific,
        require_reviewed_deployment_context=True,
    )
    if kind_name is not None:
        require_recent_timestamp(
            payload,
            "generated_at_unix",
            errors,
            now_unix=options.now_unix,
            max_age_secs=options.max_evidence_age_secs,
        )
    return kind_name, errors


def require_single_active_digest(
    digests: set[str],
    errors: list[str],
    *,
    label: str,
) -> set[str]:
    """Return one active rollout digest or fail closed on mixed anchors."""

    if len(digests) <= 1:
        return digests
    errors.append(f"{label} must contain exactly one active digest")
    return set()


def require_single_active_binding(
    bindings: set[Any],
    errors: list[str],
    *,
    label: str,
) -> set[Any]:
    """Return one active rollout binding or fail closed on mixed anchors."""

    if len(bindings) <= 1:
        return bindings
    errors.append(f"{label} must contain exactly one active binding")
    return set()


def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    summary_out: Path | None,
    options: ValidationOptions,
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    valid_source_batch_digests: set[str] = set()
    source_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_cycle_digests: set[str] = set()
    valid_publication_bindings: set[tuple[str, str]] = set()
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
        kind_name, validation_errors = validate_evidence_payload(payload, options)
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
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            source_batch = fingerprint.get("source_batch_digest_hex")
            cycle_digest = fingerprint.get("cycle_digest_hex")
            if kind_name == "source_entry" and isinstance(source_batch, str):
                valid_source_batch_digests.add(source_batch)
            elif kind_name in SOURCE_BOUND_KINDS:
                source_bound_artifacts.append((kind_name, artifact))
            if kind_name == "publication" and isinstance(cycle_digest, str):
                publication_cycle_artifacts.append(artifact)
            elif kind_name in CYCLE_BOUND_KINDS:
                cycle_bound_artifacts.append((kind_name, artifact))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        record_evidence_validation_errors(path, validation_errors, errors)

    valid_source_batch_digests = require_single_active_digest(
        valid_source_batch_digests,
        errors,
        label="valid_source_batch_digests",
    )
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
            fingerprint = evidence_artifact_fingerprint(artifact)
            source_batch = fingerprint.get("source_batch_digest_hex")
            cycle_digest = fingerprint.get("cycle_digest_hex")
            if isinstance(source_batch, str) and isinstance(cycle_digest, str):
                source_batch_digest = source_batch
                cycle_digest_value = cycle_digest
                valid_cycle_digests.add(cycle_digest_value)
                valid_publication_bindings.add(
                    (source_batch_digest, cycle_digest_value)
                )

    valid_cycle_digests = require_single_active_digest(
        valid_cycle_digests,
        errors,
        label="valid_cycle_digests",
    )
    valid_publication_bindings = require_single_active_binding(
        valid_publication_bindings,
        errors,
        label="valid_publication_bindings",
    )
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
        "thresholds": {
            "max_evidence_bytes": MAX_EVIDENCE_BYTES,
            "max_evidence_age_secs": options.max_evidence_age_secs,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "recognized_artifacts": recognized_evidence_artifacts(artifacts_by_kind),
        "valid_source_batch_digests": sorted(valid_source_batch_digests),
        "valid_cycle_digests": sorted(valid_cycle_digests),
        "valid_publication_bindings": [
            {
                "source_batch_digest_hex": source_batch_digest,
                "cycle_digest_hex": cycle_digest,
            }
            for source_batch_digest, cycle_digest in sorted(
                valid_publication_bindings
            )
        ],
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS transparency rollout evidence artifacts."
    )
    add_topology_qualification_argument(parser)
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
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        required=True,
        help="Required reviewed validator clock used for age checks.",
    )
    parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )
    try:
        expanded = expand_response_args(sys.argv[1:] if argv is None else argv, parser)
    except ValueError as error:
        emit_checker_exception(error)
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
        emit_checker_exception(error)
        return 2

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
    )
    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    summary, errors = build_summary(
        args.evidence_dir,
        args.evidence,
        required_kinds,
        args.summary_out,
        options,
    )
    errors.extend(
        bind_lane_summary_to_topology(
            summary, args.topology_qualification_summary
        )
    )
    summary["status"] = evidence_gate_status(errors)
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
