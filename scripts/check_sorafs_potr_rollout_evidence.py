#!/usr/bin/env python3
"""Validate SoraFS PoTR rollout evidence artifacts."""

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


SUMMARY_SCHEMA = "sorafs.potr.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_HOT_LATENCY_MS = 90_000
DEFAULT_MAX_WARM_LATENCY_MS = 300_000
DEFAULT_MIN_PROVIDERS = 3
DEFAULT_MIN_RECEIPTS = 6
HEX64_LEN = 64

REQUIRED_TIERS = ("hot", "warm")
REQUIRED_ROUTES = ("gateway_range_fetch", "proof_stream_potr", "proof_stream_filter")
REQUIRED_METRICS = (
    "torii_sorafs_proof_stream_events_total",
    "torii_sorafs_proof_stream_latency_ms_bucket",
    "torii_sorafs_proof_stream_inflight",
    "torii_sorafs_proof_health_potr_breaches",
    "torii_da_potr_bonus_micro_total",
)
RECEIPT_SUMMARY_BOUND_KINDS = (
    "receipt_validation",
    "proof_stream",
    "reputation_integration",
    "observability",
    "governance_approval",
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "evidence_json",
    "fetch_transcript",
    "mnemonic",
    "norito_bytes",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_evidence",
    "raw_fetch",
    "raw_receipt",
    "raw_receipts",
    "raw_request",
    "raw_response",
    "receipt",
    "receipt_bytes",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signed_transaction",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SF-14 rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("multi_provider_probe", "sorafs.potr.multi_provider_probe_canary.v1"),
    EvidenceKind("receipt_validation", "sorafs.potr.receipt_validation_canary.v1"),
    EvidenceKind("proof_stream", "sorafs.potr.proof_stream_canary.v1"),
    EvidenceKind("reputation_integration", "sorafs.potr.reputation_integration_canary.v1"),
    EvidenceKind("observability", "sorafs.potr.observability_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.potr.governance_approval.v1"),
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
    "multi_provider_probe": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "tiers_observed",
        "gateway_receipts_captured",
        "range_fetch_verified",
        "deadline_headers_verified",
        "proof_stream_replay_verified",
        "trace_correlation_verified",
        "provider_count",
        "receipt_count",
        "max_hot_latency_ms",
        "max_warm_latency_ms",
        "receipt_summary_digest_hex",
        "raw_receipts_included",
        "fetch_transcripts_included",
    ),
    "receipt_validation": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "sorafs_validate_potr_passed",
        "schema_version_verified",
        "range_bounds_verified",
        "timestamp_ordering_verified",
        "deadline_policy_verified",
        "gateway_signature_verified",
        "provider_signature_policy_enforced",
        "provider_pq_keys_governed",
        "ml_dsa_provider_signature_verified",
        "receipts_validated",
        "receipt_summary_digest_hex",
        "validation_bundle_digest_hex",
        "raw_receipt_bytes_included",
    ),
    "proof_stream": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "route_count",
        "passed_route_count",
        "routes",
        "manifest_filter_verified",
        "provider_filter_verified",
        "tier_filter_verified",
        "replay_window_bounded",
        "invalid_receipts_suppressed",
        "receipt_summary_digest_hex",
        "response_bodies_included",
    ),
    "reputation_integration": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "reputation_pipeline_consumed_receipts",
        "success_ratio_updated",
        "latency_percentiles_updated",
        "degradation_alert_linked",
        "reputation_weight_governed",
        "missed_deadline_penalty_bound",
        "receipt_summary_digest_hex",
        "stats_digest_hex",
        "raw_reputation_inputs_included",
    ),
    "observability": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "deadline_breach_alert_tested",
        "critical_alerts_firing",
        "metrics",
        "receipt_summary_digest_hex",
        "response_bodies_included",
    ),
    "governance_approval": COMMON_EVIDENCE_REQUIRED_FIELDS
    + (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "potr_policy_bound",
        "pq_key_roster_bound",
        "reputation_weight_bound",
        "governance_dag_bound",
        "receipt_summary_digest_hex",
        "config_source",
        "policy_digest_hex",
    ),
}


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SF-14 PoTR rollout gate."""

    now_unix: int
    max_evidence_age_secs: int
    max_route_latency_ms: int
    max_hot_latency_ms: int
    max_warm_latency_ms: int
    min_providers: int
    min_receipts: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "receipt_summary_digest_hex",
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
        require_bool_true(
            record,
            "norito_verified",
            errors,
            path=f"routes[{index}].norito_verified",
        )


def validate_multi_provider_probe(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_string_coverage(payload, "tiers_observed", "", REQUIRED_TIERS, errors)
    require_bool_true(payload, "gateway_receipts_captured", errors)
    require_bool_true(payload, "range_fetch_verified", errors)
    require_bool_true(payload, "deadline_headers_verified", errors)
    require_bool_true(payload, "proof_stream_replay_verified", errors)
    require_bool_true(payload, "trace_correlation_verified", errors)
    require_minimum_int(payload, "provider_count", options.min_providers, errors)
    require_minimum_int(payload, "receipt_count", options.min_receipts, errors)
    require_maximum_number(payload, "max_hot_latency_ms", options.max_hot_latency_ms, errors)
    require_maximum_number(
        payload,
        "max_warm_latency_ms",
        options.max_warm_latency_ms,
        errors,
    )
    require_hex(payload, "receipt_summary_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_receipts_included", errors)
    require_false(payload, "fetch_transcripts_included", errors)


def validate_receipt_validation(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "sorafs_validate_potr_passed", errors)
    require_bool_true(payload, "schema_version_verified", errors)
    require_bool_true(payload, "range_bounds_verified", errors)
    require_bool_true(payload, "timestamp_ordering_verified", errors)
    require_bool_true(payload, "deadline_policy_verified", errors)
    require_bool_true(payload, "gateway_signature_verified", errors)
    require_bool_true(payload, "provider_signature_policy_enforced", errors)
    require_bool_true(payload, "provider_pq_keys_governed", errors)
    require_bool_true(payload, "ml_dsa_provider_signature_verified", errors)
    require_positive_int(payload, "receipts_validated", errors)
    require_hex(payload, "receipt_summary_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "validation_bundle_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_receipt_bytes_included", errors)


def validate_proof_stream(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_ROUTES, errors)
    require_bool_true(payload, "manifest_filter_verified", errors)
    require_bool_true(payload, "provider_filter_verified", errors)
    require_bool_true(payload, "tier_filter_verified", errors)
    require_bool_true(payload, "replay_window_bounded", errors)
    require_bool_true(payload, "invalid_receipts_suppressed", errors)
    require_hex(payload, "receipt_summary_digest_hex", HEX64_LEN, errors)
    require_false(payload, "response_bodies_included", errors)
    validate_routes(payload, errors, options)


def validate_reputation_integration(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "reputation_pipeline_consumed_receipts", errors)
    require_bool_true(payload, "success_ratio_updated", errors)
    require_bool_true(payload, "latency_percentiles_updated", errors)
    require_bool_true(payload, "degradation_alert_linked", errors)
    require_bool_true(payload, "reputation_weight_governed", errors)
    require_bool_true(payload, "missed_deadline_penalty_bound", errors)
    require_hex(payload, "receipt_summary_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "stats_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_reputation_inputs_included", errors)


def validate_observability(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_bool_true(payload, "deadline_breach_alert_tested", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_hex(payload, "receipt_summary_digest_hex", HEX64_LEN, errors)
    require_false(payload, "response_bodies_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "potr_policy_bound", errors)
    require_bool_true(payload, "pq_key_roster_bound", errors)
    require_bool_true(payload, "reputation_weight_bound", errors)
    require_bool_true(payload, "governance_dag_bound", errors)
    require_hex(payload, "receipt_summary_digest_hex", HEX64_LEN, errors)
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

    if kind.name == "multi_provider_probe":
        validate_multi_provider_probe(payload, errors, options)
    elif kind.name == "receipt_validation":
        validate_receipt_validation(payload, errors)
    elif kind.name == "proof_stream":
        validate_proof_stream(payload, errors, options)
    elif kind.name == "reputation_integration":
        validate_reputation_integration(payload, errors)
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
        "SoraFS SF-14 rollout artifact",
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
    valid_receipt_summary_digests: set[str] = set()
    valid_receipt_summary_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
            digest = evidence_artifact_fingerprint(artifact).get("receipt_summary_digest_hex")
            if kind_name == "multi_provider_probe" and isinstance(digest, str):
                valid_receipt_summary_digests.add(digest.lower())
            elif kind_name in RECEIPT_SUMMARY_BOUND_KINDS:
                valid_receipt_summary_bound_artifacts.append((kind_name, artifact))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        record_evidence_validation_errors(path, validation_errors, errors)

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("multi_provider_probe",)
        + RECEIPT_SUMMARY_BOUND_KINDS,
        bound_artifacts=valid_receipt_summary_bound_artifacts,
        valid_anchor_digests=valid_receipt_summary_digests,
        digest_field="receipt_summary_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} receipt_summary_digest_hex must reference a valid "
            "multi_provider_probe receipt_summary_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} receipt_summary_digest_hex requires a valid "
            "multi_provider_probe receipt_summary_digest_hex"
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
            "max_hot_latency_ms": options.max_hot_latency_ms,
            "max_warm_latency_ms": options.max_warm_latency_ms,
            "min_providers": options.min_providers,
            "min_receipts": options.min_receipts,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_receipt_summary_digests": sorted(valid_receipt_summary_digests),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SF-14 PoTR rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SF-14 kinds.",
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
        "--max-hot-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_HOT_LATENCY_MS,
    )
    parser.add_argument(
        "--max-warm-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_WARM_LATENCY_MS,
    )
    parser.add_argument("--min-providers", type=positive_int_arg, default=DEFAULT_MIN_PROVIDERS)
    parser.add_argument("--min-receipts", type=positive_int_arg, default=DEFAULT_MIN_RECEIPTS)
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
        max_evidence_age_secs=args.max_evidence_age_secs,
        max_route_latency_ms=args.max_route_latency_ms,
        max_hot_latency_ms=args.max_hot_latency_ms,
        max_warm_latency_ms=args.max_warm_latency_ms,
        min_providers=args.min_providers,
        min_receipts=args.min_receipts,
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
        emit_checker_error_block("ERROR: SoraFS PoTR rollout evidence is incomplete:", errors)
        return 1

    emit_checker_notice(
        "SoraFS PoTR rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
