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
from sorafs_path_identity import error_diagnostic_label  # noqa: E402
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
    """One reputation rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]




def split_provider_proof_spec(spec: str) -> tuple[str, Path]:
    provider_id, separator, path = spec.partition("=")
    provider_id = provider_id.strip()
    path = path.strip()
    if not separator or not provider_id or not path:
        raise ValueError(f"--provider-proof must use PROVIDER_ID=PATH form, got `{spec}`")
    return provider_id, Path(path)


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(args, summary_filename="rollout-summary.json")
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    if not args.provider_id:
        errors.append("at least one --provider-id is required")
    seen_provider_ids: set[str] = set()
    for provider_id in args.provider_id:
        if not provider_id.strip():
            errors.append("--provider-id must be non-empty")
            continue
        if provider_id in seen_provider_ids:
            errors.append(f"duplicate --provider-id `{provider_id}`")
        seen_provider_ids.add(provider_id)

    proof_specs: dict[str, Path] = {}
    for spec in args.provider_proof:
        try:
            provider_id, path = split_provider_proof_spec(spec)
        except ValueError as error:
            errors.append(error_diagnostic_label(error))
            continue
        if provider_id in proof_specs:
            errors.append(f"duplicate --provider-proof for `{provider_id}`")
        proof_specs[provider_id] = path

    for provider_id in args.provider_id:
        if provider_id not in proof_specs:
            errors.append(f"missing --provider-proof for `{provider_id}`")

    extra_proofs = sorted(set(proof_specs) - set(args.provider_id))
    for provider_id in extra_proofs:
        errors.append(f"--provider-proof supplied for unrequested provider `{provider_id}`")

    errors.extend(require_existing_files([args.snapshot], "--snapshot", seen=seen_input_files))
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
    publish_out = out_dir / "publish.json"
    events_out = out_dir / "events.json"
    cli = args.sorafs_cli_bin
    torii_url = args.torii_url

    plan = [
        CommandPlan(
            "publish_snapshot",
            publish_out,
            [
                cli,
                "reputation",
                "publish",
                f"--torii-url={torii_url}",
                f"--snapshot={args.snapshot}",
                f"--summary-out={publish_out}",
            ],
        ),
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
        f"metrics={args.metrics_evidence}",
        "--evidence",
        f"transport={args.transport_evidence}",
        "--evidence",
        f"consumption={args.consumption_evidence}",
        "--summary-out",
        str(summary_out),
    ]
    for provider_id in args.provider_id:
        verifier_command.extend(["--require-provider", provider_id])
    plan.append(CommandPlan("rollout_evidence_gate", summary_out, verifier_command))
    return plan


def plan_json(plan: Sequence[CommandPlan]) -> dict[str, object]:
    return {
        "schema": "sorafs.reputation.rollout_evidence_collection_plan.v1",
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "evidence_contract": {
            kind: {
                "schema": KIND_BY_NAME[kind].schema,
                "required_payload_fields": list(EVIDENCE_REQUIRED_FIELDS[kind]),
            }
            for kind in DEFAULT_REQUIRED_KINDS
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
        help="Canonical Norito ReputationSnapshotV1 bytes to publish and verify.",
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
    if args.dry_run:
        plan_errors = write_runner_plan(plan_json(plan))
        if plan_errors:
            emit_runner_error_lines(plan_errors)
            return 2
        return 0
    return run_plan(plan, args.out_dir)


if __name__ == "__main__":
    raise SystemExit(main())
