#!/usr/bin/env python3
"""Build payload-free SoraFS PoTR rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import secrets
import sys
from collections.abc import Iterable, Sequence
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_potr_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_HOT_LATENCY_MS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MAX_WARM_LATENCY_MS,
    DEFAULT_MIN_PROVIDERS,
    DEFAULT_MIN_RECEIPTS,
    KIND_BY_NAME,
    RECEIPT_SUMMARY_BOUND_KINDS,
    REQUIRED_METRICS,
    REQUIRED_ROUTES,
    REQUIRED_TIERS,
    ValidationOptions,
    validate_evidence_payload,
)
from sorafs_checker_preflight import (  # noqa: E402
    emit_checker_error_block,
    emit_checker_error_lines,
    emit_checker_exception,
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
RECEIPT_SUMMARY_DIGEST_KINDS = ("multi_provider_probe",) + RECEIPT_SUMMARY_BOUND_KINDS
HEX64_LEN = 64


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


def validate_canonical_string(value: str | None, *, label: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control characters."""

    if (
        not isinstance(value, str)
        or not value.strip()
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        errors.append(f"{label} must be a non-empty canonical string")


def require_kind_options(
    args: argparse.Namespace,
    errors: list[str],
    required: Sequence[tuple[str, Any]],
) -> None:
    """Require kind-specific options by stable CLI flag."""

    for option, value in required:
        if value is None:
            errors.append(f"{option} is required for {args.kind}")


def common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by PoTR canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "generated_at_unix": args.generated_at_unix,
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
    }


def build_route_records(args: argparse.Namespace, routes: Sequence[str]) -> list[dict[str, Any]]:
    """Build payload-free PoTR route probe records."""

    return [
        {
            "name": name,
            "passed": True,
            "status_code": args.route_status_code,
            "latency_ms": args.route_latency_ms,
            "norito_verified": True,
        }
        for name in routes
    ]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free PoTR rollout canary payload."""

    payload = common_payload(args)
    if args.kind == "multi_provider_probe":
        payload.update(
            {
                "tiers_observed": args.tiers,
                "gateway_receipts_captured": True,
                "range_fetch_verified": True,
                "deadline_headers_verified": True,
                "proof_stream_replay_verified": True,
                "trace_correlation_verified": True,
                "provider_count": args.provider_count,
                "receipt_count": args.receipt_count,
                "max_hot_latency_ms": args.hot_latency_ms,
                "max_warm_latency_ms": args.warm_latency_ms,
                "receipt_summary_digest_hex": args.receipt_summary_digest_hex,
                "raw_receipts_included": False,
                "fetch_transcripts_included": False,
            }
        )
    elif args.kind == "receipt_validation":
        payload.update(
            {
                "sorafs_validate_potr_passed": True,
                "schema_version_verified": True,
                "range_bounds_verified": True,
                "timestamp_ordering_verified": True,
                "deadline_policy_verified": True,
                "gateway_signature_verified": True,
                "provider_signature_policy_enforced": True,
                "provider_pq_keys_governed": True,
                "pq_key_roster_digest_hex": args.pq_key_roster_digest_hex,
                "ml_dsa_provider_signature_verified": True,
                "receipts_validated": args.receipts_validated,
                "receipt_summary_digest_hex": args.receipt_summary_digest_hex,
                "validation_bundle_digest_hex": args.validation_bundle_digest_hex,
                "raw_receipt_bytes_included": False,
            }
        )
    elif args.kind == "proof_stream":
        routes = build_route_records(args, args.routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "manifest_filter_verified": True,
                "provider_filter_verified": True,
                "tier_filter_verified": True,
                "replay_window_bounded": True,
                "invalid_receipts_suppressed": True,
                "receipt_summary_digest_hex": args.receipt_summary_digest_hex,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "reputation_integration":
        payload.update(
            {
                "reputation_pipeline_consumed_receipts": True,
                "success_ratio_updated": True,
                "latency_percentiles_updated": True,
                "degradation_alert_linked": True,
                "reputation_weight_governed": True,
                "reputation_weight_policy_digest_hex": (
                    args.reputation_weight_policy_digest_hex
                ),
                "missed_deadline_penalty_bound": True,
                "receipt_summary_digest_hex": args.receipt_summary_digest_hex,
                "stats_digest_hex": args.stats_digest_hex,
                "raw_reputation_inputs_included": False,
            }
        )
    elif args.kind == "observability":
        payload.update(
            {
                "metrics_scrape_success": True,
                "dashboard_provisioned": True,
                "alert_rules_installed": True,
                "deadline_breach_alert_tested": True,
                "critical_alerts_firing": False,
                "metrics": args.metrics,
                "receipt_summary_digest_hex": args.receipt_summary_digest_hex,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "approved": True,
                "governance_vote_recorded": True,
                "iroha_config_bound": True,
                "potr_policy_bound": True,
                "pq_key_roster_bound": True,
                "pq_key_roster_digest_hex": args.pq_key_roster_digest_hex,
                "reputation_weight_bound": True,
                "reputation_weight_policy_digest_hex": (
                    args.reputation_weight_policy_digest_hex
                ),
                "governance_dag_bound": True,
                "config_source": "iroha_config",
                "receipt_summary_digest_hex": args.receipt_summary_digest_hex,
                "policy_digest_hex": args.policy_digest_hex,
            }
        )
    return payload


def validate_thresholds(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate threshold-bound facts before payload construction."""

    if args.route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS:
        errors.append(f"--route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if args.hot_latency_ms > DEFAULT_MAX_HOT_LATENCY_MS:
        errors.append(f"--hot-latency-ms must be <= {DEFAULT_MAX_HOT_LATENCY_MS}")
    if args.warm_latency_ms > DEFAULT_MAX_WARM_LATENCY_MS:
        errors.append(f"--warm-latency-ms must be <= {DEFAULT_MAX_WARM_LATENCY_MS}")
    if args.provider_count < DEFAULT_MIN_PROVIDERS:
        errors.append(f"--provider-count must be >= {DEFAULT_MIN_PROVIDERS}")
    if args.receipt_count < DEFAULT_MIN_RECEIPTS:
        errors.append(f"--receipt-count must be >= {DEFAULT_MIN_RECEIPTS}")


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    validate_thresholds(args, errors)
    if args.kind in RECEIPT_SUMMARY_DIGEST_KINDS:
        validate_hex64(
            args.receipt_summary_digest_hex,
            option="--receipt-summary-digest-hex",
            errors=errors,
        )
    if args.kind == "multi_provider_probe":
        args.tiers = validate_name_set(
            split_csv_values(args.tier),
            allowed=REQUIRED_TIERS,
            option="--tier",
            errors=errors,
        )
    elif args.kind == "receipt_validation":
        require_kind_options(
            args,
            errors,
            (
                ("--validation-bundle-digest-hex", args.validation_bundle_digest_hex),
                ("--pq-key-roster-digest-hex", args.pq_key_roster_digest_hex),
            ),
        )
        validate_hex64(
            args.validation_bundle_digest_hex,
            option="--validation-bundle-digest-hex",
            errors=errors,
        )
        validate_hex64(
            args.pq_key_roster_digest_hex,
            option="--pq-key-roster-digest-hex",
            errors=errors,
        )
    elif args.kind == "proof_stream":
        args.routes = validate_name_set(
            split_csv_values(args.route),
            allowed=REQUIRED_ROUTES,
            option="--route",
            errors=errors,
        )
        if args.route_status_code < 200 or args.route_status_code > 299:
            errors.append("--route-status-code must be a 2xx HTTP status code")
    elif args.kind == "reputation_integration":
        require_kind_options(
            args,
            errors,
            (
                ("--stats-digest-hex", args.stats_digest_hex),
                (
                    "--reputation-weight-policy-digest-hex",
                    args.reputation_weight_policy_digest_hex,
                ),
            ),
        )
        validate_hex64(
            args.stats_digest_hex,
            option="--stats-digest-hex",
            errors=errors,
        )
        validate_hex64(
            args.reputation_weight_policy_digest_hex,
            option="--reputation-weight-policy-digest-hex",
            errors=errors,
        )
    elif args.kind == "observability":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    elif args.kind == "governance_approval":
        require_kind_options(
            args,
            errors,
            (
                ("--policy-digest-hex", args.policy_digest_hex),
                ("--pq-key-roster-digest-hex", args.pq_key_roster_digest_hex),
                (
                    "--reputation-weight-policy-digest-hex",
                    args.reputation_weight_policy_digest_hex,
                ),
            ),
        )
        validate_hex64(
            args.policy_digest_hex,
            option="--policy-digest-hex",
            errors=errors,
        )
        validate_hex64(
            args.pq_key_roster_digest_hex,
            option="--pq-key-roster-digest-hex",
            errors=errors,
        )
        validate_hex64(
            args.reputation_weight_policy_digest_hex,
            option="--reputation-weight-policy-digest-hex",
            errors=errors,
        )
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_hot_latency_ms=DEFAULT_MAX_HOT_LATENCY_MS,
        max_warm_latency_ms=DEFAULT_MAX_WARM_LATENCY_MS,
        min_providers=DEFAULT_MIN_PROVIDERS,
        min_receipts=DEFAULT_MIN_RECEIPTS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the PoTR gate contract."""

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
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            fd = -1
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp_path, path)
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
        description="Build payload-free SoraFS SF-14 PoTR canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--receipt-summary-digest-hex")
    parser.add_argument("--validation-bundle-digest-hex")
    parser.add_argument("--stats-digest-hex")
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--pq-key-roster-digest-hex")
    parser.add_argument("--reputation-weight-policy-digest-hex")
    parser.add_argument("--tier", action="append", default=[])
    parser.add_argument("--route", action="append", default=[])
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=200)
    parser.add_argument("--hot-latency-ms", type=non_negative_int_arg, default=80_000)
    parser.add_argument("--warm-latency-ms", type=non_negative_int_arg, default=260_000)
    parser.add_argument("--provider-count", type=positive_int_arg, default=3)
    parser.add_argument("--receipt-count", type=positive_int_arg, default=6)
    parser.add_argument("--receipts-validated", type=positive_int_arg, default=6)
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
            "ERROR: SoraFS PoTR canary inputs are incomplete:",
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
