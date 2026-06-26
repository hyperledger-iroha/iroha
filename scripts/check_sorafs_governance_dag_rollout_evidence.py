#!/usr/bin/env python3
"""Validate SoraFS Governance DAG rollout evidence artifacts."""

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


SUMMARY_SCHEMA = "sorafs.governance_dag.rollout_evidence_gate.v1"
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
DEFAULT_MAX_EVIDENCE_AGE_SECS = 7 * 24 * 60 * 60
DEFAULT_MAX_ROUTE_LATENCY_MS = 1_500
DEFAULT_MAX_PIN_LAG_SECS = 30 * 60
DEFAULT_MAX_HEAD_AGE_SECS = 30 * 60
DEFAULT_MIN_BLOCKS = 4
DEFAULT_MIN_PAYLOAD_KINDS = 6
HEX64_LEN = 64

REQUIRED_PAYLOAD_KINDS = (
    "deal-settlement",
    "repair-audit",
    "reconciliation",
    "reputation-snapshot",
    "moderation-ballot-event",
    "appeal-finance-report",
    "appeal-finance-settlement-receipt",
    "orderbook-settlement-receipt",
)
REQUIRED_DASHBOARD_ROUTES = (
    "dashboard",
    "head",
    "block_lookup",
    "node_lookup",
    "digest_lookup",
    "checkpoint",
)
REQUIRED_METRICS = (
    "sorafs_governance_dag_publish_total",
    "sorafs_governance_dag_published_bytes_total",
    "sorafs_governance_dag_last_publish_timestamp_seconds",
    "sorafs_governance_dag_backlog",
    "sorafs_governance_dag_head_age_seconds",
    "sorafs_governance_dag_ipfs_pin_lag_seconds",
    "sorafs_governance_dag_ipns_update_total",
    "sorafs_governance_dag_mirror_drift",
)
PUBLIC_HEAD_BOUND_KINDS = (
    "mirror_datastore",
    "operator_recovery",
    "dashboard_api",
    "observability",
    "ipfs_ipns_e2e",
    "governance_approval",
)

SENSITIVE_KEYS = {
    "authorization",
    "bearer_token",
    "body",
    "car_payload",
    "dag_block",
    "dag_head",
    "head_bytes",
    "ledger",
    "mnemonic",
    "node_payload",
    "payload",
    "payload_b64",
    "payload_body",
    "payload_bytes",
    "private_key",
    "raw_block",
    "raw_blocks",
    "raw_car",
    "raw_checkpoint",
    "raw_head",
    "raw_ledger",
    "raw_node",
    "raw_nodes",
    "raw_payload",
    "raw_response",
    "request_body",
    "response_body",
    "secret",
    "seed",
    "signed_transaction",
    "token",
}


@dataclass(frozen=True)
class EvidenceKind:
    """One SF-12 rollout evidence class."""

    name: str
    schema: str


EVIDENCE_KINDS: tuple[EvidenceKind, ...] = (
    EvidenceKind("ingest_service", "sorafs.governance_dag.ingest_service_canary.v1"),
    EvidenceKind("publisher_service", "sorafs.governance_dag.publisher_service_canary.v1"),
    EvidenceKind("mirror_datastore", "sorafs.governance_dag.mirror_datastore_canary.v1"),
    EvidenceKind("operator_recovery", "sorafs.governance_dag.operator_recovery_canary.v1"),
    EvidenceKind("dashboard_api", "sorafs.governance_dag.dashboard_api_canary.v1"),
    EvidenceKind("observability", "sorafs.governance_dag.observability_canary.v1"),
    EvidenceKind("ipfs_ipns_e2e", "sorafs.governance_dag.ipfs_ipns_e2e_canary.v1"),
    EvidenceKind("governance_approval", "sorafs.governance_dag.governance_approval.v1"),
)

SCHEMA_TO_KIND = {kind.schema: kind for kind in EVIDENCE_KINDS}
KIND_BY_NAME = {kind.name: kind for kind in EVIDENCE_KINDS}
DEFAULT_REQUIRED_KINDS = tuple(kind.name for kind in EVIDENCE_KINDS)


@dataclass(frozen=True)
class ValidationOptions:
    """Thresholds for the SF-12 Governance DAG rollout gate."""

    now_unix: int
    max_evidence_age_secs: int
    max_route_latency_ms: int
    max_pin_lag_secs: int
    max_head_age_secs: int
    min_blocks: int
    min_payload_kinds: int



FINGERPRINT_FIELDS: tuple[str, ...] = (
    "schema",
    "generated_at_unix",
    "deployment_id",
    "environment",
    "public_head_cid_hex",
    "checkpoint_digest_hex",
    "policy_digest_hex",
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
        for field in ("publisher_identity_present", "verification_valid"):
            require_bool_true(record, field, errors, path=f"routes[{index}].{field}")


def validate_ingest_service(payload: dict[str, Any], errors: list[str]) -> None:
    require_bool_true(payload, "daemonized", errors)
    require_bool_true(payload, "payload_validation_enabled", errors)
    require_bool_true(payload, "publisher_signature_verified", errors)
    require_bool_true(payload, "dedupe_by_digest_enabled", errors)
    require_bool_true(payload, "quarantine_invalid_blocks", errors)
    require_positive_int(payload, "source_count", errors)
    require_string_coverage(payload, "payload_kinds", "", REQUIRED_PAYLOAD_KINDS, errors)
    require_false(payload, "payload_bytes_included", errors)


def validate_publisher_service(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_bool_true(payload, "dag_builder_daemonized", errors)
    require_bool_true(payload, "ipfs_cluster_pinning_enabled", errors)
    require_bool_true(payload, "ipns_head_publication_enabled", errors)
    require_bool_true(payload, "signed_head_verified", errors)
    require_bool_true(payload, "parent_chain_verified", errors)
    require_bool_true(payload, "car_segments_pinned", errors)
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_maximum_number(payload, "pin_lag_seconds", options.max_pin_lag_secs, errors)
    require_maximum_number(payload, "head_age_seconds", options.max_head_age_secs, errors)
    require_minimum_int(payload, "block_count", options.min_blocks, errors)
    require_minimum_int(
        payload,
        "payload_kind_count",
        options.min_payload_kinds,
        errors,
    )
    require_false(payload, "raw_head_included", errors)
    require_false(payload, "raw_car_included", errors)


def validate_mirror_datastore(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_bool_true(payload, "rocksdb_ipld_enabled", errors)
    require_bool_true(payload, "query_service_enabled", errors)
    require_bool_true(payload, "mirror_index_verified", errors)
    require_bool_true(payload, "head_lookup_verified", errors)
    require_bool_true(payload, "block_lookup_verified", errors)
    require_bool_true(payload, "node_lookup_verified", errors)
    require_bool_true(payload, "digest_lookup_verified", errors)
    require_false(payload, "mirror_drift_detected", errors)
    require_zero_count(payload, "missing_block_count", errors)
    require_false(payload, "raw_blocks_included", errors)


def validate_operator_recovery(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_bool_true(payload, "live_head_fetch_verified", errors)
    require_bool_true(payload, "public_checkpoint_published", errors)
    require_bool_true(payload, "checkpoint_recovery_verified", errors)
    require_bool_true(payload, "public_recovery_cli_verified", errors)
    require_bool_true(payload, "recovered_head_matches_public_head", errors)
    require_hex(payload, "checkpoint_digest_hex", HEX64_LEN, errors)
    require_false(payload, "raw_checkpoint_included", errors)


def validate_dashboard_api(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_count_equal(payload, "route_count", "passed_route_count", errors)
    require_string_coverage(payload, "routes", "name", REQUIRED_DASHBOARD_ROUTES, errors)
    require_bool_true(payload, "runtime_ipfs_backed", errors)
    require_false(payload, "response_bodies_included", errors)
    validate_routes(payload, errors, options)


def validate_observability(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_bool_true(payload, "metrics_scrape_success", errors)
    require_bool_true(payload, "dashboard_provisioned", errors)
    require_bool_true(payload, "alert_rules_installed", errors)
    require_bool_true(payload, "ipfs_ipns_metrics_present", errors)
    require_false(payload, "critical_alerts_firing", errors)
    require_string_coverage(payload, "metrics", "", REQUIRED_METRICS, errors)
    require_false(payload, "response_bodies_included", errors)


def validate_ipfs_ipns_e2e(
    payload: dict[str, Any],
    errors: list[str],
    options: ValidationOptions,
) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_bool_true(payload, "local_ipfs_backed_tests_passed", errors)
    require_bool_true(payload, "public_head_resolved", errors)
    require_bool_true(payload, "block_replay_verified", errors)
    require_bool_true(payload, "duplicate_payload_rejected", errors)
    require_bool_true(payload, "invalid_parent_quarantined", errors)
    require_bool_true(payload, "pinning_outage_tested", errors)
    require_bool_true(payload, "publisher_key_failure_tested", errors)
    require_minimum_int(payload, "block_count", options.min_blocks, errors)
    require_minimum_int(
        payload,
        "payload_kind_count",
        options.min_payload_kinds,
        errors,
    )
    require_false(payload, "raw_blocks_included", errors)


def validate_governance_approval(payload: dict[str, Any], errors: list[str]) -> None:
    require_hex(payload, "public_head_cid_hex", HEX64_LEN, errors)
    require_config_backed_governance_approval(payload, errors)
    require_bool_true(payload, "publisher_keys_governed", errors)
    require_bool_true(payload, "ipns_name_governed", errors)
    require_bool_true(payload, "mirror_retention_policy_bound", errors)
    require_bool_true(payload, "emergency_pause_tested", errors)
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

    if kind.name == "ingest_service":
        validate_ingest_service(payload, errors)
    elif kind.name == "publisher_service":
        validate_publisher_service(payload, errors, options)
    elif kind.name == "mirror_datastore":
        validate_mirror_datastore(payload, errors)
    elif kind.name == "operator_recovery":
        validate_operator_recovery(payload, errors)
    elif kind.name == "dashboard_api":
        validate_dashboard_api(payload, errors, options)
    elif kind.name == "observability":
        validate_observability(payload, errors)
    elif kind.name == "ipfs_ipns_e2e":
        validate_ipfs_ipns_e2e(payload, errors, options)
    elif kind.name == "governance_approval":
        validate_governance_approval(payload, errors)


def validate_evidence_payload(
    payload: dict[str, Any],
    options: ValidationOptions,
) -> tuple[str | None, list[str]]:
    return validate_standard_evidence_payload(
        payload,
        SCHEMA_TO_KIND,
        "SoraFS SF-12 rollout artifact",
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
    valid_public_head_cids: set[str] = set()
    public_head_bound_artifacts: list[tuple[str, dict[str, Any]]] = []
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
        record_evidence_artifact(artifacts_by_kind, kind_name, artifact)
        if evidence_artifact_is_valid(artifact):
            digest = evidence_artifact_fingerprint(artifact).get("public_head_cid_hex")
            if kind_name == "publisher_service":
                if isinstance(digest, str):
                    valid_public_head_cids.add(digest.lower())
            elif kind_name in PUBLIC_HEAD_BOUND_KINDS:
                public_head_bound_artifacts.append((kind_name, artifact))
        record_evidence_validation_errors(path, validation_errors, errors)

    validate_bound_evidence_digest_references(
        required_kinds=required_kinds,
        missing_anchor_required_kinds=PUBLIC_HEAD_BOUND_KINDS,
        bound_artifacts=public_head_bound_artifacts,
        valid_anchor_digests=valid_public_head_cids,
        digest_field="public_head_cid_hex",
        errors=errors,
        binding_error_template=(
            "{kind_name} public_head_cid_hex must match a valid "
            "publisher_service public_head_cid_hex"
        ),
        missing_anchor_error_template=(
            "{kind_name} public_head_cid_hex requires a valid "
            "publisher_service public_head_cid_hex"
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
            "max_pin_lag_secs": options.max_pin_lag_secs,
            "max_head_age_secs": options.max_head_age_secs,
            "min_blocks": options.min_blocks,
            "min_payload_kinds": options.min_payload_kinds,
        },
        "evidence_file_count": count_evidence_files(files),
        "recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind),
        "valid_public_head_cids": sorted(valid_public_head_cids),
        "required": required,
        "errors": errors,
    }
    return summary, errors


def main(argv: list[str] | None = None) -> int:
    parser = EvidenceArgumentParser(
        description="Validate SoraFS SF-12 Governance DAG rollout evidence artifacts."
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
        help="Required evidence kind, or comma-separated kinds. Defaults to all SF-12 kinds.",
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
        "--max-pin-lag-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_PIN_LAG_SECS,
    )
    parser.add_argument(
        "--max-head-age-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_HEAD_AGE_SECS,
    )
    parser.add_argument("--min-blocks", type=positive_int_arg, default=DEFAULT_MIN_BLOCKS)
    parser.add_argument(
        "--min-payload-kinds",
        type=positive_int_arg,
        default=DEFAULT_MIN_PAYLOAD_KINDS,
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
        max_pin_lag_secs=args.max_pin_lag_secs,
        max_head_age_secs=args.max_head_age_secs,
        min_blocks=args.min_blocks,
        min_payload_kinds=args.min_payload_kinds,
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
        emit_checker_error_block("ERROR: SoraFS Governance DAG rollout evidence is incomplete:", errors)
        return 1

    emit_checker_notice(
        "SoraFS Governance DAG rollout evidence is ready: "
        f"{summary['recognized_artifact_count']} recognized artifact(s) cover "
        f"{len(required_kinds)} required kind(s).",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
