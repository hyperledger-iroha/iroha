#!/usr/bin/env python3
"""Validate SoraFS PoR rollout evidence artifacts."""

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
    require_string_in,
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


SUMMARY_SCHEMA = "sorafs.por.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_SCHEDULER_LAG_SECS = 15 * 60
DEFAULT_MAX_REPORT_LATENCY_MS = 3_000
DEFAULT_MIN_PROVIDERS = 3
DEFAULT_MIN_CHALLENGES = 3
HEX64_LEN = 64

REQUIRED_RUNTIME_ROUTES = (
    "por_status",
    "por_export",
    "por_report",
    "por_ingestion",
    "capacity_por_challenge",
    "capacity_por_proof",
    "capacity_por_verdict",
)
REQUIRED_REPORTING_ROUTES = ("por_status", "por_export", "por_report")
REQUIRED_METRICS = (
    "torii_sorafs_por_challenges_total",
    "torii_sorafs_por_forced_challenges_total",
    "torii_sorafs_por_sampling_duplicates_total",
    "torii_sorafs_por_ingest_backlog",
    "torii_sorafs_por_ingest_failures_total",
    "sorafs_por_response_latency_seconds_bucket",
    "sorafs_vrf_missing_total",
    "sorafs_por_seed_verification_failures_total",
)
ALLOWED_MANUAL_TRIGGER_STATES = ("wired", "retired")
SEED_REPLAY_BOUND_KINDS = (
    "scheduler_runtime",
    "validator_replay",
    "reporting_archive",
    "observability",
    "governance_approval",
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "challenge",
    "challenge_bytes",
    "drand_randomness",
    "drand_signature",
    "evidence_json",
    "mnemonic",
    "norito_bytes",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "proof",
    "proof_bytes",
    "raw_archive",
    "raw_challenge",
    "raw_drand",
    "raw_evidence",
    "raw_export",
    "raw_proof",
    "raw_randomness",
    "raw_report",
    "raw_request",
    "raw_response",
    "raw_vrf",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signed_transaction",
    "token",
    "vrf_output",
    "vrf_proof",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SF-9 rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("randomness", "sorafs.por.randomness_canary.v1"),
    EvidenceKind("scheduler_runtime", "sorafs.por.scheduler_runtime_canary.v1"),
    EvidenceKind("validator_replay", "sorafs.por.validator_replay_canary.v1"),
    EvidenceKind("reporting_archive", "sorafs.por.reporting_archive_canary.v1"),
    EvidenceKind("observability", "sorafs.por.observability_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.por.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SF-9 PoR rollout gate."""

    now_unix: int
    max_evidence_age_secs: int
    max_route_latency_ms: int
    max_scheduler_lag_secs: int
    max_report_latency_ms: int
    min_providers: int
    min_challenges: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "seed_replay_digest_hex",
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
            "authz_enforced",
            errors,
            path=f"routes[{index}].authz_enforced",
        )
        require_bool_true(
            record,
            "norito_verified",
            errors,
            path=f"routes[{index}].norito_verified",
        )


def validate_randomness(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_bool_true(payload, "drand_round_verified", errors)
    require_bool_true(payload, "drand_signature_verified", errors)
    require_bool_true(payload, "drand_round_fresh", errors)
    require_bool_true(payload, "vrf_proofs_verified", errors)
    require_bool_true(payload, "provider_manifest_binding_verified", errors)
    require_bool_true(payload, "deterministic_seed_replay_verified", errors)
    require_bool_true(payload, "forced_challenge_policy_verified", errors)
    require_minimum_int(payload, "provider_count", options.min_providers, errors)
    require_minimum_int(payload, "challenge_count", options.min_challenges, errors)
    require_hex(payload, "seed_replay_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_randomness_included", errors)
    require_false(payload, "raw_vrf_included", errors)


def validate_scheduler_runtime(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_RUNTIME_ROUTES, errors)
    require_bool_true(payload, "scheduler_runtime_enabled", errors)
    require_bool_true(payload, "norito_snapshot_persisted", errors)
    require_bool_true(payload, "governance_dag_challenge_published", errors)
    require_bool_true(payload, "repair_handoff_verified", errors)
    require_bool_true(payload, "ingestion_backlog_bounded", errors)
    require_bool_true(payload, "duplicate_samples_within_budget", errors)
    require_hex(payload, "seed_replay_digest_hex", HEX64_LEN, errors)
    require_maximum_number(
        payload,
        "max_scheduler_lag_seconds",
        options.max_scheduler_lag_secs,
        errors,
    )
    require_false(payload, "response_bodies_included", errors)
    validate_routes(payload, errors, options)


def validate_validator_replay(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "sorafs_validate_por_passed", errors)
    require_bool_true(payload, "challenge_proof_binding_verified", errors)
    require_bool_true(payload, "sample_coverage_verified", errors)
    require_bool_true(payload, "deadline_policy_verified", errors)
    require_bool_true(payload, "merkle_replay_verified", errors)
    require_bool_true(payload, "validation_outcome_schema_verified", errors)
    require_positive_int(payload, "pairs_replayed", errors)
    require_hex(payload, "seed_replay_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "validation_bundle_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_challenge_bytes_included", errors)
    require_false(payload, "raw_proof_bytes_included", errors)


def validate_reporting_archive(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_REPORTING_ROUTES, errors)
    require_bool_true(payload, "weekly_report_generated", errors)
    require_bool_true(payload, "status_export_verified", errors)
    require_bool_true(payload, "governance_archive_handoff_verified", errors)
    require_bool_true(payload, "archive_retention_bound", errors)
    require_bool_true(payload, "operator_archive_decision_recorded", errors)
    require_bool_true(payload, "manual_trigger_route_decided", errors)
    require_string_in(
        payload,
        "manual_trigger_route_state",
        ALLOWED_MANUAL_TRIGGER_STATES,
        errors,
    )
    require_maximum_number(
        payload,
        "report_latency_ms",
        options.max_report_latency_ms,
        errors,
    )
    require_hex(payload, "seed_replay_digest_hex", HEX64_LEN, errors)
    require_hex(payload, "report_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_report_included", errors)
    require_false(payload, "raw_export_included", errors)
    validate_routes(payload, errors, options)


def validate_observability(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_bool_true(payload, "forced_challenge_alert_tested", errors)
    require_bool_true(payload, "ingest_backlog_alert_tested", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_hex(payload, "seed_replay_digest_hex", HEX64_LEN, errors)
    require_false(payload, "response_bodies_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "por_policy_bound", errors)
    require_bool_true(payload, "auditor_roster_bound", errors)
    require_bool_true(payload, "archive_policy_bound", errors)
    require_bool_true(payload, "governance_dag_bound", errors)
    require_hex(payload, "seed_replay_digest_hex", HEX64_LEN, errors)
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

    if kind.name == "randomness":
        validate_randomness(payload, errors, options)
    elif kind.name == "scheduler_runtime":
        validate_scheduler_runtime(payload, errors, options)
    elif kind.name == "validator_replay":
        validate_validator_replay(payload, errors)
    elif kind.name == "reporting_archive":
        validate_reporting_archive(payload, errors, options)
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
        "SoraFS SF-9 rollout artifact",
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
    valid_seed_replay_digests: set[str] = set()
    valid_seed_replay_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
            digest = evidence_artifact_fingerprint(artifact).get("seed_replay_digest_hex")
            if kind_name == "randomness" and isinstance(digest, str):
                valid_seed_replay_digests.add(digest.lower())
            elif kind_name in SEED_REPLAY_BOUND_KINDS:
                valid_seed_replay_bound_artifacts.append((kind_name, artifact))
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)
        record_evidence_validation_errors(path, validation_errors, errors)

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("randomness",) + SEED_REPLAY_BOUND_KINDS,
        bound_artifacts=valid_seed_replay_bound_artifacts,
        valid_anchor_digests=valid_seed_replay_digests,
        digest_field="seed_replay_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} seed_replay_digest_hex must reference a valid "
            "randomness seed_replay_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} seed_replay_digest_hex requires a valid randomness "
            "seed_replay_digest_hex"
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
            "max_scheduler_lag_secs": options.max_scheduler_lag_secs,
            "max_report_latency_ms": options.max_report_latency_ms,
            "min_providers": options.min_providers,
            "min_challenges": options.min_challenges,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_seed_replay_digests": sorted(valid_seed_replay_digests),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SF-9 PoR rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SF-9 kinds.",
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
        "--max-scheduler-lag-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_SCHEDULER_LAG_SECS,
    )
    parser.add_argument(
        "--max-report-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_REPORT_LATENCY_MS,
    )
    parser.add_argument("--min-providers", type=positive_int_arg, default=DEFAULT_MIN_PROVIDERS)
    parser.add_argument("--min-challenges", type=positive_int_arg, default=DEFAULT_MIN_CHALLENGES)
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
        max_scheduler_lag_secs=args.max_scheduler_lag_secs,
        max_report_latency_ms=args.max_report_latency_ms,
        min_providers=args.min_providers,
        min_challenges=args.min_challenges,
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
        emit_checker_error_block("ERROR: SoraFS PoR rollout evidence is incomplete:", errors)
        return 1

    emit_checker_notice(
        "SoraFS PoR rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
