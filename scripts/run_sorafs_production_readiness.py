#!/usr/bin/env python3
"""Run the aggregate SoraFS production-readiness gate."""

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

from check_sorafs_production_readiness import (  # noqa: E402
    DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    DEFAULT_REQUIRED_GATES,
    GATE_BY_NAME,
    SUMMARY_SCHEMA,
    canonical_string,
    is_production_ready_environment,
    require_reviewed_deployment_id_value,
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
    PLAN_RENDERED_PATH_ERROR,
    emit_runner_error_block,
    emit_runner_error_lines,
    emit_runner_exception,
    plan_rendered_path_is_safe,
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_positive_int,
    render_runner_plan,
    run_command_plan,
    validate_runner_preflight,
    write_runner_plan,
)


PLAN_SCHEMA = "sorafs.production_readiness.collection_plan.v1"
PLAN_FIELDS = frozenset(
    {
        "schema",
        "verifier_summary_schema",
        "required_gates",
        "thresholds",
        "deployment_context",
        "external_summaries",
        "summary_contract",
        "steps",
    }
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


def summary_input_path_is_plan_safe(path: Path) -> bool:
    """Return whether a summary input path can be rendered in runner plans."""

    return plan_rendered_path_is_safe(path)


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
        elif len(paths) > 1:
            errors.append(
                "production readiness runner requires exactly one summary input per required gate"
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
    if any(
        not summary_input_path_is_plan_safe(path)
        for paths in paths_by_gate.values()
        for path in paths
    ):
        errors.append(
            "production readiness runner summary input paths must not contain "
            "secret-looking, control-character, parent, current, or "
            "platform-specific components"
        )
    require_runner_positive_int(args, "now_unix", errors, allow_none=True)
    require_runner_non_negative_int(args, "max_summary_artifact_age_secs", errors)
    if args.deployment_id is None or args.environment is None:
        errors.append(
            "production readiness runner requires --deployment-id and --environment"
        )
    elif (
        canonical_string(args.deployment_id) is None
        or canonical_string(args.environment) is None
    ):
        errors.append(
            "production readiness runner deployment context must use canonical labels"
        )
    else:
        require_reviewed_deployment_id_value(
            args.deployment_id,
            errors,
            "production readiness runner deployment_id",
        )
        if not is_production_ready_environment(args.environment):
            errors.append("production readiness runner environment must be production")
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
        "schema": PLAN_SCHEMA,
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


def validate_plan_json(
    rendered: object,
    plan: Sequence[CommandPlan],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the production-readiness collection-plan envelope."""

    errors: list[str] = []
    if not isinstance(rendered, Mapping):
        return ["production readiness runner plan must be an object"]
    if set(rendered) != PLAN_FIELDS:
        errors.append(
            "production readiness runner plan fields must match the schema-closed contract"
        )
    if rendered.get("schema") != PLAN_SCHEMA:
        errors.append("production readiness runner plan schema must match the contract")
    if rendered.get("verifier_summary_schema") != SUMMARY_SCHEMA:
        errors.append(
            "production readiness runner plan verifier schema must match aggregate schema"
        )
    required_gates = list(args.required_gates)
    if rendered.get("required_gates") != required_gates:
        errors.append("production readiness runner plan required_gates must match args")

    thresholds = rendered.get("thresholds")
    expected_thresholds: dict[str, int] = {
        "max_summary_artifact_age_secs": args.max_summary_artifact_age_secs,
    }
    if args.now_unix is not None:
        expected_thresholds["now_unix"] = args.now_unix
    if thresholds != expected_thresholds:
        errors.append("production readiness runner plan thresholds must match args")

    deployment_context = rendered.get("deployment_context")
    expected_deployment_context = {
        "deployment_id": args.deployment_id,
        "environment": args.environment,
    }
    if deployment_context != expected_deployment_context:
        errors.append(
            "production readiness runner plan deployment_context must match args"
        )
    elif any(
        canonical_string(value) is None
        for value in expected_deployment_context.values()
    ):
        errors.append(
            "production readiness runner plan deployment_context must be canonical"
        )

    external_summaries = rendered.get("external_summaries")
    paths_by_gate = summary_paths_by_gate(args)
    expected_external_summaries = {
        gate: [str(paths[0])]
        for gate, paths in paths_by_gate.items()
        if gate in required_gates and len(paths) == 1
    }
    if external_summaries != expected_external_summaries:
        errors.append(
            "production readiness runner plan external_summaries must contain exactly one summary per required gate"
        )

    summary_contract = rendered.get("summary_contract")
    expected_summary_contract = {
        gate: {
            "schema": GATE_BY_NAME[gate].schema,
            "required_kinds": list(GATE_BY_NAME[gate].required_kinds),
        }
        for gate in required_gates
    }
    if summary_contract != expected_summary_contract:
        errors.append(
            "production readiness runner plan summary_contract must match required gates"
        )

    expected_steps = [
        {
            "label": step.label,
            "artifact": None if step.artifact is None else str(step.artifact),
            "command": step.command,
        }
        for step in plan
    ]
    if rendered.get("steps") != expected_steps:
        errors.append("production readiness runner plan steps must match command plan")
    try:
        render_runner_plan(rendered)
    except (TypeError, ValueError):
        errors.append(
            "production readiness runner plan must be strict JSON renderable"
        )
    return errors


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
    plan_errors = validate_plan_json(rendered_plan, plan, args)
    if plan_errors:
        emit_runner_error_lines(plan_errors)
        return 2
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
