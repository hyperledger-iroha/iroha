#!/usr/bin/env python3
"""Collect and verify SoraFS Governance DAG rollout evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_governance_dag_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_HEAD_AGE_SECS,
    DEFAULT_MAX_PIN_LAG_SECS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_BLOCKS,
    DEFAULT_MIN_PAYLOAD_KINDS,
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
    """One Governance DAG rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




EVIDENCE_OPTIONS_BY_KIND = {
    "ingest_service": "ingest_service_evidence",
    "publisher_service": "publisher_service_evidence",
    "mirror_datastore": "mirror_datastore_evidence",
    "operator_recovery": "operator_recovery_evidence",
    "dashboard_api": "dashboard_api_evidence",
    "observability": "observability_evidence",
    "ipfs_ipns_e2e": "ipfs_ipns_e2e_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "ingest_service": "--ingest-service-evidence",
    "publisher_service": "--publisher-service-evidence",
    "mirror_datastore": "--mirror-datastore-evidence",
    "operator_recovery": "--operator-recovery-evidence",
    "dashboard_api": "--dashboard-api-evidence",
    "observability": "--observability-evidence",
    "ipfs_ipns_e2e": "--ipfs-ipns-e2e-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied rollout evidence paths keyed by SF-12 evidence kind."""

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
    require_runner_non_negative_int(args, "max_evidence_age_secs", errors)
    require_runner_positive_int(args, "max_route_latency_ms", errors)
    require_runner_positive_int(args, "max_pin_lag_secs", errors)
    require_runner_positive_int(args, "max_head_age_secs", errors)
    require_runner_positive_int(args, "min_blocks", errors)
    require_runner_positive_int(args, "min_payload_kinds", errors)
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
            "--max-route-latency-ms",
            str(args.max_route_latency_ms),
            "--max-pin-lag-secs",
            str(args.max_pin_lag_secs),
            "--max-head-age-secs",
            str(args.max_head_age_secs),
            "--min-blocks",
            str(args.min_blocks),
            "--min-payload-kinds",
            str(args.min_payload_kinds),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])

    return [CommandPlan("rollout_evidence_gate", summary_out, verifier_command)]


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    thresholds: dict[str, int] = {
        "max_evidence_age_secs": args.max_evidence_age_secs,
        "max_route_latency_ms": args.max_route_latency_ms,
        "max_pin_lag_secs": args.max_pin_lag_secs,
        "max_head_age_secs": args.max_head_age_secs,
        "min_blocks": args.min_blocks,
        "min_payload_kinds": args.min_payload_kinds,
    }
    if args.now_unix is not None:
        thresholds["now_unix"] = args.now_unix

    return {
        "schema": "sorafs.governance_dag.rollout_evidence_collection_plan.v1",
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
        description="Collect and verify SoraFS SF-12 Governance DAG rollout evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_governance_dag_rollout_evidence.py",
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
            "Defaults to every SF-12 rollout kind."
        ),
    )
    parser.add_argument(
        "--ingest-service-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free ingest service canary JSON.",
    )
    parser.add_argument(
        "--publisher-service-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free IPFS/IPNS publisher service canary JSON.",
    )
    parser.add_argument(
        "--mirror-datastore-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free RocksDB/IPLD mirror datastore canary JSON.",
    )
    parser.add_argument(
        "--operator-recovery-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free live-head/checkpoint recovery canary JSON.",
    )
    parser.add_argument(
        "--dashboard-api-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free runtime/IPFS-backed dashboard API canary JSON.",
    )
    parser.add_argument(
        "--observability-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free live metric and alert canary JSON.",
    )
    parser.add_argument(
        "--ipfs-ipns-e2e-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Payload-free IPFS/IPNS end-to-end test canary JSON.",
    )
    parser.add_argument(
        "--governance-approval-evidence",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Governance approval and iroha_config binding evidence JSON.",
    )
    parser.add_argument("--now-unix", type=positive_int_arg, help="Validator clock used for age checks.")
    parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        help="Maximum allowed age for rollout evidence artifacts.",
    )
    parser.add_argument(
        "--max-route-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_ROUTE_LATENCY_MS,
        help="Maximum allowed dashboard/query route latency.",
    )
    parser.add_argument(
        "--max-pin-lag-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_PIN_LAG_SECS,
        help="Maximum allowed IPFS pin lag.",
    )
    parser.add_argument(
        "--max-head-age-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_HEAD_AGE_SECS,
        help="Maximum allowed public head age.",
    )
    parser.add_argument(
        "--min-blocks",
        type=positive_int_arg,
        default=DEFAULT_MIN_BLOCKS,
        help="Minimum public DAG blocks required by rollout evidence.",
    )
    parser.add_argument(
        "--min-payload-kinds",
        type=positive_int_arg,
        default=DEFAULT_MIN_PAYLOAD_KINDS,
        help="Minimum payload-kind coverage required by rollout evidence.",
    )
    parser.add_argument("--dry-run", action="store_true", help="Print the command plan JSON.")
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
            "ERROR: SoraFS Governance DAG rollout evidence inputs are incomplete:",
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
