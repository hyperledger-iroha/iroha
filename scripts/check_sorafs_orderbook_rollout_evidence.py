#!/usr/bin/env python3
"""Validate SoraFS orderbook and streaming-settlement rollout evidence artifacts."""

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
    require_count_length_match,
    require_false,
    require_false_or_absent,
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
    require_zero_count,
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


SUMMARY_SCHEMA = "sorafs.orderbook.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_STREAM_LAG_MS = 2_000
DEFAULT_MAX_MATCHER_LAG_MS = 1_000
DEFAULT_MIN_RECONCILIATION_PEERS = 4
HEX64_LEN = 64

REQUIRED_API_ROUTES = (
    "orders_post",
    "cancel_post",
    "receipts_post",
    "book_get",
    "trades_get",
    "channels_get",
    "receipts_get",
    "events_get",
)
REQUIRED_STREAMS = ("sse_orderbook_events", "websocket_orderbook_events")
REQUIRED_METRICS = (
    "torii_sorafs_orderbook_order_flow_total",
    "torii_sorafs_orderbook_open_depth",
    "torii_sorafs_orderbook_matcher_lag_ms",
    "torii_sorafs_orderbook_settlement_backlog",
    "torii_sorafs_orderbook_api_error_ratio",
    "torii_sorafs_orderbook_escrow_runway_seconds",
    "torii_sorafs_orderbook_contract_mirror_divergence",
)
REQUIRED_RECONCILIATION_SOURCES = (
    "contract",
    "matcher",
    "torii-mirror",
    "settlement-service",
    "governance-dag",
)
REQUIRED_SDK_LANGUAGES = (
    "rust",
    "javascript",
    "python",
    "kotlin-jvm",
    "java-android",
    "swift",
)
CONTRACT_BOUND_KINDS = (
    "matcher_service",
    "settlement_service",
    "api_gateway",
    "event_streams",
    "sdk_release",
    "observability",
    "reconciliation",
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "ledger",
    "mnemonic",
    "norito_bytes",
    "order_payload",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_channels",
    "raw_contract_state",
    "raw_ledger",
    "raw_order",
    "raw_orderbook",
    "raw_receipt",
    "raw_receipts",
    "raw_request",
    "raw_response",
    "raw_snapshot",
    "raw_trades",
    "receipt_payload",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signed_transaction",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SFM-2 rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("contract_surface", "sorafs.orderbook.contract_surface_canary.v1"),
    EvidenceKind("matcher_service", "sorafs.orderbook.matcher_service_canary.v1"),
    EvidenceKind("settlement_service", "sorafs.orderbook.settlement_service_canary.v1"),
    EvidenceKind("api_gateway", "sorafs.orderbook.api_gateway_canary.v1"),
    EvidenceKind("event_streams", "sorafs.orderbook.event_streams_canary.v1"),
    EvidenceKind("sdk_release", "sorafs.orderbook.sdk_release_canary.v1"),
    EvidenceKind("observability", "sorafs.orderbook.observability_canary.v1"),
    EvidenceKind("reconciliation", "sorafs.orderbook.reconciliation_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.orderbook.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SFM-2 orderbook rollout gate."""

    now_unix: int
    max_evidence_age_secs: int
    max_route_latency_ms: int
    max_stream_lag_ms: int
    max_matcher_lag_ms: int
    min_reconciliation_peers: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "contract_digest_hex",
)


def validate_route_records(
    payload: dict[str, Any], errors: list[str], options: ValidationOptions
) -> None:
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


def validate_contract_surface(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "contract_deployed", errors)
    require_bool_true(payload, "deterministic_matching_verified", errors)
    require_bool_true(payload, "escrow_enforced", errors)
    require_bool_true(payload, "pause_control_configured", errors)
    require_bool_true(payload, "fee_policy_config_bound", errors)
    require_bool_true(payload, "capability_policy_configured", errors)
    require_string_equal(payload, "contract_state_source", "on-chain", errors)
    require_hex(payload, "contract_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_contract_state_included", errors)


def validate_matcher_service(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_bool_true(payload, "daemonized", errors)
    require_bool_true(payload, "contract_forwarding_enabled", errors)
    require_bool_true(payload, "price_time_priority_verified", errors)
    require_bool_true(payload, "replay_snapshot_verified", errors)
    require_bool_true(payload, "durable_checkpoint_verified", errors)
    require_hex(payload, "contract_digest_hex", HEX64_LEN, errors)
    require_false(payload, "divergence_detected", errors)
    require_maximum_number(payload, "matcher_lag_ms", options.max_matcher_lag_ms, errors)
    require_positive_int(payload, "accepted_order_count", errors)
    require_positive_int(payload, "matched_order_count", errors)
    require_non_negative_int(payload, "rejected_invalid_order_count", errors)
    require_false(payload, "raw_snapshot_included", errors)


def validate_settlement_service(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "daemonized", errors)
    require_hex(payload, "contract_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "escrow_custody_mutation_verified", errors)
    require_bool_true(payload, "receipt_authorization_verified", errors)
    require_bool_true(payload, "non_overlapping_ranges_enforced", errors)
    require_bool_true(payload, "governance_receipts_published", errors)
    require_positive_int(payload, "open_channel_count", errors)
    require_positive_int(payload, "settled_receipt_count", errors)
    require_non_negative_int(payload, "settlement_backlog_count", errors)
    require_false(payload, "raw_receipts_included", errors)


def validate_api_gateway(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_hex(payload, "contract_digest_hex", HEX64_LEN, errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_API_ROUTES, errors)
    require_bool_true(payload, "canonical_request_auth_enforced", errors)
    require_bool_true(payload, "owner_account_binding_verified", errors)
    require_bool_true(payload, "provider_role_binding_verified", errors)
    require_bool_true(payload, "capability_policy_enforced", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_route_records(payload, errors, options)


def validate_event_streams(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_string_coverage(payload, "streams", "name", REQUIRED_STREAMS, errors)
    require_hex(payload, "contract_digest_hex", HEX64_LEN, errors)
    stream_records = require_object_array(payload, "streams", errors)
    if not stream_records:
        return
    for index, record in stream_records:
        require_bool_true(record, "passed", errors, path=f"streams[{index}].passed")
        for field in ("backlog_replay_verified", "live_delivery_verified", "contract_backed"):
            require_bool_true(record, field, errors, path=f"streams[{index}].{field}")
        require_maximum_number(
            record,
            "lag_ms",
            options.max_stream_lag_ms,
            errors,
            path=f"streams[{index}].lag_ms",
        )
    require_false(payload, "response_bodies_included", errors)


def validate_sdk_release(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "artifact_hashes_verified", errors)
    require_hex(payload, "contract_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "live_smoke_passed", errors)
    require_bool_true(payload, "submitter_helpers_verified", errors)
    require_false_or_absent(payload, "debug_artifacts", errors)
    require_string_coverage(payload, "languages", "name", REQUIRED_SDK_LANGUAGES, errors)
    artifact_count = require_positive_int(payload, "artifact_count", errors)
    artifact_records = require_object_array(payload, "artifacts", errors)
    if not artifact_records:
        return
    require_count_length_match(
        artifact_count,
        artifact_records,
        "artifact_count",
        "artifacts",
        errors,
    )
    for _index, record in artifact_records:
        require_string(record, "id", errors)
        require_hex(record, "sha256", HEX64_LEN, errors)


def validate_observability(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_hex(payload, "contract_digest_hex", HEX64_LEN, errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_bool_true(payload, "live_dashboard_wired", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_false(payload, "response_bodies_included", errors)


def validate_reconciliation(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_minimum_int(payload, "peer_count", options.min_reconciliation_peers, errors)
    require_hex(payload, "contract_digest_hex", HEX64_LEN, errors)
    require_positive_int(payload, "source_count", errors)
    require_string_coverage(payload, "sources", "name", REQUIRED_RECONCILIATION_SOURCES, errors)
    require_bool_true(payload, "contract_mirror_reconciliation_passed", errors)
    require_bool_true(payload, "evidence_dag_published", errors)
    require_false(payload, "contract_mirror_divergence", errors)
    require_zero_count(payload, "mismatch_count", errors)
    require_zero_count(payload, "unreconciled_event_count", errors)
    require_false(payload, "raw_ledger_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "orderbook_activation_governed", errors)
    require_bool_true(payload, "emergency_pause_tested", errors)
    require_bool_true(payload, "capability_policy_bound", errors)
    require_bool_true(payload, "treasury_policy_bound", errors)
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

    if kind.name == "contract_surface":
        validate_contract_surface(payload, errors)
    elif kind.name == "matcher_service":
        validate_matcher_service(payload, errors, options)
    elif kind.name == "settlement_service":
        validate_settlement_service(payload, errors)
    elif kind.name == "api_gateway":
        validate_api_gateway(payload, errors, options)
    elif kind.name == "event_streams":
        validate_event_streams(payload, errors, options)
    elif kind.name == "sdk_release":
        validate_sdk_release(payload, errors)
    elif kind.name == "observability":
        validate_observability(payload, errors)
    elif kind.name == "reconciliation":
        validate_reconciliation(payload, errors, options)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS SFM-2 rollout artifact",
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
    valid_contract_digests: set[str] = set()
    valid_contract_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
            digest = fingerprint.get("contract_digest_hex")
            if kind_name == "contract_surface" and isinstance(digest, str):
                valid_contract_digests.add(digest.lower())
            elif kind_name in CONTRACT_BOUND_KINDS:
                valid_contract_bound_artifacts.append((kind_name, artifact))
        record_evidence_validation_errors(path, validation_errors, errors)

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=("contract_surface",) + CONTRACT_BOUND_KINDS,
        bound_artifacts=valid_contract_bound_artifacts,
        valid_anchor_digests=valid_contract_digests,
        digest_field="contract_digest_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} contract_digest_hex must reference a valid "
            "contract_surface contract_digest_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} contract_digest_hex requires a valid contract_surface "
            "contract_digest_hex"
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
            "max_stream_lag_ms": options.max_stream_lag_ms,
            "max_matcher_lag_ms": options.max_matcher_lag_ms,
            "min_reconciliation_peers": options.min_reconciliation_peers,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_contract_digests": sorted(valid_contract_digests),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SFM-2 orderbook and settlement rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SFM-2 kinds.",
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
        "--max-stream-lag-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_STREAM_LAG_MS,
    )
    parser.add_argument(
        "--max-matcher-lag-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_MATCHER_LAG_MS,
    )
    parser.add_argument(
        "--min-reconciliation-peers",
        type=positive_int_arg,
        default=DEFAULT_MIN_RECONCILIATION_PEERS,
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

    options = ValidationOptions(
        now_unix=args.now_unix,
        max_evidence_age_secs=args.max_evidence_age_secs,
        max_route_latency_ms=args.max_route_latency_ms,
        max_stream_lag_ms=args.max_stream_lag_ms,
        max_matcher_lag_ms=args.max_matcher_lag_ms,
        min_reconciliation_peers=args.min_reconciliation_peers,
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
        emit_checker_error_block("ERROR: SoraFS orderbook rollout evidence is incomplete:", errors)
        return 1

    emit_checker_notice(
        "SoraFS orderbook rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
