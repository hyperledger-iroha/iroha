#!/usr/bin/env python3
"""Run the aggregate SoraFS production-readiness gate."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_production_readiness import (  # noqa: E402
    DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    DEFAULT_REQUIRED_GATES,
    GATE_BY_NAME,
    SUMMARY_SCHEMA,
)
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_gates,
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
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_positive_int,
    run_command_plan,
    validate_runner_preflight,
    write_runner_plan,
)


@dataclass(frozen=True)
class CommandPlan:
    """One aggregate readiness command."""

    label: str
    artifact: Path | None
    command: list[str]


SUMMARY_OPTIONS_BY_GATE = {
    "ai_prescreen": "ai_prescreen_summary",
    "appeal_finance": "appeal_finance_summary",
    "gateway_compliance": "gateway_compliance_summary",
    "gateway_load": "gateway_load_summary",
    "governance_dag": "governance_dag_summary",
    "hedging_billing": "hedging_billing_summary",
    "moderation_panel": "moderation_panel_summary",
    "orderbook": "orderbook_summary",
    "pdp": "pdp_summary",
    "pop_credentials": "pop_credentials_summary",
    "por": "por_summary",
    "potr": "potr_summary",
    "reference_sdk_release": "reference_sdk_release_summary",
    "repair": "repair_summary",
    "reputation": "reputation_summary",
    "reserve_rent": "reserve_rent_summary",
    "transparency": "transparency_summary",
}

SUMMARY_FLAGS_BY_GATE = {
    "ai_prescreen": "--ai-prescreen-summary",
    "appeal_finance": "--appeal-finance-summary",
    "gateway_compliance": "--gateway-compliance-summary",
    "gateway_load": "--gateway-load-summary",
    "governance_dag": "--governance-dag-summary",
    "hedging_billing": "--hedging-billing-summary",
    "moderation_panel": "--moderation-panel-summary",
    "orderbook": "--orderbook-summary",
    "pdp": "--pdp-summary",
    "pop_credentials": "--pop-credentials-summary",
    "por": "--por-summary",
    "potr": "--potr-summary",
    "reference_sdk_release": "--reference-sdk-release-summary",
    "repair": "--repair-summary",
    "reputation": "--reputation-summary",
    "reserve_rent": "--reserve-rent-summary",
    "transparency": "--transparency-summary",
}


def summary_paths_by_gate(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied lane summary paths keyed by gate name."""

    return {
        gate: list(getattr(args, option))
        for gate, option in SUMMARY_OPTIONS_BY_GATE.items()
    }


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate runner inputs before command-plan construction."""

    errors = validate_runner_preflight(
        args,
        summary_filename="sorafs-production-readiness-summary.json",
    )
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    paths_by_gate = summary_paths_by_gate(args)
    for gate in args.required_gates:
        paths = paths_by_gate[gate]
        if not paths:
            errors.append(
                "missing required production readiness summary input"
            )
    required_gate_names = set(args.required_gates)
    for gate, paths in paths_by_gate.items():
        if paths and gate not in required_gate_names:
            errors.append(
                "summary supplied for unrequired production readiness gate"
            )
    for gate, paths in paths_by_gate.items():
        errors.extend(
            require_existing_files(
                paths,
                SUMMARY_FLAGS_BY_GATE[gate],
                seen=seen_input_files,
            )
        )
    require_runner_positive_int(args, "now_unix", errors, allow_none=True)
    require_runner_non_negative_int(args, "max_summary_artifact_age_secs", errors)
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    """Build the aggregate verifier command plan."""

    summary_out = (
        args.summary_out
        or args.out_dir / "sorafs-production-readiness-summary.json"
    )
    verifier_command = [sys.executable, str(args.verifier)]
    for paths in summary_paths_by_gate(args).values():
        for path in paths:
            verifier_command.extend(["--evidence", str(path)])
    for required_gate in args.required_gates:
        verifier_command.extend(["--require-gate", required_gate])
    verifier_command.extend(
        [
            "--summary-out",
            str(summary_out),
            "--max-summary-artifact-age-secs",
            str(args.max_summary_artifact_age_secs),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])
    if args.deployment_id is not None:
        verifier_command.extend(["--deployment-id", args.deployment_id])
    if args.environment is not None:
        verifier_command.extend(["--environment", args.environment])
    return [CommandPlan("sorafs_production_readiness_gate", summary_out, verifier_command)]


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    """Render the aggregate dry-run plan."""

    thresholds: dict[str, int] = {
        "max_summary_artifact_age_secs": args.max_summary_artifact_age_secs,
    }
    if args.now_unix is not None:
        thresholds["now_unix"] = args.now_unix

    deployment_context: dict[str, str] = {}
    if args.deployment_id is not None:
        deployment_context["deployment_id"] = args.deployment_id
    if args.environment is not None:
        deployment_context["environment"] = args.environment

    return {
        "schema": "sorafs.production_readiness.collection_plan.v1",
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "required_gates": list(args.required_gates),
        "thresholds": thresholds,
        "deployment_context": deployment_context,
        "external_summaries": {
            gate: [str(path) for path in paths]
            for gate, paths in summary_paths_by_gate(args).items()
            if paths
        },
        "summary_contract": {
            gate: {
                "schema": GATE_BY_NAME[gate].schema,
                "required_kinds": list(GATE_BY_NAME[gate].required_kinds),
            }
            for gate in args.required_gates
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


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse aggregate readiness runner arguments."""

    parser = EvidenceArgumentParser(
        description="Run the aggregate SoraFS production-readiness gate.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_production_readiness.py",
        help="Aggregate readiness verifier script path.",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        required=True,
        help="Directory where the aggregate summary will be written.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional aggregate summary path. Defaults under --out-dir.",
    )
    parser.add_argument(
        "--require-gate",
        action="append",
        default=[],
        help=(
            "Required gate name, or comma-separated names. "
            "Defaults to every SoraFS production-readiness gate."
        ),
    )
    for gate, flag in SUMMARY_FLAGS_BY_GATE.items():
        parser.add_argument(
            flag,
            dest=SUMMARY_OPTIONS_BY_GATE[gate],
            action="append",
            type=Path,
            default=[],
            help=f"Existing ready summary for `{gate}`.",
        )
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument(
        "--max-summary-artifact-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    )
    parser.add_argument("--deployment-id")
    parser.add_argument("--environment")
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the aggregate verifier command plan without executing it.",
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
    args = parser.parse_args(expanded_args)
    try:
        args.required_gates = parse_required_gates(
            args.require_gate,
            allowed_kinds=GATE_BY_NAME,
            default_required=DEFAULT_REQUIRED_GATES,
        )
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
    return args


def main(argv: list[str] | None = None) -> int:
    """Run the aggregate SoraFS production-readiness plan."""

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
    if args.dry_run:
        render_errors = write_runner_plan(rendered_plan)
        if render_errors:
            emit_runner_error_lines(render_errors)
            return 2
        return 0
    exit_code = run_command_plan(plan, args.out_dir)
    if exit_code != 0:
        emit_runner_error_block(
            "SoraFS production readiness collection failed:",
            [f"command plan exited with {exit_code}"],
        )
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
