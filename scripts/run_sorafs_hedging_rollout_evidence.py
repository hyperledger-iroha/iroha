#!/usr/bin/env python3
"""Collect and verify SoraFS hedging and billing rollout evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_hedging_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_CYCLE_AGE_SECS,
    DEFAULT_MAX_DIVERGENCE_BPS,
    DEFAULT_MAX_FEED_LAG_SECS,
    DEFAULT_MIN_BILLING_CYCLES,
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
    """One hedging rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




EVIDENCE_OPTIONS_BY_KIND = {
    "feed_collector": "feed_collector_evidence",
    "reference_price": "reference_price_evidence",
    "billing_cycle": "billing_cycle_evidence",
    "statement_publication": "statement_publication_evidence",
    "reconciliation": "reconciliation_evidence",
    "metrics_alerts": "metrics_alerts_evidence",
    "native_bridge_release": "native_bridge_release_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "feed_collector": "--feed-collector-evidence",
    "reference_price": "--reference-price-evidence",
    "billing_cycle": "--billing-cycle-evidence",
    "statement_publication": "--statement-publication-evidence",
    "reconciliation": "--reconciliation-evidence",
    "metrics_alerts": "--metrics-alerts-evidence",
    "native_bridge_release": "--native-bridge-release-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied rollout evidence paths keyed by SFM-5 evidence kind."""

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

    min_billing_cycles_valid = require_runner_positive_int(
        args,
        "min_billing_cycles",
        errors,
    )
    if min_billing_cycles_valid and "billing_cycle" in args.required_kinds:
        cycle_count = len(paths_by_kind["billing_cycle"])
        if cycle_count < args.min_billing_cycles:
            errors.append(
                "--billing-cycle-evidence count must be at least "
                f"--min-billing-cycles ({args.min_billing_cycles})"
            )

    for kind, paths in paths_by_kind.items():
        errors.extend(require_existing_files(paths, EVIDENCE_FLAGS_BY_KIND[kind], seen=seen_input_files))

    require_runner_positive_int(args, "now_unix", errors, allow_none=True)
    require_runner_non_negative_int(args, "max_feed_lag_secs", errors)
    require_runner_non_negative_int(args, "max_cycle_age_secs", errors)
    require_runner_non_negative_int(args, "max_divergence_bps", errors)
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
            "--max-feed-lag-secs",
            str(args.max_feed_lag_secs),
            "--max-cycle-age-secs",
            str(args.max_cycle_age_secs),
            "--max-divergence-bps",
            str(args.max_divergence_bps),
            "--min-billing-cycles",
            str(args.min_billing_cycles),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])

    return [CommandPlan("rollout_evidence_gate", summary_out, verifier_command)]


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    thresholds: dict[str, int] = {
        "max_feed_lag_secs": args.max_feed_lag_secs,
        "max_cycle_age_secs": args.max_cycle_age_secs,
        "max_divergence_bps": args.max_divergence_bps,
        "min_billing_cycles": args.min_billing_cycles,
    }
    if args.now_unix is not None:
        thresholds["now_unix"] = args.now_unix

    return {
        "schema": "sorafs.hedging_billing.rollout_evidence_collection_plan.v1",
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
        description="Collect and verify SoraFS SFM-5 hedging and billing rollout evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_hedging_rollout_evidence.py",
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
            "Defaults to every SFM-5 rollout kind."
        ),
    )
    parser.add_argument(
        "--feed-collector-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free feed collector canary JSON.",
    )
    parser.add_argument(
        "--reference-price-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free reference price canary JSON.",
    )
    parser.add_argument(
        "--billing-cycle-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free staged billing cycle canary JSON. Repeat per cycle.",
    )
    parser.add_argument(
        "--statement-publication-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free billing statement publication canary JSON.",
    )
    parser.add_argument(
        "--reconciliation-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free billing reconciliation canary JSON.",
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
        "--native-bridge-release-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Native bridge release and SDK wrapper verification JSON.",
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
        "--max-feed-lag-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_FEED_LAG_SECS,
        help="Maximum allowed feed or reference decision lag.",
    )
    parser.add_argument(
        "--max-cycle-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_CYCLE_AGE_SECS,
        help="Maximum allowed staged billing cycle age.",
    )
    parser.add_argument(
        "--max-divergence-bps",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_DIVERGENCE_BPS,
        help="Maximum allowed feed divergence in basis points.",
    )
    parser.add_argument(
        "--min-billing-cycles",
        type=positive_int_arg,
        default=DEFAULT_MIN_BILLING_CYCLES,
        help="Minimum distinct staged billing cycles required by the gate.",
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
            "ERROR: SoraFS hedging/billing rollout evidence inputs are incomplete:",
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
