#!/usr/bin/env python3
"""Build payload-free SoraFS reputation rollout canary artifacts."""

from __future__ import annotations

import argparse
import json
import os
import secrets
import sys
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_reputation_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_INGEST_LAG_SECS,
    DEFAULT_MAX_SNAPSHOT_AGE_SECS,
    KIND_BY_NAME,
    LoadedEvidence,
    SNAPSHOT_ANCHOR_KINDS,
    SNAPSHOT_BOUND_KINDS,
    validate_evidence_set,
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
HEX32_LEN = 32
HEX64_LEN = 64
DEFAULT_PROOF_SIBLING_HEX = "33" * 32


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


def validate_hex(value: str | None, *, option: str, length: int, errors: list[str]) -> None:
    """Validate an exact lowercase hex string."""

    if (
        not isinstance(value, str)
        or len(value) != length
        or any(character not in "0123456789abcdef" for character in value)
    ):
        errors.append(f"{option} must be exact lowercase hex length {length}")


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
    """Build fields shared by reputation canary payloads."""

    return {
        "schema": KIND_BY_NAME[args.kind].schema,
        "generated_at_unix": args.generated_at_unix,
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
    }


def snapshot_fields(args: argparse.Namespace) -> dict[str, Any]:
    """Return the shared snapshot binding fields."""

    return {
        "snapshot_id_hex": args.snapshot_id_hex,
        "merkle_root_hex": args.merkle_root_hex,
    }


def build_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a payload-free reputation rollout canary payload."""

    payload = common_payload(args)
    payload.update(snapshot_fields(args))
    if args.kind in SNAPSHOT_ANCHOR_KINDS:
        payload.update(
            {
                "status": "accepted",
                "provider_count": args.provider_count,
            }
        )
    elif args.kind == "provider":
        payload.update(
            {
                "provider": {
                    "provider_id": args.provider_id,
                    "score_bps": args.provider_score_bps,
                },
                "proof": {
                    "provider_id": args.provider_id,
                    "leaf_index": args.leaf_index,
                    "siblings_hex": args.sibling_hex,
                },
            }
        )
    elif args.kind == "events":
        payload.update(
            {
                "since": args.since,
                "limit": args.event_count,
                "count": args.event_count,
                "next_since": args.next_since,
                "events": [
                    {
                        "version": 1,
                        "sequence": args.next_since,
                        "snapshot_id_hex": args.snapshot_id_hex,
                        "generated_at_unix": args.generated_at_unix,
                        "merkle_root_hex": args.merkle_root_hex,
                        "provider_count": args.provider_count,
                    }
                ],
            }
        )
    elif args.kind == "verify":
        payload.update(
            {
                "valid": True,
                "proof_verified": True,
                "provider_count": args.provider_count,
                "provider_id": args.provider_id,
                "provider_score_bps": args.provider_score_bps,
            }
        )
    elif args.kind == "metrics":
        payload.update(
            {
                "status": "passed",
                "metrics_scrape_success": True,
                "provider_count": args.provider_count,
                "snapshot_age_seconds": args.snapshot_age_seconds,
                "ingest_lag_seconds": args.ingest_lag_seconds,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "transport":
        payload.update(
            {
                "status": "passed",
                "sse_connected": True,
                "websocket_connected": True,
                "sse_event_count": args.sse_event_count,
                "websocket_event_count": args.websocket_event_count,
                "response_bodies_included": False,
            }
        )
    elif args.kind == "consumption":
        payload.update(
            {
                "status": "passed",
                "routing_score_consumed": True,
                "routing_weight_changed": True,
                "incentive_score_consumed": True,
                "provider_count": args.provider_count,
                "raw_provider_records_included": False,
            }
        )
    return payload


def validate_thresholds(args: argparse.Namespace, errors: list[str]) -> None:
    """Validate threshold-bound facts before payload construction."""

    if args.snapshot_age_seconds > DEFAULT_MAX_SNAPSHOT_AGE_SECS:
        errors.append(
            f"--snapshot-age-seconds must be <= {DEFAULT_MAX_SNAPSHOT_AGE_SECS}"
        )
    if args.ingest_lag_seconds > DEFAULT_MAX_INGEST_LAG_SECS:
        errors.append(f"--ingest-lag-seconds must be <= {DEFAULT_MAX_INGEST_LAG_SECS}")
    if args.next_since <= args.since:
        errors.append("--next-since must be greater than --since")


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate reviewed operator inputs before building the canary."""

    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_canonical_string(args.deployment_id, label="--deployment-id", errors=errors)
    validate_canonical_string(args.environment, label="--environment", errors=errors)
    validate_canonical_string(args.provider_id, label="--provider-id", errors=errors)
    validate_hex(args.snapshot_id_hex, option="--snapshot-id-hex", length=HEX32_LEN, errors=errors)
    validate_hex(args.merkle_root_hex, option="--merkle-root-hex", length=HEX64_LEN, errors=errors)
    validate_thresholds(args, errors)
    if args.provider_score_bps > 10_000:
        errors.append("--provider-score-bps must be <= 10000")
    if args.kind == "provider":
        if not args.sibling_hex:
            errors.append("--sibling-hex is required for provider")
        seen_siblings: set[str] = set()
        for sibling in args.sibling_hex:
            validate_hex(sibling, option="--sibling-hex", length=HEX64_LEN, errors=errors)
            if isinstance(sibling, str):
                normalized = sibling.lower()
                if normalized in seen_siblings:
                    errors.append("duplicate --sibling-hex")
                seen_siblings.add(normalized)
    return errors


def loaded(kind: str, payload: dict[str, Any], path: Path) -> LoadedEvidence:
    """Wrap generated payload for reputation gate validation."""

    return LoadedEvidence(kind, path, payload, "ab" * 32)


def anchor_payload(args: argparse.Namespace, kind: str) -> dict[str, Any]:
    """Build a matching publish/latest anchor payload for bound canaries."""

    anchor_args = argparse.Namespace(**vars(args))
    anchor_args.kind = kind
    return build_payload(anchor_args)


def provider_anchor_payload(args: argparse.Namespace) -> dict[str, Any]:
    """Build a matching provider proof anchor for reputation gate validation."""

    provider_args = argparse.Namespace(**vars(args))
    provider_args.kind = "provider"
    provider_args.sibling_hex = args.sibling_hex or [DEFAULT_PROOF_SIBLING_HEX]
    return build_payload(provider_args)


def validate_generated_payload(
    payload: dict[str, Any],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the generated canary through the reputation gate contract."""

    payloads = {
        "publish": anchor_payload(args, "publish"),
        "latest": anchor_payload(args, "latest"),
        "provider": provider_anchor_payload(args),
        args.kind: payload,
    }
    evidence = [
        loaded(kind, item, args.out.with_name(f"{kind}.json"))
        for kind, item in payloads.items()
    ]
    required_kinds = tuple(payloads)
    summary = validate_evidence_set(
        evidence,
        required_kinds=required_kinds,
        required_providers=(args.provider_id,) if args.kind == "provider" else (),
        now_unix=args.now_unix or args.generated_at_unix,
        max_snapshot_age_secs=DEFAULT_MAX_SNAPSHOT_AGE_SECS,
        max_ingest_lag_secs=DEFAULT_MAX_INGEST_LAG_SECS,
    )
    errors: list[str] = []
    if summary["status"] != "ready":
        errors.extend(summary.get("errors", []))
        for row in summary.get("required", {}).values():
            errors.extend(row.get("errors", []))
            for artifact in row.get("artifacts", []):
                errors.extend(artifact.get("errors", []))
    return sorted(set(errors))


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
        description="Build payload-free SoraFS SFM-3 reputation canary JSON.",
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", type=positive_int_arg, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument("--snapshot-id-hex", required=True)
    parser.add_argument("--merkle-root-hex", required=True)
    parser.add_argument("--provider-id", default="provider-a")
    parser.add_argument("--provider-count", type=positive_int_arg, default=2)
    parser.add_argument("--provider-score-bps", type=non_negative_int_arg, default=9400)
    parser.add_argument("--leaf-index", type=positive_int_arg, default=1)
    parser.add_argument("--sibling-hex", action="append", default=[])
    parser.add_argument("--since", type=non_negative_int_arg, default=0)
    parser.add_argument("--next-since", type=positive_int_arg, default=1)
    parser.add_argument("--event-count", type=positive_int_arg, default=1)
    parser.add_argument("--snapshot-age-seconds", type=non_negative_int_arg, default=120)
    parser.add_argument("--ingest-lag-seconds", type=non_negative_int_arg, default=60)
    parser.add_argument("--sse-event-count", type=positive_int_arg, default=1)
    parser.add_argument("--websocket-event-count", type=positive_int_arg, default=1)
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
            "ERROR: SoraFS reputation canary inputs are incomplete:",
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
