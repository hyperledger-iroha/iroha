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
    inspect_runner_path_exists,
    inspect_runner_path_is_dir,
    inspect_runner_path_is_symlink,
    run_command_plan,
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_passthrough_args,
    require_runner_positive_int,
    require_runner_url_args,
    validate_runner_fixed_evidence_plan,
    validate_runner_output_dir,
    validate_runner_output_parent,
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
AUTH_ACCOUNT_ERROR = "--auth-account must be a canonical non-alias I105 literal"
I105_BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
I105_KANA_ALPHABET = (
    "ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ"
)
I105_PAYLOAD_ALPHABET = frozenset(I105_BASE58_ALPHABET + I105_KANA_ALPHABET)
I105_KNOWN_SENTINELS = ("sora", "test", "dev")
I105_NAMED_DISCRIMINANTS = frozenset({0x0000, 0x0171, 0x02F1})
I105_CHECKSUM_SYMBOLS = 6
I105_MAX_LITERAL_BYTES = 512
REPUTATION_PROVIDER_ID_MAX_BYTES = 256
REPUTATION_PROVIDER_ID_PUNCTUATION = frozenset("-_.:")
REPUTATION_PROVIDER_ID_ERROR = (
    "--provider-id must be canonical 1..=256 ASCII [A-Za-z0-9_.:-], excluding . and .."
)
PROVIDER_ARTIFACT_HEX_CHUNK_CHARS = 64
PROVIDER_ARTIFACT_FILENAME = "artifact.json"
PROVIDER_ARTIFACT_NAMESPACES = {
    "provider": "provider-by-provider-id",
    "verify": "verify-by-provider-id",
}
REPUTATION_MAX_PAGE_ITEMS = 500
U64_MAX = (1 << 64) - 1
REPEATABLE_OPTIONS = frozenset({"--provider-id", "--provider-proof"})
VALUE_OPTIONS = frozenset(
    {
        "--sorafs-cli-bin",
        "--verifier",
        "--torii-url",
        "--auth-account",
        "--auth-private-key-file",
        "--snapshot",
        "--publish-evidence",
        "--provider-id",
        "--provider-proof",
        "--metrics-evidence",
        "--transport-evidence",
        "--consumption-evidence",
        "--out-dir",
        "--summary-out",
        "--now-unix",
        "--max-snapshot-age-secs",
        "--max-ingest-lag-secs",
        "--watch-since",
        "--watch-limit",
        "--watch-max-polls",
        "--watch-poll-interval-ms",
    }
)
FLAG_OPTIONS = frozenset({"--dry-run", "--help", "-h"})
OPTION_DIAGNOSTIC_LABELS = {
    "--auth-private-key-file": "runtime signer-file option",
}


@dataclass(frozen=True)
class CommandPlan:
    """One reputation rollout evidence command."""

    label: str
    artifact: Path | None
    command: list[str]


def i105_payload_is_candidate(payload: str) -> bool:
    """Return whether text has the minimum shape of a canonical I105 payload."""

    return (
        len(payload) > I105_CHECKSUM_SYMBOLS
        and all(character in I105_PAYLOAD_ALPHABET for character in payload)
    )


def auth_account_is_i105_candidate(value: str) -> bool:
    """Reject aliases and malformed I105 shapes before rendering a command plan.

    The Rust CLI performs the authoritative checksum, canonical-byte, chain,
    single-key, and private-key/account-match checks immediately before each
    request is signed.
    """

    if (
        not diagnostic_text_is_canonical(value)
        or len(value.encode("utf-8")) > I105_MAX_LITERAL_BYTES
    ):
        return False
    for sentinel in I105_KNOWN_SENTINELS:
        if value.startswith(sentinel):
            return i105_payload_is_candidate(value[len(sentinel) :])
    if not value.startswith("n"):
        return False

    rest = value[1:]
    for sentinel_length in range(1, min(5, len(rest)) + 1):
        numeric_sentinel = rest[:sentinel_length]
        if not numeric_sentinel.isascii() or not numeric_sentinel.isdigit():
            break
        if len(numeric_sentinel) > 1 and numeric_sentinel.startswith("0"):
            continue
        discriminant = int(numeric_sentinel)
        if discriminant > 0xFFFF or discriminant in I105_NAMED_DISCRIMINANTS:
            continue
        if i105_payload_is_candidate(rest[sentinel_length:]):
            return True
    return False


def provider_id_is_canonical(value: str) -> bool:
    """Return whether a provider id matches the committed Torii path contract."""

    return (
        diagnostic_text_is_canonical(value)
        and value not in {".", ".."}
        and value.isascii()
        and len(value) <= REPUTATION_PROVIDER_ID_MAX_BYTES
        and all(
            character.isascii()
            and (character.isalnum() or character in REPUTATION_PROVIDER_ID_PUNCTUATION)
            for character in value
        )
    )


def validate_expanded_options(args: Sequence[str]) -> None:
    """Reject unknown, abbreviated, or repeated scalar options without values."""

    counts: dict[str, int] = {}
    index = 0
    while index < len(args):
        argument = args[index]
        if not isinstance(argument, str) or not diagnostic_text_is_canonical(argument):
            raise ValueError("reputation rollout arguments must be canonical strings")
        option, separator, _inline_value = argument.partition("=")
        if option not in VALUE_OPTIONS and option not in FLAG_OPTIONS:
            raise ValueError("unsupported reputation rollout option")

        count = counts.get(option, 0) + 1
        counts[option] = count
        option_label = OPTION_DIAGNOSTIC_LABELS.get(option, option)
        if count > 1 and option not in REPEATABLE_OPTIONS:
            raise ValueError(f"{option_label} must be supplied at most once")

        if option in FLAG_OPTIONS:
            if separator:
                raise ValueError(f"{option_label} does not accept a value")
            index += 1
            continue
        if separator:
            index += 1
            continue
        if index + 1 >= len(args) or args[index + 1].startswith("-"):
            raise ValueError(f"{option_label} requires a value")
        index += 2


def split_provider_proof_spec(spec: str) -> tuple[str, Path]:
    provider_id, separator, path = spec.partition("=")
    if (
        not separator
        or not provider_id
        or not path
        or not provider_id_is_canonical(provider_id)
        or not diagnostic_text_is_canonical(path)
    ):
        raise ValueError("--provider-proof must use PROVIDER_ID=PATH form")
    return provider_id, Path(path)


def validate_inputs(args: argparse.Namespace) -> list[str]:
    errors = validate_runner_preflight(args, summary_filename="rollout-summary.json")
    require_runner_passthrough_args(
        args,
        ("sorafs_cli_bin", "auth_account"),
        (),
        errors,
    )
    if not auth_account_is_i105_candidate(args.auth_account):
        errors.append(AUTH_ACCOUNT_ERROR)
    require_runner_url_args(args, ("torii_url",), errors)
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    errors.extend(
        require_existing_files(
            [args.auth_private_key_file],
            "--auth-private-key-file",
            seen=seen_input_files,
        )
    )
    if not args.provider_id:
        errors.append("at least one --provider-id is required")
    seen_provider_ids: set[str] = set()
    for provider_id in args.provider_id:
        if not provider_id_is_canonical(provider_id):
            errors.append(REPUTATION_PROVIDER_ID_ERROR)
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
    watch_since_valid = require_runner_non_negative_int(args, "watch_since", errors)
    watch_limit_valid = require_runner_positive_int(args, "watch_limit", errors)
    watch_max_polls_valid = require_runner_positive_int(args, "watch_max_polls", errors)
    watch_poll_interval_valid = require_runner_non_negative_int(
        args,
        "watch_poll_interval_ms",
        errors,
    )
    if watch_since_valid and args.watch_since > U64_MAX:
        errors.append("--watch-since must fit an unsigned 64-bit integer")
    if watch_limit_valid and args.watch_limit > REPUTATION_MAX_PAGE_ITEMS:
        errors.append(
            f"--watch-limit must be within 1..={REPUTATION_MAX_PAGE_ITEMS}"
        )
    if watch_max_polls_valid and args.watch_max_polls > U64_MAX:
        errors.append("--watch-max-polls must fit an unsigned 64-bit integer")
    if watch_poll_interval_valid and args.watch_poll_interval_ms > U64_MAX:
        errors.append("--watch-poll-interval-ms must fit an unsigned 64-bit integer")
    return errors


def provider_artifact_name(provider_id: str, suffix: str) -> Path:
    """Return an injective, component-bounded relative provider artifact path."""

    if not provider_id_is_canonical(provider_id):
        raise ValueError(REPUTATION_PROVIDER_ID_ERROR)
    namespace = PROVIDER_ARTIFACT_NAMESPACES.get(suffix)
    if namespace is None:
        raise ValueError("provider artifact suffix must be provider or verify")
    provider_id_hex = provider_id.encode("ascii").hex()
    chunks = tuple(
        provider_id_hex[index : index + PROVIDER_ARTIFACT_HEX_CHUNK_CHARS]
        for index in range(0, len(provider_id_hex), PROVIDER_ARTIFACT_HEX_CHUNK_CHARS)
    )
    return Path(namespace, *chunks, PROVIDER_ARTIFACT_FILENAME)


def prepare_reputation_artifact_parent(
    step: CommandPlan,
    out_dir: Path,
) -> list[str]:
    """Create one canonical provider artifact parent immediately before launch."""

    command = step.command
    if len(command) < 3 or command[1] != "reputation":
        return []
    operation = command[2]
    suffix = {"fetch": "provider", "verify": "verify"}.get(operation)
    if suffix is None:
        return []

    errors: list[str] = []
    provider_args = [
        argument.removeprefix("--provider-id=")
        for argument in command
        if argument.startswith("--provider-id=")
    ]
    if len(provider_args) != 1 or not provider_id_is_canonical(provider_args[0]):
        return ["provider artifact step must bind one canonical provider id"]
    expected_artifact = out_dir / provider_artifact_name(provider_args[0], suffix)
    if step.artifact != expected_artifact:
        return ["provider artifact path must match its canonical sharded layout"]
    if not validate_runner_output_dir(
        out_dir,
        errors,
        require_exists=True,
    ):
        return errors

    relative_parent = expected_artifact.parent.relative_to(out_dir)
    current = out_dir
    for component in relative_parent.parts:
        current /= component
        directory_is_symlink = inspect_runner_path_is_symlink(
            current,
            errors,
            label="provider artifact directory",
        )
        if directory_is_symlink is None:
            return errors
        if directory_is_symlink:
            errors.append("provider artifact directory must not be a symlink")
            return errors
        directory_exists = inspect_runner_path_exists(
            current,
            errors,
            label="provider artifact directory",
        )
        if directory_exists is None:
            return errors
        if not directory_exists:
            try:
                current.mkdir(mode=0o700)
            except (OSError, RuntimeError) as error:
                errors.append(
                    "provider artifact directory could not be created: "
                    f"{error_diagnostic_label(error)}"
                )
                return errors
        directory_is_symlink = inspect_runner_path_is_symlink(
            current,
            errors,
            label="provider artifact directory",
        )
        directory_is_dir = inspect_runner_path_is_dir(
            current,
            errors,
            label="provider artifact directory",
        )
        if errors:
            return errors
        if directory_is_symlink:
            errors.append("provider artifact directory must not be a symlink")
            return errors
        if not directory_is_dir:
            errors.append("provider artifact directory must be a directory")
            return errors

    if not validate_runner_output_parent(
        expected_artifact,
        errors,
        label="provider artifact",
    ):
        return errors
    artifact_is_symlink = inspect_runner_path_is_symlink(
        expected_artifact,
        errors,
        label="provider artifact",
    )
    artifact_exists = inspect_runner_path_exists(
        expected_artifact,
        errors,
        label="provider artifact",
    )
    if errors:
        return errors
    if artifact_is_symlink:
        errors.append("provider artifact must not be a symlink")
    elif artifact_exists:
        errors.append("provider artifact must not already exist")
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    out_dir = args.out_dir
    summary_out = args.summary_out or out_dir / "rollout-summary.json"
    latest_out = out_dir / "latest.json"
    events_out = out_dir / "events.json"
    cli = args.sorafs_cli_bin
    torii_url = args.torii_url
    auth_args = [
        f"--auth-account={args.auth_account}",
        f"--auth-private-key-file={args.auth_private_key_file}",
    ]

    plan = [
        CommandPlan(
            "fetch_latest_snapshot",
            latest_out,
            [
                cli,
                "reputation",
                "snapshot",
                f"--torii-url={torii_url}",
                *auth_args,
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
                    *auth_args,
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
                *auth_args,
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
    return run_command_plan(
        plan,
        out_dir,
        prepare_step=lambda step: prepare_reputation_artifact_parent(step, out_dir),
    )


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = EvidenceArgumentParser(
        description="Collect and verify SoraFS reputation rollout evidence.",
        allow_abbrev=False,
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
        "--auth-account",
        required=True,
        help="Exact canonical single-key I105 account used to authenticate live reads.",
    )
    parser.add_argument(
        "--auth-private-key-file",
        type=Path,
        required=True,
        help=(
            "Runtime-only private-key file for --auth-account. "
            "The path is forwarded to sorafs_cli; key material is never read by this runner."
        ),
    )
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
        validate_expanded_options(expanded_args)
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
