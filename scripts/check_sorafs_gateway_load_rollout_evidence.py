#!/usr/bin/env python3
"""Validate SoraFS gateway load rollout evidence artifacts."""

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
    build_required_evidence_summary,
    count_evidence_artifacts,
    recognized_evidence_artifacts,
    count_evidence_files,
    evidence_artifact_fingerprint,
    evidence_artifact_is_valid,
    evidence_gate_status,
    evidence_schema_by_kind,
    hashable_evidence_values,
    init_evidence_artifact_buckets,
    record_evidence_artifact,
    record_evidence_validation_errors,
    record_explicit_evidence_validation_errors,
    record_observed_evidence_value,
    require_bool_true,
    require_config_backed_governance_approval,
    require_false,
    require_hex,
    require_maximum_int,
    require_minimum_int,
    require_object,
    require_object_array,
    require_passed_status,
    require_policy_digest,
    require_positive_int,
    require_recent_timestamp,
    require_string,
    require_string_coverage,
    require_string_equal,
    require_string_in,
    require_string_inventory_count_match,
    required_evidence_kind_names,
    validate_bound_evidence_digest_references,
    validate_standard_evidence_payload,
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


SUMMARY_SCHEMA = "sorafs.gateway_load.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MIN_STAGING_DURATION_SECS = 3_600
DEFAULT_MIN_STREAMS = 1_000
DEFAULT_MIN_SUCCESS_RATE_BPS = 9_900
MAX_BASIS_POINTS = 10_000
MAX_SUCCESS_RATE_BPS = MAX_BASIS_POINTS
MAX_ERROR_RATE_BPS = MAX_BASIS_POINTS
DEFAULT_MAX_ERROR_RATE_BPS = 100
DEFAULT_MAX_P95_LATENCY_MS = 1_500
DEFAULT_MAX_P99_LATENCY_MS = 3_000
HEX64_LEN = 64
DEFAULT_GATEWAY_CONFORMANCE_CARGO_COMMAND = (
    "cargo test -p integration_tests --test nexus_and_streaming "
    "sorafs_gateway_conformance -- --nocapture"
)
LOCKED_GATEWAY_CONFORMANCE_CARGO_COMMAND = (
    "cargo test --locked -p integration_tests --test nexus_and_streaming "
    "sorafs_gateway_conformance -- --nocapture"
)
ALLOWED_GATEWAY_CONFORMANCE_CARGO_COMMANDS = (
    DEFAULT_GATEWAY_CONFORMANCE_CARGO_COMMAND,
    LOCKED_GATEWAY_CONFORMANCE_CARGO_COMMAND,
)
GATEWAY_VERSION_PATTERN = re.compile(
    r"^iroha-gateway "
    r"(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)\."
    r"(0|[1-9][0-9]*)"
    r"(?:-rc\.([1-9][0-9]*))?"
    r"\Z"
)
GATEWAY_VERSION_ERROR = (
    "gateway_version must match `iroha-gateway X.Y.Z` or "
    "`iroha-gateway X.Y.Z-rc.N`"
)
HARDWARE_PROFILE_PATTERN = re.compile(
    r"^gateway-load-hardware-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
STREAM_NAME_PATTERN = re.compile(r"^gateway-load-stream-[0-9]{4,}\Z")
PROVIDER_NAME_PATTERN = re.compile(
    r"^gateway-load-provider-[a-z0-9]+(?:-[a-z0-9]+)*\Z"
)
HARDWARE_PROFILE_ERROR = (
    "hardware_profile.name must match reviewed `gateway-load-hardware-*` label"
)
STREAM_NAME_ERROR = (
    "streams[].name must match generated `gateway-load-stream-NNNN` label"
)
PROVIDER_NAME_ERROR = (
    "providers[].name must match reviewed `gateway-load-provider-*` label"
)
CACHE_STATE_ERROR = "cache_state.mode must be a reviewed cache-state value"
REQUIRED_CACHE_STATES = (
    "cold-cache",
    "warm-cache",
    "mixed-cache",
)
FORBIDDEN_STAGING_METADATA_MARKERS = frozenset(
    (
        "debug",
        "dev",
        "draft",
        "example",
        "fake",
        "latest",
        "local",
        "laptop",
        "placeholder",
        "sample",
        "secret",
        "test",
        "todo",
        "tmp",
    )
)

REQUIRED_SCENARIOS = (
    "full_car_replay",
    "aligned_range_replay",
    "misaligned_range_refusal",
    "multi_range_replay",
    "unsupported_chunker",
    "missing_headers",
    "corrupted_por_proof",
    "corrupted_car_payload",
    "provider_not_admitted",
    "gateway_rate_limit",
    "gar_denylist_refusal",
    "capability_refusal",
)
REQUIRED_METRICS = (
    "sorafs_gateway_latency_ms_bucket",
    "sorafs_gateway_refusals_total",
    "sorafs_gateway_bytes_total",
    "sorafs_gateway_concurrency_active",
)
STAGING_REPORT_BOUND_KINDS = (
    "telemetry_slo",
    "transport_scope",
    "governance_approval",
)
POLICY_BOUND_KINDS = ("governance_approval",)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "fixture_payload",
    "gateway_private_key",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_attestation",
    "raw_car",
    "raw_fixture",
    "raw_payload",
    "raw_report",
    "raw_request",
    "raw_response",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signing_key",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SF-5a gateway load rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("local_conformance", "sorafs.gateway_load.local_conformance.v1"),
    EvidenceKind("staging_load", "sorafs.gateway_load.staging_load.v1"),
    EvidenceKind("telemetry_slo", "sorafs.gateway_load.telemetry_slo.v1"),
    EvidenceKind("transport_scope", "sorafs.gateway_load.transport_scope.v1"),
    EvidenceKind("governance_approval", "sorafs.gateway_load.governance_approval.v1"),
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
    "local_conformance": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "ci_script",
        "cargo_command",
        "deterministic_harness_passed",
        "attestation_verified",
        "suite_report_digest_hex",
        "scenario_count",
        "load_profile_streams",
        "load_profile_window_seconds",
        "scenarios",
        "raw_report_included",
        "private_keys_included",
    ),
    "staging_load": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "suite_report_digest_hex",
        "staging_report_digest_hex",
        "fixture_bundle_digest_hex",
        "policy_digest_hex",
        "gateway_version",
        "hardware_profile",
        "cache_state",
        "duration_seconds",
        "stream_count",
        "streams",
        "provider_count",
        "providers",
        "success_rate_bps",
        "error_rate_bps",
        "p95_latency_ms",
        "p99_latency_ms",
        "response_bodies_included",
        "raw_payloads_included",
    ),
    "telemetry_slo": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "staging_report_digest_hex",
        "metrics_scrape_success",
        "dashboard_archived",
        "slo_baseline_recorded",
        "cold_cache_baseline_recorded",
        "critical_alerts_firing",
        "metrics",
        "metric_count",
        "response_bodies_included",
    ),
    "transport_scope": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "staging_report_digest_hex",
        "http3_endpoint_committed",
        "http3_scenarios_deferred",
        "http3_config_surface_documented",
        "http3_scenarios_passed",
        "transport_scope_reviewed",
        "response_bodies_included",
    ),
    "governance_approval": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "gateway_release_bound",
        "local_conformance_bound",
        "staging_load_bound",
        "telemetry_bound",
        "transport_scope_bound",
        "suite_report_digest_hex",
        "staging_report_digest_hex",
        "config_source",
        "policy_digest_hex",
    ),
}


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SF-5a gateway load rollout gate."""

    now_unix: int
    max_evidence_age_secs: int
    min_staging_duration_secs: int
    min_streams: int
    min_success_rate_bps: int
    max_error_rate_bps: int
    max_p95_latency_ms: int
    max_p99_latency_ms: int


def validate_threshold_options(args: argparse.Namespace) -> list[str]:
    """Validate checker threshold options before evidence evaluation."""

    errors: list[str] = []
    if args.min_success_rate_bps > MAX_SUCCESS_RATE_BPS:
        errors.append(f"--min-success-rate-bps must be <= {MAX_SUCCESS_RATE_BPS}")
    if args.max_error_rate_bps > MAX_ERROR_RATE_BPS:
        errors.append(f"--max-error-rate-bps must be <= {MAX_ERROR_RATE_BPS}")
    return errors


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


FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "deployment_context_reviewed",
    "suite_report_digest_hex",
    "staging_report_digest_hex",
    "policy_digest_hex",
    "metric_count",
    "metrics",
)


def require_gateway_version(payload: dict[str, Any], errors: list[str]) -> str:
    """Require a concrete gateway release or release-candidate version label."""

    version = require_string(payload, "gateway_version", errors)
    if version and GATEWAY_VERSION_PATTERN.fullmatch(version) is None:
        errors.append(GATEWAY_VERSION_ERROR)
        return ""
    return version


def require_staging_metadata_label(
    record: dict[str, Any],
    field: str,
    errors: list[str],
    *,
    path: str,
    pattern: re.Pattern[str],
    pattern_error: str,
) -> str:
    """Require a reviewed lowercase staging metadata label."""

    value = require_string(record, field, errors)
    if not value:
        return ""
    if pattern.fullmatch(value) is None:
        errors.append(pattern_error)
        return ""
    forbidden = forbidden_non_production_markers(value, FORBIDDEN_STAGING_METADATA_MARKERS)
    if forbidden:
        errors.append(f"{path} must not contain non-production markers {forbidden}")
        return ""
    return value


def require_hardware_profile(payload: dict[str, Any], errors: list[str]) -> str:
    """Require a reviewed hardware profile label for live staging load evidence."""

    raw_record = payload.get("hardware_profile")
    if not isinstance(raw_record, dict):
        require_object(raw_record, "hardware_profile", errors)
        return ""
    record = require_object(raw_record, "hardware_profile", errors)
    return require_staging_metadata_label(
        record,
        "name",
        errors,
        path="hardware_profile.name",
        pattern=HARDWARE_PROFILE_PATTERN,
        pattern_error=HARDWARE_PROFILE_ERROR,
    )


def require_cache_state(payload: dict[str, Any], errors: list[str]) -> str:
    """Require a reviewed cache-state mode for live staging load evidence."""

    raw_record = payload.get("cache_state")
    if not isinstance(raw_record, dict):
        require_object(raw_record, "cache_state", errors)
        return ""
    record = require_object(raw_record, "cache_state", errors)
    mode = require_string(record, "mode", errors)
    if mode and mode not in REQUIRED_CACHE_STATES:
        errors.append(CACHE_STATE_ERROR)
        return ""
    return mode


def validate_local_conformance(payload: dict[str, Any], errors: list[str]) -> None:
    require_string_equal(
        payload,
        "ci_script",
        "ci/check_sorafs_gateway_conformance.sh",
        errors,
    )
    require_string_in(
        payload,
        "cargo_command",
        ALLOWED_GATEWAY_CONFORMANCE_CARGO_COMMANDS,
        errors,
        quote_values=False,
    )
    require_bool_true(payload, "deterministic_harness_passed", errors)
    require_bool_true(payload, "attestation_verified", errors)
    require_hex(payload, "suite_report_digest_hex", HEX64_LEN, errors)
    require_minimum_int(payload, "scenario_count", len(REQUIRED_SCENARIOS), errors)
    require_minimum_int(payload, "load_profile_streams", DEFAULT_MIN_STREAMS, errors)
    require_positive_int(payload, "load_profile_window_seconds", errors)
    require_string_coverage(payload, "scenarios", "", REQUIRED_SCENARIOS, errors)
    require_only_required_values(payload, "scenarios", "", REQUIRED_SCENARIOS, errors)
    require_string_inventory_count_match(payload, "scenarios", "scenario_count", errors)
    require_false(payload, "raw_report_included", errors)
    require_false(payload, "private_keys_included", errors)


def validate_staging_load(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "suite_report_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "staging_report_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "fixture_bundle_digest_hex", HEX64_LEN, errors)
    require_policy_digest(payload, errors)
    require_gateway_version(payload, errors)
    require_hardware_profile(payload, errors)
    require_cache_state(payload, errors)
    require_minimum_int(payload, "duration_seconds", options.min_staging_duration_secs, errors)
    require_minimum_int(payload, "stream_count", options.min_streams, errors)
    require_string_inventory_count_match(
        payload,
        "streams",
        "stream_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for _, record in require_object_array(payload, "streams", errors):
        require_staging_metadata_label(
            record,
            "name",
            errors,
            path="streams[].name",
            pattern=STREAM_NAME_PATTERN,
            pattern_error=STREAM_NAME_ERROR,
        )
    require_positive_int(payload, "provider_count", errors)
    require_string_inventory_count_match(
        payload,
        "providers",
        "provider_count",
        errors,
        field="name",
        allow_scalar_items=False,
    )
    for _, record in require_object_array(payload, "providers", errors):
        require_staging_metadata_label(
            record,
            "name",
            errors,
            path="providers[].name",
            pattern=PROVIDER_NAME_PATTERN,
            pattern_error=PROVIDER_NAME_ERROR,
    )
    require_minimum_int(payload, "success_rate_bps", options.min_success_rate_bps, errors)
    require_maximum_int(payload, "success_rate_bps", MAX_SUCCESS_RATE_BPS, errors)
    require_maximum_int(payload, "error_rate_bps", MAX_ERROR_RATE_BPS, errors)
    if options.max_error_rate_bps < MAX_ERROR_RATE_BPS:
        require_maximum_int(payload, "error_rate_bps", options.max_error_rate_bps, errors)
    require_maximum_int(payload, "p95_latency_ms", options.max_p95_latency_ms, errors)
    require_maximum_int(payload, "p99_latency_ms", options.max_p99_latency_ms, errors)
    require_false(payload, "response_bodies_included", errors)
    require_false(payload, "raw_payloads_included", errors)


def validate_telemetry_slo(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "staging_report_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_archived", errors)
    require_bool_true(payload, "slo_baseline_recorded", errors)
    require_bool_true(payload, "cold_cache_baseline_recorded", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_only_required_values(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_positive_int(payload, "metric_count", errors)
    require_string_inventory_count_match(payload, "metrics", "metric_count", errors)
    require_false(payload, "response_bodies_included", errors)


def validate_transport_scope(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "staging_report_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "transport_scope_reviewed", errors)
    committed = payload.get("http3_endpoint_committed")
    if committed is True:
        require_bool_true(payload, "http3_config_surface_documented", errors)
        require_bool_true(payload, "http3_scenarios_passed", errors)
        require_false(payload, "http3_scenarios_deferred", errors)
    elif committed is False:
        require_bool_true(payload, "http3_scenarios_deferred", errors)
        require_false(payload, "http3_config_surface_documented", errors)
        require_false(payload, "http3_scenarios_passed", errors)
    else:
        errors.append("http3_endpoint_committed must be a boolean")
    require_false(payload, "response_bodies_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "gateway_release_bound", errors)
    require_bool_true(payload, "local_conformance_bound", errors)
    require_bool_true(payload, "staging_load_bound", errors)
    require_bool_true(payload, "telemetry_bound", errors)
    require_bool_true(payload, "transport_scope_bound", errors)
    require_hex(payload, "suite_report_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "staging_report_digest_hex", HEX64_LEN, errors)
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

    if kind.name == "local_conformance":
        validate_local_conformance(payload, errors)
    elif kind.name == "staging_load":
        validate_staging_load(payload, errors, options)
    elif kind.name == "telemetry_slo":
        validate_telemetry_slo(payload, errors)
    elif kind.name == "transport_scope":
        validate_transport_scope(payload, errors)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS gateway load rollout artifact",
        SENSITIVE_KEYS,
        "rollout evidence",
        lambda kind, checked_payload, errors: validate_kind_specific(
            kind, checked_payload, errors, options
        ),
        require_reviewed_deployment_context=True,
    )


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


def build_summary(
    evidence_dirs: list[Path],
    evidence_files: list[Path],
    required_kinds: tuple[str, ...],
    options: ValidationOptions,
    summary_out: Path | None,
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []

    artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)
    valid_suite_report_digests: set[str] = set()
    valid_staging_report_digests: set[str] = set()
    suite_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    staging_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    valid_policy_digests: set[str] = set()
    policy_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
    metric_counts: set[int] = set()
    metric_names: set[str] = set()
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
        if kind_name == "telemetry_slo":
            record_observed_evidence_value(metric_counts, payload.get("metric_count"))
            metric_names.update(hashable_evidence_values(payload.get("metrics")))
        if evidence_artifact_is_valid(artifact):
            fingerprint = evidence_artifact_fingerprint(artifact)
            suite_digest = fingerprint.get("suite_report_digest_hex")
            staging_digest = fingerprint.get("staging_report_digest_hex")
            policy_digest = fingerprint.get("policy_digest_hex")
            if kind_name == "local_conformance" and isinstance(suite_digest, str):
                valid_suite_report_digests.add(suite_digest)
            if kind_name == "staging_load":
                if isinstance(suite_digest, str):
                    suite_bound_artifacts.append((kind_name, artifact))
                if isinstance(staging_digest, str):
                    valid_staging_report_digests.add(staging_digest)
                if isinstance(policy_digest, str):
                    valid_policy_digests.add(policy_digest)
            if kind_name in STAGING_REPORT_BOUND_KINDS:
                staging_bound_artifacts.append((kind_name, artifact))
            if kind_name in POLICY_BOUND_KINDS:
                policy_bound_artifacts.append((kind_name, artifact))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        record_evidence_validation_errors(path, validation_errors, errors)

    valid_suite_report_digests = require_single_active_digest(
        valid_suite_report_digests,
        errors,
        label="valid_suite_report_digests",
    )
    valid_staging_report_digests = require_single_active_digest(
        valid_staging_report_digests,
        errors,
        label="valid_staging_report_digests",
    )
    valid_policy_digests = require_single_active_digest(
        valid_policy_digests,
        errors,
        label="valid_policy_digests",
    )

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("local_conformance", "staging_load"),
        bound_artifacts=suite_bound_artifacts,
        valid_anchor_digests=valid_suite_report_digests,
        digest_field="suite_report_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} suite_report_digest_hex must reference a valid "
            "local_conformance suite_report_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} suite_report_digest_hex requires a valid "
            "local_conformance suite_report_digest_hex"
        ),
    )
    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("staging_load",) + STAGING_REPORT_BOUND_KINDS,
        bound_artifacts=staging_bound_artifacts,
        valid_anchor_digests=valid_staging_report_digests,
        digest_field="staging_report_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} staging_report_digest_hex must reference a valid "
            "staging_load staging_report_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} staging_report_digest_hex requires a valid "
            "staging_load staging_report_digest_hex"
        ),
    )
    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("staging_load",),
        bound_artifacts=policy_bound_artifacts,
        valid_anchor_digests=valid_policy_digests,
        digest_field="policy_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} policy_digest_hex must reference a valid "
            "staging_load policy_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} policy_digest_hex requires a valid "
            "staging_load policy_digest_hex"
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
            "min_staging_duration_secs": options.min_staging_duration_secs,
            "min_streams": options.min_streams,
            "min_success_rate_bps": options.min_success_rate_bps,
            "max_error_rate_bps": options.max_error_rate_bps,
            "max_p95_latency_ms": options.max_p95_latency_ms,
            "max_p99_latency_ms": options.max_p99_latency_ms,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "recognized_artifacts": recognized_evidence_artifacts(artifacts_by_kind),
        "valid_suite_report_digests": sorted(valid_suite_report_digests),
        "valid_staging_report_digests": sorted(valid_staging_report_digests),
        "valid_policy_digests": sorted(valid_policy_digests),
        "metrics": sorted(metric_names),
        "metric_count_values": sorted(metric_counts),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SF-5a gateway load rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all gateway load kinds.",
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
        "--min-staging-duration-secs",
        type=positive_int_arg,
        default=DEFAULT_MIN_STAGING_DURATION_SECS,
    )
    parser.add_argument("--min-streams", type=positive_int_arg, default=DEFAULT_MIN_STREAMS)
    parser.add_argument(
        "--min-success-rate-bps",
        type=positive_int_arg,
        default=DEFAULT_MIN_SUCCESS_RATE_BPS,
    )
    parser.add_argument(
        "--max-error-rate-bps",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_ERROR_RATE_BPS,
    )
    parser.add_argument(
        "--max-p95-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_P95_LATENCY_MS,
    )
    parser.add_argument(
        "--max-p99-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_P99_LATENCY_MS,
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

    threshold_errors = validate_threshold_options(args)
    if threshold_errors:
        emit_checker_error_lines(threshold_errors)
        return 2

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
        min_staging_duration_secs=args.min_staging_duration_secs,
        min_streams=args.min_streams,
        min_success_rate_bps=args.min_success_rate_bps,
        max_error_rate_bps=args.max_error_rate_bps,
        max_p95_latency_ms=args.max_p95_latency_ms,
        max_p99_latency_ms=args.max_p99_latency_ms,
    )
    preflight_errors = validate_checker_preflight(args)
    if preflight_errors:
        emit_checker_error_lines(preflight_errors)
        return 2

    summary, errors = build_summary(
        args.evidence_dir, args.evidence, required_kinds, options, args.summary_out
    )
    _rendered_summary, summary_errors = render_and_write_checker_summary(
        args.summary_out, summary
    )
    if summary_errors:
        emit_checker_error_lines(summary_errors)
        return 2

    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS gateway load rollout evidence is incomplete:",
            errors,
        )
        return 1

    emit_checker_notice(
        "SoraFS gateway load rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
