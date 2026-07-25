#!/usr/bin/env python3
"""Canonicalize an observed gateway-compliance probe artifact."""

from __future__ import annotations

import argparse
import copy
import json
import os
import secrets
import sys
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_gateway_compliance_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_RELOAD_LATENCY_MS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_CATALOG_CHANGES,
    DEFAULT_MIN_CATALOG_ENTRIES,
    DEFAULT_MIN_GATEWAYS,
    DEFAULT_MIN_HONEY_PROBES,
    KIND_BY_NAME,
    MAX_EVIDENCE_BYTES,
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
from sorafs_evidence_json import (  # noqa: E402
    load_evidence_json_with_sha256_or_record_error,
)
from sorafs_path_identity import (  # noqa: E402
    error_diagnostic_label,
    path_diagnostic_label,
    resolve_path_identity,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    positive_int_arg,
)


CANARY_KINDS = tuple(KIND_BY_NAME)


def validate_output_path(path: Path, errors: list[str]) -> None:
    """Reject unsafe output targets before writing an artifact."""

    try:
        if path.is_symlink():
            errors.append(f"--out `{path_diagnostic_label(path)}` must not be a symlink")
            return
        if path.exists() and path.is_dir():
            errors.append(
                f"--out `{path_diagnostic_label(path)}` must not be a directory"
            )
            return
    except (OSError, RuntimeError):
        errors.append(f"--out `{path_diagnostic_label(path)}` cannot be inspected")
        return
    validate_checker_output_parent(path, errors, label="--out")


def validation_options(args: argparse.Namespace) -> ValidationOptions:
    return ValidationOptions(
        now_unix=args.now_unix,
        max_evidence_age_secs=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        max_route_latency_ms=DEFAULT_MAX_ROUTE_LATENCY_MS,
        max_reload_latency_ms=DEFAULT_MAX_RELOAD_LATENCY_MS,
        min_gateways=DEFAULT_MIN_GATEWAYS,
        min_catalog_entries=DEFAULT_MIN_CATALOG_ENTRIES,
        min_catalog_changes=DEFAULT_MIN_CATALOG_CHANGES,
        min_honey_probes=DEFAULT_MIN_HONEY_PROBES,
    )


def load_probe_artifact(
    args: argparse.Namespace, errors: list[str]
) -> dict[str, Any] | None:
    """Load one bounded observed probe artifact without following symlinks."""

    loaded = load_evidence_json_with_sha256_or_record_error(
        args.probe_artifact, MAX_EVIDENCE_BYTES, errors
    )
    if loaded is None:
        return None
    payload, _digest = loaded
    return payload


def validate_distinct_paths(args: argparse.Namespace, errors: list[str]) -> None:
    """Prevent an output replacement from overwriting the input probe."""

    input_errors: list[str] = []
    output_errors: list[str] = []
    input_identity = resolve_path_identity(
        args.probe_artifact, input_errors, label="--probe-artifact"
    )
    output_identity = resolve_path_identity(
        args.out, output_errors, label="--out"
    )
    errors.extend(input_errors)
    errors.extend(output_errors)
    if (
        input_identity is not None
        and output_identity is not None
        and input_identity == output_identity
    ):
        errors.append("--out must not replace --probe-artifact")


def canonical_payload(
    payload: dict[str, Any], *, non_production_fixture: bool
) -> dict[str, Any]:
    """Return a sorted-write payload without manufacturing observation claims."""

    canonical = copy.deepcopy(payload)
    if non_production_fixture:
        canonical["status"] = "non_production"
        canonical["evidence_scope"] = "non_production_fixture"
    return canonical


def validate_inputs(
    args: argparse.Namespace, payload: dict[str, Any] | None
) -> list[str]:
    errors: list[str] = []
    validate_output_path(args.out, errors)
    validate_distinct_paths(args, errors)
    if payload is None:
        return errors
    kind, validation_errors = validate_evidence_payload(
        payload,
        validation_options(args),
        require_production=not args.non_production_fixture,
    )
    errors.extend(validation_errors)
    if kind != args.kind:
        errors.append(f"--probe-artifact must validate as {args.kind}")
    return errors


def write_payload_atomic(path: Path, payload: dict[str, Any]) -> list[str]:
    """Write canonical JSON atomically with a private temporary file."""

    text = json.dumps(payload, indent=2, sort_keys=True, allow_nan=False) + "\n"
    parent = path.parent
    try:
        parent.mkdir(parents=True, exist_ok=True)
    except (OSError, RuntimeError) as error:
        parent_label = path_diagnostic_label(parent)
        return [
            f"--out parent `{parent_label}` cannot be created: "
            f"{error_diagnostic_label(error, path_label=parent_label)}"
        ]
    tmp_path = parent / (
        f".{path.name}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    )
    fd = -1
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        if nofollow := getattr(os, "O_NOFOLLOW", 0):
            flags |= nofollow
        fd = os.open(tmp_path, flags, 0o600)
        write_all_checker_summary_bytes(fd, text.encode("utf-8"))
        os.fsync(fd)
        os.close(fd)
        fd = -1
        os.replace(tmp_path, path)
        return fsync_checker_output_parent(path, label="--out")
    except (OSError, RuntimeError) as error:
        path_label = path_diagnostic_label(path)
        try:
            if fd >= 0:
                os.close(fd)
        finally:
            try:
                tmp_path.unlink()
            except (FileNotFoundError, OSError, RuntimeError):
                pass
        return [
            f"--out `{path_label}` cannot be written: "
            f"{error_diagnostic_label(error, path_label=path_label)}"
        ]


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description=(
            "Canonicalize one observed, payload-free SoraFS gateway-compliance "
            "probe artifact. Production claims are never synthesized."
        )
    )
    parser.add_argument("--kind", choices=CANARY_KINDS, required=True)
    parser.add_argument("--probe-artifact", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--now-unix", type=positive_int_arg, required=True)
    parser.add_argument(
        "--non-production-fixture",
        action="store_true",
        help=(
            "Mark copied probe data as a non-production fixture. Such output is "
            "rejected by the release checker."
        ),
    )
    try:
        expanded = expand_response_args(
            sys.argv[1:] if argv is None else argv, parser
        )
        return parser.parse_args(expanded)
    except ValueError as error:
        emit_checker_exception(error)
        raise SystemExit(2) from error


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1

    load_errors: list[str] = []
    loaded = load_probe_artifact(args, load_errors)
    if loaded is None:
        emit_checker_error_block(
            "ERROR: gateway-compliance probe artifact could not be loaded:",
            load_errors,
        )
        return 2
    payload = canonical_payload(
        loaded, non_production_fixture=args.non_production_fixture
    )
    errors = load_errors + validate_inputs(args, payload)
    if errors:
        emit_checker_error_block(
            "ERROR: gateway-compliance probe artifact is not canonical:",
            errors,
        )
        return 2
    write_errors = write_payload_atomic(args.out, payload)
    if write_errors:
        emit_checker_error_lines(write_errors)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
