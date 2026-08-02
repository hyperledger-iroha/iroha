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

BUNDLED_VERIFIER = SCRIPT_DIR / "check_sorafs_ai_prescreen_rollout_evidence.py"

from check_sorafs_ai_prescreen_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_EVIDENCE_AGE_SECS,
    DEFAULT_REQUIRED_KINDS,
    EVIDENCE_REQUIRED_FIELDS,
    KIND_BY_NAME,
    MAX_EVIDENCE_BYTES,
    SUMMARY_SCHEMA,
    ValidationOptions,
    validate_evidence_payload,
)
from sorafs_evidence_json import load_evidence_json  # noqa: E402
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
    require_equals_form_option_values,
)
from sorafs_evidence_validation import (  # noqa: E402
    require_rollout_deployment_context_review,
    require_rollout_deployment_id,
    require_rollout_environment,
)
from sorafs_runner_preflight import (  # noqa: E402
    emit_runner_error_block,
    emit_runner_error_lines,
    emit_runner_exception,
    run_command_plan,
    require_existing_dirs,
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_passthrough_args,
    require_runner_positive_int,
    require_runner_url_args,
    validate_runner_fixed_evidence_plan,
    validate_runner_plan_steps,
    validate_runner_preflight,
    write_runner_plan,
)


from sorafs_topology_qualification import add_topology_qualification_argument  # noqa: E402

PLAN_SCHEMA = "sorafs.moderation.ai_prescreen.rollout_evidence_collection_plan.v1"
PLAN_FIELDS = frozenset(
    {
        "schema",
        "verifier_summary_schema",
        "external_evidence",
        "evidence_contract",
        "steps",
    }
)
PLAN_EXTERNAL_EVIDENCE_FIELDS = frozenset(
    {"governance_dag", "end_to_end_workflow", "transparency_publication"}
)


@dataclass(frozen=True)
class CommandPlan:
    """One rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]


IROHA_ARG_EQUALS_FORM_DIAGNOSTIC = (
    "SoraFS runner --iroha-arg values must use --iroha-arg=VALUE form"
)
TRANSPARENCY_PRODUCER_EVIDENCE_READ_DIAGNOSTIC = (
    "pre-collected moderation transparency producer evidence cannot be read"
)
TRANSPARENCY_PRODUCER_EVIDENCE_INVALID_DIAGNOSTIC = (
    "pre-collected moderation transparency producer evidence is invalid"
)
TRANSPARENCY_PRODUCER_EVIDENCE_CONTEXT_DIAGNOSTIC = (
    "pre-collected moderation transparency producer evidence must match rollout context"
)


def validate_transparency_producer_evidence(args: argparse.Namespace) -> list[str]:
    """Validate trusted producer evidence before contacting live services."""

    try:
        payload = load_evidence_json(
            args.transparency_producer_evidence,
            MAX_EVIDENCE_BYTES,
        )
    except (OSError, RuntimeError, UnicodeDecodeError, ValueError):
        return [TRANSPARENCY_PRODUCER_EVIDENCE_READ_DIAGNOSTIC]
    kind_name, validation_errors = validate_evidence_payload(
        payload,
        ValidationOptions(
            now_unix=args.now_unix,
            max_evidence_age_secs=args.max_evidence_age_secs,
        ),
    )
    if kind_name != "transparency_publication" or validation_errors:
        return [TRANSPARENCY_PRODUCER_EVIDENCE_INVALID_DIAGNOSTIC]
    if (
        payload.get("deployment_id") != args.deployment_id
        or payload.get("environment") != args.environment
        or payload.get("deployment_context_reviewed") is not True
    ):
        return [TRANSPARENCY_PRODUCER_EVIDENCE_CONTEXT_DIAGNOSTIC]
    return []


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(
        args,
        summary_filename="rollout-summary.json",
        bundled_verifier=BUNDLED_VERIFIER,
    )
    require_runner_passthrough_args(
        args,
        ("sorafs_cli_bin", "iroha_bin"),
        ("iroha_arg",),
        errors,
    )
    require_runner_url_args(
        args,
        (
            "runner_url",
            "committee_url",
            "operator_url",
            "notification_webhook_url",
        ),
        errors,
    )
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    seen_input_dirs: dict[Path, tuple[str, Path]] = {}
    errors.extend(require_existing_files([args.manifest], "--manifest", seen=seen_input_files))
    errors.extend(require_existing_files([args.runner_payload], "--runner-payload", seen=seen_input_files))
    errors.extend(require_existing_files(args.committee_result, "--committee-result", seen=seen_input_files))
    errors.extend(
        require_existing_files([args.juror_notifications_manifest], "--juror-notifications-manifest", seen=seen_input_files)
    )
    errors.extend(
        require_existing_files([args.executor_execution_summary], "--executor-execution-summary", seen=seen_input_files)
    )
    producer_evidence_errors = require_existing_files(
        [args.transparency_producer_evidence],
        "--transparency-producer-evidence",
        seen=seen_input_files,
    )
    errors.extend(producer_evidence_errors)
    if not producer_evidence_errors:
        errors.extend(validate_transparency_producer_evidence(args))
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
    require_runner_positive_int(args, "now_unix", errors)
    require_runner_positive_int(args, "generated_at_unix", errors)
    require_runner_positive_int(args, "runner_checked_at", errors)
    require_runner_positive_int(args, "runner_process_isolation_verified_at", errors)
    require_runner_positive_int(args, "committee_checked_at", errors)
    require_runner_positive_int(args, "committee_process_isolation_verified_at", errors)
    require_runner_non_negative_int(args, "max_evidence_age_secs", errors)
    context = {
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": args.deployment_context_reviewed == "true",
    }
    require_rollout_deployment_id(context, errors)
    require_rollout_environment(context, errors)
    require_rollout_deployment_context_review(context, errors)
    if args.runner_checked_at != args.generated_at_unix:
        errors.append("--runner-checked-at must equal --generated-at-unix")
    if args.committee_checked_at != args.generated_at_unix:
        errors.append("--committee-checked-at must equal --generated-at-unix")
    if args.committee_process_isolation_verified_at > args.generated_at_unix:
        errors.append(
            "--committee-process-isolation-verified-at must not be after --generated-at-unix"
        )
    if args.screened_at > args.runner_checked_at:
        errors.append("--screened-at must not be after --runner-checked-at")
    if args.runner_process_isolation_verified_at > args.generated_at_unix:
        errors.append(
            "--runner-process-isolation-verified-at must not be after --generated-at-unix"
        )
    isolation_digest = args.runner_process_isolation_attestation_digest
    if len(isolation_digest) != 64 or any(
        character not in "0123456789abcdef" for character in isolation_digest
    ):
        errors.append(
            "--runner-process-isolation-attestation-digest must be exactly 64 lowercase hexadecimal characters"
        )
    else:
        digest_bytes = bytes.fromhex(isolation_digest)
        if len(set(digest_bytes)) == 1 or digest_bytes[:16] == digest_bytes[16:]:
            errors.append(
                "--runner-process-isolation-attestation-digest must not be a zero/repeated placeholder digest"
            )
    committee_isolation_digest = args.committee_process_isolation_attestation_digest
    if len(committee_isolation_digest) != 64 or any(
        character not in "0123456789abcdef" for character in committee_isolation_digest
    ):
        errors.append(
            "--committee-process-isolation-attestation-digest must be exactly 64 lowercase hexadecimal characters"
        )
    else:
        digest_bytes = bytes.fromhex(committee_isolation_digest)
        if len(set(digest_bytes)) == 1 or digest_bytes[:16] == digest_bytes[16:]:
            errors.append(
                "--committee-process-isolation-attestation-digest must not be a zero/repeated placeholder digest"
            )
    if args.generated_at_unix > args.now_unix:
        errors.append("--generated-at-unix must not be after --now-unix")
    elif args.now_unix - args.generated_at_unix > args.max_evidence_age_secs:
        errors.append(
            "--generated-at-unix exceeds --max-evidence-age-secs at --now-unix"
        )
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
        f"--generated-at-unix={args.generated_at_unix}",
        f"--deployment-id={args.deployment_id}",
        f"--environment={args.environment}",
        f"--deployment-context-reviewed={args.deployment_context_reviewed}",
        f"--process-isolation-enforcement={args.runner_process_isolation_enforcement}",
        f"--process-isolation-attestation-digest={args.runner_process_isolation_attestation_digest}",
        f"--process-isolation-verified-at={args.runner_process_isolation_verified_at}",
        f"--process-isolation-reviewed={args.runner_process_isolation_reviewed}",
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
        f"--generated-at-unix={args.generated_at_unix}",
        f"--deployment-id={args.deployment_id}",
        f"--environment={args.environment}",
        f"--deployment-context-reviewed={args.deployment_context_reviewed}",
        f"--process-isolation-enforcement={args.committee_process_isolation_enforcement}",
        f"--process-isolation-attestation-digest={args.committee_process_isolation_attestation_digest}",
        f"--process-isolation-verified-at={args.committee_process_isolation_verified_at}",
        f"--process-isolation-reviewed={args.committee_process_isolation_reviewed}",
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

    verifier_command = [
        sys.executable,
        str(BUNDLED_VERIFIER),
        "--evidence-dir",
        str(out_dir),
        "--evidence",
        str(args.governance_dag_evidence),
        "--evidence",
        str(args.e2e_evidence),
        "--evidence",
        str(args.transparency_producer_evidence),
        "--summary-out",
        str(summary_out),
        "--topology-qualification-summary",
        str(args.topology_qualification_summary),
        "--max-evidence-age-secs",
        str(args.max_evidence_age_secs),
    ]
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])

    return [
        CommandPlan("runner_canary", runner_out, runner_command),
        CommandPlan("committee_canary", committee_out, committee_command),
        CommandPlan("operator_workflow_canary", operator_out, operator_command),
        CommandPlan("notification_transport_canary", notification_out, notification_command),
        CommandPlan("commit_reveal_executor_canary", executor_out, executor_command),
        CommandPlan("rollout_evidence_gate", summary_out, verifier_command),
    ]


def external_evidence(args: argparse.Namespace) -> dict[str, str]:
    """Return reviewed external evidence paths rendered in dry-run plans."""

    return {
        "governance_dag": str(args.governance_dag_evidence),
        "end_to_end_workflow": str(args.e2e_evidence),
        "transparency_publication": str(args.transparency_producer_evidence),
    }


def evidence_contract() -> dict[str, dict[str, object]]:
    """Return the checker-backed evidence contract rendered in dry-run plans."""

    return {
        kind: {
            "schema": KIND_BY_NAME[kind].schema,
            "required_payload_fields": list(EVIDENCE_REQUIRED_FIELDS[kind]),
        }
        for kind in DEFAULT_REQUIRED_KINDS
    }


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    return {
        "schema": PLAN_SCHEMA,
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "external_evidence": external_evidence(args),
        "evidence_contract": evidence_contract(),
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
    """Validate the AI pre-screen collection-plan envelope before use."""

    return validate_runner_fixed_evidence_plan(
        rendered,
        plan,
        diagnostic_prefix="AI pre-screen rollout runner plan",
        plan_schema=PLAN_SCHEMA,
        plan_fields=PLAN_FIELDS,
        summary_schema=SUMMARY_SCHEMA,
        external_evidence=external_evidence(args),
        external_evidence_fields=PLAN_EXTERNAL_EVIDENCE_FIELDS,
        known_kinds=KIND_BY_NAME,
        evidence_contract=evidence_contract(),
        evidence_required_fields=EVIDENCE_REQUIRED_FIELDS,
    )


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
        default=BUNDLED_VERIFIER,
        help="Bundled rollout evidence verifier path; substitutions are rejected.",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        required=True,
        help="Directory where generated rollout evidence artifacts will be written.",
    )
    parser.add_argument(
        "--deployment-id",
        required=True,
        help="Reviewed rollout deployment identifier shared by all canaries.",
    )
    parser.add_argument(
        "--environment",
        required=True,
        help="Reviewed rollout environment shared by all canaries.",
    )
    parser.add_argument(
        "--deployment-context-reviewed",
        choices=("true",),
        required=True,
        help="Explicit acknowledgement that deployment id and environment were reviewed.",
    )
    parser.add_argument(
        "--generated-at-unix",
        type=positive_int_arg,
        required=True,
        help="Reviewed completion timestamp emitted by runner and committee canaries.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional verifier summary path. Defaults under --out-dir.",
    )
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        required=True,
        help="Validator clock used for verifier freshness checks.",
    )
    parser.add_argument(
        "--max-evidence-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_EVIDENCE_AGE_SECS,
        help="Maximum accepted age for generated evidence timestamps.",
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
    parser.add_argument(
        "--runner-checked-at",
        type=positive_int_arg,
        required=True,
        help="Reviewed runner canary check time; must equal --generated-at-unix.",
    )
    parser.add_argument("--runner-notes", help="Optional runner canary notes.")
    parser.add_argument("--runner-timeout-ms", type=positive_int_arg, default=30_000)
    parser.add_argument(
        "--runner-process-isolation-enforcement",
        choices=("systemd_ip_filter", "container_network_policy", "host_firewall"),
        required=True,
    )
    parser.add_argument(
        "--runner-process-isolation-attestation-digest",
        required=True,
    )
    parser.add_argument(
        "--runner-process-isolation-verified-at",
        type=positive_int_arg,
        required=True,
    )
    parser.add_argument(
        "--runner-process-isolation-reviewed",
        choices=("true",),
        required=True,
    )
    parser.add_argument("--committee-url", required=True, help="Deployed committee base URL.")
    parser.add_argument("--quorum", type=positive_int_arg, required=True, help="Committee quorum.")
    parser.add_argument(
        "--committee-result",
        action="append",
        type=Path,
        default=[],
        help="Payload-free runner result JSON for committee canary. Repeat at least quorum times.",
    )
    parser.add_argument(
        "--committee-checked-at",
        type=positive_int_arg,
        required=True,
        help="Reviewed committee check time; must equal --generated-at-unix.",
    )
    parser.add_argument("--committee-notes", help="Optional committee canary notes.")
    parser.add_argument("--committee-timeout-ms", type=positive_int_arg, default=30_000)
    parser.add_argument(
        "--committee-process-isolation-enforcement",
        choices=("systemd_ip_filter", "container_network_policy", "host_firewall"),
        required=True,
    )
    parser.add_argument(
        "--committee-process-isolation-attestation-digest",
        required=True,
    )
    parser.add_argument(
        "--committee-process-isolation-verified-at",
        type=positive_int_arg,
        required=True,
    )
    parser.add_argument(
        "--committee-process-isolation-reviewed",
        choices=("true",),
        required=True,
    )
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
        "--transparency-producer-evidence",
        type=Path,
        required=True,
        help=(
            "Pre-collected, checker-valid evidence from trusted internal moderation "
            "transparency producers. Generic live source-entry submission is not supported."
        ),
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
    add_topology_qualification_argument(parser)
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
        expanded_args = require_equals_form_option_values(
            expanded_args,
            "--iroha-arg",
            IROHA_ARG_EQUALS_FORM_DIAGNOSTIC,
        )
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
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
