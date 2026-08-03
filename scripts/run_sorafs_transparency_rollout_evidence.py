#!/usr/bin/env python3
"""Collect and verify SoraFS transparency rollout evidence."""

from __future__ import annotations

import argparse
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

BUNDLED_VERIFIER = SCRIPT_DIR / "check_sorafs_transparency_rollout_evidence.py"

from check_sorafs_transparency_rollout_evidence import (  # noqa: E402
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
from sorafs_checker_preflight import fsync_checker_output_parent  # noqa: E402
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
    require_equals_form_option_values,
)
from sorafs_evidence_validation import (  # noqa: E402
    require_rollout_deployment_id,
    require_rollout_environment,
)
from sorafs_runner_preflight import (  # noqa: E402
    emit_runner_error_block,
    emit_runner_error_lines,
    emit_runner_exception,
    inspect_runner_path_is_file,
    inspect_runner_path_is_symlink,
    run_command_plan,
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_passthrough_args,
    require_runner_positive_int,
    require_runner_url_args,
    render_runner_plan,
    validate_runner_input_parent_chain,
    validate_runner_context_evidence_plan,
    validate_runner_plan_steps,
    validate_runner_preflight,
    write_runner_plan,
)

DEPLOYMENT_CONTEXT_ARTIFACT_INSPECTION_DIAGNOSTIC = (
    "deployment-context artifact cannot be inspected"
)
DEPLOYMENT_CONTEXT_ARTIFACT_SYMLINK_DIAGNOSTIC = (
    "deployment-context artifact must not be a symlink"
)
DEPLOYMENT_CONTEXT_ARTIFACT_MISSING_DIAGNOSTIC = (
    "deployment-context artifact must exist and be a file"
)
DEPLOYMENT_CONTEXT_ARTIFACT_PARENT_DIAGNOSTIC = (
    "deployment-context artifact parent is invalid"
)
DEPLOYMENT_CONTEXT_ARTIFACT_READ_DIAGNOSTIC = (
    "generated evidence artifact cannot be read"
)
DEPLOYMENT_CONTEXT_ARTIFACT_WRITE_DIAGNOSTIC = (
    "deployment context cannot be written into generated artifact"
)
DEPLOYMENT_CONTEXT_ARTIFACT_CONFLICT_DIAGNOSTIC = (
    "generated evidence artifact has conflicting deployment context"
)
CYCLE_ID_HEX_PATTERN = re.compile(r"^[0-9a-f]{32}$")
from sorafs_topology_qualification import add_topology_qualification_argument  # noqa: E402

PLAN_SCHEMA = "sorafs.transparency.rollout_evidence_collection_plan.v1"
PLAN_FIELDS = frozenset(
    {
        "schema",
        "verifier_summary_schema",
        "deployment_context",
        "external_evidence",
        "evidence_contract",
        "steps",
    }
)
PLAN_DEPLOYMENT_CONTEXT_FIELDS = frozenset(
    {"deployment_id", "environment", "deployment_context_reviewed"}
)
PLAN_EXTERNAL_EVIDENCE_FIELDS = frozenset({"source_entry"})


@dataclass(frozen=True)
class CommandPlan:
    """One rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]


IROHA_ARG_EQUALS_FORM_DIAGNOSTIC = (
    "SoraFS runner --iroha-arg values must use --iroha-arg=VALUE form"
)
SOURCE_PRODUCER_EVIDENCE_READ_DIAGNOSTIC = (
    "pre-collected source-entry producer evidence cannot be read"
)
SOURCE_PRODUCER_EVIDENCE_INVALID_DIAGNOSTIC = (
    "pre-collected source-entry producer evidence is invalid"
)
SOURCE_PRODUCER_EVIDENCE_CONTEXT_DIAGNOSTIC = (
    "pre-collected source-entry producer evidence must match rollout context"
)


def validate_source_entry_producer_evidence(args: argparse.Namespace) -> list[str]:
    """Validate trusted producer evidence before contacting any live service."""

    try:
        payload = load_evidence_json(
            args.source_entry_producer_evidence,
            MAX_EVIDENCE_BYTES,
        )
    except (OSError, RuntimeError, UnicodeDecodeError, ValueError):
        return [SOURCE_PRODUCER_EVIDENCE_READ_DIAGNOSTIC]
    kind_name, validation_errors = validate_evidence_payload(
        payload,
        ValidationOptions(
            now_unix=args.now_unix,
            max_evidence_age_secs=args.max_evidence_age_secs,
        ),
    )
    if kind_name != "source_entry" or validation_errors:
        return [SOURCE_PRODUCER_EVIDENCE_INVALID_DIAGNOSTIC]
    if (
        payload.get("deployment_id") != args.deployment_id
        or payload.get("environment") != args.environment
        or payload.get("deployment_context_reviewed") is not True
    ):
        return [SOURCE_PRODUCER_EVIDENCE_CONTEXT_DIAGNOSTIC]
    return []


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(
        args,
        summary_filename="rollout-summary.json",
        bundled_verifier=BUNDLED_VERIFIER,
    )
    require_runner_passthrough_args(
        args,
        ("iroha_bin",),
        ("iroha_arg",),
        errors,
    )
    require_runner_url_args(args, ("torii_url",), errors)
    require_rollout_deployment_id({"deployment_id": args.deployment_id}, errors)
    require_rollout_environment({"environment": args.environment}, errors)
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    producer_evidence_errors = require_existing_files(
        [args.source_entry_producer_evidence],
        "--source-entry-producer-evidence",
        seen=seen_input_files,
    )
    errors.extend(producer_evidence_errors)
    if not producer_evidence_errors:
        errors.extend(validate_source_entry_producer_evidence(args))
    errors.extend(require_existing_files(args.privacy_source_event, "--privacy-source-event", seen=seen_input_files))
    errors.extend(require_existing_files(args.privacy_publish_due, "--privacy-publish-due", seen=seen_input_files))
    errors.extend(require_existing_files(args.proof_token_issuance, "--proof-token-issuance", seen=seen_input_files))

    if not args.privacy_source_event:
        errors.append("at least one --privacy-source-event payload is required")
    if not args.privacy_publish_due:
        errors.append("at least one --privacy-publish-due payload is required")
    if not args.proof_token_issuance:
        errors.append("at least one --proof-token-issuance payload is required")
    if not args.cycle_id:
        errors.append("at least one --cycle-id is required for publication detail evidence")
    elif any(CYCLE_ID_HEX_PATTERN.fullmatch(cycle_id) is None for cycle_id in args.cycle_id):
        errors.append("--cycle-id must be a 16-byte lowercase hex string")
    require_runner_positive_int(args, "limit", errors)
    require_runner_positive_int(args, "timeout_secs", errors)
    require_runner_positive_int(args, "now_unix", errors, allow_none=True)
    require_runner_non_negative_int(args, "max_evidence_age_secs", errors)
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    out_dir = args.out_dir
    verifier = BUNDLED_VERIFIER
    iroha_prefix = [args.iroha_bin, *args.iroha_arg]
    summary_out = args.summary_out or out_dir / "rollout-summary.json"
    privacy_aggregate_out = out_dir / "privacy-aggregate.json"
    proof_token_out = out_dir / "proof-token-issuance.json"
    publication_out = out_dir / "publication.json"
    explorer_out = out_dir / "explorer.json"

    privacy_command = [
        *iroha_prefix,
        "sorafs",
        "transparency",
        "privacy-aggregate",
        "canary",
        "--out",
        str(privacy_aggregate_out),
    ]
    for path in args.privacy_source_event:
        privacy_command.extend(["--source-event", str(path)])
    for path in args.privacy_publish_due:
        privacy_command.extend(["--publish-due", str(path)])

    proof_token_command = [
        *iroha_prefix,
        "sorafs",
        "transparency",
        "token-issuance",
        "canary",
        "--out",
        str(proof_token_out),
    ]
    for path in args.proof_token_issuance:
        proof_token_command.extend(["--issuance", str(path)])

    publication_command = [
        *iroha_prefix,
        "sorafs",
        "transparency",
        "publication-canary",
        "--torii-url",
        args.torii_url,
        "--limit",
        str(args.limit),
        "--timeout-secs",
        str(args.timeout_secs),
        "--out",
        str(publication_out),
    ]
    for cycle_id in args.cycle_id:
        publication_command.extend(["--cycle-id", cycle_id])

    explorer_command = [
        *iroha_prefix,
        "sorafs",
        "transparency",
        "explorer-canary",
        "--torii-url",
        args.torii_url,
        "--limit",
        str(args.limit),
        "--timeout-secs",
        str(args.timeout_secs),
        "--out",
        str(explorer_out),
    ]

    verifier_command = [
        sys.executable,
        str(verifier),
        "--evidence-dir",
        str(out_dir),
        "--evidence",
        str(args.source_entry_producer_evidence),
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
        CommandPlan("privacy_aggregate_canary", privacy_aggregate_out, privacy_command),
        CommandPlan("proof_token_issuance_canary", proof_token_out, proof_token_command),
        CommandPlan("publication_canary", publication_out, publication_command),
        CommandPlan("explorer_canary", explorer_out, explorer_command),
        CommandPlan("rollout_evidence_gate", summary_out, verifier_command),
    ]


def evidence_contract() -> dict[str, dict[str, object]]:
    """Return the checker-backed evidence contract rendered in dry-run plans."""

    return {
        kind: {
            "schema": KIND_BY_NAME[kind].schema,
            "required_payload_fields": list(EVIDENCE_REQUIRED_FIELDS[kind]),
        }
        for kind in DEFAULT_REQUIRED_KINDS
    }


def deployment_context(args: argparse.Namespace) -> dict[str, object]:
    """Return the reviewed deployment context rendered in dry-run plans."""

    return {
        "deployment_id": args.deployment_id,
        "environment": args.environment,
        "deployment_context_reviewed": True,
    }


def external_evidence(args: argparse.Namespace) -> dict[str, str]:
    """Return trusted producer evidence rendered in dry-run plans."""

    return {"source_entry": str(args.source_entry_producer_evidence)}


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    return {
        "schema": PLAN_SCHEMA,
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "deployment_context": deployment_context(args),
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
    """Validate the transparency collection-plan envelope before use."""

    expected_context = deployment_context(args)
    context_errors: list[str] = []
    require_rollout_deployment_id(expected_context, context_errors)
    require_rollout_environment(expected_context, context_errors)
    return validate_runner_context_evidence_plan(
        rendered,
        plan,
        diagnostic_prefix="transparency rollout runner plan",
        plan_schema=PLAN_SCHEMA,
        plan_fields=PLAN_FIELDS,
        summary_schema=SUMMARY_SCHEMA,
        deployment_context=expected_context,
        deployment_context_fields=PLAN_DEPLOYMENT_CONTEXT_FIELDS,
        deployment_context_errors=context_errors,
        known_kinds=KIND_BY_NAME,
        evidence_contract=evidence_contract(),
        evidence_required_fields=EVIDENCE_REQUIRED_FIELDS,
        external_evidence=external_evidence(args),
        external_evidence_fields=PLAN_EXTERNAL_EVIDENCE_FIELDS,
    )


def deployment_context_write_open_flags() -> int:
    """Return descriptor flags for rewriting generated evidence artifacts."""

    flags = os.O_WRONLY | os.O_TRUNC
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    if nofollow:
        flags |= nofollow
    return flags


def validate_deployment_context_artifact_parent(path: Path) -> list[str]:
    """Return generic parent-chain errors for generated artifact rewrites."""

    parent_errors: list[str] = []
    if validate_runner_input_parent_chain(
        path,
        parent_errors,
        label="deployment-context artifact",
    ):
        return []
    return [DEPLOYMENT_CONTEXT_ARTIFACT_PARENT_DIAGNOSTIC]


def write_all_deployment_context_bytes(fd: int, payload: bytes) -> None:
    """Write all rendered deployment-context artifact bytes."""

    view = memoryview(payload)
    while view:
        written = os.write(fd, view)
        if written <= 0:
            raise OSError("short deployment-context artifact write")
        view = view[written:]


def write_deployment_context_artifact(path: Path, payload: dict[str, object]) -> list[str]:
    """Rewrite a generated artifact after stamping reviewed deployment context."""

    parent_errors = validate_deployment_context_artifact_parent(path)
    if parent_errors:
        return parent_errors
    fd = -1
    try:
        rendered = render_runner_plan(payload).encode("utf-8")
        fd = os.open(path, deployment_context_write_open_flags())
        write_all_deployment_context_bytes(fd, rendered)
        os.fsync(fd)
    except (OSError, RuntimeError, TypeError, ValueError) as error:
        del error
        return [DEPLOYMENT_CONTEXT_ARTIFACT_WRITE_DIAGNOSTIC]
    finally:
        if fd >= 0:
            os.close(fd)
    if fsync_checker_output_parent(path, label="deployment-context artifact"):
        return [DEPLOYMENT_CONTEXT_ARTIFACT_WRITE_DIAGNOSTIC]
    return []


def annotate_evidence_artifact(
    path: Path,
    *,
    deployment_id: str,
    environment: str,
) -> list[str]:
    """Attach reviewed rollout context to a generated canary artifact."""

    errors: list[str] = []
    artifact_is_symlink = inspect_runner_path_is_symlink(
        path,
        errors,
        label="deployment-context artifact",
    )
    if errors:
        return [DEPLOYMENT_CONTEXT_ARTIFACT_INSPECTION_DIAGNOSTIC]
    if artifact_is_symlink:
        return [DEPLOYMENT_CONTEXT_ARTIFACT_SYMLINK_DIAGNOSTIC]
    artifact_is_file = inspect_runner_path_is_file(
        path,
        errors,
        label="deployment-context artifact",
    )
    if errors:
        return [DEPLOYMENT_CONTEXT_ARTIFACT_INSPECTION_DIAGNOSTIC]
    if artifact_is_file is False:
        return [DEPLOYMENT_CONTEXT_ARTIFACT_MISSING_DIAGNOSTIC]

    try:
        payload = load_evidence_json(path, MAX_EVIDENCE_BYTES)
    except (OSError, RuntimeError, UnicodeDecodeError, ValueError) as error:
        del error
        return [DEPLOYMENT_CONTEXT_ARTIFACT_READ_DIAGNOSTIC]

    for field, value in (
        ("deployment_id", deployment_id),
        ("environment", environment),
        ("deployment_context_reviewed", True),
    ):
        existing = payload.get(field)
        if existing is not None and existing != value:
            return [DEPLOYMENT_CONTEXT_ARTIFACT_CONFLICT_DIAGNOSTIC]
        payload[field] = value

    return write_deployment_context_artifact(path, payload)


def annotate_evidence_artifacts(
    plan: Sequence[CommandPlan],
    *,
    deployment_id: str,
    environment: str,
) -> list[str]:
    """Attach rollout context to generated canary artifacts before verification."""

    errors: list[str] = []
    for step in plan:
        if step.artifact is None:
            continue
        errors.extend(
            annotate_evidence_artifact(
                step.artifact,
                deployment_id=deployment_id,
                environment=environment,
            )
        )
    return errors


def run_plan(plan: Sequence[CommandPlan], out_dir: Path, args: argparse.Namespace) -> int:
    canary_plan = plan[:-1]
    verifier_plan = plan[-1:]
    exit_code = run_command_plan(canary_plan, out_dir)
    if exit_code != 0:
        return exit_code
    annotation_errors = annotate_evidence_artifacts(
        canary_plan,
        deployment_id=args.deployment_id,
        environment=args.environment,
    )
    if annotation_errors:
        emit_runner_error_lines(annotation_errors)
        return 1
    return run_command_plan(verifier_plan, out_dir)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Collect and verify SoraFS transparency rollout evidence.",
    )
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
        "--torii-url",
        required=True,
        help="Deployed Torii or public transparency gateway base URL.",
    )
    parser.add_argument(
        "--deployment-id",
        required=True,
        help="Reviewed production/staging deployment id to stamp onto generated evidence.",
    )
    parser.add_argument(
        "--environment",
        required=True,
        help="Reviewed rollout environment label: staging, production, prod, or release.",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        required=True,
        help="Directory where rollout evidence artifacts will be written.",
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
    parser.add_argument(
        "--source-entry-producer-evidence",
        type=Path,
        required=True,
        help=(
            "Pre-collected, checker-valid source-entry evidence from trusted internal "
            "producers. Generic live source-entry submission is not supported."
        ),
    )
    parser.add_argument(
        "--privacy-source-event",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Privacy aggregate source-event canary payload.",
    )
    parser.add_argument(
        "--privacy-publish-due",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Privacy aggregate publish-due canary payload.",
    )
    parser.add_argument(
        "--proof-token-issuance",
        action="append",
        type=Path,
        default=[],
        metavar="PATH",
        help="Proof-token issuance canary payload.",
    )
    parser.add_argument(
        "--cycle-id",
        action="append",
        default=[],
        metavar="HEX",
        help="Published transparency cycle id to verify through cycle detail.",
    )
    parser.add_argument("--limit", type=positive_int_arg, default=50, help="Readback canary limit.")
    parser.add_argument(
        "--timeout-secs",
        type=positive_int_arg,
        default=30,
        help="HTTP timeout passed to readback canaries.",
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
        emit_runner_error_block("ERROR: SoraFS rollout evidence inputs are incomplete:", errors)
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
    return run_plan(plan, args.out_dir, args)


if __name__ == "__main__":
    raise SystemExit(main())
