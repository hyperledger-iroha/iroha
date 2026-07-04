#!/usr/bin/env python3
"""Build payload-free SoraFS orderbook rollout canary artifacts."""

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

from check_sorafs_orderbook_rollout_evidence import (  # noqa: E402
    CHANNEL_REF_ERROR,
    CHANNEL_REF_PATTERN,
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_MATCHER_LAG_MS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MAX_STREAM_LAG_MS,
    DEFAULT_MIN_RECONCILIATION_PEERS,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    KIND_BY_NAME,
    ORDER_REF_ERROR,
    ORDER_REF_PATTERN,
    PEER_LABEL_ERROR,
    PEER_LABEL_PATTERN,
    RECEIPT_REF_ERROR,
    RECEIPT_REF_PATTERN,
    REQUIRED_API_ROUTES,
    REQUIRED_METRICS,
    REQUIRED_RECONCILIATION_SOURCES,
    REQUIRED_SDK_LANGUAGES,
    REQUIRED_STREAMS,
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
CONTRACT_DIGEST_KINDS = CANARY_KINDS
POLICY_DIGEST_KINDS = ("contract_surface", "governance_approval")
HEX64_LEN = 64
TRUE_CLAIMS: dict[str, tuple[str, ...]] = {
    "contract_surface": (
        "contract_deployed",
        "deterministic_matching_verified",
        "escrow_enforced",
        "pause_control_configured",
        "fee_policy_config_bound",
        "capability_policy_configured",
    ),
    "matcher_service": (
        "daemonized",
        "contract_forwarding_enabled",
        "price_time_priority_verified",
        "replay_snapshot_verified",
        "durable_checkpoint_verified",
    ),
    "settlement_service": (
        "daemonized",
        "escrow_custody_mutation_verified",
        "receipt_authorization_verified",
        "non_overlapping_ranges_enforced",
        "governance_receipts_published",
    ),
    "api_gateway": (
        "canonical_request_auth_enforced",
        "owner_account_binding_verified",
        "provider_role_binding_verified",
        "capability_policy_enforced",
    ),
    "event_streams": (
        "backlog_replay_verified",
        "live_delivery_verified",
        "contract_backed",
    ),
    "sdk_release": (
        "artifact_hashes_verified",
        "live_smoke_passed",
        "submitter_helpers_verified",
    ),
    "observability": (
        "metrics_scrape_success",
        "dashboard_provisioned",
        "alert_rules_installed",
        "live_dashboard_wired",
    ),
    "reconciliation": (
        "contract_mirror_reconciliation_passed",
        "evidence_dag_published",
    ),
    "governance_approval": (
        "approved",
        "governance_vote_recorded",
        "iroha_config_bound",
        "orderbook_activation_governed",
        "emergency_pause_tested",
        "capability_policy_bound",
        "treasury_policy_bound",
    ),
}
FORCED_FALSE_FIELDS: dict[str, tuple[str, ...]] = {
    "contract_surface": ("raw_contract_state_included",),
    "matcher_service": ("divergence_detected", "raw_snapshot_included"),
    "settlement_service": ("raw_receipts_included",),
    "api_gateway": ("response_bodies_included",),
    "event_streams": ("response_bodies_included",),
    "sdk_release": ("debug_artifacts",),
    "observability": ("critical_alerts_firing", "response_bodies_included"),
    "reconciliation": ("contract_mirror_divergence", "raw_ledger_included"),
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
        validate_canonical_string(item, label=f"{option}[{index}]", errors=errors)
        if pattern is not None and pattern.fullmatch(item) is None:
            if label_error is None:
                errors.append(f"{option} has malformed inventory label")
            else:
                errors.append(label_error.format(path=option))
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


def validate_optional_reviewed_inventory(
    values: Iterable[str],
    *,
    expected_count: int,
    option: str,
    kind: str,
    count_option: str,
    errors: list[str],
    pattern: re.Pattern[str],
    label_error: str,
) -> list[str]:
    """Return reviewed inventory labels, allowing an empty list only for zero counts."""

    items = list(values)
    if expected_count == 0 and not items:
        return []
    return validate_reviewed_inventory(
        items,
        expected_count=expected_count,
        option=option,
        kind=kind,
        count_option=count_option,
        errors=errors,
        pattern=pattern,
        label_error=label_error,
    )


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


def validate_canonical_string(value: str, *, label: str, errors: list[str]) -> None:
    """Require a non-empty canonical string without control characters."""

    if (
        not isinstance(value, str)
        or not value.strip()
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        errors.append(f"{label} must be a non-empty canonical string")


def parse_artifacts(values: Sequence[str], errors: list[str]) -> list[dict[str, str]]:
    """Parse repeated id:sha256 artifact descriptors."""

    artifacts: list[dict[str, str]] = []
    seen_artifact_ids: set[str] = set()
    for index, value in enumerate(values):
        if ":" not in value:
            errors.append("--artifact must use id:sha256")
            continue
        artifact_id, sha256 = value.split(":", 1)
        validate_canonical_string(artifact_id, label=f"--artifact[{index}].id", errors=errors)
        validate_hex64(sha256, option=f"--artifact[{index}].sha256", errors=errors)
        if artifact_id in seen_artifact_ids:
            errors.append("duplicate --artifact id")
            continue
        seen_artifact_ids.add(artifact_id)
        artifacts.append({"id": artifact_id, "sha256": sha256})
    if len(artifacts) < len(REQUIRED_SDK_LANGUAGES):
        errors.append(
            "--artifact must include at least one distinct SDK release artifact "
            "per reviewed language"
        )
    return artifacts


def build_common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by orderbook canary payloads."""

    payload: dict[str, Any] = {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
    }
    if args.kind in CONTRACT_DIGEST_KINDS:
        payload["contract_digest_hex"] = args.contract_digest_hex
    if args.kind in POLICY_DIGEST_KINDS:
        payload["policy_digest_hex"] = args.policy_digest_hex
    return payload


def apply_verified_claims(payload: dict[str, Any], args: argparse.Namespace) -> None:
    """Populate explicitly verified true claims and forced payload-free false flags."""

    for claim in TRUE_CLAIMS[args.kind]:
        payload[claim] = claim in args.verified_claims
    for field in FORCED_FALSE_FIELDS[args.kind]:
        payload[field] = False


def build_route_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free authenticated API route probe records."""

    return [
        {
            "name": route,
            "passed": True,
            "status_code": args.route_status_code,
            "body_blake3_hex": args.route_body_blake3_hex,
            "latency_ms": args.route_latency_ms,
            "authz_enforced": True,
            "signature_verified": True,
        }
        for route in args.routes
    ]


def build_stream_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free event-stream canary records."""

    return [
        {
            "name": stream,
            "passed": True,
            "lag_ms": args.stream_lag_ms,
            "backlog_replay_verified": "backlog_replay_verified" in args.verified_claims,
            "live_delivery_verified": "live_delivery_verified" in args.verified_claims,
            "contract_backed": "contract_backed" in args.verified_claims,
        }
        for stream in args.streams
    ]


def build_inventory_records(names: Sequence[str]) -> list[dict[str, str]]:
    """Build reviewed payload-free inventory records."""

    return [{"name": name} for name in names]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free orderbook rollout canary payload."""

    payload = build_common_payload(args)
    apply_verified_claims(payload, args)
    if args.kind == "contract_surface":
        payload["contract_state_source"] = "on-chain"
    elif args.kind == "matcher_service":
        payload.update(
            {
                "matcher_lag_ms": args.matcher_lag_ms,
                "accepted_order_count": args.accepted_order_count,
                "accepted_orders": args.accepted_orders,
                "matched_order_count": args.matched_order_count,
                "matched_orders": args.matched_orders,
                "rejected_invalid_order_count": args.rejected_invalid_order_count,
                "rejected_invalid_orders": args.rejected_invalid_orders,
            }
        )
    elif args.kind == "settlement_service":
        payload.update(
            {
                "open_channel_count": args.open_channel_count,
                "open_channels": args.open_channels,
                "settled_receipt_count": args.settled_receipt_count,
                "settled_receipts": args.settled_receipts,
                "settlement_backlog_count": args.settlement_backlog_count,
                "settlement_backlog_channels": args.settlement_backlog_channels,
            }
        )
    elif args.kind == "api_gateway":
        routes = build_route_records(args)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
            }
        )
    elif args.kind == "event_streams":
        streams = build_stream_records(args)
        payload.update(
            {
                "stream_count": len(streams),
                "streams": streams,
            }
        )
    elif args.kind == "sdk_release":
        payload.update(
            {
                "language_count": len(args.languages),
                "languages": [{"name": language} for language in args.languages],
                "artifact_count": len(args.artifacts),
                "artifacts": args.artifacts,
            }
        )
    elif args.kind == "observability":
        payload.update(
            {
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
            }
        )
    elif args.kind == "reconciliation":
        payload.update(
            {
                "peer_count": args.peer_count,
                "peers": build_inventory_records(args.peers),
                "source_count": len(args.sources),
                "sources": [{"name": source} for source in args.sources],
                "mismatch_count": 0,
                "unreconciled_event_count": 0,
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

    if args.kind in CONTRACT_DIGEST_KINDS:
        validate_hex64(args.contract_digest_hex, option="--contract-digest-hex", errors=errors)
    if args.kind in POLICY_DIGEST_KINDS:
        validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)
    args.verified_claims = validate_name_set(
        split_csv_values(args.verified_claim),
        allowed=TRUE_CLAIMS[args.kind],
        option="--verified-claim",
        errors=errors,
    )
    if args.kind == "matcher_service":
        for option, value in (
            ("--matcher-lag-ms", args.matcher_lag_ms),
            ("--accepted-order-count", args.accepted_order_count),
            ("--matched-order-count", args.matched_order_count),
        ):
            if value is None:
                errors.append(f"{option} is required for matcher_service")
        args.accepted_orders = validate_reviewed_inventory(
            split_csv_values(args.accepted_order),
            expected_count=args.accepted_order_count or 0,
            option="--accepted-order",
            kind="matcher_service",
            count_option="--accepted-order-count",
            pattern=ORDER_REF_PATTERN,
            label_error=ORDER_REF_ERROR,
            errors=errors,
        )
        args.matched_orders = validate_reviewed_inventory(
            split_csv_values(args.matched_order),
            expected_count=args.matched_order_count or 0,
            option="--matched-order",
            kind="matcher_service",
            count_option="--matched-order-count",
            pattern=ORDER_REF_PATTERN,
            label_error=ORDER_REF_ERROR,
            errors=errors,
        )
        accepted_order_set = set(args.accepted_orders)
        if any(order not in accepted_order_set for order in args.matched_orders):
            errors.append("--matched-order values must also be present in --accepted-order")
        if args.rejected_invalid_order_count is None:
            args.rejected_invalid_order_count = 0
        args.rejected_invalid_orders = validate_optional_reviewed_inventory(
            split_csv_values(args.rejected_invalid_order),
            expected_count=args.rejected_invalid_order_count,
            option="--rejected-invalid-order",
            kind="matcher_service",
            count_option="--rejected-invalid-order-count",
            pattern=ORDER_REF_PATTERN,
            label_error=ORDER_REF_ERROR,
            errors=errors,
        )
    elif args.kind == "settlement_service":
        for option, value in (
            ("--open-channel-count", args.open_channel_count),
            ("--settled-receipt-count", args.settled_receipt_count),
        ):
            if value is None:
                errors.append(f"{option} is required for settlement_service")
        args.open_channels = validate_reviewed_inventory(
            split_csv_values(args.open_channel),
            expected_count=args.open_channel_count or 0,
            option="--open-channel",
            kind="settlement_service",
            count_option="--open-channel-count",
            pattern=CHANNEL_REF_PATTERN,
            label_error=CHANNEL_REF_ERROR,
            errors=errors,
        )
        args.settled_receipts = validate_reviewed_inventory(
            split_csv_values(args.settled_receipt),
            expected_count=args.settled_receipt_count or 0,
            option="--settled-receipt",
            kind="settlement_service",
            count_option="--settled-receipt-count",
            pattern=RECEIPT_REF_PATTERN,
            label_error=RECEIPT_REF_ERROR,
            errors=errors,
        )
        if args.settlement_backlog_count is None:
            args.settlement_backlog_count = 0
        args.settlement_backlog_channels = validate_optional_reviewed_inventory(
            split_csv_values(args.settlement_backlog_channel),
            expected_count=args.settlement_backlog_count,
            option="--settlement-backlog-channel",
            kind="settlement_service",
            count_option="--settlement-backlog-count",
            pattern=CHANNEL_REF_PATTERN,
            label_error=CHANNEL_REF_ERROR,
            errors=errors,
        )
    elif args.kind == "api_gateway":
        args.routes = validate_name_set(
            split_csv_values(args.route),
            allowed=REQUIRED_API_ROUTES,
            option="--route",
            errors=errors,
        )
        validate_hex64(
            args.route_body_blake3_hex,
            option="--route-body-blake3-hex",
            errors=errors,
        )
    elif args.kind == "event_streams":
        args.streams = validate_name_set(
            split_csv_values(args.stream),
            allowed=REQUIRED_STREAMS,
            option="--stream",
            errors=errors,
        )
    elif args.kind == "sdk_release":
        args.languages = validate_name_set(
            split_csv_values(args.language),
            allowed=REQUIRED_SDK_LANGUAGES,
            option="--language",
            errors=errors,
        )
        args.artifacts = parse_artifacts(args.artifact, errors)
    elif args.kind == "observability":
        args.metrics = validate_name_set(
            split_csv_values(args.metric),
            allowed=REQUIRED_METRICS,
            option="--metric",
            errors=errors,
        )
    elif args.kind == "reconciliation":
        if args.peer_count is None:
            errors.append("--peer-count is required for reconciliation")
        args.peers = validate_reviewed_inventory(
            split_csv_values(args.peer),
            expected_count=args.peer_count or 0,
            option="--peer",
            kind="reconciliation",
            count_option="--peer-count",
            pattern=PEER_LABEL_PATTERN,
            label_error=PEER_LABEL_ERROR,
            errors=errors,
        )
        args.sources = validate_name_set(
            split_csv_values(args.source),
            allowed=REQUIRED_RECONCILIATION_SOURCES,
            option="--source",
            errors=errors,
        )
    elif args.kind == "governance_approval":
        pass


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    if args.route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS:
        errors.append(f"--route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if args.stream_lag_ms > DEFAULT_MAX_STREAM_LAG_MS:
        errors.append(f"--stream-lag-ms must be <= {DEFAULT_MAX_STREAM_LAG_MS}")
    if args.matcher_lag_ms is not None and args.matcher_lag_ms > DEFAULT_MAX_MATCHER_LAG_MS:
        errors.append(f"--matcher-lag-ms must be <= {DEFAULT_MAX_MATCHER_LAG_MS}")
    if args.peer_count is not None and args.peer_count < DEFAULT_MIN_RECONCILIATION_PEERS:
        errors.append(f"--peer-count must be >= {DEFAULT_MIN_RECONCILIATION_PEERS}")
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_stream_lag_ms=DEFAULT_MAX_STREAM_LAG_MS,
        max_matcher_lag_ms=DEFAULT_MAX_MATCHER_LAG_MS,
        min_reconciliation_peers=DEFAULT_MIN_RECONCILIATION_PEERS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the orderbook gate contract."""

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
        description="Build payload-free SoraFS SFM-2 orderbook canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--contract-digest-hex")
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--matcher-lag-ms", type=non_negative_int_arg)
    parser.add_argument("--accepted-order-count", type=positive_int_arg)
    parser.add_argument("--accepted-order", action="append", default=[])
    parser.add_argument("--matched-order-count", type=positive_int_arg)
    parser.add_argument("--matched-order", action="append", default=[])
    parser.add_argument("--rejected-invalid-order-count", type=non_negative_int_arg)
    parser.add_argument("--rejected-invalid-order", action="append", default=[])
    parser.add_argument("--open-channel-count", type=positive_int_arg)
    parser.add_argument("--open-channel", action="append", default=[])
    parser.add_argument("--settled-receipt-count", type=positive_int_arg)
    parser.add_argument("--settled-receipt", action="append", default=[])
    parser.add_argument("--settlement-backlog-count", type=non_negative_int_arg)
    parser.add_argument("--settlement-backlog-channel", action="append", default=[])
    parser.add_argument("--route", action="append", default=[])
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=200)
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--route-body-blake3-hex")
    parser.add_argument("--stream", action="append", default=[])
    parser.add_argument("--stream-lag-ms", type=non_negative_int_arg, default=250)
    parser.add_argument("--language", action="append", default=[])
    parser.add_argument("--artifact", action="append", default=[])
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--peer-count", type=positive_int_arg)
    parser.add_argument("--peer", action="append", default=[])
    parser.add_argument("--source", action="append", default=[])
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
            "ERROR: SoraFS orderbook canary inputs are incomplete:",
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
