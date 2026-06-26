#!/usr/bin/env python3
"""Collect and verify SoraFS appeal finance rollout evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_appeal_finance_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_CANARY_AGE_SECS,
    DEFAULT_MAX_DASHBOARD_AGE_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MAX_SETTLEMENT_LAG_SECS,
    DEFAULT_MIN_PEERS,
    DEFAULT_REQUIRED_KINDS,
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
    run_command_plan,
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_positive_int,
    validate_runner_preflight,
    write_runner_plan,
)


@dataclass(frozen=True)
class CommandPlan:
    """One appeal finance rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




EVIDENCE_OPTIONS_BY_KIND = {
    "pricing_config": "pricing_config_evidence",
    "quote_api": "quote_api_evidence",
    "deposit_lifecycle": "deposit_lifecycle_evidence",
    "settlement_execution": "settlement_execution_evidence",
    "settlement_submitter": "settlement_submitter_evidence",
    "moderation_worker": "moderation_worker_evidence",
    "governance_dag_publication": "governance_dag_publication_evidence",
    "dashboard_metrics": "dashboard_metrics_evidence",
    "multi_peer_reconciliation": "multi_peer_reconciliation_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "pricing_config": "--pricing-config-evidence",
    "quote_api": "--quote-api-evidence",
    "deposit_lifecycle": "--deposit-lifecycle-evidence",
    "settlement_execution": "--settlement-execution-evidence",
    "settlement_submitter": "--settlement-submitter-evidence",
    "moderation_worker": "--moderation-worker-evidence",
    "governance_dag_publication": "--governance-dag-publication-evidence",
    "dashboard_metrics": "--dashboard-metrics-evidence",
    "multi_peer_reconciliation": "--multi-peer-reconciliation-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied rollout evidence paths keyed by SFM-4b2 evidence kind."""

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
    require_runner_non_negative_int(args, "max_canary_age_secs", errors)
    require_runner_non_negative_int(args, "max_dashboard_age_secs", errors)
    require_runner_positive_int(args, "max_route_latency_ms", errors)
    require_runner_non_negative_int(args, "max_settlement_lag_secs", errors)
    require_runner_positive_int(args, "min_peers", errors)
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
            "--max-canary-age-secs",
            str(args.max_canary_age_secs),
            "--max-dashboard-age-secs",
            str(args.max_dashboard_age_secs),
            "--max-route-latency-ms",
            str(args.max_route_latency_ms),
            "--max-settlement-lag-secs",
            str(args.max_settlement_lag_secs),
            "--min-peers",
            str(args.min_peers),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])

    return [CommandPlan("rollout_evidence_gate", summary_out, verifier_command)]


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    thresholds: dict[str, int] = {
        "max_canary_age_secs": args.max_canary_age_secs,
        "max_dashboard_age_secs": args.max_dashboard_age_secs,
        "max_route_latency_ms": args.max_route_latency_ms,
        "max_settlement_lag_secs": args.max_settlement_lag_secs,
        "min_peers": args.min_peers,
    }
    if args.now_unix is not None:
        thresholds["now_unix"] = args.now_unix

    return {
        "schema": "sorafs.appeal_finance.rollout_evidence_collection_plan.v1",
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "required_kinds": list(args.required_kinds),
        "thresholds": thresholds,
        "external_evidence": {
            kind: [str(path) for path in paths]
            for kind, paths in evidence_paths_by_kind(args).items()
            if paths
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
        description="Collect and verify SoraFS SFM-4b2 appeal finance rollout evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_appeal_finance_rollout_evidence.py",
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
            "Defaults to every SFM-4b2 rollout kind."
        ),
    )
    parser.add_argument(
        "--pricing-config-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free pricing/config status canary JSON.",
    )
    parser.add_argument(
        "--quote-api-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free quote/settle/disburse API canary JSON.",
    )
    parser.add_argument(
        "--deposit-lifecycle-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free native asset-lock deposit lifecycle canary JSON.",
    )
    parser.add_argument(
        "--settlement-execution-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free settlement execution and reconciliation canary JSON.",
    )
    parser.add_argument(
        "--settlement-submitter-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free configured-signer submitter canary JSON.",
    )
    parser.add_argument(
        "--moderation-worker-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free moderation-derived settlement worker canary JSON.",
    )
    parser.add_argument(
        "--governance-dag-publication-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free appeal finance Governance DAG publication canary JSON.",
    )
    parser.add_argument(
        "--dashboard-metrics-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free hosted dashboard and alert evidence JSON.",
    )
    parser.add_argument(
        "--multi-peer-reconciliation-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free multi-peer ledger reconciliation evidence JSON.",
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
        "--max-canary-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_CANARY_AGE_SECS,
        help="Maximum allowed deployed canary evidence age.",
    )
    parser.add_argument(
        "--max-dashboard-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_DASHBOARD_AGE_SECS,
        help="Maximum allowed hosted dashboard evidence age.",
    )
    parser.add_argument(
        "--max-route-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_ROUTE_LATENCY_MS,
        help="Maximum allowed route latency.",
    )
    parser.add_argument(
        "--max-settlement-lag-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SETTLEMENT_LAG_SECS,
        help="Maximum allowed settlement submitter/worker lag.",
    )
    parser.add_argument(
        "--min-peers",
        type=positive_int_arg,
        default=DEFAULT_MIN_PEERS,
        help="Minimum peers required for multi-peer reconciliation evidence.",
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
        emit_runner_error_lines((str(error),))
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
        emit_runner_error_lines((str(error),))
        return 2

    errors = validate_inputs(args)
    if errors:
        emit_runner_error_block(
            "ERROR: SoraFS appeal finance rollout evidence inputs are incomplete:",
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
