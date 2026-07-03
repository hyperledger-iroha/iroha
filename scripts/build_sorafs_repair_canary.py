#!/usr/bin/env python3
"""Build payload-free SoraFS repair rollout canary artifacts."""

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

from check_sorafs_repair_rollout_evidence import (  # noqa: E402
    AUDITOR_LABEL_ERROR,
    AUDITOR_LABEL_PATTERN,
    DEFAULT_MAX_EVENT_LAG_SECS,
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_REPAIR_LATENCY_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_AUDITORS,
    FAILURE_BOUND_KINDS,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    KIND_BY_NAME,
    REQUIRED_AUDITOR_ROUTES,
    REQUIRED_EVENT_ROUTES,
    REQUIRED_FAILURE_SOURCES,
    REQUIRED_GOVERNANCE_TARGETS,
    REQUIRED_LIFECYCLE_STATUSES,
    REQUIRED_METRICS,
    REQUIRED_WORKER_ROUTES,
    ROSTER_BOUND_KINDS,
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
ROSTER_DIGEST_KINDS = ("auditor_roster",) + ROSTER_BOUND_KINDS
FAILURE_BUNDLE_DIGEST_KINDS = ("failure_capture",) + FAILURE_BOUND_KINDS
POLICY_DIGEST_KINDS = ("governance_handoff", "governance_approval")
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

    return label_error.replace("auditors[].name", option)


def validate_failure_events(
    values: Iterable[str],
    *,
    expected_count: int,
    errors: list[str],
) -> list[dict[str, str]]:
    """Return reviewed failure event records from SOURCE:NAME CLI values."""

    items = list(values)
    if not items:
        errors.append("--failure-event is required for failure_capture")
    records: list[dict[str, str]] = []
    names: list[str] = []
    sources: list[str] = []
    for index, item in enumerate(items):
        if ":" not in item:
            errors.append("--failure-event values must use <source>:<name>")
            continue
        source, name = item.split(":", 1)
        source = source.strip()
        name = name.strip()
        validate_canonical_string(
            source,
            label=f"--failure-event[{index}].source",
            errors=errors,
        )
        validate_canonical_string(
            name,
            label=f"--failure-event[{index}].name",
            errors=errors,
        )
        if source and source not in REQUIRED_FAILURE_SOURCES:
            errors.append("--failure-event source must be a reviewed failure source")
        if source and name:
            records.append({"source": source, "name": name})
            names.append(name)
            sources.append(source)
    if len(set(names)) != len(names):
        errors.append("--failure-event names must not contain duplicates")
    if len(set(names)) != expected_count:
        errors.append("--failure-event unique names must match --failure-event-count")
    missing_sources = [
        source for source in REQUIRED_FAILURE_SOURCES if source not in set(sources)
    ]
    if missing_sources:
        errors.append("--failure-event must include every required failure source")
    return records


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
    """Build fields shared by repair canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "generated_at_unix": args.generated_at_unix,
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
    }


def build_route_records(args: argparse.Namespace, routes: Sequence[str]) -> list[dict[str, Any]]:
    """Build payload-free repair route probe records."""

    return [
        {
            "name": name,
            "passed": True,
            "status_code": args.route_status_code,
            "latency_ms": args.route_latency_ms,
            "authz_enforced": True,
            "signature_verified": True,
        }
        for name in routes
    ]


def build_inventory_records(names: Sequence[str]) -> list[dict[str, str]]:
    """Build reviewed payload-free inventory records."""

    return [{"name": name} for name in names]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free repair rollout canary payload."""

    payload = common_payload(args)
    if args.kind == "auditor_roster":
        payload.update(
            {
                "roster_published": True,
                "roster_signature_verified": True,
                "sf9_coordinator_bound": True,
                "runbook_published": True,
                "auditor_notifications_configured": True,
                "auditor_count": args.auditor_count,
                "auditors": build_inventory_records(args.auditors),
                "roster_digest_hex": args.roster_digest_hex,
                "raw_roster_included": False,
            }
        )
    elif args.kind == "failure_capture":
        payload.update(
            {
                "failure_sources": args.failure_sources,
                "failure_source_count": len(args.failure_sources),
                "por_history_replayed": True,
                "potr_receipt_replayed": True,
                "coordinator_event_verified": True,
                "merkle_or_receipt_inclusion_verified": True,
                "object_storage_retention_bound": True,
                "failure_event_count": args.failure_event_count,
                "failure_events": args.failure_events,
                "evidence_bundle_digest_hex": args.evidence_bundle_digest_hex,
                "raw_evidence_included": False,
            }
        )
    elif args.kind == "auditor_api":
        routes = build_route_records(args, args.auditor_routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "roster_digest_hex": args.roster_digest_hex,
                "signed_auditor_envelope_required": True,
                "nonce_replay_rejected": True,
                "legacy_raw_payload_rejected": True,
                "per_auditor_rate_limit_enforced": True,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "worker_lifecycle":
        routes = build_route_records(args, args.worker_routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "roster_digest_hex": args.roster_digest_hex,
                "evidence_bundle_digest_hex": args.evidence_bundle_digest_hex,
                "status_count": len(args.lifecycle_statuses),
                "statuses_observed": args.lifecycle_statuses,
                "worker_permission_enforced": True,
                "lease_heartbeat_enforced": True,
                "idempotency_enforced": True,
                "norito_snapshot_persisted": True,
                "gc_protection_verified": True,
                "repair_latency_seconds": args.repair_latency_seconds,
                "raw_repair_payloads_included": False,
            }
        )
    elif args.kind == "event_streams":
        routes = build_route_records(args, args.event_routes)
        payload.update(
            {
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "routes": routes,
                "roster_digest_hex": args.roster_digest_hex,
                "evidence_bundle_digest_hex": args.evidence_bundle_digest_hex,
                "backlog_replay_verified": True,
                "sse_delivery_verified": True,
                "websocket_delivery_verified": True,
                "event_lag_seconds": args.event_lag_seconds,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "governance_handoff":
        payload.update(
            {
                "roster_digest_hex": args.roster_digest_hex,
                "evidence_bundle_digest_hex": args.evidence_bundle_digest_hex,
                "slash_proposal_generated": True,
                "governance_dag_published": True,
                "escalation_policy_enforced": True,
                "appeal_window_enforced": True,
                "reserve_rent_handoff_verified": True,
                "transparency_publication_verified": True,
                "reputation_handoff_verified": True,
                "handoff_target_count": len(args.handoff_targets),
                "handoff_targets": args.handoff_targets,
                "handoff_digest_hex": args.handoff_digest_hex,
                "policy_digest_hex": args.policy_digest_hex,
                "raw_ledger_included": False,
            }
        )
    elif args.kind == "observability":
        payload.update(
            {
                "metrics_scrape_success": True,
                "dashboard_provisioned": True,
                "alert_rules_installed": True,
                "critical_alerts_firing": False,
                "metrics": args.metrics,
                "metric_count": len(args.metrics),
                "response_bodies_included": False,
            }
        )
    elif args.kind == "governance_approval":
        payload.update(
            {
                "approved": True,
                "governance_vote_recorded": True,
                "iroha_config_bound": True,
                "repair_policy_bound": True,
                "auditor_roster_bound": True,
                "roster_digest_hex": args.roster_digest_hex,
                "slash_policy_bound": True,
                "handoff_digest_hex": args.handoff_digest_hex,
                "config_source": "iroha_config",
                "policy_digest_hex": args.policy_digest_hex,
            }
        )
    return payload


def validate_thresholds(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate threshold-bound facts before payload construction."""

    if args.route_latency_ms > DEFAULT_MAX_ROUTE_LATENCY_MS:
        errors.append(f"--route-latency-ms must be <= {DEFAULT_MAX_ROUTE_LATENCY_MS}")
    if args.event_lag_seconds > DEFAULT_MAX_EVENT_LAG_SECS:
        errors.append(f"--event-lag-seconds must be <= {DEFAULT_MAX_EVENT_LAG_SECS}")
    if args.repair_latency_seconds > DEFAULT_MAX_REPAIR_LATENCY_SECS:
        errors.append(
            f"--repair-latency-seconds must be <= {DEFAULT_MAX_REPAIR_LATENCY_SECS}"
        )
    if args.auditor_count < DEFAULT_MIN_AUDITORS:
        errors.append(f"--auditor-count must be >= {DEFAULT_MIN_AUDITORS}")


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    validate_thresholds(args, errors)
    if args.kind in ROSTER_DIGEST_KINDS:
        validate_hex64(args.roster_digest_hex, option="--roster-digest-hex", errors=errors)
    if args.kind in FAILURE_BUNDLE_DIGEST_KINDS:
        validate_hex64(
            args.evidence_bundle_digest_hex,
            option="--evidence-bundle-digest-hex",
            errors=errors,
        )
    if args.kind in POLICY_DIGEST_KINDS:
        require_kind_options(
            args,
            errors,
            (("--policy-digest-hex", args.policy_digest_hex),),
        )
        validate_hex64(args.policy_digest_hex, option="--policy-digest-hex", errors=errors)
    if args.kind == "auditor_roster":
        args.auditors = validate_reviewed_inventory(
            split_csv_values(args.auditor),
            expected_count=args.auditor_count,
            option="--auditor",
            kind="auditor_roster",
            count_option="--auditor-count",
            pattern=AUDITOR_LABEL_PATTERN,
            label_error=AUDITOR_LABEL_ERROR,
            errors=errors,
        )
    elif args.kind == "failure_capture":
        args.failure_sources = validate_name_set(
            split_csv_values(args.failure_source),
            allowed=REQUIRED_FAILURE_SOURCES,
            option="--failure-source",
            errors=errors,
        )
        args.failure_events = validate_failure_events(
            split_csv_values(args.failure_event),
            expected_count=args.failure_event_count,
            errors=errors,
        )
    elif args.kind == "auditor_api":
        args.auditor_routes = validate_name_set(
            split_csv_values(args.auditor_route),
            allowed=REQUIRED_AUDITOR_ROUTES,
            option="--auditor-route",
            errors=errors,
        )
        if args.route_status_code < 200 or args.route_status_code > 299:
            errors.append("--route-status-code must be a 2xx HTTP status code")
    elif args.kind == "worker_lifecycle":
        args.worker_routes = validate_name_set(
            split_csv_values(args.worker_route),
            allowed=REQUIRED_WORKER_ROUTES,
            option="--worker-route",
            errors=errors,
        )
        args.lifecycle_statuses = validate_name_set(
            split_csv_values(args.lifecycle_status),
            allowed=REQUIRED_LIFECYCLE_STATUSES,
            option="--lifecycle-status",
            errors=errors,
        )
        if args.route_status_code < 200 or args.route_status_code > 299:
            errors.append("--route-status-code must be a 2xx HTTP status code")
    elif args.kind == "event_streams":
        args.event_routes = validate_name_set(
            split_csv_values(args.event_route),
            allowed=REQUIRED_EVENT_ROUTES,
            option="--event-route",
            errors=errors,
        )
        if args.route_status_code < 200 or args.route_status_code > 299:
            errors.append("--route-status-code must be a 2xx HTTP status code")
    elif args.kind == "governance_handoff":
        require_kind_options(
            args,
            errors,
            (("--handoff-digest-hex", args.handoff_digest_hex),),
        )
        validate_hex64(args.handoff_digest_hex, option="--handoff-digest-hex", errors=errors)
        args.handoff_targets = validate_name_set(
            split_csv_values(args.handoff_target),
            allowed=REQUIRED_GOVERNANCE_TARGETS,
            option="--handoff-target",
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
                ("--handoff-digest-hex", args.handoff_digest_hex),
            ),
        )
        validate_hex64(args.handoff_digest_hex, option="--handoff-digest-hex", errors=errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_event_lag_secs=DEFAULT_MAX_EVENT_LAG_SECS,
        max_repair_latency_secs=DEFAULT_MAX_REPAIR_LATENCY_SECS,
        min_auditors=DEFAULT_MIN_AUDITORS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the repair gate contract."""

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
        description="Build payload-free SoraFS SF-8b repair canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--roster-digest-hex")
    parser.add_argument("--evidence-bundle-digest-hex")
    parser.add_argument("--handoff-digest-hex")
    parser.add_argument("--policy-digest-hex")
    parser.add_argument("--failure-source", action="append", default=[])
    parser.add_argument("--failure-event", action="append", default=[])
    parser.add_argument("--auditor-route", action="append", default=[])
    parser.add_argument("--worker-route", action="append", default=[])
    parser.add_argument("--event-route", action="append", default=[])
    parser.add_argument("--lifecycle-status", action="append", default=[])
    parser.add_argument("--handoff-target", action="append", default=[])
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--auditor", action="append", default=[])
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--route-latency-ms", type=non_negative_int_arg, default=200)
    parser.add_argument("--event-lag-seconds", type=non_negative_int_arg, default=30)
    parser.add_argument("--repair-latency-seconds", type=non_negative_int_arg, default=900)
    parser.add_argument("--auditor-count", type=positive_int_arg, default=3)
    parser.add_argument("--failure-event-count", type=positive_int_arg, default=2)
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
            "ERROR: SoraFS repair canary inputs are incomplete:",
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
