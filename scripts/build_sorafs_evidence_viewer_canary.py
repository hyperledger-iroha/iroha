#!/usr/bin/env python3
"""Build payload-free SoraFS evidence-viewer rollout canary artifacts."""

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

from check_sorafs_moderation_panel_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_CANARY_AGE_SECS,
    DEFAULT_MAX_EVENT_LAG_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MAX_VIEWER_URL_TTL_SECS,
    DEFAULT_MIN_PANEL_SIZE,
    DEFAULT_MIN_PEERS,
    FORBIDDEN_INVENTORY_LABEL_MARKERS,
    KIND_BY_NAME,
    REQUIRED_VIEWER_EVENT_KINDS,
    REQUIRED_VIEWER_EXPORT_TARGETS,
    REQUIRED_VIEWER_ROLES,
    REQUIRED_VIEWER_SECURITY_CONTROLS,
    ValidationOptions,
    VIEWER_SESSION_LABEL_ERROR,
    VIEWER_SESSION_LABEL_PATTERN,
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
    positive_int_arg,
)


SCHEMA = KIND_BY_NAME["evidence_viewer"].schema
HEX64_LEN = 64
VERIFIED_TRUE_CLAIMS = (
    "attested_viewer_enabled",
    "role_scoped_manifest_verified",
    "short_lived_urls_verified",
    "session_key_workflow_verified",
    "strict_csp_enforced",
    "offline_mode_disabled",
    "per_session_access_logged",
    "append_only_log_verified",
    "audit_log_tamper_rejected",
    "anomaly_events_recorded",
    "watermark_overlay_rendered",
    "watermark_metadata_hashed",
    "watermark_metadata_mismatch_rejected",
    "audit_digest_exported",
    "transparency_report_exported",
    "daily_digest_published",
    "payload_redaction_verified",
    "denylisted_digest_blocked",
    "unauthorized_access_rejected",
    "stale_url_rejected",
    "session_replay_rejected",
    "legal_hold_policy_bound",
)
FORBIDDEN_PAYLOAD_CLAIMS = (
    "raw_evidence_included",
    "session_tokens_included",
    "signed_urls_included",
    "watermark_secrets_included",
    "response_bodies_included",
)
DIGEST_FIELDS = (
    "session_manifest_digest_hex",
    "watermark_metadata_digest_hex",
    "access_log_digest_hex",
    "legal_hold_receipt_digest_hex",
    "transparency_report_digest_hex",
    "audit_digest_hex",
)


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


def validate_hex64(value: str, *, option: str, errors: list[str]) -> None:
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


def validate_reviewed_inventory(
    values: Iterable[str],
    *,
    expected_count: int,
    option: str,
    count_option: str,
    errors: list[str],
    pattern=None,
    label_error: str | None = None,
) -> list[str]:
    """Return reviewed unique inventory labels whose count matches a CLI count."""

    items = list(values)
    if not items:
        errors.append(f"{option} is required")
    for index, item in enumerate(items):
        validate_canonical_string(item, label=f"{option}[{index}]", errors=errors)
        if pattern is None or not isinstance(item, str):
            continue
        if pattern.fullmatch(item) is None:
            errors.append(
                (label_error or f"{option} uses an invalid label").replace(
                    "sessions[].name",
                    option,
                )
            )
            continue
        forbidden = sorted(
            marker
            for marker in FORBIDDEN_INVENTORY_LABEL_MARKERS
            if marker in item.split("-")
        )
        if forbidden:
            errors.append(
                f"{option}[{index}] must not contain non-production markers {forbidden}"
            )
    unique_items = set(items)
    if len(unique_items) != len(items):
        errors.append(f"{option} must not contain duplicates")
    if len(unique_items) != expected_count:
        errors.append(f"{option} unique values must match {count_option}")
    return items


def build_session_records(names: Sequence[str]) -> list[dict[str, Any]]:
    """Build reviewed payload-free evidence-viewer session records."""

    return [{"name": name, "attested": True, "logged": True} for name in names]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build the payload-free evidence-viewer canary payload."""

    payload: dict[str, Any] = {
        "schema": SCHEMA,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
        "case_digest_hex": args.case_digest_hex,
        "roster_hash_hex": args.roster_hash_hex,
        "session_count": args.session_count,
        "attested_session_count": args.session_count,
        "logged_session_count": args.session_count,
        "sessions": build_session_records(args.viewer_sessions),
        "max_url_ttl_secs": args.max_url_ttl_secs,
        "role_count": len(args.roles),
        "roles_tested": args.roles,
        "security_control_count": len(args.security_controls),
        "viewer_security_controls": args.security_controls,
        "access_event_kind_count": len(args.access_event_kinds),
        "access_event_kinds": args.access_event_kinds,
        "export_target_count": len(args.export_targets),
        "export_targets": args.export_targets,
    }
    for claim in VERIFIED_TRUE_CLAIMS:
        payload[claim] = claim in args.verified_claims
    for claim in FORBIDDEN_PAYLOAD_CLAIMS:
        payload[claim] = False
    for field in DIGEST_FIELDS:
        payload[field] = getattr(args, field)
    return payload


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    for option, value in (
        ("--case-digest-hex", args.case_digest_hex),
        ("--roster-hash-hex", args.roster_hash_hex),
        ("--session-manifest-digest-hex", args.session_manifest_digest_hex),
        ("--watermark-metadata-digest-hex", args.watermark_metadata_digest_hex),
        ("--access-log-digest-hex", args.access_log_digest_hex),
        ("--legal-hold-receipt-digest-hex", args.legal_hold_receipt_digest_hex),
        ("--transparency-report-digest-hex", args.transparency_report_digest_hex),
        ("--audit-digest-hex", args.audit_digest_hex),
    ):
        validate_hex64(value, option=option, errors=errors)

    args.roles = validate_name_set(
        split_csv_values(args.role),
        allowed=REQUIRED_VIEWER_ROLES,
        option="--role",
        errors=errors,
    )
    args.security_controls = validate_name_set(
        split_csv_values(args.security_control),
        allowed=REQUIRED_VIEWER_SECURITY_CONTROLS,
        option="--security-control",
        errors=errors,
    )
    args.access_event_kinds = validate_name_set(
        split_csv_values(args.access_event_kind),
        allowed=REQUIRED_VIEWER_EVENT_KINDS,
        option="--access-event-kind",
        errors=errors,
    )
    args.export_targets = validate_name_set(
        split_csv_values(args.export_target),
        allowed=REQUIRED_VIEWER_EXPORT_TARGETS,
        option="--export-target",
        errors=errors,
    )
    args.verified_claims = validate_name_set(
        split_csv_values(args.verified_claim),
        allowed=VERIFIED_TRUE_CLAIMS,
        option="--verified-claim",
        errors=errors,
    )
    args.viewer_sessions = validate_reviewed_inventory(
        split_csv_values(args.viewer_session),
        expected_count=args.session_count,
        option="--viewer-session",
        count_option="--session-count",
        pattern=VIEWER_SESSION_LABEL_PATTERN,
        label_error=VIEWER_SESSION_LABEL_ERROR,
        errors=errors,
    )
    if args.max_url_ttl_secs > DEFAULT_MAX_VIEWER_URL_TTL_SECS:
        errors.append(
            f"--max-url-ttl-secs must be <= {DEFAULT_MAX_VIEWER_URL_TTL_SECS}"
        )
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_canary_age_secs=DEFAULT_MAX_CANARY_AGE_SECS,
        max_event_lag_secs=DEFAULT_MAX_EVENT_LAG_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        min_panel_size=DEFAULT_MIN_PANEL_SIZE,
        min_peers=DEFAULT_MIN_PEERS,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the moderation-panel gate contract."""

    kind, errors = validate_evidence_payload(payload, validation_options(args))
    if kind != "evidence_viewer":
        errors.append("generated canary must validate as evidence_viewer")
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
        description=(
            "Build a payload-free SoraFS SFM-4b3 evidence-viewer canary JSON."
        ),
    )
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--case-digest-hex", required=True)
    parser.add_argument("--roster-hash-hex", required=True)
    parser.add_argument("--session-count", type=positive_int_arg, required=True)
    parser.add_argument("--viewer-session", action="append", default=[])
    parser.add_argument(
        "--max-url-ttl-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_VIEWER_URL_TTL_SECS,
    )
    parser.add_argument("--role", action="append", default=[])
    parser.add_argument("--security-control", action="append", default=[])
    parser.add_argument("--access-event-kind", action="append", default=[])
    parser.add_argument("--export-target", action="append", default=[])
    parser.add_argument("--verified-claim", action="append", default=[])
    for field in DIGEST_FIELDS:
        parser.add_argument(f"--{field.replace('_', '-')}", required=True)
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
            "ERROR: SoraFS evidence-viewer canary inputs are incomplete:",
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
