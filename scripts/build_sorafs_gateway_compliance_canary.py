#!/usr/bin/env python3
"""Build payload-free SoraFS gateway compliance rollout canary artifacts."""

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

from check_sorafs_gateway_compliance_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_RELOAD_LATENCY_MS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_DENYLIST_ENTRIES,
    DEFAULT_MIN_GATEWAYS,
    DEFAULT_MIN_HONEY_PROBES,
    KIND_BY_NAME,
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
    positive_int_arg,
)


CANARY_KINDS = ("controller_runtime", "moderation_toggle")
HEX64_LEN = 64
CONTROLLER_TRUE_CLAIMS = (
    "iroha_config_bound",
    "controller_service_enabled",
    "scheduler_config_bound",
    "external_feeds_fetched",
    "feed_signature_verified",
    "normalization_deterministic",
    "bundle_pack_verified",
    "update_history_persisted",
    "gateway_reload_requested",
    "failure_backoff_configured",
    "rollback_plan_verified",
)
MODERATION_TRUE_CLAIMS = (
    "iroha_config_bound",
    "operator_role_enforced",
    "approval_workflow_verified",
    "expiry_enforced",
    "cache_invalidation_verified",
    "operator_audit_trail_persisted",
    "rollback_verified",
)
FORBIDDEN_PAYLOAD_CLAIMS = {
    "controller_runtime": (
        "raw_feeds_included",
        "feed_payloads_included",
        "response_bodies_included",
    ),
    "moderation_toggle": (
        "raw_toggle_payloads_included",
        "response_bodies_included",
    ),
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


def validate_feed_names(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate reviewed feed names and bind the optional count cross-check."""

    feed_names = split_csv_values(args.feed)
    if not feed_names:
        errors.append("--feed is required for controller_runtime")
    for name in feed_names:
        validate_canonical_string(name, option="--feed", errors=errors)
    if len(set(feed_names)) != len(feed_names):
        errors.append("--feed must not contain duplicates")
    unique_feed_count = len(set(feed_names))
    if args.feed_count is None:
        errors.append("--feed-count is required for controller_runtime")
    elif unique_feed_count != args.feed_count:
        errors.append("--feed-count must match the number of unique --feed values")
    args.feeds = feed_names


def validate_toggle_names(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate reviewed moderation toggle names and bind the count cross-check."""

    toggle_names = split_csv_values(args.toggle)
    if not toggle_names:
        errors.append("--toggle is required for moderation_toggle")
    for name in toggle_names:
        validate_canonical_string(name, option="--toggle", errors=errors)
    if len(set(toggle_names)) != len(toggle_names):
        errors.append("--toggle must not contain duplicates")
    unique_toggle_count = len(set(toggle_names))
    if args.toggle_count is None:
        errors.append("--toggle-count is required for moderation_toggle")
    elif unique_toggle_count != args.toggle_count:
        errors.append("--toggle-count must match the number of unique --toggle values")
    args.toggles = toggle_names


def build_common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by gateway compliance canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
        "generated_at_unix": args.generated_at_unix,
        "bundle_digest_hex": args.bundle_digest_hex,
        "iroha_config_bound": "iroha_config_bound" in args.verified_claims,
        "config_source": "iroha_config",
    }


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free gateway compliance canary payload."""

    payload = build_common_payload(args)
    if args.kind == "controller_runtime":
        payload.update(
            {
                "controller_instance_id": args.controller_instance_id,
                "external_feed_count": len(args.feeds),
                "fetched_feed_count": len(args.feeds),
                "normalized_feed_count": len(args.feeds),
                "signed_feed_count": len(args.feeds),
                "feeds": [{"name": name} for name in args.feeds],
            }
        )
        for claim in CONTROLLER_TRUE_CLAIMS:
            payload[claim] = claim in args.verified_claims
    elif args.kind == "moderation_toggle":
        payload.update(
            {
                "toggle_api_url": args.toggle_api_url,
                "toggle_count": len(args.toggles),
                "approved_toggle_count": len(args.toggles),
                "toggles": [{"name": name} for name in args.toggles],
                "toggle_digest_hex": args.toggle_digest_hex,
            }
        )
        for claim in MODERATION_TRUE_CLAIMS:
            payload[claim] = claim in args.verified_claims
    for claim in FORBIDDEN_PAYLOAD_CLAIMS[args.kind]:
        payload[claim] = False
    return payload


def validate_kind_inputs(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate kind-specific reviewed operator inputs."""

    if args.kind == "controller_runtime":
        validate_canonical_string(
            args.controller_instance_id,
            option="--controller-instance-id",
            errors=errors,
        )
        validate_feed_names(args, errors)
        args.verified_claims = validate_name_set(
            split_csv_values(args.verified_claim),
            allowed=CONTROLLER_TRUE_CLAIMS,
            option="--verified-claim",
            errors=errors,
        )
        return

    if args.kind == "moderation_toggle":
        validate_canonical_string(
            args.toggle_api_url,
            option="--toggle-api-url",
            errors=errors,
        )
        validate_toggle_names(args, errors)
        validate_hex64(args.toggle_digest_hex, option="--toggle-digest-hex", errors=errors)
        args.verified_claims = validate_name_set(
            split_csv_values(args.verified_claim),
            allowed=MODERATION_TRUE_CLAIMS,
            option="--verified-claim",
            errors=errors,
        )


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, option="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, option="--environment", errors=errors)
    validate_hex64(args.bundle_digest_hex, option="--bundle-digest-hex", errors=errors)
    validate_kind_inputs(args, errors)
    return errors


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    """Return checker options used to prevalidate the generated canary."""

    return ValidationOptions(
        now_unix=args.now_unix or args.generated_at_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_reload_latency_ms=DEFAULT_MAX_RELOAD_LATENCY_MS,
        min_gateways=DEFAULT_MIN_GATEWAYS,
        min_denylist_entries=DEFAULT_MIN_DENYLIST_ENTRIES,
        min_honey_probes=DEFAULT_MIN_HONEY_PROBES,
    )


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the gateway compliance gate contract."""

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
        description=(
            "Build payload-free SoraFS SFM-4 gateway compliance canary JSON."
        ),
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--bundle-digest-hex", required=True)
    parser.add_argument("--verified-claim", action="append", default=[])
    parser.add_argument("--controller-instance-id")
    parser.add_argument("--feed-count", type=positive_int_arg)
    parser.add_argument("--feed", action="append", default=[])
    parser.add_argument("--toggle-api-url")
    parser.add_argument("--toggle-count", type=positive_int_arg)
    parser.add_argument("--toggle", action="append", default=[])
    parser.add_argument("--toggle-digest-hex")
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
            "ERROR: SoraFS gateway compliance canary inputs are incomplete:",
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
