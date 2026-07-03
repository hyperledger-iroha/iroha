#!/usr/bin/env python3
"""Build payload-free SoraFS Governance DAG rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import sys
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_governance_dag_rollout_evidence import (  # noqa: E402
    BLOCK_REF_LABEL_ERROR,
    BLOCK_REF_LABEL_PATTERN,
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_HEAD_AGE_SECS,
    DEFAULT_MAX_PIN_LAG_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_BLOCKS,
    DEFAULT_MIN_PAYLOAD_KINDS,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    KIND_BY_NAME,
    REQUIRED_DASHBOARD_ROUTES,
    REQUIRED_METRICS,
    REQUIRED_PAYLOAD_KINDS,
    ValidationOptions,
    validate_evidence_payload,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
    fsync_checker_output_parent,
    write_all_checker_summary_bytes,
    validate_checker_output_parent,
)
from sorafs_path_identity import path_diagnostic_label  # noqa: E402
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)


CANARY_KINDS = tuple(KIND_BY_NAME)
HEX64_LEN = 64
PUBLIC_HEAD_KINDS = tuple(kind for kind in CANARY_KINDS if kind != "ingest_service")
POLICY_DIGEST_KINDS = ("publisher_service", "governance_approval")
TRUE_CLAIMS: dict[str, tuple[str, ...]] = {
    "ingest_service": (
        "daemonized",
        "payload_validation_enabled",
        "publisher_signature_verified",
        "dedupe_by_digest_enabled",
        "quarantine_invalid_blocks",
    ),
    "publisher_service": (
        "dag_builder_daemonized",
        "ipfs_cluster_pinning_enabled",
        "ipns_head_publication_enabled",
        "signed_head_verified",
        "parent_chain_verified",
        "car_segments_pinned",
    ),
    "mirror_datastore": (
        "rocksdb_ipld_enabled",
        "query_service_enabled",
        "mirror_index_verified",
        "head_lookup_verified",
        "block_lookup_verified",
        "node_lookup_verified",
        "digest_lookup_verified",
    ),
    "operator_recovery": (
        "live_head_fetch_verified",
        "public_checkpoint_published",
        "checkpoint_recovery_verified",
        "public_recovery_cli_verified",
        "recovered_head_matches_public_head",
    ),
    "dashboard_api": ("runtime_ipfs_backed",),
    "observability": (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "ipfs_ipns_metrics_present",
    ),
    "ipfs_ipns_e2e": (
        "local_ipfs_backed_tests_passed",
        "public_head_resolved",
        "block_replay_verified",
        "duplicate_payload_rejected",
        "invalid_parent_quarantined",
        "pinning_outage_tested",
        "publisher_key_failure_tested",
    ),
    "governance_approval": (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "publisher_keys_governed",
        "ipns_name_governed",
        "mirror_retention_policy_bound",
        "emergency_pause_tested",
    ),
}
FORCED_FALSE_FIELDS: dict[str, tuple[str, ...]] = {
    "ingest_service": ("payload_bytes_included",),
    "publisher_service": ("raw_head_included", "raw_car_included"),
    "mirror_datastore": ("mirror_drift_detected", "raw_blocks_included"),
    "operator_recovery": ("raw_checkpoint_included",),
    "dashboard_api": ("response_bodies_included",),
    "observability": ("critical_alerts_firing", "response_bodies_included"),
    "ipfs_ipns_e2e": ("raw_blocks_included",),
    "governance_approval": (),
}


def split_csv_values(values: Sequence[str]) -> list[str]:
    """Split repeated comma-separated CLI values into canonical strings."""

    items: list[str] = []
    for value in values:
        for item in value.split(","):
            stripped = item.strip()
            if stripped:
                items.append(stripped)
    return items


def validate_name_set(
    values: Iterable[str],
    *,
    allowed: Sequence[str],
    option: str,
    errors: list[str],
) -> list[str]:
    """Return allowed-order values, requiring complete known non-duplicate coverage."""

    values = tuple(values)
    allowed_set = frozenset(allowed)
    value_set = frozenset(values)
    if len(value_set) != len(values):
        errors.append(f"{option} must not contain duplicates")
    if any(name not in allowed_set for name in value_set):
        errors.append(f"{option} contains an unknown value")
    missing = [name for name in allowed if name not in value_set]
    if missing:
        errors.append(f"{option} must include every required value")
    return [name for name in allowed if name in value_set]


def validate_reviewed_inventory(
    values: Iterable[str],
    *,
    expected_count: int,
    option: str,
    kind: str,
    count_option: str,
    errors: list[str],
    pattern: re.Pattern[str] | None = None,
    label_error: str | None = None,
) -> list[str]:
    """Return reviewed unique inventory labels whose count matches a CLI count."""

    items = list(values)
    if not items:
        errors.append(f"{option} is required for {kind}")
    for index, item in enumerate(items):
        validate_canonical_string(item, option=f"{option}[{index}]", errors=errors)
        if pattern is None:
            continue
        if pattern.fullmatch(item) is None:
            if label_error is None:
                errors.append(f"{option} has malformed inventory label")
            else:
                errors.append(render_inventory_label_error(label_error, option))
            continue
        forbidden = sorted(
            marker
            for marker in FORBIDDEN_INVENTORY_LABEL_MARKERS
            if marker in item.split("-")
        )
        if forbidden:
            errors.append(f"{option} must not contain non-production markers {forbidden}")
    unique_items = set(items)
    if len(unique_items) != len(items):
        errors.append(f"{option} must not contain duplicates")
    if len(unique_items) != expected_count:
        errors.append(f"{option} unique values must match {count_option}")
    return items


def render_inventory_label_error(label_error: str, option: str) -> str:
    """Render checker inventory-label diagnostics against a CLI option."""

    return label_error.replace("block_refs entries", option)


def validate_output_path(path: Path, errors: list[str]) -> None:
    """Reject unsafe output targets before writing a canary artifact."""

    if not isinstance(path, Path):
        errors.append(f"--out `{path_diagnostic_label(path)}` must be a path")
        return
    try:
        if path.is_symlink():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a symlink")
            return
        if path.exists() and path.is_dir():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a directory")
            return
    except (OSError, RuntimeError) as error:
        del error
        errors.append(f"--out `{path_diagnostic_label(path)}` cannot be inspected")
        return
    validate_checker_output_parent(path, errors, label="--out")


def validate_hex64(value: str | None, *, option: str, errors: list[str]) -> None:
    """Validate an exact lowercase 32-byte digest hex string."""

    if (
        not isinstance(value, str)
        or len(value) != HEX64_LEN
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{option} must be exact lowercase 32-byte hex")


def validate_canonical_string(value: str | None, *, option: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control characters."""

    if (
        not isinstance(value, str)
        or not value.strip()
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        errors.append(f"{option} must be a non-empty canonical string")


def required_positive(value: int | None, *, option: str, errors: list[str]) -> int:
    """Return a required positive integer value, recording a stable error otherwise."""

    if value is None:
        errors.append(f"{option} is required for {option_kind(errors)}")
        return 0
    return value


def option_kind(errors: list[str]) -> str:
    """Return a generic kind label for required-option diagnostics."""

    del errors
    return "this canary kind"


def build_common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by Governance DAG canary payloads."""

    payload: dict[str, Any] = {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
    }
    if args.kind in PUBLIC_HEAD_KINDS:
        payload["public_head_cid_hex"] = args.public_head_cid_hex
    return payload


def apply_verified_claims(payload: dict[str, Any], args: argparse.Namespace) -> None:
    """Populate explicitly verified true claims and forced payload-free false flags."""

    for claim in TRUE_CLAIMS[args.kind]:
        payload[claim] = claim in args.verified_claims
    for field in FORCED_FALSE_FIELDS[args.kind]:
        payload[field] = False


def build_routes(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free dashboard route probe records."""

    return [
        {
            "name": route,
            "passed": True,
            "status_code": args.route_status_code,
            "latency_ms": args.route_latency_ms,
            "publisher_identity_present": True,
            "verification_valid": True,
        }
        for route in args.routes
    ]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free Governance DAG canary payload."""

    payload = build_common_payload(args)
    apply_verified_claims(payload, args)
    if args.kind == "ingest_service":
        payload.update(
            {
                "source_count": args.source_count,
                "payload_kinds": args.payload_kinds,
            }
        )
    elif args.kind == "publisher_service":
        payload.update(
            {
                "policy_digest_hex": args.policy_digest_hex,
                "pin_lag_seconds": args.pin_lag_seconds,
                "head_age_seconds": args.head_age_seconds,
                "block_count": args.block_count,
                "block_refs": args.block_refs,
                "payload_kind_count": len(args.payload_kinds),
                "payload_kinds": args.payload_kinds,
            }
        )
    elif args.kind == "mirror_datastore":
        payload["missing_block_count"] = 0
    elif args.kind == "operator_recovery":
        payload["checkpoint_digest_hex"] = args.checkpoint_digest_hex
    elif args.kind == "dashboard_api":
        routes = build_routes(args)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
            }
        )
    elif args.kind == "observability":
        payload.update(
            {
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
            }
        )
    elif args.kind == "ipfs_ipns_e2e":
        payload.update(
            {
                "block_count": args.block_count,
                "block_refs": args.block_refs,
                "payload_kind_count": len(args.payload_kinds),
                "payload_kinds": args.payload_kinds,
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "config_source": "iroha_config",
                "policy_digest_hex": args.policy_digest_hex,
            }
        )
    return payload


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    if args.kind in PUBLIC_HEAD_KINDS:
        validate_hex64(
            args.public_head_cid_hex,
            option="--public-head-cid-hex",
            errors=errors,
        )
    if args.kind in POLICY_DIGEST_KINDS:
        validate_hex64(
            args.policy_digest_hex,
            option="--policy-digest-hex",
            errors=errors,
        )
    args.verified_claims = validate_name_set(
        split_csv_values(args.verified_claim),
        allowed=TRUE_CLAIMS[args.kind],
        option="--verified-claim",
        errors=errors,
    )
    if args.kind == "ingest_service":
        source_count = required_positive(
            args.source_count,
            option="--source-count",
            errors=errors,
        )
        args.payload_kinds = validate_name_set(
            split_csv_values(args.payload_kind),
            allowed=REQUIRED_PAYLOAD_KINDS,
            option="--payload-kind",
            errors=errors,
        )
        if source_count and source_count != len(args.payload_kinds):
            errors.append("--source-count must match unique --payload-kind count")
    elif args.kind == "publisher_service":
        required_positive(args.pin_lag_seconds, option="--pin-lag-seconds", errors=errors)
        required_positive(args.head_age_seconds, option="--head-age-seconds", errors=errors)
        block_count = required_positive(args.block_count, option="--block-count", errors=errors)
        args.block_refs = validate_reviewed_inventory(
            split_csv_values(args.block_ref),
            expected_count=block_count,
            option="--block-ref",
            kind="publisher_service",
            count_option="--block-count",
            pattern=BLOCK_REF_LABEL_PATTERN,
            label_error=BLOCK_REF_LABEL_ERROR,
            errors=errors,
        )
        args.payload_kinds = validate_name_set(
            split_csv_values(args.payload_kind),
            allowed=REQUIRED_PAYLOAD_KINDS,
            option="--payload-kind",
            errors=errors,
        )
    elif args.kind == "operator_recovery":
        validate_hex64(
            args.checkpoint_digest_hex,
            option="--checkpoint-digest-hex",
            errors=errors,
        )
    elif args.kind == "dashboard_api":
        args.routes = validate_name_set(
            split_csv_values(args.route),
            allowed=REQUIRED_DASHBOARD_ROUTES,
            option="--route",
            errors=errors,
        )
    elif args.kind == "observability":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    elif args.kind == "ipfs_ipns_e2e":
        block_count = required_positive(args.block_count, option="--block-count", errors=errors)
        args.block_refs = validate_reviewed_inventory(
            split_csv_values(args.block_ref),
            expected_count=block_count,
            option="--block-ref",
            kind="ipfs_ipns_e2e",
            count_option="--block-count",
            pattern=BLOCK_REF_LABEL_PATTERN,
            label_error=BLOCK_REF_LABEL_ERROR,
            errors=errors,
        )
        args.payload_kinds = validate_name_set(
            split_csv_values(args.payload_kind),
            allowed=REQUIRED_PAYLOAD_KINDS,
            option="--payload-kind",
            errors=errors,
        )


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, option="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, option="--environment", errors=errors)
    if args.route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS:
        errors.append(f"--route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if args.pin_lag_seconds is not None and args.pin_lag_seconds > DEFAULT_MAX_PIN_LAG_SECS:
        errors.append(f"--pin-lag-seconds must be <= {DEFAULT_MAX_PIN_LAG_SECS}")
    if args.head_age_seconds is not None and args.head_age_seconds > DEFAULT_MAX_HEAD_AGE_SECS:
        errors.append(f"--head-age-seconds must be <= {DEFAULT_MAX_HEAD_AGE_SECS}")
    if args.block_count is not None and args.block_count < DEFAULT_MIN_BLOCKS:
        errors.append(f"--block-count must be >= {DEFAULT_MIN_BLOCKS}")
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_pin_lag_secs=DEFAULT_MAX_PIN_LAG_SECS,
        max_head_age_secs=DEFAULT_MAX_HEAD_AGE_SECS,
        min_blocks=DEFAULT_MIN_BLOCKS,
        min_payload_kinds=DEFAULT_MIN_PAYLOAD_KINDS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the Governance DAG gate contract."""

    kind, errors = validate_evidence_payload(payload, validation_options(args))
    if kind != args.kind:
        errors.append(f"generated canary must validate as {args.kind}")
    return errors


def write_payload_atomic(path: Path, payload: dict[str, Any]) -> list[str]:
    """Write the canary JSON atomically without following output symlinks."""

    text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    parent = path.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        del error
        return [f"--out parent `{path_diagnostic_label(parent)}` cannot be created"]
    tmp_name = f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    tmp_path = parent / tmp_name
    fd = -1
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        nofollow = getattr(os, "O_NOFOLLOW", 0)
        if nofollow:
            flags |= nofollow
        fd = os.open(tmp_path, flags, 0o600)
        write_all_checker_summary_bytes(fd, text.encode("utf-8"))
        os.fsync(fd)
        os.close(fd)
        fd = -1
        os.replace(tmp_path, path)
        parent_sync_errors = fsync_checker_output_parent(path, label="--out")
        if parent_sync_errors:
            return parent_sync_errors
    except (OSError, RuntimeError) as error:
        del error
        try:
            if fd >= 0:
                os.close(fd)
        finally:
            try:
                tmp_path.unlink()
            except FileNotFoundError:
                pass
            except (OSError, RuntimeError):
                pass
        return [f"--out `{path_diagnostic_label(path)}` cannot be written"]
    return []


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Build payload-free SoraFS SF-12 Governance DAG canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--public-head-cid-hex")
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--source-count", type=positive_int_arg)
    parser.add_argument("--payload-kind", action="append", default=[])
    parser.add_argument("--pin-lag-seconds", type=non_negative_int_arg)
    parser.add_argument("--head-age-seconds", type=non_negative_int_arg)
    parser.add_argument("--block-count", type=positive_int_arg)
    parser.add_argument("--block-ref", action="append", default=[])
    parser.add_argument("--checkpoint-digest-hex")
    parser.add_argument("--route", action="append", default=[])
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=200)
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--policy-digest-hex")
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
        return parser.parse_args(expanded_args)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    errors = validate_inputs(args)
    if errors:
        emit_checker_error_block(
            "ERROR: SoraFS Governance DAG canary inputs are incomplete:",
            errors,
        )
        return 2

    payload = build_payload(args)
    payload_errors = validate_generated_payload(payload, args)
    if payload_errors:
        emit_checker_error_lines(payload_errors)
        return 2

    write_errors = write_payload_atomic(args.out, payload)
    if write_errors:
        emit_checker_error_lines(write_errors)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
