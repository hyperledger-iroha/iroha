#!/usr/bin/env python3
"""Build payload-free SoraFS transparency rollout canary artifacts."""

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

from check_sorafs_transparency_rollout_evidence import (  # noqa: E402
    CYCLE_BOUND_KINDS,
    DEFAULT_REQUIRED_SOURCE_KINDS,
    KIND_BY_NAME,
    REQUIRED_EXPLORER_ROUTES,
    REQUIRED_PRIVACY_AGGREGATE_ACTIONS,
    REQUIRED_PROOF_TOKEN_ISSUANCE_ACTIONS,
    REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES,
    REQUIRED_PUBLICATION_ROUTES,
    SOURCE_BOUND_KINDS,
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


CANARY_KINDS = tuple(KIND_BY_NAME)
SOURCE_BATCH_DIGEST_KINDS = ("source_entry",) + SOURCE_BOUND_KINDS
CYCLE_DIGEST_KINDS = ("publication",) + CYCLE_BOUND_KINDS
HEX64_LEN = 64
DEFAULT_REQUEST_BODY_HASH = "a" * 64
DEFAULT_RESPONSE_BODY_HASH = "b" * 64
DEFAULT_ROUTE_BODY_HASH = "c" * 64


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


def common_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build fields shared by transparency canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "status": "passed",
        "generated_at_unix": args.generated_at_unix,
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
    }


def build_probe_records(
    names: Sequence[str],
    *,
    field: str | None,
    args: argparse.Namespace,
) -> list[dict[str, Any]]:
    """Build payload-free transparency probe records."""

    probes = []
    for name in names:
        record = {
            "response_success": True,
            "response_status": args.probe_status_code,
            "request_body_blake3": args.request_body_blake3,
            "response_body_blake3": args.response_body_blake3,
        }
        if field is not None:
            record[field] = name
        probes.append(record)
    return probes


def build_publication_routes(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free publication route records."""

    return [
        {
            "name": name,
            "passed": True,
            "http_success": True,
            "status_code": args.route_status_code,
            "body_blake3_hex": args.route_body_blake3,
            "anchor_metadata_present": True,
            "publisher_identity_present": True,
            "verification_valid": True,
        }
        for name in args.publication_routes
    ]


def build_explorer_routes(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free explorer route records."""

    return [
        {
            "name": name,
            "http_success": True,
            "status_code": args.route_status_code,
            "body_blake3_hex": args.route_body_blake3,
        }
        for name in args.explorer_routes
    ]


def build_cycle_detail_probe_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    """Build payload-free publication cycle-detail probe records."""

    return [
        {
            "name": name,
            "status_code": args.route_status_code,
            "body_blake3_hex": args.route_body_blake3,
            "anchor_metadata_present": True,
            "publisher_identity_present": True,
            "verification_valid": True,
        }
        for name in args.cycle_detail_probes
    ]


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free transparency rollout canary payload."""

    payload = common_payload(args)
    if args.kind == "source_entry":
        probes = build_probe_records(args.source_kinds, field="source_kind", args=args)
        payload.update(
            {
                "source_batch_digest_hex": args.source_batch_digest_hex,
                "probe_count": len(probes),
                "passed_probe_count": len(probes),
                "source_entry_probe_count": len(probes),
                "payload_bytes_included": False,
                "private_payloads_included": False,
                "response_bodies_included": False,
                "probes": probes,
            }
        )
    elif args.kind == "publication":
        routes = build_publication_routes(args)
        cycle_detail_probes = build_cycle_detail_probe_records(args)
        payload.update(
            {
                "source_batch_digest_hex": args.source_batch_digest_hex,
                "cycle_digest_hex": args.cycle_digest_hex,
                "route_count": len(routes),
                "passed_route_count": len(routes),
                "cycle_detail_probe_count": len(cycle_detail_probes),
                "cycle_detail_probes": cycle_detail_probes,
                "publisher_identity_required": True,
                "payload_bytes_included": False,
                "publication_bodies_included": False,
                "private_payloads_included": False,
                "routes": routes,
            }
        )
    elif args.kind == "privacy_aggregate":
        probes = build_probe_records(args.privacy_actions, field="action", args=args)
        payload.update(
            {
                "cycle_digest_hex": args.cycle_digest_hex,
                "probe_count": len(probes),
                "passed_probe_count": len(probes),
                "source_event_probe_count": 1,
                "publish_due_probe_count": 1,
                "payload_bytes_included": False,
                "raw_metric_values_included": False,
                "private_payloads_included": False,
                "probes": probes,
            }
        )
    elif args.kind == "proof_token_issuance":
        probes = build_probe_records(
            REQUIRED_PROOF_TOKEN_ISSUANCE_ACTIONS,
            field="action",
            args=args,
        )
        payload.update(
            {
                "cycle_digest_hex": args.cycle_digest_hex,
                "probe_count": len(probes),
                "passed_probe_count": len(probes),
                "issuance_probe_count": len(probes),
                "payload_bytes_included": False,
                "proof_token_frames_included": False,
                "private_digest_keys_included": False,
                "response_bodies_included": False,
                "probes": probes,
            }
        )
    elif args.kind == "explorer":
        routes = build_explorer_routes(args)
        payload.update(
            {
                "cycle_digest_hex": args.cycle_digest_hex,
                "route_count": len(routes),
                "payload_bytes_included": False,
                "private_digest_keys_included": False,
                "routes": routes,
            }
        )
    return payload


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    validate_hex64(args.request_body_blake3, option="--request-body-blake3", errors=errors)
    validate_hex64(args.response_body_blake3, option="--response-body-blake3", errors=errors)
    validate_hex64(args.route_body_blake3, option="--route-body-blake3", errors=errors)
    if args.probe_status_code < 200 or args.probe_status_code > 299:
        errors.append("--probe-status-code must be a 2xx HTTP status code")
    if args.route_status_code < 200 or args.route_status_code > 299:
        errors.append("--route-status-code must be a 2xx HTTP status code")
    if args.kind in SOURCE_BATCH_DIGEST_KINDS:
        validate_hex64(
            args.source_batch_digest_hex,
            option="--source-batch-digest-hex",
            errors=errors,
        )
    if args.kind in CYCLE_DIGEST_KINDS:
        validate_hex64(
            args.cycle_digest_hex,
            option="--cycle-digest-hex",
            errors=errors,
        )
    if args.kind == "source_entry":
        args.source_kinds = validate_name_set(
            split_csv_values(args.source_kind),
            allowed=DEFAULT_REQUIRED_SOURCE_KINDS,
            option="--source-kind",
            errors=errors,
        )
    elif args.kind == "publication":
        args.publication_routes = validate_name_set(
            split_csv_values(args.publication_route),
            allowed=REQUIRED_PUBLICATION_ROUTES,
            option="--publication-route",
            errors=errors,
        )
        args.cycle_detail_probes = validate_name_set(
            split_csv_values(args.cycle_detail_probe),
            allowed=REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES,
            option="--cycle-detail-probe",
            errors=errors,
        )
        if (
            args.cycle_detail_probe_count is not None
            and args.cycle_detail_probes
            and args.cycle_detail_probe_count != len(args.cycle_detail_probes)
        ):
            errors.append(
                "--cycle-detail-probe-count must match "
                "--cycle-detail-probe inventory"
            )
    elif args.kind == "privacy_aggregate":
        args.privacy_actions = validate_name_set(
            split_csv_values(args.privacy_action),
            allowed=REQUIRED_PRIVACY_AGGREGATE_ACTIONS,
            option="--privacy-action",
            errors=errors,
        )
    elif args.kind == "explorer":
        args.explorer_routes = validate_name_set(
            split_csv_values(args.explorer_route),
            allowed=REQUIRED_EXPLORER_ROUTES,
            option="--explorer-route",
            errors=errors,
        )
    return errors


def validate_generated_payload(payload: dict[str, Any], args: argparse.Namespace) -> list[str]:
    """Validate the generated canary through the transparency gate contract."""

    kind, errors = validate_evidence_payload(payload)
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
        description="Build payload-free SoraFS SFM-4c transparency canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--source-batch-digest-hex")
    parser.add_argument("--cycle-digest-hex")
    parser.add_argument("--source-kind", action="append", default=[])
    parser.add_argument("--publication-route", action="append", default=[])
    parser.add_argument("--cycle-detail-probe", action="append", default=[])
    parser.add_argument("--privacy-action", action="append", default=[])
    parser.add_argument("--explorer-route", action="append", default=[])
    parser.add_argument("--probe-status-code", type=positive_int_arg, default=202)
    parser.add_argument("--route-status-code", type=positive_int_arg, default=200)
    parser.add_argument("--cycle-detail-probe-count", type=positive_int_arg, default=1)
    parser.add_argument("--request-body-blake3", default=DEFAULT_REQUEST_BODY_HASH)
    parser.add_argument("--response-body-blake3", default=DEFAULT_RESPONSE_BODY_HASH)
    parser.add_argument("--route-body-blake3", default=DEFAULT_ROUTE_BODY_HASH)
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
            "ERROR: SoraFS transparency canary inputs are incomplete:",
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
