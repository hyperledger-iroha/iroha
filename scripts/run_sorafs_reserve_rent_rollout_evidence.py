#!/usr/bin/env python3
"""Collect and verify SoraFS reserve/rent rollout evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_reserve_rent_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_BAKE_AGE_SECS,
    DEFAULT_MAX_LEDGER_AGE_SECS,
    DEFAULT_MAX_LIFECYCLE_LAG_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
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
    emit_runner_error_block,
    emit_runner_error_lines,
    emit_runner_exception,
    run_command_plan,
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_positive_int,
    validate_runner_preflight,
    write_runner_plan,
)


@dataclass(frozen=True)
class CommandPlan:
    """One reserve/rent rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




EVIDENCE_OPTIONS_BY_KIND = {
    "policy_config": "policy_config_evidence",
    "quote_matrix": "quote_matrix_evidence",
    "ledger_digest": "ledger_digest_evidence",
    "lifecycle_service": "lifecycle_service_evidence",
    "signed_routes": "signed_routes_evidence",
    "reserve_movement": "reserve_movement_evidence",
    "credit_line": "credit_line_evidence",
    "appeal_policy": "appeal_policy_evidence",
    "metrics_alerts": "metrics_alerts_evidence",
    "provider_bake": "provider_bake_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "policy_config": "--policy-config-evidence",
    "quote_matrix": "--quote-matrix-evidence",
    "ledger_digest": "--ledger-digest-evidence",
    "lifecycle_service": "--lifecycle-service-evidence",
    "signed_routes": "--signed-routes-evidence",
    "reserve_movement": "--reserve-movement-evidence",
    "credit_line": "--credit-line-evidence",
    "appeal_policy": "--appeal-policy-evidence",
    "metrics_alerts": "--metrics-alerts-evidence",
    "provider_bake": "--provider-bake-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied rollout evidence paths keyed by SFM-6 evidence kind."""

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
                f"missing {EVIDENCE_FLAGS_BY_KIND[kind]} for required `{kind}` rollout evidence"
            )

    for kind, paths in paths_by_kind.items():
        errors.extend(require_existing_files(paths, EVIDENCE_FLAGS_BY_KIND[kind], seen=seen_input_files))

    require_runner_positive_int(args, "now_unix", errors, allow_none=True)
    require_runner_non_negative_int(args, "max_ledger_age_secs", errors)
    require_runner_non_negative_int(args, "max_lifecycle_lag_secs", errors)
    require_runner_positive_int(args, "max_route_latency_ms", errors)
    require_runner_non_negative_int(args, "max_bake_age_secs", errors)
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
            "--max-ledger-age-secs",
            str(args.max_ledger_age_secs),
            "--max-lifecycle-lag-secs",
            str(args.max_lifecycle_lag_secs),
            "--max-route-latency-ms",
            str(args.max_route_latency_ms),
            "--max-bake-age-secs",
            str(args.max_bake_age_secs),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])

    return [CommandPlan("rollout_evidence_gate", summary_out, verifier_command)]


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    thresholds: dict[str, int] = {
        "max_ledger_age_secs": args.max_ledger_age_secs,
        "max_lifecycle_lag_secs": args.max_lifecycle_lag_secs,
        "max_route_latency_ms": args.max_route_latency_ms,
        "max_bake_age_secs": args.max_bake_age_secs,
    }
    if args.now_unix is not None:
        thresholds["now_unix"] = args.now_unix

    return {
        "schema": "sorafs.reserve_rent.rollout_evidence_collection_plan.v1",
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "required_kinds": list(args.required_kinds),
        "thresholds": thresholds,
        "external_evidence": {
            kind: [str(path) for path in paths]
            for kind, paths in evidence_paths_by_kind(args).items()
            if paths
        },
        "evidence_contract": {
            kind: {
                "schema": KIND_BY_NAME[kind].schema,
                "required_payload_fields": list(EVIDENCE_REQUIRED_FIELDS[kind]),
            }
            for kind in args.required_kinds
        },
        "steps": [
            {
                "label": step.label,
                "artifact": None if step.artifact is None else str(step.artifact),
                "command": step.command,
            }
            for step in plan
        ],
    }


def run_plan(plan: Sequence[CommandPlan], out_dir: Path) -> int:
    return run_command_plan(plan, out_dir)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Collect and verify SoraFS SFM-6 reserve/rent rollout evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_reserve_rent_rollout_evidence.py",
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
            "Defaults to every SFM-6 rollout kind."
        ),
    )
    parser.add_argument(
        "--policy-config-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free reserve policy config canary JSON.",
    )
    parser.add_argument(
        "--quote-matrix-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free quote matrix canary JSON.",
    )
    parser.add_argument(
        "--ledger-digest-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free ledger digest canary JSON.",
    )
    parser.add_argument(
        "--lifecycle-service-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free reserve lifecycle service canary JSON.",
    )
    parser.add_argument(
        "--signed-routes-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free signed route canary JSON.",
    )
    parser.add_argument(
        "--reserve-movement-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free reserve movement canary JSON.",
    )
    parser.add_argument(
        "--credit-line-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free credit-line accrual canary JSON.",
    )
    parser.add_argument(
        "--appeal-policy-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free appeal and policy-update canary JSON.",
    )
    parser.add_argument(
        "--metrics-alerts-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free metrics and alerts canary JSON.",
    )
    parser.add_argument(
        "--provider-bake-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free staged provider bake evidence JSON.",
    )
    parser.add_argument(
        "--governance-approval-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Governance approval and iroha_config binding evidence JSON.",
    )
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        help="Validator clock used for age checks. Defaults to verifier wall clock.",
    )
    parser.add_argument(
        "--max-ledger-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_LEDGER_AGE_SECS,
        help="Maximum allowed ledger digest age.",
    )
    parser.add_argument(
        "--max-lifecycle-lag-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_LIFECYCLE_LAG_SECS,
        help="Maximum allowed reserve lifecycle service lag.",
    )
    parser.add_argument(
        "--max-route-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_ROUTE_LATENCY_MS,
        help="Maximum allowed signed route latency.",
    )
    parser.add_argument(
        "--max-bake-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_BAKE_AGE_SECS,
        help="Maximum allowed staged provider bake evidence age.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the command plan JSON without running the verifier.",
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
    return parser.parse_args(expanded_args)


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1
    try:
        args.required_kinds = parse_required_evidence_kinds(
            args.require_kind,
            allowed_kinds=KIND_BY_NAME,
            default_required=DEFAULT_REQUIRED_KINDS,
        )
    except ValueError as error:
        emit_runner_exception(error)
        return 2

    errors = validate_inputs(args)
    if errors:
        emit_runner_error_block(
            "ERROR: SoraFS reserve/rent rollout evidence inputs are incomplete:",
            errors,
        )
        return 2

    plan = build_command_plan(args)
    if args.dry_run:
        plan_errors = write_runner_plan(plan_json(plan, args))
        if plan_errors:
            emit_runner_error_lines(plan_errors)
            return 2
        return 0
    return run_plan(plan, args.out_dir)


if __name__ == "__main__":
    raise SystemExit(main())
