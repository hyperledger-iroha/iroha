#!/usr/bin/env python3
"""Collect and verify SoraFS reference SDK release evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

BUNDLED_VERIFIER = SCRIPT_DIR / "check_sorafs_reference_sdk_release_evidence.py"

from check_sorafs_reference_sdk_release_evidence import (  # noqa: E402
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_MAX_SMOKE_DURATION_SECS,
    DEFAULT_MIN_DOWNSTREAM_PACKAGES,
    DEFAULT_MIN_RELEASE_TARGETS,
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

PLAN_SCHEMA = "sorafs.reference_sdk.release_evidence_collection_plan.v1"
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
        "max_evidence_age_secs",
        "min_release_targets",
        "min_downstream_packages",
        "max_smoke_duration_secs",
    }
)
PLAN_POSITIVE_THRESHOLD_FIELDS = frozenset(
    {
        "min_release_targets",
        "min_downstream_packages",
        "max_smoke_duration_secs",
        "now_unix",
    }
)
PLAN_NON_NEGATIVE_THRESHOLD_FIELDS = frozenset({"max_evidence_age_secs"})


@dataclass(frozen=True)
class CommandPlan:
    """One reference SDK release evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




EVIDENCE_OPTIONS_BY_KIND = {
    "release_archive": "release_archive_evidence",
    "signed_manifest": "signed_manifest_evidence",
    "supply_chain": "supply_chain_evidence",
    "downstream_bindings": "downstream_bindings_evidence",
    "cookbook_smoke": "cookbook_smoke_evidence",
    "ffi_header_contract": "ffi_header_contract_evidence",
    "governance_approval": "governance_approval_evidence",
}

EVIDENCE_FLAGS_BY_KIND = {
    "release_archive": "--release-archive-evidence",
    "signed_manifest": "--signed-manifest-evidence",
    "supply_chain": "--supply-chain-evidence",
    "downstream_bindings": "--downstream-bindings-evidence",
    "cookbook_smoke": "--cookbook-smoke-evidence",
    "ffi_header_contract": "--ffi-header-contract-evidence",
    "governance_approval": "--governance-approval-evidence",
}


def evidence_paths_by_kind(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied release evidence paths keyed by SF-11 evidence kind."""

    return {
        kind: list(getattr(args, option))
        for kind, option in EVIDENCE_OPTIONS_BY_KIND.items()
    }


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(
        args,
        summary_filename="release-summary.json",
        bundled_verifier=BUNDLED_VERIFIER,
    )
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    paths_by_kind = evidence_paths_by_kind(args)
    for kind in args.required_kinds:
        paths = paths_by_kind[kind]
        if not paths:
            errors.append(
                "missing required release evidence input"
            )
    require_no_unrequired_evidence(
        paths_by_kind,
        args.required_kinds,
        errors,
        diagnostic="release evidence supplied for unrequired kind",
    )

    for kind, paths in paths_by_kind.items():
        errors.extend(require_existing_files(paths, EVIDENCE_FLAGS_BY_KIND[kind], seen=seen_input_files))

    require_runner_positive_int(args, "now_unix", errors, allow_none=True)
    require_runner_non_negative_int(args, "max_evidence_age_secs", errors)
    require_runner_positive_int(args, "min_release_targets", errors)
    require_runner_positive_int(args, "min_downstream_packages", errors)
    require_runner_positive_int(args, "max_smoke_duration_secs", errors)
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    summary_out = args.summary_out or args.out_dir / "release-summary.json"
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
            "--max-evidence-age-secs",
            str(args.max_evidence_age_secs),
            "--min-release-targets",
            str(args.min_release_targets),
            "--min-downstream-packages",
            str(args.min_downstream_packages),
            "--max-smoke-duration-secs",
            str(args.max_smoke_duration_secs),
        ]
    )
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])

    return [CommandPlan("release_evidence_gate", summary_out, verifier_command)]


def threshold_values(args: argparse.Namespace) -> dict[str, int]:
    """Return threshold values rendered in dry-run plans."""

    thresholds: dict[str, int] = {
        "max_evidence_age_secs": args.max_evidence_age_secs,
        "min_release_targets": args.min_release_targets,
        "min_downstream_packages": args.min_downstream_packages,
        "max_smoke_duration_secs": args.max_smoke_duration_secs,
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
    """Validate the SF-11 collection-plan envelope before use."""

    return validate_runner_evidence_plan(
        rendered,
        plan,
        diagnostic_prefix="reference SDK release runner plan",
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
        description="Collect and verify SoraFS SF-11 reference SDK release evidence.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=BUNDLED_VERIFIER,
        help="Bundled release evidence verifier path; substitutions are rejected.",
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
            "Defaults to every SF-11 release kind."
        ),
    )
    for kind, flag in EVIDENCE_FLAGS_BY_KIND.items():
        parser.add_argument(
            flag,
            dest=EVIDENCE_OPTIONS_BY_KIND[kind],
            action="append",
            type=Path,
            default=[],
            help=f"Existing JSON artifact for `{kind}` release evidence.",
        )
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
    )
    parser.add_argument("--min-release-targets", type=positive_int_arg, default=DEFAULT_MIN_RELEASE_TARGETS)
    parser.add_argument(
        "--min-downstream-packages",
        type=positive_int_arg,
        default=DEFAULT_MIN_DOWNSTREAM_PACKAGES,
    )
    parser.add_argument(
        "--max-smoke-duration-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_SMOKE_DURATION_SECS,
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the verifier command plan without executing it.",
    )
    add_topology_qualification_argument(parser)
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
