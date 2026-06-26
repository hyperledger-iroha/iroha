#!/usr/bin/env python3
"""Collect and verify SoraFS AI pre-screening rollout evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_ai_prescreen_rollout_evidence import (  # noqa: E402
    REQUIRED_TRANSPARENCY_SOURCE_KINDS,
    SUMMARY_SCHEMA,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    positive_int_arg,
)
from sorafs_runner_preflight import (  # noqa: E402
    emit_runner_error_block,
    emit_runner_error_lines,
    run_command_plan,
    require_existing_dirs,
    require_existing_files,
    require_runner_positive_int,
    validate_runner_preflight,
    write_runner_plan,
)


@dataclass(frozen=True)
class CommandPlan:
    """One rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




def normalize_iroha_arg_values(args: Sequence[str]) -> list[str]:
    """Let --iroha-arg accept values that look like options."""

    normalized: list[str] = []
    index = 0
    while index < len(args):
        arg = args[index]
        if arg == "--iroha-arg" and index + 1 < len(args):
            normalized.append(f"--iroha-arg={args[index + 1]}")
            index += 2
            continue
        normalized.append(arg)
        index += 1
    return normalized


def split_source_entry_spec(spec: str) -> tuple[str, Path]:
    source_kind, separator, path = spec.partition("=")
    source_kind = source_kind.strip()
    path = path.strip()
    if not separator or not source_kind or not path:
        raise ValueError(f"--source-entry must use KIND=PATH form, got `{spec}`")
    return source_kind, Path(path)


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(args, summary_filename="rollout-summary.json")
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    seen_input_dirs: dict[Path, tuple[str, Path]] = {}
    source_entries: list[tuple[str, Path]] = []
    for spec in args.source_entry:
        try:
            source_entries.append(split_source_entry_spec(spec))
        except ValueError as error:
            errors.append(str(error))

    present_source_kinds = {source_kind for source_kind, _ in source_entries}
    for source_kind in REQUIRED_TRANSPARENCY_SOURCE_KINDS:
        if source_kind not in present_source_kinds:
            errors.append(f"missing --source-entry coverage for `{source_kind}`")

    source_paths = [path for _, path in source_entries]
    errors.extend(require_existing_files([args.manifest], "--manifest", seen=seen_input_files))
    errors.extend(require_existing_files([args.runner_payload], "--runner-payload", seen=seen_input_files))
    errors.extend(require_existing_files(args.committee_result, "--committee-result", seen=seen_input_files))
    errors.extend(
        require_existing_files([args.juror_notifications_manifest], "--juror-notifications-manifest", seen=seen_input_files)
    )
    errors.extend(
        require_existing_files([args.executor_execution_summary], "--executor-execution-summary", seen=seen_input_files)
    )
    errors.extend(require_existing_files(source_paths, "--source-entry", seen=seen_input_files))
    errors.extend(require_existing_files([args.governance_dag_evidence], "--governance-dag-evidence", seen=seen_input_files))
    errors.extend(require_existing_files([args.e2e_evidence], "--e2e-evidence", seen=seen_input_files))
    errors.extend(
        require_existing_dirs(
            [args.executor_bundle],
            "--executor-bundle",
            seen=seen_input_dirs,
        )
    )

    quorum_valid = require_runner_positive_int(args, "quorum", errors)
    if quorum_valid and len(args.committee_result) < args.quorum:
        errors.append("--committee-result count must be at least --quorum")
    require_runner_positive_int(args, "screened_at", errors)
    require_runner_positive_int(args, "runner_timeout_ms", errors)
    require_runner_positive_int(args, "committee_timeout_ms", errors)
    require_runner_positive_int(args, "operator_timeout_secs", errors)
    require_runner_positive_int(args, "notification_timeout_secs", errors)
    require_runner_positive_int(args, "limit", errors, allow_none=True)
    return errors


def maybe_append_option(command: list[str], name: str, value: object | None) -> None:
    if value is not None:
        command.append(f"{name}={value}")


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    out_dir = args.out_dir
    summary_out = args.summary_out or out_dir / "rollout-summary.json"
    runner_out = out_dir / "runner.json"
    committee_out = out_dir / "committee.json"
    operator_out = out_dir / "operator-workflow.json"
    notification_out = out_dir / "notification-transport.json"
    executor_out = out_dir / "commit-reveal-executor.json"
    transparency_out = out_dir / "transparency-publication.json"
    iroha_prefix = [args.iroha_bin, *args.iroha_arg]

    runner_command = [
        args.sorafs_cli_bin,
        "moderation",
        "runner-canary",
        f"--manifest={args.manifest}",
        f"--format={args.manifest_format}",
        f"--runner-url={args.runner_url}",
        f"--payload={args.runner_payload}",
        f"--subject={args.runner_subject}",
        f"--screened-at={args.screened_at}",
        f"--timeout-ms={args.runner_timeout_ms}",
        f"--json-out={runner_out}",
    ]
    maybe_append_option(runner_command, "--checked-at", args.runner_checked_at)
    maybe_append_option(runner_command, "--notes", args.runner_notes)

    committee_command = [
        args.sorafs_cli_bin,
        "moderation",
        "committee-canary",
        f"--manifest={args.manifest}",
        f"--format={args.manifest_format}",
        f"--committee-url={args.committee_url}",
        f"--quorum={args.quorum}",
        f"--timeout-ms={args.committee_timeout_ms}",
        f"--json-out={committee_out}",
    ]
    for result in args.committee_result:
        committee_command.append(f"--result={result}")
    maybe_append_option(committee_command, "--checked-at", args.committee_checked_at)
    maybe_append_option(committee_command, "--notes", args.committee_notes)

    operator_command = [
        *iroha_prefix,
        "sorafs",
        "moderation",
        "quarantine",
        "operator-canary",
        "--operator-url",
        args.operator_url,
        "--quarantine-id",
        args.quarantine_id,
        "--timeout-secs",
        str(args.operator_timeout_secs),
        "--out",
        str(operator_out),
    ]
    if args.limit is not None:
        operator_command.extend(["--limit", str(args.limit)])

    notification_command = [
        *iroha_prefix,
        "sorafs",
        "moderation",
        "quarantine",
        "notifications",
        "canary",
        "--manifest",
        str(args.juror_notifications_manifest),
        "--webhook-url",
        args.notification_webhook_url,
        "--timeout-secs",
        str(args.notification_timeout_secs),
        "--out",
        str(notification_out),
    ]

    executor_command = [
        *iroha_prefix,
        "sorafs",
        "moderation",
        "ballots",
        "executor-canary",
        "--bundle",
        str(args.executor_bundle),
        "--execution-summary",
        str(args.executor_execution_summary),
        "--out",
        str(executor_out),
    ]

    transparency_command = [
        *iroha_prefix,
        "sorafs",
        "transparency",
        "source-entry",
        "canary",
        "--out",
        str(transparency_out),
    ]
    for spec in args.source_entry:
        transparency_command.extend(["--source-entry", spec])

    verifier_command = [
        sys.executable,
        str(args.verifier),
        "--evidence-dir",
        str(out_dir),
        "--evidence",
        str(args.governance_dag_evidence),
        "--evidence",
        str(args.e2e_evidence),
        "--summary-out",
        str(summary_out),
    ]

    return [
        CommandPlan("runner_canary", runner_out, runner_command),
        CommandPlan("committee_canary", committee_out, committee_command),
        CommandPlan("operator_workflow_canary", operator_out, operator_command),
        CommandPlan("notification_transport_canary", notification_out, notification_command),
        CommandPlan("commit_reveal_executor_canary", executor_out, executor_command),
        CommandPlan("transparency_source_entry_canary", transparency_out, transparency_command),
        CommandPlan("rollout_evidence_gate", summary_out, verifier_command),
    ]


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    return {
        "schema": "sorafs.moderation.ai_prescreen.rollout_evidence_collection_plan.v1",
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "external_evidence": {
            "governance_dag": str(args.governance_dag_evidence),
            "end_to_end_workflow": str(args.e2e_evidence),
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
        description="Collect and verify SoraFS AI pre-screening rollout evidence.",
    )
    parser.add_argument("--sorafs-cli-bin", default="sorafs_cli", help="sorafs_cli binary to run.")
    parser.add_argument("--iroha-bin", default="iroha", help="iroha CLI binary to run.")
    parser.add_argument(
        "--iroha-arg",
        action="append",
        default=[],
        help="Global iroha CLI argument to pass before `sorafs`. Repeat as needed.",
    )
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_ai_prescreen_rollout_evidence.py",
        help="Rollout evidence verifier script path.",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        required=True,
        help="Directory where generated rollout evidence artifacts will be written.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional verifier summary path. Defaults under --out-dir.",
    )
    parser.add_argument("--manifest", type=Path, required=True, help="Moderation manifest path.")
    parser.add_argument(
        "--manifest-format",
        default="json",
        choices=("json", "norito"),
        help="Format passed to runner and committee canaries.",
    )
    parser.add_argument("--runner-url", required=True, help="Deployed runner base URL.")
    parser.add_argument(
        "--runner-payload",
        type=Path,
        required=True,
        help="Payload bytes used by runner canary.",
    )
    parser.add_argument("--runner-subject", required=True, help="Subject id for runner canary.")
    parser.add_argument("--screened-at", type=positive_int_arg, required=True, help="Runner screening time.")
    parser.add_argument("--runner-checked-at", type=positive_int_arg, help="Pinned runner canary check time.")
    parser.add_argument("--runner-notes", help="Optional runner canary notes.")
    parser.add_argument("--runner-timeout-ms", type=positive_int_arg, default=30_000)
    parser.add_argument("--committee-url", required=True, help="Deployed committee base URL.")
    parser.add_argument("--quorum", type=positive_int_arg, required=True, help="Committee quorum.")
    parser.add_argument(
        "--committee-result",
        action="append",
        type=Path,
        default=[],
        help="Payload-free runner result JSON for committee canary. Repeat at least quorum times.",
    )
    parser.add_argument("--committee-checked-at", type=positive_int_arg, help="Pinned committee check time.")
    parser.add_argument("--committee-notes", help="Optional committee canary notes.")
    parser.add_argument("--committee-timeout-ms", type=positive_int_arg, default=30_000)
    parser.add_argument("--operator-url", required=True, help="Deployed operator service URL.")
    parser.add_argument("--quarantine-id", required=True, help="16-byte quarantine id hex.")
    parser.add_argument("--limit", type=positive_int_arg, help="Optional operator readback limit.")
    parser.add_argument("--operator-timeout-secs", type=positive_int_arg, default=30)
    parser.add_argument(
        "--juror-notifications-manifest",
        type=Path,
        required=True,
        help="Payload-free juror notifications manifest for notification canary.",
    )
    parser.add_argument(
        "--notification-webhook-url",
        required=True,
        help="Deployed juror notification webhook URL.",
    )
    parser.add_argument("--notification-timeout-secs", type=positive_int_arg, default=10)
    parser.add_argument(
        "--executor-bundle",
        type=Path,
        required=True,
        help="Executor bundle directory produced by executor-bundle.",
    )
    parser.add_argument(
        "--executor-execution-summary",
        type=Path,
        required=True,
        help="Payload-free deployed executor run summary.",
    )
    parser.add_argument(
        "--source-entry",
        action="append",
        default=[],
        metavar="KIND=PATH",
        help="Moderation transparency source-entry canary payload. Repeat for every required kind.",
    )
    parser.add_argument(
        "--governance-dag-evidence",
        type=Path,
        required=True,
        help="Payload-free Governance DAG rollout evidence JSON.",
    )
    parser.add_argument(
        "--e2e-evidence",
        type=Path,
        required=True,
        help="Payload-free end-to-end workflow evidence JSON.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the command plan JSON without running canaries.",
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_runner_error_lines((str(error),))
        raise SystemExit(2) from error
    expanded_args = normalize_iroha_arg_values(expanded_args)
    return parser.parse_args(expanded_args)


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
