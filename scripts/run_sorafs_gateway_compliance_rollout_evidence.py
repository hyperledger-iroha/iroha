#!/usr/bin/env python3
"""Collect and verify SoraFS gateway compliance rollout evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_gateway_compliance_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_RELOAD_LATENCY_MS,
    DEFAULT_MAX_ROUTE_LATENCY_MS,
    DEFAULT_MIN_DENYLIST_ENTRIES,
    DEFAULT_MIN_GATEWAYS,
    DEFAULT_MIN_HONEY_PROBES,
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
    """One gateway compliance rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




EVIDENCE_OPTIONS_BY_KIND = {
    "feed_promotion": "feed_promotion_evidence",
    "gateway_reload": "gateway_reload_evidence",
    "enforcement_probe": "enforcement_probe_evidence",
    "honey_audit": "honey_audit_evidence",
    "appeal_override": "appeal_override_evidence",
    "transparency_publication": "transparency_publication_evidence",
    "observability": "observability_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "feed_promotion": "--feed-promotion-evidence",
    "gateway_reload": "--gateway-reload-evidence",
    "enforcement_probe": "--enforcement-probe-evidence",
    "honey_audit": "--honey-audit-evidence",
    "appeal_override": "--appeal-override-evidence",
    "transparency_publication": "--transparency-publication-evidence",
    "observability": "--observability-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied rollout evidence paths keyed by SFM-4 evidence kind."""

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
    require_runner_positive_int(args, "max_reload_latency_ms", errors)
    require_runner_positive_int(args, "min_gateways", errors)
    require_runner_positive_int(args, "min_denylist_entries", errors)
    require_runner_positive_int(args, "min_honey_probes", errors)
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    summary_out = args.summary_out or args.out_dir / "rollout-summary.json"
    verifier_command = [sys.executable, str(args.verifier)]
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
            "--max-reload-latency-ms",
            str(args.max_reload_latency_ms),
            "--min-gateways",
            str(args.min_gateways),
            "--min-denylist-entries",
            str(args.min_denylist_entries),
            "--min-honey-probes",
            str(args.min_honey_probes),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])
    return [CommandPlan("rollout_evidence_gate", summary_out, verifier_command)]


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    thresholds: dict[str, int] = {
        "max_evidence_age_secs": args.max_evidence_age_secs,
        "max_route_latency_ms": args.max_route_latency_ms,
        "max_reload_latency_ms": args.max_reload_latency_ms,
        "min_gateways": args.min_gateways,
        "min_denylist_entries": args.min_denylist_entries,
        "min_honey_probes": args.min_honey_probes,
    }
    if args.now_unix is not None:
        thresholds["now_unix"] = args.now_unix

    return {
        "schema": "sorafs.gateway_compliance.rollout_evidence_collection_plan.v1",
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
        description="Collect and verify SoraFS SFM-4 gateway compliance rollout evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_gateway_compliance_rollout_evidence.py",
    )
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--summary-out", type=Path)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument(
        "--require-kind",
        action="append",
        default=[],
        help="Required evidence kind, or comma-separated kinds. Defaults to all SFM-4 kinds.",
    )
    for kind, flag in EVIDENCE_FLAGS_BY_KIND.items():
        parser.add_argument(
            flag,
            dest=EVIDENCE_OPTIONS_BY_KIND[kind],
            action="append",
            type=Path,
            default=[],
        )
    parser.add_argument("--max-evidence-age-secs", type=non_negative_int_arg, default=DEFAULT_MAX_EVIDENCE_AGE_SECS)
    parser.add_argument("--max-route-latency-ms", type=positive_int_arg, default=DEFAULT_MAX_ROUTE_LATENCY_MS)
    parser.add_argument("--max-reload-latency-ms", type=positive_int_arg, default=DEFAULT_MAX_RELOAD_LATENCY_MS)
    parser.add_argument("--min-gateways", type=positive_int_arg, default=DEFAULT_MIN_GATEWAYS)
    parser.add_argument("--min-denylist-entries", type=positive_int_arg, default=DEFAULT_MIN_DENYLIST_ENTRIES)
    parser.add_argument("--min-honey-probes", type=positive_int_arg, default=DEFAULT_MIN_HONEY_PROBES)

    try:
        expanded = expand_response_args(sys.argv[1:] if argv is None else argv, parser)
        args = parser.parse_args(expanded)
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
    if args.dry_run:
        plan_errors = write_runner_plan(plan_json(plan, args))
        if plan_errors:
            emit_runner_error_lines(plan_errors)
            return 2
        return 0
    return run_plan(plan, args.out_dir)


if __name__ == "__main__":
    raise SystemExit(main())
