#!/usr/bin/env python3
"""Collect and verify SoraFS reputation rollout evidence."""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_reputation_rollout_evidence import (  # noqa: E402
    DEFAULT_MAX_INGEST_LAG_SECS,
    DEFAULT_MAX_SNAPSHOT_AGE_SECS,
    DEFAULT_REQUIRED_KINDS,
    EVIDENCE_REQUIRED_FIELDS,
    KIND_BY_NAME,
    SUMMARY_SCHEMA,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)
from sorafs_path_identity import diagnostic_text_is_canonical  # noqa: E402
from sorafs_path_identity import error_diagnostic_label  # noqa: E402
from sorafs_runner_preflight import (  # noqa: E402
    emit_runner_error_block,
    emit_runner_error_lines,
    emit_runner_exception,
    run_command_plan,
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


PLAN_SCHEMA = "sorafs.reputation.rollout_evidence_collection_plan.v1"
PLAN_FIELDS = frozenset(
    {
        "schema",
        "verifier_summary_schema",
        "external_evidence",
        "evidence_contract",
        "steps",
    }
)
EXTERNAL_EVIDENCE_FIELDS = frozenset({"publish", "metrics", "transport", "consumption"})


@dataclass(frozen=True)
class CommandPlan:
    """One reputation rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]


def split_provider_proof_spec(spec: str) -> tuple[str, Path]:
    provider_id, separator, path = spec.partition("=")
    if (
        not separator
        or not provider_id
        or not path
        or not diagnostic_text_is_canonical(provider_id)
        or not diagnostic_text_is_canonical(path)
    ):
        raise ValueError("--provider-proof must use PROVIDER_ID=PATH form")
    return provider_id, Path(path)


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(args, summary_filename="rollout-summary.json")
    require_runner_passthrough_args(args, ("sorafs_cli_bin",), (), errors)
    require_runner_url_args(args, ("torii_url",), errors)
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    if not args.provider_id:
        errors.append("at least one --provider-id is required")
    seen_provider_ids: set[str] = set()
    for provider_id in args.provider_id:
        if not diagnostic_text_is_canonical(provider_id):
            errors.append("--provider-id must be canonical")
            continue
        if provider_id in seen_provider_ids:
            errors.append("duplicate --provider-id")
        seen_provider_ids.add(provider_id)

    proof_specs: dict[str, Path] = {}
    for spec in args.provider_proof:
        try:
            provider_id, path = split_provider_proof_spec(spec)
        except ValueError as error:
            errors.append(error_diagnostic_label(error))
            continue
        if provider_id in proof_specs:
            errors.append("duplicate --provider-proof")
        proof_specs[provider_id] = path

    for provider_id in args.provider_id:
        if provider_id not in proof_specs:
            errors.append("missing --provider-proof for requested provider")

    extra_proofs = sorted(set(proof_specs) - set(args.provider_id))
    if extra_proofs:
        errors.append("--provider-proof supplied for unrequested provider")

    errors.extend(require_existing_files([args.snapshot], "--snapshot", seen=seen_input_files))
    errors.extend(
        require_existing_files(
            [args.publish_evidence],
            "--publish-evidence",
            seen=seen_input_files,
        )
    )
    errors.extend(
        require_existing_files(
            list(proof_specs.values()),
            "--provider-proof",
            seen=seen_input_files,
        )
    )
    errors.extend(require_existing_files([args.metrics_evidence], "--metrics-evidence", seen=seen_input_files))
    errors.extend(require_existing_files([args.transport_evidence], "--transport-evidence", seen=seen_input_files))
    errors.extend(require_existing_files([args.consumption_evidence], "--consumption-evidence", seen=seen_input_files))

    require_runner_positive_int(args, "now_unix", errors, allow_none=True)
    require_runner_positive_int(args, "max_snapshot_age_secs", errors)
    require_runner_positive_int(args, "max_ingest_lag_secs", errors)
    require_runner_non_negative_int(args, "watch_since", errors)
    require_runner_positive_int(args, "watch_limit", errors)
    require_runner_positive_int(args, "watch_max_polls", errors)
    require_runner_non_negative_int(args, "watch_poll_interval_ms", errors)
    return errors


def provider_artifact_name(provider_id: str, suffix: str) -> str:
    safe = "".join(char if char.isalnum() or char in ".-_" else "_" for char in provider_id)
    return f"{suffix}-{safe}.json"


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    out_dir = args.out_dir
    summary_out = args.summary_out or out_dir / "rollout-summary.json"
    latest_out = out_dir / "latest.json"
    events_out = out_dir / "events.json"
    cli = args.sorafs_cli_bin
    torii_url = args.torii_url

    plan = [
        CommandPlan(
            "fetch_latest_snapshot",
            latest_out,
            [
                cli,
                "reputation",
                "snapshot",
                f"--torii-url={torii_url}",
                f"--output={latest_out}",
            ],
        ),
    ]

    proof_specs = dict(split_provider_proof_spec(spec) for spec in args.provider_proof)
    for provider_id in args.provider_id:
        provider_out = out_dir / provider_artifact_name(provider_id, "provider")
        verify_out = out_dir / provider_artifact_name(provider_id, "verify")
        plan.append(
            CommandPlan(
                f"fetch_provider_{provider_id}",
                provider_out,
                [
                    cli,
                    "reputation",
                    "fetch",
                    f"--torii-url={torii_url}",
                    f"--provider-id={provider_id}",
                    "--format=json",
                    f"--summary-out={provider_out}",
                ],
            )
        )
        plan.append(
            CommandPlan(
                f"verify_provider_{provider_id}",
                verify_out,
                [
                    cli,
                    "reputation",
                    "verify",
                    f"--snapshot={args.snapshot}",
                    f"--provider-id={provider_id}",
                    f"--proof={proof_specs[provider_id]}",
                    f"--summary-out={verify_out}",
                ],
            )
        )

    plan.append(
        CommandPlan(
            "watch_reputation_events",
            events_out,
            [
                cli,
                "reputation",
                "watch",
                f"--torii-url={torii_url}",
                f"--since={args.watch_since}",
                f"--limit={args.watch_limit}",
                f"--max-polls={args.watch_max_polls}",
                f"--poll-interval-ms={args.watch_poll_interval_ms}",
                f"--summary-out={events_out}",
            ],
        )
    )

    verifier_command = [
        sys.executable,
        str(args.verifier),
        "--evidence-dir",
        str(out_dir),
        "--evidence",
        f"publish={args.publish_evidence}",
        "--evidence",
        f"metrics={args.metrics_evidence}",
        "--evidence",
        f"transport={args.transport_evidence}",
        "--evidence",
        f"consumption={args.consumption_evidence}",
        "--summary-out",
        str(summary_out),
        "--max-snapshot-age-secs",
        str(args.max_snapshot_age_secs),
        "--max-ingest-lag-secs",
        str(args.max_ingest_lag_secs),
    ]
    if args.now_unix is not None:
        verifier_command.extend(["--now-unix", str(args.now_unix)])
    for provider_id in args.provider_id:
        verifier_command.extend(["--require-provider", provider_id])
    plan.append(CommandPlan("rollout_evidence_gate", summary_out, verifier_command))
    return plan


def external_evidence(args: argparse.Namespace) -> dict[str, str]:
    """Return reviewed external evidence paths rendered in dry-run plans."""

    return {
        "publish": str(args.publish_evidence),
        "metrics": str(args.metrics_evidence),
        "transport": str(args.transport_evidence),
        "consumption": str(args.consumption_evidence),
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
    """Validate the reputation collection-plan envelope before use."""

    return validate_runner_fixed_evidence_plan(
        rendered,
        plan,
        diagnostic_prefix="reputation rollout runner plan",
        plan_schema=PLAN_SCHEMA,
        plan_fields=PLAN_FIELDS,
        summary_schema=SUMMARY_SCHEMA,
        external_evidence=external_evidence(args),
        external_evidence_fields=EXTERNAL_EVIDENCE_FIELDS,
        known_kinds=KIND_BY_NAME,
        evidence_contract=evidence_contract(),
        evidence_required_fields=EVIDENCE_REQUIRED_FIELDS,
    )


def run_plan(plan: Sequence[CommandPlan], out_dir: Path) -> int:
    return run_command_plan(plan, out_dir)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Collect and verify SoraFS reputation rollout evidence.",
    )
    parser.add_argument("--sorafs-cli-bin", default="sorafs_cli", help="sorafs_cli binary to run.")
    parser.add_argument(
        "--verifier",
        type=Path,
        default=SCRIPT_DIR / "check_sorafs_reputation_rollout_evidence.py",
        help="Rollout evidence verifier script path.",
    )
    parser.add_argument("--torii-url", required=True, help="Deployed Torii base URL.")
    parser.add_argument(
        "--snapshot",
        type=Path,
        required=True,
        help="Canonical Norito ReputationSnapshotV1 bytes to verify.",
    )
    parser.add_argument(
        "--publish-evidence",
        type=Path,
        required=True,
        help="Reviewed payload-free external threshold-signing/publication evidence JSON.",
    )
    parser.add_argument(
        "--provider-id",
        action="append",
        default=[],
        help="Provider id that must have fetch and proof-replay evidence.",
    )
    parser.add_argument(
        "--provider-proof",
        action="append",
        default=[],
        metavar="PROVIDER_ID=PATH",
        help="Canonical Norito ReputationMerkleProofV1 bytes for a provider. Repeat per provider.",
    )
    parser.add_argument(
        "--metrics-evidence",
        type=Path,
        required=True,
        help="Payload-free deployed metrics canary JSON.",
    )
    parser.add_argument(
        "--transport-evidence",
        type=Path,
        required=True,
        help="Payload-free SSE/WebSocket transport canary JSON.",
    )
    parser.add_argument(
        "--consumption-evidence",
        type=Path,
        required=True,
        help="Payload-free routing/incentive consumption evidence JSON.",
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
    parser.add_argument(
        "--now-unix",
        type=positive_int_arg,
        help="Validator clock used for verifier freshness checks.",
    )
    parser.add_argument(
        "--max-snapshot-age-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_SNAPSHOT_AGE_SECS,
        help="Maximum accepted latest snapshot age passed to the verifier.",
    )
    parser.add_argument(
        "--max-ingest-lag-secs",
        type=positive_int_arg,
        default=DEFAULT_MAX_INGEST_LAG_SECS,
        help="Maximum accepted reputation ingest lag passed to the verifier.",
    )
    parser.add_argument("--watch-since", type=non_negative_int_arg, default=0, help="Initial event cursor.")
    parser.add_argument("--watch-limit", type=positive_int_arg, default=100, help="Event watch limit.")
    parser.add_argument("--watch-max-polls", type=positive_int_arg, default=1, help="Bounded event poll count.")
    parser.add_argument(
        "--watch-poll-interval-ms",
        type=non_negative_int_arg,
        default=1000,
        help="Delay between repeated event polls.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the command plan JSON without running commands.",
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
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
        emit_runner_error_block("ERROR: SoraFS reputation rollout evidence inputs are incomplete:", errors)
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
