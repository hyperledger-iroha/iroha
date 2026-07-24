#!/usr/bin/env python3
"""Run the aggregate SoraFS production-readiness gate."""

from __future__ import annotations

import argparse
import hashlib
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
    FOUNDATIONAL_PREREQUISITE_IDS,
    FOUNDATIONAL_PREREQUISITE_SCHEMA,
    GATE_BY_NAME,
    MAX_FOUNDATIONAL_RELEASE_SEQUENCE,
    SUMMARY_SCHEMA,
    canonical_lower_hex,
    canonical_string,
    is_production_ready_environment,
    parse_foundational_signer_public_key,
    require_production_deployment_id_value,
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
    run_command_plan,
    validate_runner_aggregate_readiness_plan,
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
        "foundational_prerequisite",
        "summary_contract",
        "steps",
    }
)
PLAN_REQUIRED_THRESHOLD_FIELDS = frozenset(
    {"max_summary_artifact_age_secs", "now_unix"}
)
PLAN_POSITIVE_THRESHOLD_FIELDS = frozenset({"now_unix"})
PLAN_NON_NEGATIVE_THRESHOLD_FIELDS = frozenset({"max_summary_artifact_age_secs"})
PLAN_THRESHOLD_FIELDS_LABEL = "max_summary_artifact_age_secs and now_unix"
PLAN_DEPLOYMENT_CONTEXT_FIELDS = frozenset({"deployment_id", "environment"})
COMMAND_PATH_FLAGS = frozenset({"--evidence", "--summary-out"})
PLAN_FOUNDATIONAL_PREREQUISITE_FIELDS = frozenset(
    {
        "schema",
        "summary",
        "required_ids",
        "signer_public_key_fingerprint_sha256",
        "release_sequence",
        "previous_envelope_sha256",
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
    foundational_paths = list(args.foundational_prerequisite_summary)
    if not foundational_paths:
        errors.append(
            "production readiness runner requires exactly one foundational prerequisite summary"
        )
    elif len(foundational_paths) > 1:
        errors.append(
            "production readiness runner requires exactly one foundational prerequisite summary"
        )
    errors.extend(
        require_existing_files(
            foundational_paths,
            "--foundational-prerequisite-summary",
            seen=seen_input_files,
        )
    )
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
        for paths in [*paths_by_gate.values(), foundational_paths]
        for path in paths
    ):
        errors.append(
            "production readiness runner summary input paths must not contain "
            "secret-looking, control-character, parent, current, or "
            "platform-specific components"
        )
    require_runner_positive_int(args, "now_unix", errors)
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
        require_production_deployment_id_value(
            args.deployment_id,
            errors,
            "production readiness runner deployment_id",
        )
        if not is_production_ready_environment(args.environment):
            errors.append("production readiness runner environment must be production")
    if args.foundational_signer_public_key_hex is None:
        errors.append(
            "production readiness runner requires a foundational prerequisite signer public key"
        )
    else:
        parse_foundational_signer_public_key(
            args.foundational_signer_public_key_hex,
            errors,
            path="production readiness runner foundational signer public key",
        )
    require_runner_positive_int(
        args,
        "foundational_release_sequence",
        errors,
    )
    if (
        isinstance(args.foundational_release_sequence, int)
        and not isinstance(args.foundational_release_sequence, bool)
        and args.foundational_release_sequence > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
    ):
        errors.append(
            "production readiness runner foundational release sequence must be in 1..2^63-1"
        )
    if args.foundational_previous_envelope_sha256 is None:
        errors.append(
            "production readiness runner requires a foundational prerequisite predecessor digest"
        )
    elif (
        canonical_lower_hex(args.foundational_previous_envelope_sha256, 64)
        is None
    ):
        errors.append(
            "production readiness runner foundational predecessor must be canonical lowercase SHA-256"
        )
    elif (
        isinstance(args.foundational_release_sequence, int)
        and not isinstance(args.foundational_release_sequence, bool)
        and args.foundational_release_sequence > 0
    ):
        predecessor_is_zero = not any(
            bytes.fromhex(args.foundational_previous_envelope_sha256)
        )
        if args.foundational_release_sequence == 1 and not predecessor_is_zero:
            errors.append(
                "production readiness runner foundational sequence 1 requires the zero predecessor"
            )
        if args.foundational_release_sequence > 1 and predecessor_is_zero:
            errors.append(
                "production readiness runner foundational sequence after 1 requires a non-zero predecessor"
            )
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    """Build the aggregate verifier command plan."""

    summary_out = (
        args.summary_out
        or args.out_dir / "sorafs-production-readiness-summary.json"
    )
    verifier_command = [sys.executable, str(args.verifier)]
    for path in args.foundational_prerequisite_summary:
        verifier_command.extend(["--evidence", str(path)])
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
    verifier_command.extend(["--now-unix", str(args.now_unix)])
    if args.deployment_id is not None:
        verifier_command.extend(["--deployment-id", args.deployment_id])
    if args.environment is not None:
        verifier_command.extend(["--environment", args.environment])
    if args.foundational_signer_public_key_hex is not None:
        verifier_command.extend(
            [
                "--foundational-prerequisite-signer-public-key-hex",
                args.foundational_signer_public_key_hex,
            ]
        )
    if args.foundational_release_sequence is not None:
        verifier_command.extend(
            [
                "--foundational-prerequisite-release-sequence",
                str(args.foundational_release_sequence),
            ]
        )
    if args.foundational_previous_envelope_sha256 is not None:
        verifier_command.extend(
            [
                "--foundational-prerequisite-previous-envelope-sha256",
                args.foundational_previous_envelope_sha256,
            ]
        )
    return [CommandPlan("sorafs_production_readiness_gate", summary_out, verifier_command)]


def foundational_prerequisite_plan(args: argparse.Namespace) -> dict[str, object]:
    """Render the payload-free foundational prerequisite plan row."""

    foundational_prerequisite: dict[str, object] = {
        "schema": FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "summary": (
            str(args.foundational_prerequisite_summary[0])
            if len(args.foundational_prerequisite_summary) == 1
            else ""
        ),
        "required_ids": list(FOUNDATIONAL_PREREQUISITE_IDS),
        "signer_public_key_fingerprint_sha256": "",
        "release_sequence": args.foundational_release_sequence,
        "previous_envelope_sha256": (
            args.foundational_previous_envelope_sha256
        ),
    }
    key_errors: list[str] = []
    public_key = parse_foundational_signer_public_key(
        args.foundational_signer_public_key_hex,
        key_errors,
        path="production readiness runner foundational signer public key",
    )
    if public_key is not None:
        foundational_prerequisite["signer_public_key_fingerprint_sha256"] = (
            hashlib.sha256(public_key).hexdigest()
        )
    return foundational_prerequisite


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    """Render the aggregate dry-run plan."""

    thresholds: dict[str, int] = {
        "max_summary_artifact_age_secs": args.max_summary_artifact_age_secs,
    }
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
        "foundational_prerequisite": foundational_prerequisite_plan(args),
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


def rendered_plan_paths_are_safe(rendered: Mapping[str, object]) -> bool:
    """Return whether all rendered plan path strings are safe to expose."""

    paths: list[str] = []
    external_summaries = rendered.get("external_summaries")
    if isinstance(external_summaries, Mapping):
        for gate_paths in external_summaries.values():
            if not isinstance(gate_paths, list):
                continue
            paths.extend(path for path in gate_paths if isinstance(path, str))
    foundational_prerequisite = rendered.get("foundational_prerequisite")
    if isinstance(foundational_prerequisite, Mapping):
        foundational_summary = foundational_prerequisite.get("summary")
        if isinstance(foundational_summary, str):
            paths.append(foundational_summary)
    steps = rendered.get("steps")
    if isinstance(steps, list):
        for step in steps:
            if not isinstance(step, Mapping):
                continue
            artifact = step.get("artifact")
            if isinstance(artifact, str):
                paths.append(artifact)
            command = step.get("command")
            if isinstance(command, list):
                for index, argument in enumerate(command):
                    if not isinstance(argument, str):
                        continue
                    previous = command[index - 1] if index > 0 else None
                    if index in {0, 1} or previous in COMMAND_PATH_FLAGS:
                        paths.append(argument)
    return all(plan_rendered_path_is_safe(Path(path)) for path in paths)


def validate_plan_json(
    rendered: object,
    plan: Sequence[CommandPlan],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the production-readiness collection-plan envelope."""

    expected_thresholds: dict[str, int] = {
        "max_summary_artifact_age_secs": args.max_summary_artifact_age_secs,
    }
    expected_thresholds["now_unix"] = args.now_unix

    expected_deployment_context = {
        "deployment_id": args.deployment_id,
        "environment": args.environment,
    }

    paths_by_gate = summary_paths_by_gate(args)
    expected_external_summaries = {
        gate: [str(paths[0])]
        for gate, paths in paths_by_gate.items()
        if gate in args.required_gates and len(paths) == 1
    }

    expected_summary_contract = {
        gate: {
            "schema": GATE_BY_NAME[gate].schema,
            "required_kinds": list(GATE_BY_NAME[gate].required_kinds),
        }
        for gate in args.required_gates
    }
    expected_foundational_prerequisite = foundational_prerequisite_plan(args)

    def deployment_context_value_errors(
        context: Mapping[str, object],
    ) -> list[str]:
        context_errors: list[str] = []
        require_production_deployment_id_value(
            context.get("deployment_id"),
            context_errors,
            "production readiness runner plan deployment_id",
        )
        if not is_production_ready_environment(context.get("environment")):
            context_errors.append(
                "production readiness runner plan environment must be production"
            )
        return context_errors

    errors = validate_runner_aggregate_readiness_plan(
        rendered,
        plan,
        diagnostic_prefix="production readiness runner plan",
        plan_schema=PLAN_SCHEMA,
        plan_fields=PLAN_FIELDS,
        summary_schema=SUMMARY_SCHEMA,
        required_gates=args.required_gates,
        known_gates=GATE_BY_NAME,
        thresholds=expected_thresholds,
        required_threshold_fields=PLAN_REQUIRED_THRESHOLD_FIELDS,
        positive_threshold_fields=PLAN_POSITIVE_THRESHOLD_FIELDS,
        non_negative_threshold_fields=PLAN_NON_NEGATIVE_THRESHOLD_FIELDS,
        threshold_fields_label=PLAN_THRESHOLD_FIELDS_LABEL,
        deployment_context=expected_deployment_context,
        deployment_context_fields=PLAN_DEPLOYMENT_CONTEXT_FIELDS,
        deployment_context_value_errors=deployment_context_value_errors,
        external_summaries=expected_external_summaries,
        summary_contract=expected_summary_contract,
    )
    if isinstance(rendered, Mapping) and not rendered_plan_paths_are_safe(rendered):
        errors.append(PLAN_RENDERED_PATH_ERROR)
    if isinstance(rendered, Mapping):
        foundational_prerequisite = rendered.get("foundational_prerequisite")
        if not isinstance(foundational_prerequisite, Mapping):
            errors.append(
                "production readiness runner plan foundational_prerequisite must be an object"
            )
        else:
            if set(foundational_prerequisite) != PLAN_FOUNDATIONAL_PREREQUISITE_FIELDS:
                errors.append(
                    "production readiness runner plan foundational_prerequisite fields must match the schema-closed contract"
                )
            if foundational_prerequisite != expected_foundational_prerequisite:
                errors.append(
                    "production readiness runner plan foundational_prerequisite must match reviewed inputs"
                )
            if (
                foundational_prerequisite.get("schema")
                != FOUNDATIONAL_PREREQUISITE_SCHEMA
            ):
                errors.append(
                    "production readiness runner plan foundational prerequisite schema must match the contract"
                )
            summary_path = canonical_string(
                foundational_prerequisite.get("summary")
            )
            if summary_path is None:
                errors.append(
                    "production readiness runner plan foundational prerequisite summary must be canonical"
                )
            if foundational_prerequisite.get("required_ids") != list(
                FOUNDATIONAL_PREREQUISITE_IDS
            ):
                errors.append(
                    "production readiness runner plan foundational prerequisite ids must match the exact contract"
                )
            if (
                canonical_lower_hex(
                    foundational_prerequisite.get(
                        "signer_public_key_fingerprint_sha256"
                    ),
                    64,
                )
                is None
            ):
                errors.append(
                    "production readiness runner plan foundational signer fingerprint must be canonical lowercase SHA-256"
                )
            release_sequence = foundational_prerequisite.get("release_sequence")
            if (
                not isinstance(release_sequence, int)
                or isinstance(release_sequence, bool)
                or release_sequence <= 0
                or release_sequence > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
            ):
                errors.append(
                    "production readiness runner plan foundational release sequence must be in 1..2^63-1"
                )
            predecessor = canonical_lower_hex(
                foundational_prerequisite.get("previous_envelope_sha256"),
                64,
            )
            if predecessor is None:
                errors.append(
                    "production readiness runner plan foundational predecessor must be canonical lowercase SHA-256"
                )
            elif isinstance(release_sequence, int) and not isinstance(
                release_sequence,
                bool,
            ):
                predecessor_is_zero = not any(bytes.fromhex(predecessor))
                if release_sequence == 1 and not predecessor_is_zero:
                    errors.append(
                        "production readiness runner plan foundational sequence 1 requires the zero predecessor"
                    )
                if release_sequence > 1 and predecessor_is_zero:
                    errors.append(
                        "production readiness runner plan foundational sequence after 1 requires a non-zero predecessor"
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
    parser.add_argument(
        "--foundational-prerequisite-summary",
        action="append",
        type=Path,
        default=[],
        help=(
            "Exactly one existing signed foundational prerequisite summary for "
            "SFM-1, SF-1, SF-2/SF-2c, SF-3, SF-4, SF-5b, SF-6, and SF-8a."
        ),
    )
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument(
        "--max-summary-artifact-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    )
    parser.add_argument(
        "--deployment-id",
        help=(
            "Required final deployment id shared by every required lane summary "
            "before aggregate production readiness can run."
        ),
    )
    parser.add_argument(
        "--environment",
        help=(
            "Required final prod/production environment shared by every required "
            "lane summary before aggregate production readiness can run."
        ),
    )
    parser.add_argument(
        "--foundational-prerequisite-signer-public-key-hex",
        dest="foundational_signer_public_key_hex",
        help="Required operator-trusted 32-byte Ed25519 public key.",
    )
    parser.add_argument(
        "--foundational-prerequisite-release-sequence",
        dest="foundational_release_sequence",
        type=positive_int_arg,
        help="Required operator-reviewed monotonic foundational release sequence.",
    )
    parser.add_argument(
        "--foundational-prerequisite-previous-envelope-sha256",
        dest="foundational_previous_envelope_sha256",
        help="Required operator-reviewed predecessor envelope SHA-256.",
    )
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
