#!/usr/bin/env python3
"""Collect and verify SoraFS gateway load rollout evidence."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_gateway_load_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_ERROR_RATE_BPS,
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_P95_LATENCY_MS,
    DEFAULT_MAX_P99_LATENCY_MS,
    DEFAULT_MIN_STAGING_DURATION_SECS,
    DEFAULT_MIN_STREAMS,
    DEFAULT_MIN_SUCCESS_RATE_BPS,
    DEFAULT_REQUIRED_KINDS,
    EVIDENCE_REQUIRED_FIELDS,
    KIND_BY_NAME,
    SUMMARY_SCHEMA,
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
from sorafs_runner_preflight import (  # noqa: E402
    emit_runner_error_lines,
    emit_runner_exception,
    require_existing_files,
    require_no_unrequired_evidence,
    require_runner_non_negative_int,
    require_runner_positive_int,
    run_command_plan,
    validate_runner_plan_steps,
    validate_runner_preflight,
    write_runner_plan,
)


PLAN_SCHEMA = "sorafs.gateway_load.rollout_evidence_collection_plan.v1"
PLAN_FIELDS = frozenset(
    {
        "schema",
        "verifier_summary_schema",
        "required_kinds",
        "thresholds",
        "external_evidence",
        "evidence_contract",
        "steps",
    }
)


@dataclass(frozen=True)
class CommandPlan:
    """One gateway load rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]


EVIDENCE_OPTIONS_BY_KIND = {
    "local_conformance": "local_conformance_evidence",
    "staging_load": "staging_load_evidence",
    "telemetry_slo": "telemetry_slo_evidence",
    "transport_scope": "transport_scope_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "local_conformance": "--local-conformance-evidence",
    "staging_load": "--staging-load-evidence",
    "telemetry_slo": "--telemetry-slo-evidence",
    "transport_scope": "--transport-scope-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied rollout evidence paths keyed by SF-5a evidence kind."""

    return {
        kind: list(getattr(args, option))
        for kind, option in EVIDENCE_OPTIONS_BY_KIND.items()
    }


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(args, summary_filename="rollout-summary.json")
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    paths_by_kind = evidence_paths_by_kind(args)
    for kind in args.required_kinds:
        paths = paths_by_kind[kind]
        if not paths:
            errors.append(
                "missing required rollout evidence input"
            )
    require_no_unrequired_evidence(
        paths_by_kind,
        args.required_kinds,
        errors,
        diagnostic="rollout evidence supplied for unrequired kind",
    )

    for kind, paths in paths_by_kind.items():
        errors.extend(
            require_existing_files(paths, EVIDENCE_FLAGS_BY_KIND[kind], seen=seen_input_files)
        )

    require_runner_positive_int(args, "now_unix", errors, allow_none=True)
    require_runner_non_negative_int(args, "max_evidence_age_secs", errors)
    require_runner_positive_int(args, "min_staging_duration_secs", errors)
    require_runner_positive_int(args, "min_streams", errors)
    require_runner_positive_int(args, "min_success_rate_bps", errors)
    require_runner_non_negative_int(args, "max_error_rate_bps", errors)
    require_runner_positive_int(args, "max_p95_latency_ms", errors)
    require_runner_positive_int(args, "max_p99_latency_ms", errors)
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    summary_out = args.summary_out or args.out_dir / "rollout-summary.json"
    verifier_command = [
        sys.executable,
        str(args.verifier),
    ]
    for paths in evidence_paths_by_kind(args).values():
        for path in paths:
            verifier_command.extend(["--evidence", str(path)])
    for required_kind in args.required_kinds:
        verifier_command.extend(["--require-kind", required_kind])
    verifier_command.extend(
        [
            "--summary-out",
            str(summary_out),
            "--max-evidence-age-secs",
            str(args.max_evidence_age_secs),
            "--min-staging-duration-secs",
            str(args.min_staging_duration_secs),
            "--min-streams",
            str(args.min_streams),
            "--min-success-rate-bps",
            str(args.min_success_rate_bps),
            "--max-error-rate-bps",
            str(args.max_error_rate_bps),
            "--max-p95-latency-ms",
            str(args.max_p95_latency_ms),
            "--max-p99-latency-ms",
            str(args.max_p99_latency_ms),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])

    return [CommandPlan("rollout_evidence_gate", summary_out, verifier_command)]


def threshold_values(args: argparse.Namespace) -> dict[str, int]:
    """Return threshold values rendered in dry-run plans."""

    thresholds: dict[str, int] = {
        "max_evidence_age_secs": args.max_evidence_age_secs,
        "min_staging_duration_secs": args.min_staging_duration_secs,
        "min_streams": args.min_streams,
        "min_success_rate_bps": args.min_success_rate_bps,
        "max_error_rate_bps": args.max_error_rate_bps,
        "max_p95_latency_ms": args.max_p95_latency_ms,
        "max_p99_latency_ms": args.max_p99_latency_ms,
    }
    if args.now_unix is not None:
        thresholds["now_unix"] = args.now_unix
    return thresholds


def external_evidence(args: argparse.Namespace) -> dict[str, list[str]]:
    """Return reviewed external evidence paths rendered in dry-run plans."""

    return {
        kind: [str(path) for path in paths]
        for kind, paths in evidence_paths_by_kind(args).items()
        if paths
    }


def evidence_contract(args: argparse.Namespace) -> dict[str, dict[str, object]]:
    """Return the checker-backed evidence contract rendered in dry-run plans."""

    return {
        kind: {
            "schema": KIND_BY_NAME[kind].schema,
            "required_payload_fields": list(EVIDENCE_REQUIRED_FIELDS[kind]),
        }
        for kind in args.required_kinds
    }


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    return {
        "schema": PLAN_SCHEMA,
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "required_kinds": list(args.required_kinds),
        "thresholds": threshold_values(args),
        "external_evidence": external_evidence(args),
        "evidence_contract": evidence_contract(args),
        "steps": [
            {
                "label": step.label,
                "artifact": None if step.artifact is None else str(step.artifact),
                "command": step.command,
            }
            for step in plan
        ],
    }


def validate_plan_json(
    rendered: object,
    plan: Sequence[CommandPlan],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the gateway-load collection-plan envelope before use."""

    errors: list[str] = []
    if not isinstance(rendered, Mapping):
        return ["gateway load rollout runner plan must be an object"]
    if set(rendered) != PLAN_FIELDS:
        errors.append(
            "gateway load rollout runner plan fields must match the schema-closed contract"
        )
    if rendered.get("schema") != PLAN_SCHEMA:
        errors.append("gateway load rollout runner plan schema must match the contract")
    if rendered.get("verifier_summary_schema") != SUMMARY_SCHEMA:
        errors.append(
            "gateway load rollout runner plan verifier schema must match checker summary"
        )
    if rendered.get("required_kinds") != list(args.required_kinds):
        errors.append("gateway load rollout runner plan required_kinds must match args")
    if rendered.get("thresholds") != threshold_values(args):
        errors.append("gateway load rollout runner plan thresholds must match args")
    if rendered.get("external_evidence") != external_evidence(args):
        errors.append("gateway load rollout runner plan external_evidence must match args")
    if rendered.get("evidence_contract") != evidence_contract(args):
        errors.append(
            "gateway load rollout runner plan evidence_contract must match checker fields"
        )
    errors.extend(validate_runner_plan_steps(rendered, plan))
    return errors


def run_plan(plan: Sequence[CommandPlan], out_dir: Path) -> int:
    return run_command_plan(plan, out_dir)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Collect and verify SoraFS SF-5a gateway load rollout evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_gateway_load_rollout_evidence.py",
        help="Rollout evidence verifier script path.",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        required=True,
        help="Directory where the verifier summary will be written.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional verifier summary path. Defaults under --out-dir.",
    )
    parser.add_argument(
        "--require-kind",
        action="append",
        default=[],
        help=(
            "Required evidence kind, or comma-separated kinds. "
            "Defaults to every SF-5a gateway load rollout kind."
        ),
    )
    for kind, flag in EVIDENCE_FLAGS_BY_KIND.items():
        parser.add_argument(
            flag,
            dest=EVIDENCE_OPTIONS_BY_KIND[kind],
            action="append",
            type=Path,
            default=[],
            help=f"Existing JSON artifact for `{kind}` rollout evidence.",
        )
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )
    parser.add_argument(
        "--min-staging-duration-secs",
        type=positive_int_arg,
        default=DEFAULT_MIN_STAGING_DURATION_SECS,
    )
    parser.add_argument("--min-streams", type=positive_int_arg, default=DEFAULT_MIN_STREAMS)
    parser.add_argument(
        "--min-success-rate-bps",
        type=positive_int_arg,
        default=DEFAULT_MIN_SUCCESS_RATE_BPS,
    )
    parser.add_argument(
        "--max-error-rate-bps",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_ERROR_RATE_BPS,
    )
    parser.add_argument(
        "--max-p95-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_P95_LATENCY_MS,
    )
    parser.add_argument(
        "--max-p99-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_P99_LATENCY_MS,
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the verifier command plan without executing it.",
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
    args = parser.parse_args(expanded_args)
    try:
        args.required_kinds = parse_required_evidence_kinds(
            args.require_kind,
            allowed_kinds=KIND_BY_NAME,
            default_required=DEFAULT_REQUIRED_KINDS,
        )
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
    return args


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1
    errors = validate_inputs(args)
    if errors:
        emit_runner_error_lines(errors)
        return 2

    plan = build_command_plan(args)
    rendered_plan = plan_json(plan, args)
    plan_errors = validate_plan_json(rendered_plan, plan, args)
    if plan_errors:
        emit_runner_error_lines(plan_errors)
        return 2
    if args.dry_run:
        plan_errors = write_runner_plan(rendered_plan)
        if plan_errors:
            emit_runner_error_lines(plan_errors)
            return 2
        return 0

    return run_plan(plan, args.out_dir)


if __name__ == "__main__":
    raise SystemExit(main())
