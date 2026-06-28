#!/usr/bin/env python3
"""Collect and verify SoraFS PDP rollout evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_pdp_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_PROOF_LATENCY_MS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_CHALLENGES,
    DEFAULT_MIN_PROOFS,
    DEFAULT_MIN_PROVIDERS,
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
    """One PDP rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




EVIDENCE_OPTIONS_BY_KIND = {
    "provider_transport": "provider_transport_evidence",
    "proof_generation": "proof_generation_evidence",
    "validator_replay": "validator_replay_evidence",
    "governance_repair": "governance_repair_evidence",
    "observability": "observability_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "provider_transport": "--provider-transport-evidence",
    "proof_generation": "--proof-generation-evidence",
    "validator_replay": "--validator-replay-evidence",
    "governance_repair": "--governance-repair-evidence",
    "observability": "--observability-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied rollout evidence paths keyed by SF-13 evidence kind."""

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
    require_runner_positive_int(args, "max_proof_latency_ms", errors)
    require_runner_positive_int(args, "min_providers", errors)
    require_runner_positive_int(args, "min_challenges", errors)
    require_runner_positive_int(args, "min_proofs", errors)
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
            "--max-proof-latency-ms",
            str(args.max_proof_latency_ms),
            "--min-providers",
            str(args.min_providers),
            "--min-challenges",
            str(args.min_challenges),
            "--min-proofs",
            str(args.min_proofs),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])

    return [CommandPlan("rollout_evidence_gate", summary_out, verifier_command)]


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    thresholds: dict[str, int] = {
        "max_evidence_age_secs": args.max_evidence_age_secs,
        "max_route_latency_ms": args.max_route_latency_ms,
        "max_proof_latency_ms": args.max_proof_latency_ms,
        "min_providers": args.min_providers,
        "min_challenges": args.min_challenges,
        "min_proofs": args.min_proofs,
    }
    if args.now_unix is not None:
        thresholds["now_unix"] = args.now_unix

    return {
        "schema": "sorafs.pdp.rollout_evidence_collection_plan.v1",
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
        description="Collect and verify SoraFS SF-13 PDP rollout evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_pdp_rollout_evidence.py",
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
            "Defaults to every SF-13 rollout kind."
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
        "--max-route-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_ROUTE_LATENCY_MS,
    )
    parser.add_argument(
        "--max-proof-latency-ms",
        type=positive_int_arg,
        default=DEFAULT_MAX_PROOF_LATENCY_MS,
    )
    parser.add_argument("--min-providers", type=positive_int_arg, default=DEFAULT_MIN_PROVIDERS)
    parser.add_argument("--min-challenges", type=positive_int_arg, default=DEFAULT_MIN_CHALLENGES)
    parser.add_argument("--min-proofs", type=positive_int_arg, default=DEFAULT_MIN_PROOFS)
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
        args.required_kinds = ()
        args.parse_error = 2
    else:
        args.parse_error = 0
    return args


def main(argv: list[str] | None = None) -> int:
    try:
        args = parse_args(argv)
    except SystemExit as error:
        return error.code if isinstance(error.code, int) else 1
    if args.parse_error:
        return args.parse_error
    errors = validate_inputs(args)
    if errors:
        emit_runner_error_lines(errors)
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
