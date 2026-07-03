#!/usr/bin/env python3
"""Validate SoraFS reputation rollout evidence artifacts."""

from __future__ import annotations

import argparse
import re
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_lines,
    emit_checker_exception,
    render_and_write_checker_summary,
    validate_checker_preflight,
)
from sorafs_evidence_paths import (  # noqa: E402
    discover_evidence_files,
    resolve_evidence_path,
)
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_evidence_validation import (  # noqa: E402
    archive_artifact_path_label,
    build_kinded_evidence_artifact,
    count_evidence_files,
    count_recognized_evidence_artifacts,
    finalize_custom_required_evidence_rows,
    hashable_evidence_values,
    record_consistent_evidence_value,
    record_custom_required_evidence_artifact,
    record_inconsistent_evidence_values_error,
    record_missing_required_evidence_value_errors,
    record_missing_required_or_observed_evidence_error,
    record_observed_evidence_value,
    record_snapshot_bound_evidence_artifact,
    validate_snapshot_bound_evidence_artifacts,
    require_advancing_int_pair,
    require_bool_true,
    require_count_length_match,
    require_false_or_absent,
    require_hex,
    require_hex_string_array,
    require_int_range,
    require_minimum_int,
    require_maximum_number,
    require_object,
    require_object_array,
    require_passed_status,
    require_positive_int,
    require_recent_timestamp,
    require_rollout_deployment_context_review,
    require_rollout_deployment_id,
    require_rollout_environment,
    required_evidence_summary_is_valid,
    recognized_evidence_artifacts_are_valid,
    require_status_in,
    require_string,
    require_string_coverage,
    require_string_inventory_count_match,
    require_string_equal,
    require_string_value_equal,
    required_evidence_kind_names,
)
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_evidence_kinds,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    positive_int_arg,
)
from sorafs_path_identity import error_diagnostic_label  # noqa: E402
from sorafs_evidence_sensitivity import visit_sensitive_fields  # noqa: E402


SUMMARY_SCHEMA = "sorafs.reputation.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_SNAPSHOT_AGE_SECS = 8 * 24 * 60 * 60
DEFAULT_MAX_INGEST_LAG_SECS = 15 * 60
HEX32_LEN = 32
HEX64_LEN = 64
REQUIRED_METRICS = (
    "sorafs_reputation_ingest_lag_seconds",
    "sorafs_reputation_snapshot_age_seconds",
    "sorafs_reputation_snapshot_generated_at_unix",
    "sorafs_reputation_provider_count",
    "sorafs_reputation_low_score_providers",
    "sorafs_reputation_score",
    "sorafs_reputation_threshold_crossings_total",
    "sorafs_reputation_iteration_count",
    "sorafs_reputation_penalty_applied_total",
)
EXPLICIT_KIND_CONFLICT_DIAGNOSTIC = "explicit evidence kind conflicts with previous kind"
UNKNOWN_SCHEMA_DIAGNOSTIC = "unknown reputation evidence schema"
EXPLICIT_KIND_SCHEMA_MISMATCH_DIAGNOSTIC = (
    "evidence schema does not match explicit kind"
)
INFER_KIND_DIAGNOSTIC = "cannot infer evidence kind"
PROVIDER_ID_PATTERN = re.compile(r"^provider-[a-z0-9]+(?:-[a-z0-9]+)*\Z")
PROVIDER_ID_ERROR = "provider_id must match canonical lowercase `provider-name`"
FORBIDDEN_PROVIDER_ID_MARKERS = frozenset(
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


@dataclass(frozen=True)
class EvidenceKind:
    """One SFM-3 rollout evidence class."""

    name: str
    schema: str
    required: bool = True


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("publish", "sorafs.reputation.publish_snapshot_summary.v1"),
    EvidenceKind("latest", "sorafs.reputation.latest_snapshot_summary.v1"),
    EvidenceKind("provider", "sorafs.reputation.provider_proof.v1"),
    EvidenceKind("events", "sorafs.reputation.events_watch.v1"),
    EvidenceKind("verify", "sorafs.reputation.provider_proof_verification.v1"),
    EvidenceKind("metrics", "sorafs.reputation.metrics_canary.v1"),
    EvidenceKind("transport", "sorafs.reputation.transport_canary.v1"),
    EvidenceKind("consumption", "sorafs.reputation.routing_incentive_consumption.v1"),
)

KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS if kind.schema}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS if kind.required)
COMMON_EVIDENCE_REQUIRED_FIELDS: tuple[str, ...] = (
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
)
EVIDENCE_REQUIRED_FIELDS: dict[str, tuple[str, ...]] = {
    "publish": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "snapshot_id_hex",
        "merkle_root_hex",
        "provider_count",
        "providers",
    ),
    "latest": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "snapshot_id_hex",
        "merkle_root_hex",
        "provider_count",
        "providers",
    ),
    "provider": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "snapshot_id_hex",
        "merkle_root_hex",
        "provider",
        "proof",
    ),
    "events": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "since",
        "limit",
        "next_since",
        "count",
        "events",
    ),
    "verify": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "valid",
        "proof_verified",
        "snapshot_id_hex",
        "merkle_root_hex",
        "provider_count",
        "providers",
        "provider_id",
        "provider_score_bps",
    ),
    "metrics": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "schema",
        "status",
        "metrics_scrape_success",
        "snapshot_id_hex",
        "merkle_root_hex",
        "provider_count",
        "providers",
        "metric_count",
        "metrics",
        "snapshot_age_seconds",
        "ingest_lag_seconds",
    ),
    "transport": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "schema",
        "status",
        "sse_connected",
        "websocket_connected",
        "snapshot_id_hex",
        "merkle_root_hex",
        "sse_event_count",
        "sse_events",
        "websocket_event_count",
        "websocket_events",
    ),
    "consumption": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "schema",
        "status",
        "routing_score_consumed",
        "routing_weight_changed",
        "incentive_score_consumed",
        "snapshot_id_hex",
        "merkle_root_hex",
        "provider_count",
        "providers",
    ),
}
SNAPSHOT_ANCHOR_KINDS = ("publish", "latest")
SNAPSHOT_BOUND_KINDS = (
    "provider",
    "events",
    "verify",
    "metrics",
    "transport",
    "consumption",
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "proof_b64",
    "proof_bytes",
    "raw_provider_records",
    "request_body",
    "response_body",
    "secret",
    "signed_transaction",
    "token",
    "snapshot_b64",
    "snapshot_bytes",
    "token_b64",
}

FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "snapshot_id_hex",
    "merkle_root_hex",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
    "provider_count",
    "provider_id",
    "metric_count",
    "metrics",
)


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


@dataclass(frozen=True)
class LoadedEvidence:
    """Loaded evidence payload plus source metadata."""

    kind: str
    path: Path
    payload: dict[str, Any]
    digest: str



def artifact_kind_from_name(path: Path) -> str | None:
    stem = path.stem.lower().replace("_", "-")
    if "verify" in stem or "proof-replay" in stem:
        return "verify"
    if "provider" in stem or "fetch" in stem or "proof" in stem:
        return "provider"
    if "watch" in stem or "event" in stem:
        return "events"
    if "metric" in stem or "prometheus" in stem:
        return "metrics"
    if "transport" in stem or "sse" in stem or "websocket" in stem:
        return "transport"
    if "routing" in stem or "incentive" in stem or "consumption" in stem:
        return "consumption"
    if "publish" in stem:
        return "publish"
    if "latest" in stem or "snapshot" in stem:
        return "latest"
    return None


def artifact_kind(path: Path, payload: dict[str, Any], explicit_kind: str | None) -> str | None:
    if explicit_kind:
        return explicit_kind
    schema = payload.get("schema")
    if isinstance(schema, str) and schema in SCHEMA_TO_KIND:
        return SCHEMA_TO_KIND[schema].name
    return artifact_kind_from_name(path)


def parse_evidence_spec(spec: str) -> tuple[str | None, Path]:
    kind, separator, path = spec.partition("=")
    if separator:
        kind = kind.strip()
        if kind not in KIND_BY_NAME:
            raise ValueError("unknown evidence kind")
        return kind, Path(path.strip())
    return None, Path(spec)


def load_evidence(
    evidence_dirs: list[Path],
    evidence_specs: list[str],
    reserved_output_paths: tuple[Path, ...] = (),
) -> tuple[list[LoadedEvidence], list[str]]:
    loaded: list[LoadedEvidence] = []
    errors: list[str] = []

    explicit_entries: list[tuple[str | None, Path]] = []
    for spec in evidence_specs:
        try:
            explicit_kind, path = parse_evidence_spec(spec)
        except ValueError as error:
            errors.append(error_diagnostic_label(error))
            continue
        explicit_entries.append((explicit_kind, path))

    explicit_kinds_by_path: dict[Path, str | None] = {}
    for explicit_kind, path in explicit_entries:
        resolved = resolve_evidence_path(path, errors)
        if resolved is None:
            continue
        if resolved in explicit_kinds_by_path:
            previous_kind = explicit_kinds_by_path[resolved]
            if previous_kind != explicit_kind:
                errors.append(EXPLICIT_KIND_CONFLICT_DIAGNOSTIC)
            continue
        explicit_kinds_by_path[resolved] = explicit_kind

    files = discover_evidence_files(
        evidence_dirs,
        [path for _, path in explicit_entries],
        errors,
        reserved_output_paths=reserved_output_paths,
    )

    for path in files:
        resolved = resolve_evidence_path(path, errors)
        if resolved is None:
            continue
        explicit_kind = explicit_kinds_by_path.get(resolved)
        explicit = resolved in explicit_kinds_by_path
        loaded_evidence = load_evidence_json_with_sha256_or_record_error(
            path,
            MAX_EVIDENCE_BYTES,
            errors,
        )
        if loaded_evidence is None:
            continue
        payload, digest = loaded_evidence
        schema = payload.get("schema")
        if isinstance(schema, str) and schema not in SCHEMA_TO_KIND and explicit:
            errors.append(UNKNOWN_SCHEMA_DIAGNOSTIC)
            continue
        if isinstance(schema, str) and schema in SCHEMA_TO_KIND and explicit_kind:
            schema_kind = SCHEMA_TO_KIND[schema].name
            if schema_kind != explicit_kind:
                errors.append(EXPLICIT_KIND_SCHEMA_MISMATCH_DIAGNOSTIC)
                continue
        kind = artifact_kind(path, payload, explicit_kind)
        if kind is None:
            if explicit:
                errors.append(INFER_KIND_DIAGNOSTIC)
            continue
        loaded.append(LoadedEvidence(kind, path, payload, digest))

    return loaded, errors


def validate_snapshot_summary(
    payload: dict[str, Any],
    errors: list[str],
    *,
    now_unix: int,
    max_snapshot_age_secs: int,
    context: str,
) -> tuple[str, str, int]:
    require_status_in(
        payload,
        ("accepted", "published", "ready", "ok"),
        errors,
        path=f"{context}.status",
        allow_absent=True,
    )
    snapshot_id = require_hex(payload, "snapshot_id_hex", HEX32_LEN, errors)
    merkle_root = require_hex(payload, "merkle_root_hex", HEX64_LEN, errors)
    provider_count = require_minimum_int(payload, "provider_count", 1, errors)
    validate_provider_inventory(payload, errors)
    require_recent_timestamp(
        payload,
        "generated_at_unix",
        errors,
        now_unix=now_unix,
        max_age_secs=max_snapshot_age_secs,
        path=f"{context}.generated_at_unix",
    )
    return snapshot_id, merkle_root, provider_count


def validate_provider_inventory(payload: dict[str, Any], errors: list[str]) -> None:
    """Require provider_count to match reviewed provider labels."""

    require_string_inventory_count_match(
        payload,
        "providers",
        "provider_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for _, provider in require_object_array(payload, "providers", errors):
        provider_name = require_string(provider, "name", errors)
        if provider_name:
            validate_provider_id_value(
                provider_name,
                errors,
                path="providers[].name",
            )


def validate_common_rollout_context(
    payload: dict[str, Any],
    errors: list[str],
) -> None:
    """Require deployment-bound rollout metadata on every reputation artifact."""

    require_minimum_int(payload, "generated_at_unix", 1, errors)
    require_rollout_deployment_id(payload, errors)
    require_rollout_environment(payload, errors)
    require_rollout_deployment_context_review(payload, errors)


def finalize_reputation_required_rows(required: dict[str, dict[str, Any]]) -> None:
    """Publish the common required-row fields consumed by aggregate readiness."""

    for kind_name, row in required.items():
        kind = KIND_BY_NAME.get(kind_name)
        if kind is not None:
            row["schema"] = kind.schema
        artifacts = row.get("artifacts")
        artifact_count = len(artifacts) if isinstance(artifacts, list) else 0
        row["present"] = artifact_count > 0
        row["artifact_count"] = artifact_count
        if kind is not None and isinstance(artifacts, list):
            for artifact in artifacts:
                if isinstance(artifact, dict):
                    artifact.update({"schema": kind.schema})


def validate_publish_or_latest(
    evidence: LoadedEvidence,
    errors: list[str],
    *,
    now_unix: int,
    max_snapshot_age_secs: int,
) -> tuple[str, str, int]:
    return validate_snapshot_summary(
        evidence.payload,
        errors,
        now_unix=now_unix,
        max_snapshot_age_secs=max_snapshot_age_secs,
        context=evidence.kind,
    )


def validate_provider_id_value(
    provider_id: str,
    errors: list[str],
    *,
    path: str,
) -> str:
    """Require a reviewed lowercase production provider identifier value."""

    if not provider_id:
        return ""
    if PROVIDER_ID_PATTERN.fullmatch(provider_id) is None:
        errors.append(PROVIDER_ID_ERROR.replace("provider_id", path))
        return ""
    forbidden = sorted(
        marker
        for marker in FORBIDDEN_PROVIDER_ID_MARKERS
        if marker in provider_id.split("-")
    )
    if forbidden:
        errors.append(f"{path} must not contain non-production markers {forbidden}")
        return ""
    return provider_id


def require_provider_id(
    payload: dict[str, Any], errors: list[str], *, path: str = "provider_id"
) -> str:
    """Require a reviewed lowercase production provider identifier."""

    provider_id = require_string(payload, "provider_id", errors)
    return validate_provider_id_value(provider_id, errors, path=path)


def validate_provider(evidence: LoadedEvidence, errors: list[str]) -> tuple[str, str, str]:
    payload = evidence.payload
    snapshot_id = require_hex(payload, "snapshot_id_hex", HEX32_LEN, errors)
    merkle_root = require_hex(payload, "merkle_root_hex", HEX64_LEN, errors)
    provider = require_object(payload.get("provider"), "provider", errors)
    proof = require_object(payload.get("proof"), "proof", errors)
    provider_id = require_provider_id(provider, errors, path="provider.provider_id")
    proof_provider_id = require_provider_id(proof, errors, path="proof.provider_id")
    require_string_value_equal(
        proof_provider_id,
        "proof.provider_id",
        provider_id,
        "provider.provider_id",
        errors,
    )
    require_int_range(
        provider,
        "score_bps",
        errors,
        min_value=0,
        max_value=10_000,
        path="provider.score_bps",
    )
    require_positive_int(proof, "leaf_index", errors)
    require_hex_string_array(
        proof,
        "siblings_hex",
        HEX64_LEN,
        errors,
        path="proof.siblings_hex",
        unique=True,
    )
    return snapshot_id, merkle_root, provider_id


def validate_events(evidence: LoadedEvidence, errors: list[str]) -> tuple[str, str, int]:
    payload = evidence.payload
    count = require_positive_int(payload, "count", errors)
    limit = require_positive_int(payload, "limit", errors)
    require_advancing_int_pair(payload, "since", "next_since", errors)
    event_records = require_object_array(payload, "events", errors)
    if (
        isinstance(count, int)
        and not isinstance(count, bool)
        and isinstance(limit, int)
        and not isinstance(limit, bool)
        and count > limit
    ):
        errors.append("count must be <= limit")
    if not event_records:
        return "", "", count
    event = event_records[-1][1]
    sequences: list[int] = []
    for _index, event_record in event_records:
        event_sequence = require_positive_int(event_record, "sequence", errors)
        if event_sequence:
            sequences.append(event_sequence)
    unique_sequences = set(sequences)
    if len(unique_sequences) != len(sequences):
        errors.append("events must not contain duplicate sequence values")
    if (
        isinstance(count, int)
        and not isinstance(count, bool)
        and count != len(unique_sequences)
    ):
        errors.append("count must match unique events sequence count")
    sequence = sequences[-1] if sequences else 0
    snapshot_id = require_hex(event, "snapshot_id_hex", HEX32_LEN, errors)
    merkle_root = require_hex(event, "merkle_root_hex", HEX64_LEN, errors)
    require_minimum_int(event, "provider_count", 1, errors)
    require_count_length_match(count, event_records, "count", "events", errors)
    return snapshot_id, merkle_root, sequence


def validate_verify(evidence: LoadedEvidence, errors: list[str]) -> tuple[str, str, str]:
    payload = evidence.payload
    require_bool_true(payload, "valid", errors)
    require_bool_true(payload, "proof_verified", errors)
    snapshot_id = require_hex(payload, "snapshot_id_hex", HEX32_LEN, errors)
    merkle_root = require_hex(payload, "merkle_root_hex", HEX64_LEN, errors)
    require_minimum_int(payload, "provider_count", 1, errors)
    validate_provider_inventory(payload, errors)
    provider_id = require_provider_id(payload, errors)
    require_int_range(
        payload,
        "provider_score_bps",
        errors,
        min_value=0,
        max_value=10_000,
    )
    return snapshot_id, merkle_root, provider_id


def validate_metrics(
    evidence: LoadedEvidence,
    errors: list[str],
    *,
    max_snapshot_age_secs: int,
    max_ingest_lag_secs: int,
) -> tuple[str, str, int]:
    payload = evidence.payload
    require_string_equal(
        payload,
        "schema",
        "sorafs.reputation.metrics_canary.v1",
        errors,
        path="metrics.schema",
        quote_expected=False,
    )
    require_passed_status(payload, errors, path="metrics.status")
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_false_or_absent(payload, "response_bodies_included", errors)
    snapshot_id = require_hex(payload, "snapshot_id_hex", HEX32_LEN, errors)
    merkle_root = require_hex(payload, "merkle_root_hex", HEX64_LEN, errors)
    provider_count = require_minimum_int(payload, "provider_count", 1, errors)
    validate_provider_inventory(payload, errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_positive_int(payload, "metric_count", errors)
    require_string_inventory_count_match(payload, "metrics", "metric_count", errors)
    require_only_required_values(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_maximum_number(
        payload,
        "snapshot_age_seconds",
        max_snapshot_age_secs,
        errors,
    )
    require_maximum_number(
        payload,
        "ingest_lag_seconds",
        max_ingest_lag_secs,
        errors,
    )
    return snapshot_id, merkle_root, provider_count


def validate_transport(evidence: LoadedEvidence, errors: list[str]) -> tuple[str, str, int, int]:
    payload = evidence.payload
    require_string_equal(
        payload,
        "schema",
        "sorafs.reputation.transport_canary.v1",
        errors,
        path="transport.schema",
        quote_expected=False,
    )
    require_passed_status(payload, errors, path="transport.status")
    require_bool_true(payload, "sse_connected", errors)
    require_bool_true(payload, "websocket_connected", errors)
    require_false_or_absent(payload, "response_bodies_included", errors)
    snapshot_id = require_hex(payload, "snapshot_id_hex", HEX32_LEN, errors)
    merkle_root = require_hex(payload, "merkle_root_hex", HEX64_LEN, errors)
    sse_count = require_positive_int(payload, "sse_event_count", errors)
    require_string_inventory_count_match(
        payload,
        "sse_events",
        "sse_event_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for _index, record in require_object_array(payload, "sse_events", errors):
        require_string(record, "name", errors)
    websocket_count = require_positive_int(payload, "websocket_event_count", errors)
    require_string_inventory_count_match(
        payload,
        "websocket_events",
        "websocket_event_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for _index, record in require_object_array(payload, "websocket_events", errors):
        require_string(record, "name", errors)
    return snapshot_id, merkle_root, sse_count, websocket_count


def validate_consumption(evidence: LoadedEvidence, errors: list[str]) -> tuple[str, str, int]:
    payload = evidence.payload
    require_string_equal(
        payload,
        "schema",
        "sorafs.reputation.routing_incentive_consumption.v1",
        errors,
        path="consumption.schema",
        quote_expected=False,
    )
    require_passed_status(payload, errors, path="consumption.status")
    require_bool_true(payload, "routing_score_consumed", errors)
    require_bool_true(payload, "routing_weight_changed", errors)
    require_bool_true(payload, "incentive_score_consumed", errors)
    require_false_or_absent(payload, "raw_provider_records_included", errors)
    snapshot_id = require_hex(payload, "snapshot_id_hex", HEX32_LEN, errors)
    merkle_root = require_hex(payload, "merkle_root_hex", HEX64_LEN, errors)
    provider_count = require_minimum_int(payload, "provider_count", 1, errors)
    validate_provider_inventory(payload, errors)
    return snapshot_id, merkle_root, provider_count


def validate_evidence_set(
    loaded: list[LoadedEvidence],
    *,
    evidence_dirs: list[Path] | None = None,
    required_kinds: tuple[str, ...],
    required_providers: tuple[str, ...],
    now_unix: int,
    max_snapshot_age_secs: int,
    max_ingest_lag_secs: int,
) -> dict[str, Any]:
    required: dict[str, dict[str, Any]] = {
        kind: {"valid": False, "errors": [], "artifacts": []} for kind in required_kinds
    }
    recognized: list[dict[str, Any]] = []
    snapshot_values: dict[str, str] = {}
    valid_snapshot_bindings: set[tuple[str, str]] = set()
    snapshot_bound_artifacts: list[dict[str, Any]] = []
    provider_ids: set[str] = set()
    provider_counts: set[int] = set()
    metric_counts: set[int] = set()
    metric_names: set[str] = set()

    for evidence in loaded:
        errors: list[str] = []
        payload = evidence.payload
        digest = evidence.digest
        record_kind = evidence.kind
        visit_sensitive_fields(
            payload,
            "",
            errors,
            sensitive_keys=SENSITIVE_KEYS,
            evidence_label="rollout evidence",
        )
        snapshot_id = ""
        merkle_root = ""
        provider_count = 0
        provider_id = ""
        validate_common_rollout_context(payload, errors)

        if evidence.kind in SNAPSHOT_ANCHOR_KINDS:
            snapshot_id, merkle_root, provider_count = validate_publish_or_latest(
                evidence,
                errors,
                now_unix=now_unix,
                max_snapshot_age_secs=max_snapshot_age_secs,
            )
        elif evidence.kind == "provider":
            snapshot_id, merkle_root, provider_id = validate_provider(evidence, errors)
            record_observed_evidence_value(provider_ids, provider_id)
        elif evidence.kind == "events":
            snapshot_id, merkle_root, _sequence = validate_events(evidence, errors)
        elif evidence.kind == "verify":
            snapshot_id, merkle_root, provider_id = validate_verify(evidence, errors)
            record_observed_evidence_value(provider_ids, provider_id)
        elif evidence.kind == "metrics":
            snapshot_id, merkle_root, provider_count = validate_metrics(
                evidence,
                errors,
                max_snapshot_age_secs=max_snapshot_age_secs,
                max_ingest_lag_secs=max_ingest_lag_secs,
            )
            record_observed_evidence_value(metric_counts, payload.get("metric_count"))
            metric_names.update(hashable_evidence_values(payload.get("metrics")))
        elif evidence.kind == "transport":
            snapshot_id, merkle_root, _sse, _websocket = validate_transport(evidence, errors)
        elif evidence.kind == "consumption":
            snapshot_id, merkle_root, provider_count = validate_consumption(evidence, errors)
        else:
            errors.append("unsupported evidence kind")
            record_kind = "<unknown>"

        record_consistent_evidence_value(
            snapshot_values,
            "snapshot_id_hex",
            snapshot_id,
            record_kind,
            errors,
        )
        record_consistent_evidence_value(
            snapshot_values,
            "merkle_root_hex",
            merkle_root,
            record_kind,
            errors,
        )
        record_observed_evidence_value(provider_counts, provider_count)

        fingerprint_values = {
            "snapshot_id_hex": snapshot_id,
            "merkle_root_hex": merkle_root,
        }
        if provider_count > 0:
            fingerprint_values["provider_count"] = provider_count
        if provider_id != "":
            fingerprint_values["provider_id"] = provider_id

        record = build_kinded_evidence_artifact(
            kind_name=record_kind,
            path=archive_artifact_path_label(evidence.path, evidence_dirs or []),
            digest=digest,
            payload=payload,
            validation_errors=errors,
            fingerprint_fields=FINGERPRINT_FIELDS,
            fingerprint_values=fingerprint_values,
        )
        kind = KIND_BY_NAME.get(record_kind)
        if kind is not None:
            record["schema"] = kind.schema
        record_snapshot_bound_evidence_artifact(
            kind_name=record_kind,
            artifact=record,
            snapshot_id=snapshot_id,
            merkle_root=merkle_root,
            valid=not errors,
            anchor_kinds=SNAPSHOT_ANCHOR_KINDS,
            bound_kinds=SNAPSHOT_BOUND_KINDS,
            valid_snapshot_bindings=valid_snapshot_bindings,
            snapshot_bound_artifacts=snapshot_bound_artifacts,
        )
        recognized.append(record)
        record_custom_required_evidence_artifact(
            required,
            record_kind,
            record,
            errors,
        )

    finalize_custom_required_evidence_rows(required, evidence_label="evidence")

    record_missing_required_evidence_value_errors(
        required,
        "provider",
        required_providers,
        provider_ids,
        lambda _provider_id: "missing provider/proof evidence for required provider",
    )

    record_missing_required_or_observed_evidence_error(
        required,
        "provider",
        required_providers,
        provider_ids,
        "at least one provider proof must be verified",
    )

    record_inconsistent_evidence_values_error(
        required,
        provider_counts,
        "latest",
        "provider counts differ across rollout evidence",
    )

    validate_snapshot_bound_evidence_artifacts(
        required=required,
        required_kinds=required_kinds,
        bound_kinds=SNAPSHOT_BOUND_KINDS,
        valid_snapshot_bindings=valid_snapshot_bindings,
        snapshot_bound_artifacts=snapshot_bound_artifacts,
        required_anchor_kind="latest",
        binding_error=(
            "snapshot_id_hex and merkle_root_hex must match a valid "
            "publish/latest artifact"
        ),
        binding_summary_error=(
            "snapshot binding must match a valid publish/latest artifact"
        ),
        missing_anchor_error=(
            "snapshot_id_hex and merkle_root_hex require a valid publish/latest "
            "artifact"
        ),
        missing_anchor_summary_error=(
            "snapshot binding requires a valid publish/latest artifact"
        ),
        missing_required_anchor_error=(
            "snapshot-bound reputation evidence requires a valid publish/latest "
            "snapshot_id_hex and merkle_root_hex"
        ),
    )
    finalize_reputation_required_rows(required)

    status = (
        "ready"
        if required_evidence_summary_is_valid(required)
        and recognized_evidence_artifacts_are_valid(recognized)
        else "failed"
    )
    errors = (
        []
        if status == "ready"
        else ["reputation rollout evidence did not satisfy required rows"]
    )
    files = [evidence.path for evidence in loaded]
    return {
        "schema": SUMMARY_SCHEMA,
        "status": status,
        "required_kinds": required_evidence_kind_names(required_kinds),
        "thresholds": {
            "max_snapshot_age_secs": max_snapshot_age_secs,
            "max_ingest_lag_secs": max_ingest_lag_secs,
            "max_evidence_bytes": MAX_EVIDENCE_BYTES,
        },
        "snapshot_id_hex": snapshot_values.get("snapshot_id_hex"),
        "merkle_root_hex": snapshot_values.get("merkle_root_hex"),
        "valid_snapshot_bindings": [
            {
                "snapshot_id_hex": snapshot_id,
                "merkle_root_hex": merkle_root,
            }
            for snapshot_id, merkle_root in sorted(valid_snapshot_bindings)
        ],
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_recognized_evidence_artifacts(recognized),
        "recognized_artifacts": recognized,
        "required": required,
        "provider_ids": sorted(provider_ids),
        "provider_count_values": sorted(provider_counts),
        "metrics": sorted(metric_names),
        "metric_count_values": sorted(metric_counts),
        "errors": errors,
    }


def build_parser() -> EvidenceArgumentParser:
    parser = EvidenceArgumentParser(description=__doc__)
    parser.add_argument(
        "--evidence-dir",
        action="append",
        type=Path,
        default=[],
        help="Directory containing reputation rollout evidence JSON artifacts.",
    )
    parser.add_argument(
        "--evidence",
        action="append",
        default=[],
        help="Evidence path, or KIND=PATH for schema-less CLI outputs.",
    )
    parser.add_argument(
        "--require-kind",
        action="append",
        help=(
            "Required evidence kind, or comma-separated kinds. "
            "Defaults to all production gate kinds."
        ),
    )
    parser.add_argument(
        "--require-provider",
        action="append",
        default=[],
        help="Provider id that must have provider/proof evidence.",
    )
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        default=None,
        help="Override current Unix time for deterministic freshness checks.",
    )
    parser.add_argument(
        "--max-snapshot-age-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_SNAPSHOT_AGE_SECS,
        help="Maximum accepted latest snapshot age.",
    )
    parser.add_argument(
        "--max-ingest-lag-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_INGEST_LAG_SECS,
        help="Maximum accepted reputation ingest lag.",
    )
    parser.add_argument("--summary-out", type=Path, help="Write gate summary JSON here.")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
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
            args.require_kind or [],
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

    loaded, load_errors = load_evidence(
        args.evidence_dir,
        args.evidence,
        () if args.summary_out is None else (args.summary_out,),
    )
    if any("conflicts with reserved output" in error for error in load_errors):
        emit_checker_error_lines(load_errors)
        return 2
    now_unix = args.now_unix if args.now_unix is not None else int(time.time())
    summary = validate_evidence_set(
        loaded,
        evidence_dirs=args.evidence_dir,
        required_kinds=required_kinds,
        required_providers=tuple(args.require_provider),
        now_unix=now_unix,
        max_snapshot_age_secs=args.max_snapshot_age_secs,
        max_ingest_lag_secs=args.max_ingest_lag_secs,
    )
    if load_errors:
        summary["status"] = "failed"
        summary["load_errors"] = load_errors
        summary["errors"] = list(summary.get("errors", [])) + load_errors

    rendered_summary, summary_errors = render_and_write_checker_summary(
        args.summary_out, summary
    )
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2
    return 0 if summary["status"] == "ready" else 1


if __name__ == "__main__":
    raise SystemExit(main())
