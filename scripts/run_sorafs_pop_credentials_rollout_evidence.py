#!/usr/bin/env python3
"""Collect and verify SoraFS proof-of-personhood rollout evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

BUNDLED_VERIFIER = SCRIPT_DIR / "check_sorafs_pop_credentials_rollout_evidence.py"

from check_sorafs_pop_credentials_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_REVOCATION_AGE_SECS,
    DEFAULT_MAX_ROOT_AGE_SECS,
    DEFAULT_MAX_SERVICE_LAG_SECS,
    DEFAULT_MAX_VERIFY_LATENCY_MS,
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
    require_no_unrequired_evidence,
    require_runner_non_negative_int,
    require_runner_positive_int,
    validate_runner_evidence_plan,
    validate_runner_plan_steps,
    validate_runner_preflight,
    write_runner_plan,
)


from sorafs_topology_qualification import add_topology_qualification_argument  # noqa: E402

PLAN_SCHEMA = "sorafs.pop_credentials.rollout_evidence_collection_plan.v1"
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
PLAN_REQUIRED_THRESHOLD_FIELDS = frozenset(
    {
        "max_root_age_secs",
        "max_revocation_age_secs",
        "max_service_lag_secs",
        "max_verify_latency_ms",
    }
)
PLAN_POSITIVE_THRESHOLD_FIELDS = frozenset(
    {"max_verify_latency_ms", "now_unix"}
)
PLAN_NON_NEGATIVE_THRESHOLD_FIELDS = frozenset(
    {"max_root_age_secs", "max_revocation_age_secs", "max_service_lag_secs"}
)


@dataclass(frozen=True)
class CommandPlan:
    """One PoP rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]


EVIDENCE_OPTIONS_BY_KIND = {
    "issuer_bundle": "issuer_bundle_evidence",
    "commitment_root": "commitment_root_evidence",
    "revocation_registry": "revocation_registry_evidence",
    "enrollment_portal": "enrollment_portal_evidence",
    "juror_client": "juror_client_evidence",
    "verifier_service": "verifier_service_evidence",
    "moderation_integration": "moderation_integration_evidence",
    "metrics_alerts": "metrics_alerts_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "issuer_bundle": "--issuer-bundle-evidence",
    "commitment_root": "--commitment-root-evidence",
    "revocation_registry": "--revocation-registry-evidence",
    "enrollment_portal": "--enrollment-portal-evidence",
    "juror_client": "--juror-client-evidence",
    "verifier_service": "--verifier-service-evidence",
    "moderation_integration": "--moderation-integration-evidence",
    "metrics_alerts": "--metrics-alerts-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied rollout evidence paths keyed by SFM-4b1 evidence kind."""

    return {
        kind: list(getattr(args, option))
        for kind, option in EVIDENCE_OPTIONS_BY_KIND.items()
    }


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(
        args,
        summary_filename="rollout-summary.json",
        bundled_verifier=BUNDLED_VERIFIER,
    )
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
        errors.extend(require_existing_files(paths, EVIDENCE_FLAGS_BY_KIND[kind], seen=seen_input_files))

    require_runner_positive_int(args, "now_unix", errors, allow_none=True)
    require_runner_non_negative_int(args, "max_root_age_secs", errors)
    require_runner_non_negative_int(args, "max_revocation_age_secs", errors)
    require_runner_non_negative_int(args, "max_service_lag_secs", errors)
    require_runner_positive_int(args, "max_verify_latency_ms", errors)
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    summary_out = args.summary_out or args.out_dir / "rollout-summary.json"
    verifier_command = [
        sys.executable,
        str(BUNDLED_VERIFIER),
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
            "--topology-qualification-summary",
            str(args.topology_qualification_summary),
            "--max-root-age-secs",
            str(args.max_root_age_secs),
            "--max-revocation-age-secs",
            str(args.max_revocation_age_secs),
            "--max-service-lag-secs",
            str(args.max_service_lag_secs),
            "--max-verify-latency-ms",
            str(args.max_verify_latency_ms),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])

    return [CommandPlan("rollout_evidence_gate", summary_out, verifier_command)]


def threshold_values(args: argparse.Namespace) -> dict[str, int]:
    """Return threshold values rendered in dry-run plans."""

    thresholds: dict[str, int] = {
        "max_root_age_secs": args.max_root_age_secs,
        "max_revocation_age_secs": args.max_revocation_age_secs,
        "max_service_lag_secs": args.max_service_lag_secs,
        "max_verify_latency_ms": args.max_verify_latency_ms,
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
    """Validate the PoP credential collection-plan envelope before use."""

    return validate_runner_evidence_plan(
        rendered,
        plan,
        diagnostic_prefix="PoP credential rollout runner plan",
        plan_schema=PLAN_SCHEMA,
        plan_fields=PLAN_FIELDS,
        summary_schema=SUMMARY_SCHEMA,
        required_kinds=args.required_kinds,
        known_kinds=KIND_BY_NAME,
        thresholds=threshold_values(args),
        required_threshold_fields=PLAN_REQUIRED_THRESHOLD_FIELDS,
        positive_threshold_fields=PLAN_POSITIVE_THRESHOLD_FIELDS,
        non_negative_threshold_fields=PLAN_NON_NEGATIVE_THRESHOLD_FIELDS,
        external_evidence=external_evidence(args),
        evidence_contract=evidence_contract(args),
        evidence_required_fields=EVIDENCE_REQUIRED_FIELDS,
    )


def run_plan(plan: Sequence[CommandPlan], out_dir: Path) -> int:
    return run_command_plan(plan, out_dir)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Collect and verify SoraFS SFM-4b1 PoP credential rollout evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=BUNDLED_VERIFIER,
        help="Bundled rollout evidence verifier path; substitutions are rejected.",
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
            "Defaults to every SFM-4b1 rollout kind."
        ),
    )
    parser.add_argument(
        "--issuer-bundle-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free issuer bundle canary JSON.",
    )
    parser.add_argument(
        "--commitment-root-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free commitment-root publication canary JSON.",
    )
    parser.add_argument(
        "--revocation-registry-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free revocation registry canary JSON.",
    )
    parser.add_argument(
        "--enrollment-portal-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free enrollment portal canary JSON.",
    )
    parser.add_argument(
        "--juror-client-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free juror client canary JSON.",
    )
    parser.add_argument(
        "--verifier-service-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free verifier service canary JSON.",
    )
    parser.add_argument(
        "--moderation-integration-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free moderation integration canary JSON.",
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
        help="Required reviewed validator clock used for age checks.",
    )
    parser.add_argument(
        "--max-root-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_ROOT_AGE_SECS,
        help="Maximum allowed commitment-root publication age.",
    )
    parser.add_argument(
        "--max-revocation-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_REVOCATION_AGE_SECS,
        help="Maximum allowed revocation-registry publication age.",
    )
    parser.add_argument(
        "--max-service-lag-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SERVICE_LAG_SECS,
        help="Maximum allowed verifier service lag.",
    )
    parser.add_argument(
        "--max-verify-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_VERIFY_LATENCY_MS,
        help="Maximum allowed verifier service proof latency.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the command plan JSON without running the verifier.",
    )
    add_topology_qualification_argument(parser)
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
        args = parser.parse_args(expanded_args)
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
        emit_runner_error_block(
            "ERROR: SoraFS PoP credential rollout evidence inputs are incomplete:",
            errors,
        )
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
