#!/usr/bin/env python3
"""Run the aggregate SoraFS production-readiness gate."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_production_readiness import (  # noqa: E402
    DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    DEFAULT_REQUIRED_GATES,
    FOUNDATIONAL_PREREQUISITE_IDS,
    FOUNDATIONAL_PREREQUISITE_SCHEMA,
    GATE_BY_NAME,
    MAX_FOUNDATIONAL_RELEASE_SEQUENCE,
    MAX_SUMMARY_BYTES,
    SUMMARY_SCHEMA,
    canonical_lower_hex,
    canonical_string,
    is_production_ready_environment,
    parse_foundational_signer_public_key,
    require_production_deployment_id_value,
    validate_aggregate_summary_output,
)
from sorafs_checker_preflight import (  # noqa: E402
    render_and_write_checker_summary,
)
from sorafs_evidence_json import (  # noqa: E402
    decode_evidence_json,
    read_evidence_bytes,
)
from sorafs_required_kinds import (  # noqa: E402
    parse_required_kinds as parse_required_gates,
)
from sorafs_response_args import (  # noqa: E402
    EvidenceArgumentParser,
    expand_response_args,
    non_negative_int_arg,
    positive_int_arg,
)
from sorafs_runner_preflight import (  # noqa: E402
    PLAN_RENDERED_PATH_ERROR,
    emit_runner_error_block,
    emit_runner_error_lines,
    emit_runner_exception,
    inspect_runner_path_exists,
    plan_rendered_path_is_safe,
    require_existing_files,
    require_runner_non_negative_int,
    require_runner_positive_int,
    run_command_plan,
    validate_command_plan_artifacts,
    validate_runner_aggregate_readiness_plan,
    validate_runner_preflight,
    write_runner_plan,
)
from sorafs_topology_qualification import (  # noqa: E402
    TOPOLOGY_BINDING_FIELDS,
    add_topology_qualification_argument,
    load_topology_qualification_binding,
)


PLAN_SCHEMA = "sorafs.production_readiness.collection_plan.v1"
BUNDLED_VERIFIER = SCRIPT_DIR / "check_sorafs_production_readiness.py"
REPLAY_MANIFEST_SCHEMA = "sorafs.production_readiness.deterministic_replay.v1"
REPLAY_SUMMARY_FILENAME = "sorafs-production-readiness-replay-summary.json"
REPLAY_MANIFEST_FILENAME = "sorafs-production-readiness-replay-manifest.json"
REPLAY_INPUT_SET_DOMAIN = (
    b"iroha:sorafs:production-readiness:deterministic-replay-input-set:v1\x00"
)
REPLAY_INPUT_SLOTS = (
    "topology_qualification",
    "foundational_prerequisite",
    *DEFAULT_REQUIRED_GATES,
)
CANONICAL_GATE_INVENTORY_ERROR = (
    "production readiness runner requires the exact canonical ordered 17-gate inventory"
)
REPLAY_INPUT_DIGEST_FIELDS = frozenset({"slot", "sha256"})
REPLAY_MANIFEST_FIELDS = frozenset(
    {
        "schema",
        "status",
        "required_gates",
        "input_count",
        "input_set_sha256",
        "input_sha256",
        "execution_count",
        "first_aggregate_sha256",
        "second_aggregate_sha256",
        "aggregate_semantic_sha256",
        "summary_file_count",
        "recognized_summary_count",
        "all_required_rows_valid",
        "errors",
    }
)
PLAN_FIELDS = frozenset(
    {
        "schema",
        "verifier_summary_schema",
        "required_gates",
        "thresholds",
        "deployment_context",
        "topology_qualification",
        "external_summaries",
        "foundational_prerequisite",
        "summary_contract",
        "steps",
    }
)
PLAN_REQUIRED_THRESHOLD_FIELDS = frozenset(
    {"max_summary_artifact_age_secs", "now_unix"}
)
PLAN_POSITIVE_THRESHOLD_FIELDS = frozenset({"now_unix"})
PLAN_NON_NEGATIVE_THRESHOLD_FIELDS = frozenset({"max_summary_artifact_age_secs"})
PLAN_THRESHOLD_FIELDS_LABEL = "max_summary_artifact_age_secs and now_unix"
PLAN_DEPLOYMENT_CONTEXT_FIELDS = frozenset({"deployment_id", "environment"})
COMMAND_PATH_FLAGS = frozenset(
    {"--evidence", "--summary-out", "--topology-qualification-summary"}
)
PLAN_FOUNDATIONAL_PREREQUISITE_FIELDS = frozenset(
    {
        "schema",
        "summary",
        "required_ids",
        "signer_public_key_fingerprint_sha256",
        "release_sequence",
        "previous_envelope_sha256",
    }
)


@dataclass(frozen=True)
class CommandPlan:
    """One aggregate readiness command."""

    label: str
    artifact: Path | None
    command: list[str]


@dataclass(frozen=True)
class ReplayAggregate:
    """Validated byte-identical aggregate replay result."""

    payload: dict[str, Any]
    first_sha256: str
    second_sha256: str
    semantic_sha256: str


InputDigestSnapshot = tuple[tuple[str, str], ...]


SUMMARY_OPTIONS_BY_GATE = {
    "ai_prescreen": "ai_prescreen_summary",
    "appeal_finance": "appeal_finance_summary",
    "gateway_compliance": "gateway_compliance_summary",
    "gateway_load": "gateway_load_summary",
    "governance_dag": "governance_dag_summary",
    "hedging_billing": "hedging_billing_summary",
    "moderation_panel": "moderation_panel_summary",
    "orderbook": "orderbook_summary",
    "pdp": "pdp_summary",
    "pop_credentials": "pop_credentials_summary",
    "por": "por_summary",
    "potr": "potr_summary",
    "reference_sdk_release": "reference_sdk_release_summary",
    "repair": "repair_summary",
    "reputation": "reputation_summary",
    "reserve_rent": "reserve_rent_summary",
    "transparency": "transparency_summary",
}

SUMMARY_FLAGS_BY_GATE = {
    "ai_prescreen": "--ai-prescreen-summary",
    "appeal_finance": "--appeal-finance-summary",
    "gateway_compliance": "--gateway-compliance-summary",
    "gateway_load": "--gateway-load-summary",
    "governance_dag": "--governance-dag-summary",
    "hedging_billing": "--hedging-billing-summary",
    "moderation_panel": "--moderation-panel-summary",
    "orderbook": "--orderbook-summary",
    "pdp": "--pdp-summary",
    "pop_credentials": "--pop-credentials-summary",
    "por": "--por-summary",
    "potr": "--potr-summary",
    "reference_sdk_release": "--reference-sdk-release-summary",
    "repair": "--repair-summary",
    "reputation": "--reputation-summary",
    "reserve_rent": "--reserve-rent-summary",
    "transparency": "--transparency-summary",
}


def summary_input_path_is_plan_safe(path: Path) -> bool:
    """Return whether a summary input path can be rendered in runner plans."""

    return plan_rendered_path_is_safe(path)


def summary_paths_by_gate(args: argparse.Namespace) -> dict[str, list[Path]]:
    """Return supplied lane summary paths keyed by gate name."""

    return {
        gate: list(getattr(args, option))
        for gate, option in SUMMARY_OPTIONS_BY_GATE.items()
    }


def primary_summary_path(args: argparse.Namespace) -> Path:
    """Return the first aggregate execution output path."""

    return (
        args.summary_out
        or args.out_dir / "sorafs-production-readiness-summary.json"
    )


def replay_summary_path(args: argparse.Namespace) -> Path:
    """Return the independent replay aggregate output path."""

    return args.out_dir / REPLAY_SUMMARY_FILENAME


def replay_manifest_path(args: argparse.Namespace) -> Path:
    """Return the deterministic replay manifest output path."""

    return args.out_dir / REPLAY_MANIFEST_FILENAME


def production_input_paths(
    args: argparse.Namespace,
) -> tuple[tuple[str, Path], ...]:
    """Return the exact topology, foundation, and canonical 17-lane input set."""

    foundational_paths = list(args.foundational_prerequisite_summary)
    paths_by_gate = summary_paths_by_gate(args)
    if len(foundational_paths) != 1 or any(
        len(paths_by_gate[gate]) != 1 for gate in DEFAULT_REQUIRED_GATES
    ):
        raise ValueError("production readiness replay input set is incomplete")
    return (
        ("topology_qualification", args.topology_qualification_summary),
        ("foundational_prerequisite", foundational_paths[0]),
        *(
            (gate, paths_by_gate[gate][0])
            for gate in DEFAULT_REQUIRED_GATES
        ),
    )


def digest_production_inputs(args: argparse.Namespace) -> InputDigestSnapshot:
    """Hash the same bounded input bytes used by the aggregate checker."""

    return tuple(
        (
            slot,
            hashlib.sha256(read_evidence_bytes(path, MAX_SUMMARY_BYTES)).hexdigest(),
        )
        for slot, path in production_input_paths(args)
    )


def input_set_sha256(snapshot: InputDigestSnapshot) -> str:
    """Bind an ordered input digest snapshot to one domain-separated digest."""

    digest = hashlib.sha256(REPLAY_INPUT_SET_DOMAIN)
    for slot, sha256 in snapshot:
        slot_bytes = slot.encode("utf-8")
        digest.update(len(slot_bytes).to_bytes(2, "big"))
        digest.update(slot_bytes)
        digest.update(bytes.fromhex(sha256))
    return digest.hexdigest()


def validate_promotion_aggregate(payload: dict[str, Any]) -> list[str]:
    """Require the exact ready/17/17/all-valid promotion aggregate contract."""

    errors: list[str] = []
    validate_aggregate_summary_output(payload, DEFAULT_REQUIRED_GATES, errors)
    if payload.get("schema") != SUMMARY_SCHEMA:
        errors.append("replayed aggregate schema must match the promotion contract")
    if payload.get("status") != "ready":
        errors.append("replayed aggregate status must be ready")
    if payload.get("required_gates") != list(DEFAULT_REQUIRED_GATES):
        errors.append(
            "replayed aggregate required_gates must match the exact canonical "
            "ordered 17-gate inventory"
        )
    if payload.get("summary_file_count") != len(DEFAULT_REQUIRED_GATES):
        errors.append("replayed aggregate summary_file_count must be 17")
    if payload.get("recognized_summary_count") != len(DEFAULT_REQUIRED_GATES):
        errors.append("replayed aggregate recognized_summary_count must be 17")
    if payload.get("errors") != []:
        errors.append("replayed aggregate errors must be empty")

    required = payload.get("required")
    if not isinstance(required, dict) or list(required) != list(
        DEFAULT_REQUIRED_GATES
    ):
        errors.append(
            "replayed aggregate required rows must use the exact canonical "
            "ordered 17-gate inventory"
        )
    elif any(
        not isinstance(row, dict)
        or row.get("present") is not True
        or row.get("valid") is not True
        or row.get("errors") != []
        for row in required.values()
    ):
        errors.append(
            "replayed aggregate required rows must all be present, valid, and "
            "error-free"
        )

    foundational = payload.get("foundational_prerequisites")
    if (
        not isinstance(foundational, dict)
        or foundational.get("present") is not True
        or foundational.get("valid") is not True
        or foundational.get("errors") != []
    ):
        errors.append(
            "replayed aggregate foundational prerequisites must be present, "
            "valid, and error-free"
        )
    return errors


def load_and_validate_replayed_aggregates(
    first_path: Path,
    second_path: Path,
) -> tuple[ReplayAggregate | None, list[str]]:
    """Load both aggregate outputs and require byte and semantic identity."""

    try:
        first_raw = read_evidence_bytes(first_path, MAX_SUMMARY_BYTES)
        second_raw = read_evidence_bytes(second_path, MAX_SUMMARY_BYTES)
        first_payload = decode_evidence_json(first_raw)
        second_payload = decode_evidence_json(second_raw)
    except (
        OSError,
        RuntimeError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        ValueError,
    ):
        return None, [
            "deterministic aggregate replay outputs must be bounded strict JSON objects"
        ]

    errors: list[str] = []
    if first_raw != second_raw:
        errors.append("deterministic aggregate replay outputs must be byte-identical")
    if first_payload != second_payload:
        errors.append(
            "deterministic aggregate replay outputs must be semantically identical"
        )
    for execution, payload in (
        ("first", first_payload),
        ("second", second_payload),
    ):
        for error in validate_promotion_aggregate(payload):
            errors.append(f"{execution} aggregate replay: {error}")
    if errors:
        return None, errors

    semantic_bytes = json.dumps(
        first_payload,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return (
        ReplayAggregate(
            payload=first_payload,
            first_sha256=hashlib.sha256(first_raw).hexdigest(),
            second_sha256=hashlib.sha256(second_raw).hexdigest(),
            semantic_sha256=hashlib.sha256(semantic_bytes).hexdigest(),
        ),
        [],
    )


def build_replay_manifest(
    snapshot: InputDigestSnapshot,
    replay: ReplayAggregate,
) -> dict[str, object]:
    """Build a schema-closed manifest containing digests, never input payloads."""

    return {
        "schema": REPLAY_MANIFEST_SCHEMA,
        "status": "verified",
        "required_gates": list(DEFAULT_REQUIRED_GATES),
        "input_count": len(snapshot),
        "input_set_sha256": input_set_sha256(snapshot),
        "input_sha256": [
            {"slot": slot, "sha256": sha256}
            for slot, sha256 in snapshot
        ],
        "execution_count": 2,
        "first_aggregate_sha256": replay.first_sha256,
        "second_aggregate_sha256": replay.second_sha256,
        "aggregate_semantic_sha256": replay.semantic_sha256,
        "summary_file_count": replay.payload["summary_file_count"],
        "recognized_summary_count": replay.payload["recognized_summary_count"],
        "all_required_rows_valid": True,
        "errors": [],
    }


def validate_replay_manifest(
    manifest: object,
    snapshot: InputDigestSnapshot,
    replay: ReplayAggregate,
) -> list[str]:
    """Validate the exact digest-only deterministic replay manifest."""

    if not isinstance(manifest, Mapping):
        return ["deterministic replay manifest must be an object"]
    errors: list[str] = []
    if set(manifest) != REPLAY_MANIFEST_FIELDS:
        errors.append(
            "deterministic replay manifest fields must match the schema-closed contract"
        )
    input_rows = manifest.get("input_sha256")
    if not isinstance(input_rows, list) or any(
        not isinstance(row, Mapping)
        or set(row) != REPLAY_INPUT_DIGEST_FIELDS
        for row in input_rows
    ):
        errors.append(
            "deterministic replay manifest input digests must match the "
            "schema-closed contract"
        )
    elif (
        tuple(row.get("slot") for row in input_rows) != REPLAY_INPUT_SLOTS
        or any(
            canonical_lower_hex(row.get("sha256"), 64) is None
            for row in input_rows
        )
    ):
        errors.append(
            "deterministic replay manifest input digests must use the exact "
            "ordered topology, foundation, and 17-lane slots"
        )
    if replay.first_sha256 != replay.second_sha256:
        errors.append(
            "deterministic replay manifest aggregate digests must be identical"
        )
    expected = build_replay_manifest(snapshot, replay)
    if manifest != expected:
        errors.append(
            "deterministic replay manifest must match the verified immutable "
            "inputs and aggregate outputs"
        )
    return errors


def execute_deterministic_replay(
    args: argparse.Namespace,
    plan: Sequence[CommandPlan],
) -> int:
    """Execute, rehash, replay, compare, and publish the promotion manifest."""

    try:
        before = digest_production_inputs(args)
    except (OSError, RuntimeError, ValueError):
        emit_runner_error_lines(
            ["production readiness replay input set could not be prehashed"]
        )
        return 1

    first_exit_code = run_command_plan([plan[0]], args.out_dir)
    if first_exit_code != 0:
        emit_runner_error_block(
            "SoraFS production readiness collection failed:",
            [f"first aggregate execution exited with {first_exit_code}"],
        )
        return first_exit_code

    try:
        between = digest_production_inputs(args)
    except (OSError, RuntimeError, ValueError):
        emit_runner_error_lines(
            ["production readiness replay input set could not be rehashed"]
        )
        return 1
    if between != before:
        emit_runner_error_lines(
            ["production readiness replay input set changed after first execution"]
        )
        return 1

    second_exit_code = run_command_plan([plan[1]], args.out_dir)
    if second_exit_code != 0:
        emit_runner_error_block(
            "SoraFS production readiness collection failed:",
            [f"second aggregate execution exited with {second_exit_code}"],
        )
        return second_exit_code

    try:
        after = digest_production_inputs(args)
    except (OSError, RuntimeError, ValueError):
        emit_runner_error_lines(
            ["production readiness replay input set could not be rehashed"]
        )
        return 1
    if after != before:
        emit_runner_error_lines(
            ["production readiness replay input set changed after second execution"]
        )
        return 1

    replay, replay_errors = load_and_validate_replayed_aggregates(
        primary_summary_path(args),
        replay_summary_path(args),
    )
    if replay is None:
        emit_runner_error_block(
            "SoraFS deterministic aggregate replay failed:",
            replay_errors,
        )
        return 1

    manifest = build_replay_manifest(before, replay)
    manifest_errors = validate_replay_manifest(manifest, before, replay)
    if manifest_errors:
        emit_runner_error_block(
            "SoraFS deterministic replay manifest failed:",
            manifest_errors,
        )
        return 1
    _, write_errors = render_and_write_checker_summary(
        replay_manifest_path(args),
        manifest,
    )
    if write_errors:
        emit_runner_error_block(
            "SoraFS deterministic replay manifest publication failed:",
            write_errors,
        )
        return 1
    return 0


def validate_inputs(args: argparse.Namespace) -> list[str]:
    """Validate runner inputs before command-plan construction."""

    errors = validate_runner_preflight(
        args,
        summary_filename="sorafs-production-readiness-summary.json",
    )
    try:
        bundled_verifier_selected = (
            isinstance(args.verifier, Path)
            and args.verifier.resolve(strict=True)
            == BUNDLED_VERIFIER.resolve(strict=True)
        )
    except (OSError, RuntimeError):
        bundled_verifier_selected = False
    if not bundled_verifier_selected:
        errors.append(
            "production readiness runner requires the bundled aggregate verifier"
        )
    if tuple(args.required_gates) != DEFAULT_REQUIRED_GATES:
        errors.append(CANONICAL_GATE_INVENTORY_ERROR)
    seen_input_files: dict[Path, tuple[str, Path]] = {}
    errors.extend(
        require_existing_files(
            [args.topology_qualification_summary],
            "--topology-qualification-summary",
            seen=seen_input_files,
        )
    )
    foundational_paths = list(args.foundational_prerequisite_summary)
    if not foundational_paths:
        errors.append(
            "production readiness runner requires exactly one foundational prerequisite summary"
        )
    elif len(foundational_paths) > 1:
        errors.append(
            "production readiness runner requires exactly one foundational prerequisite summary"
        )
    errors.extend(
        require_existing_files(
            foundational_paths,
            "--foundational-prerequisite-summary",
            seen=seen_input_files,
        )
    )
    paths_by_gate = summary_paths_by_gate(args)
    for gate in args.required_gates:
        paths = paths_by_gate[gate]
        if not paths:
            errors.append(
                "missing required production readiness summary input"
            )
        elif len(paths) > 1:
            errors.append(
                "production readiness runner requires exactly one summary input per required gate"
            )
    required_gate_names = set(args.required_gates)
    for gate, paths in paths_by_gate.items():
        if paths and gate not in required_gate_names:
            errors.append(
                "summary supplied for unrequired production readiness gate"
            )
    for gate, paths in paths_by_gate.items():
        errors.extend(
            require_existing_files(
                paths,
                SUMMARY_FLAGS_BY_GATE[gate],
                seen=seen_input_files,
            )
        )
    if any(
        not summary_input_path_is_plan_safe(path)
        for paths in [*paths_by_gate.values(), foundational_paths]
        for path in paths
    ):
        errors.append(
            "production readiness runner summary input paths must not contain "
            "secret-looking, control-character, parent, current, or "
            "platform-specific components"
        )
    if args.topology_qualification_summary is None:
        errors.append("--topology-qualification-summary is required")
    else:
        _topology_binding, topology_errors = load_topology_qualification_binding(
            args.topology_qualification_summary,
            expected_deployment_id=args.deployment_id,
            expected_environment=args.environment,
        )
        errors.extend(topology_errors)
    require_runner_positive_int(args, "now_unix", errors)
    require_runner_non_negative_int(args, "max_summary_artifact_age_secs", errors)
    if args.deployment_id is None or args.environment is None:
        errors.append(
            "production readiness runner requires --deployment-id and --environment"
        )
    elif (
        canonical_string(args.deployment_id) is None
        or canonical_string(args.environment) is None
    ):
        errors.append(
            "production readiness runner deployment context must use canonical labels"
        )
    else:
        require_production_deployment_id_value(
            args.deployment_id,
            errors,
            "production readiness runner deployment_id",
        )
        if not is_production_ready_environment(args.environment):
            errors.append("production readiness runner environment must be production")
    if args.foundational_signer_public_key_hex is None:
        errors.append(
            "production readiness runner requires a foundational prerequisite signer public key"
        )
    else:
        parse_foundational_signer_public_key(
            args.foundational_signer_public_key_hex,
            errors,
            path="production readiness runner foundational signer public key",
        )
    require_runner_positive_int(
        args,
        "foundational_release_sequence",
        errors,
    )
    if (
        isinstance(args.foundational_release_sequence, int)
        and not isinstance(args.foundational_release_sequence, bool)
        and args.foundational_release_sequence > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
    ):
        errors.append(
            "production readiness runner foundational release sequence must be in 1..2^63-1"
        )
    if args.foundational_previous_envelope_sha256 is None:
        errors.append(
            "production readiness runner requires a foundational prerequisite predecessor digest"
        )
    elif (
        canonical_lower_hex(args.foundational_previous_envelope_sha256, 64)
        is None
    ):
        errors.append(
            "production readiness runner foundational predecessor must be canonical lowercase SHA-256"
        )
    elif (
        isinstance(args.foundational_release_sequence, int)
        and not isinstance(args.foundational_release_sequence, bool)
        and args.foundational_release_sequence > 0
    ):
        predecessor_is_zero = not any(
            bytes.fromhex(args.foundational_previous_envelope_sha256)
        )
        if args.foundational_release_sequence == 1 and not predecessor_is_zero:
            errors.append(
                "production readiness runner foundational sequence 1 requires the zero predecessor"
            )
        if args.foundational_release_sequence > 1 and predecessor_is_zero:
            errors.append(
                "production readiness runner foundational sequence after 1 requires a non-zero predecessor"
            )
    return errors


def build_command_plan(args: argparse.Namespace) -> list[CommandPlan]:
    """Build two aggregate verifier executions over one reviewed input set."""

    def verifier_command(summary_out: Path) -> list[str]:
        command = [sys.executable, str(BUNDLED_VERIFIER)]
        command.extend(
            [
                "--topology-qualification-summary",
                str(args.topology_qualification_summary),
            ]
        )
        for path in args.foundational_prerequisite_summary:
            command.extend(["--evidence", str(path)])
        paths_by_gate = summary_paths_by_gate(args)
        for gate in args.required_gates:
            for path in paths_by_gate[gate]:
                command.extend(["--evidence", str(path)])
        for required_gate in args.required_gates:
            command.extend(["--require-gate", required_gate])
        command.extend(
            [
                "--summary-out",
                str(summary_out),
                "--max-summary-artifact-age-secs",
                str(args.max_summary_artifact_age_secs),
            ]
        )
        command.extend(["--now-unix", str(args.now_unix)])
        if args.deployment_id is not None:
            command.extend(["--deployment-id", args.deployment_id])
        if args.environment is not None:
            command.extend(["--environment", args.environment])
        if args.foundational_signer_public_key_hex is not None:
            command.extend(
                [
                    "--foundational-prerequisite-signer-public-key-hex",
                    args.foundational_signer_public_key_hex,
                ]
            )
        if args.foundational_release_sequence is not None:
            command.extend(
                [
                    "--foundational-prerequisite-release-sequence",
                    str(args.foundational_release_sequence),
                ]
            )
        if args.foundational_previous_envelope_sha256 is not None:
            command.extend(
                [
                    "--foundational-prerequisite-previous-envelope-sha256",
                    args.foundational_previous_envelope_sha256,
                ]
            )
        return command

    first_summary = primary_summary_path(args)
    second_summary = replay_summary_path(args)
    return [
        CommandPlan(
            "sorafs_production_readiness_gate_first",
            first_summary,
            verifier_command(first_summary),
        ),
        CommandPlan(
            "sorafs_production_readiness_gate_replay",
            second_summary,
            verifier_command(second_summary),
        ),
    ]


def foundational_prerequisite_plan(args: argparse.Namespace) -> dict[str, object]:
    """Render the payload-free foundational prerequisite plan row."""

    foundational_prerequisite: dict[str, object] = {
        "schema": FOUNDATIONAL_PREREQUISITE_SCHEMA,
        "summary": (
            str(args.foundational_prerequisite_summary[0])
            if len(args.foundational_prerequisite_summary) == 1
            else ""
        ),
        "required_ids": list(FOUNDATIONAL_PREREQUISITE_IDS),
        "signer_public_key_fingerprint_sha256": "",
        "release_sequence": args.foundational_release_sequence,
        "previous_envelope_sha256": (
            args.foundational_previous_envelope_sha256
        ),
    }
    key_errors: list[str] = []
    public_key = parse_foundational_signer_public_key(
        args.foundational_signer_public_key_hex,
        key_errors,
        path="production readiness runner foundational signer public key",
    )
    if public_key is not None:
        foundational_prerequisite["signer_public_key_fingerprint_sha256"] = (
            hashlib.sha256(public_key).hexdigest()
        )
    return foundational_prerequisite


def plan_json(plan: Sequence[CommandPlan], args: argparse.Namespace) -> dict[str, object]:
    """Render the aggregate dry-run plan."""

    thresholds: dict[str, int] = {
        "max_summary_artifact_age_secs": args.max_summary_artifact_age_secs,
    }
    thresholds["now_unix"] = args.now_unix

    deployment_context: dict[str, str] = {}
    if args.deployment_id is not None:
        deployment_context["deployment_id"] = args.deployment_id
    if args.environment is not None:
        deployment_context["environment"] = args.environment

    topology_binding, _topology_errors = load_topology_qualification_binding(
        args.topology_qualification_summary,
        expected_deployment_id=args.deployment_id,
        expected_environment=args.environment,
    )
    return {
        "schema": PLAN_SCHEMA,
        "verifier_summary_schema": SUMMARY_SCHEMA,
        "required_gates": list(args.required_gates),
        "thresholds": thresholds,
        "deployment_context": deployment_context,
        "topology_qualification": {
            "summary": str(args.topology_qualification_summary),
            **({} if topology_binding is None else topology_binding),
        },
        "foundational_prerequisite": foundational_prerequisite_plan(args),
        "external_summaries": {
            gate: [str(path) for path in paths]
            for gate, paths in summary_paths_by_gate(args).items()
            if paths
        },
        "summary_contract": {
            gate: {
                "schema": GATE_BY_NAME[gate].schema,
                "required_kinds": list(GATE_BY_NAME[gate].required_kinds),
            }
            for gate in args.required_gates
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


def rendered_plan_paths_are_safe(rendered: Mapping[str, object]) -> bool:
    """Return whether all rendered plan path strings are safe to expose."""

    paths: list[str] = []
    external_summaries = rendered.get("external_summaries")
    if isinstance(external_summaries, Mapping):
        for gate_paths in external_summaries.values():
            if not isinstance(gate_paths, list):
                continue
            paths.extend(path for path in gate_paths if isinstance(path, str))
    foundational_prerequisite = rendered.get("foundational_prerequisite")
    if isinstance(foundational_prerequisite, Mapping):
        foundational_summary = foundational_prerequisite.get("summary")
        if isinstance(foundational_summary, str):
            paths.append(foundational_summary)
    topology_qualification = rendered.get("topology_qualification")
    if isinstance(topology_qualification, Mapping):
        topology_summary = topology_qualification.get("summary")
        if isinstance(topology_summary, str):
            paths.append(topology_summary)
    steps = rendered.get("steps")
    if isinstance(steps, list):
        for step in steps:
            if not isinstance(step, Mapping):
                continue
            artifact = step.get("artifact")
            if isinstance(artifact, str):
                paths.append(artifact)
            command = step.get("command")
            if isinstance(command, list):
                for index, argument in enumerate(command):
                    if not isinstance(argument, str):
                        continue
                    previous = command[index - 1] if index > 0 else None
                    if index in {0, 1} or previous in COMMAND_PATH_FLAGS:
                        paths.append(argument)
    return all(plan_rendered_path_is_safe(Path(path)) for path in paths)


def validate_plan_json(
    rendered: object,
    plan: Sequence[CommandPlan],
    args: argparse.Namespace,
) -> list[str]:
    """Validate the production-readiness collection-plan envelope."""

    expected_thresholds: dict[str, int] = {
        "max_summary_artifact_age_secs": args.max_summary_artifact_age_secs,
    }
    expected_thresholds["now_unix"] = args.now_unix

    expected_deployment_context = {
        "deployment_id": args.deployment_id,
        "environment": args.environment,
    }

    paths_by_gate = summary_paths_by_gate(args)
    expected_external_summaries = {
        gate: [str(paths[0])]
        for gate, paths in paths_by_gate.items()
        if gate in args.required_gates and len(paths) == 1
    }

    expected_summary_contract = {
        gate: {
            "schema": GATE_BY_NAME[gate].schema,
            "required_kinds": list(GATE_BY_NAME[gate].required_kinds),
        }
        for gate in args.required_gates
    }
    expected_foundational_prerequisite = foundational_prerequisite_plan(args)
    topology_binding, _topology_errors = load_topology_qualification_binding(
        args.topology_qualification_summary,
        expected_deployment_id=args.deployment_id,
        expected_environment=args.environment,
    )
    expected_topology_qualification = {
        "summary": str(args.topology_qualification_summary),
        **({} if topology_binding is None else topology_binding),
    }

    def deployment_context_value_errors(
        context: Mapping[str, object],
    ) -> list[str]:
        context_errors: list[str] = []
        require_production_deployment_id_value(
            context.get("deployment_id"),
            context_errors,
            "production readiness runner plan deployment_id",
        )
        if not is_production_ready_environment(context.get("environment")):
            context_errors.append(
                "production readiness runner plan environment must be production"
            )
        return context_errors

    errors = validate_runner_aggregate_readiness_plan(
        rendered,
        plan,
        diagnostic_prefix="production readiness runner plan",
        plan_schema=PLAN_SCHEMA,
        plan_fields=PLAN_FIELDS,
        summary_schema=SUMMARY_SCHEMA,
        required_gates=args.required_gates,
        known_gates=GATE_BY_NAME,
        thresholds=expected_thresholds,
        required_threshold_fields=PLAN_REQUIRED_THRESHOLD_FIELDS,
        positive_threshold_fields=PLAN_POSITIVE_THRESHOLD_FIELDS,
        non_negative_threshold_fields=PLAN_NON_NEGATIVE_THRESHOLD_FIELDS,
        threshold_fields_label=PLAN_THRESHOLD_FIELDS_LABEL,
        deployment_context=expected_deployment_context,
        deployment_context_fields=PLAN_DEPLOYMENT_CONTEXT_FIELDS,
        deployment_context_value_errors=deployment_context_value_errors,
        external_summaries=expected_external_summaries,
        summary_contract=expected_summary_contract,
    )
    if list(plan) != build_command_plan(args):
        errors.append(
            "production readiness runner plan must contain the exact two "
            "deterministic aggregate executions"
        )
    if isinstance(rendered, Mapping) and not rendered_plan_paths_are_safe(rendered):
        errors.append(PLAN_RENDERED_PATH_ERROR)
    if isinstance(rendered, Mapping):
        topology_qualification = rendered.get("topology_qualification")
        if not isinstance(topology_qualification, Mapping):
            errors.append(
                "production readiness runner plan topology_qualification must be an object"
            )
        else:
            if set(topology_qualification) != TOPOLOGY_BINDING_FIELDS | {"summary"}:
                errors.append(
                    "production readiness runner plan topology_qualification fields must match the schema-closed contract"
                )
            if topology_qualification != expected_topology_qualification:
                errors.append(
                    "production readiness runner plan topology_qualification must match reviewed inputs"
                )
        foundational_prerequisite = rendered.get("foundational_prerequisite")
        if not isinstance(foundational_prerequisite, Mapping):
            errors.append(
                "production readiness runner plan foundational_prerequisite must be an object"
            )
        else:
            if set(foundational_prerequisite) != PLAN_FOUNDATIONAL_PREREQUISITE_FIELDS:
                errors.append(
                    "production readiness runner plan foundational_prerequisite fields must match the schema-closed contract"
                )
            if foundational_prerequisite != expected_foundational_prerequisite:
                errors.append(
                    "production readiness runner plan foundational_prerequisite must match reviewed inputs"
                )
            if (
                foundational_prerequisite.get("schema")
                != FOUNDATIONAL_PREREQUISITE_SCHEMA
            ):
                errors.append(
                    "production readiness runner plan foundational prerequisite schema must match the contract"
                )
            summary_path = canonical_string(
                foundational_prerequisite.get("summary")
            )
            if summary_path is None:
                errors.append(
                    "production readiness runner plan foundational prerequisite summary must be canonical"
                )
            if foundational_prerequisite.get("required_ids") != list(
                FOUNDATIONAL_PREREQUISITE_IDS
            ):
                errors.append(
                    "production readiness runner plan foundational prerequisite ids must match the exact contract"
                )
            if (
                canonical_lower_hex(
                    foundational_prerequisite.get(
                        "signer_public_key_fingerprint_sha256"
                    ),
                    64,
                )
                is None
            ):
                errors.append(
                    "production readiness runner plan foundational signer fingerprint must be canonical lowercase SHA-256"
                )
            release_sequence = foundational_prerequisite.get("release_sequence")
            if (
                not isinstance(release_sequence, int)
                or isinstance(release_sequence, bool)
                or release_sequence <= 0
                or release_sequence > MAX_FOUNDATIONAL_RELEASE_SEQUENCE
            ):
                errors.append(
                    "production readiness runner plan foundational release sequence must be in 1..2^63-1"
                )
            predecessor = canonical_lower_hex(
                foundational_prerequisite.get("previous_envelope_sha256"),
                64,
            )
            if predecessor is None:
                errors.append(
                    "production readiness runner plan foundational predecessor must be canonical lowercase SHA-256"
                )
            elif isinstance(release_sequence, int) and not isinstance(
                release_sequence,
                bool,
            ):
                predecessor_is_zero = not any(bytes.fromhex(predecessor))
                if release_sequence == 1 and not predecessor_is_zero:
                    errors.append(
                        "production readiness runner plan foundational sequence 1 requires the zero predecessor"
                    )
                if release_sequence > 1 and predecessor_is_zero:
                    errors.append(
                        "production readiness runner plan foundational sequence after 1 requires a non-zero predecessor"
                    )
    return errors


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse aggregate readiness runner arguments."""

    parser = EvidenceArgumentParser(
        description="Run the aggregate SoraFS production-readiness gate.",
    )
    add_topology_qualification_argument(parser, required=False)
    parser.add_argument(
        "--verifier",
        type=Path,
        default=BUNDLED_VERIFIER,
        help=(
            "Bundled aggregate readiness verifier path. Substituted verifier "
            "files are rejected."
        ),
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        required=True,
        help="Directory where the aggregate summary will be written.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional aggregate summary path. Defaults under --out-dir.",
    )
    parser.add_argument(
        "--require-gate",
        action="append",
        default=[],
        help=(
            "Required gate name, or comma-separated names. "
            "Defaults to every SoraFS production-readiness gate."
        ),
    )
    for gate, flag in SUMMARY_FLAGS_BY_GATE.items():
        parser.add_argument(
            flag,
            dest=SUMMARY_OPTIONS_BY_GATE[gate],
            action="append",
            type=Path,
            default=[],
            help=f"Existing ready summary for `{gate}`.",
        )
    parser.add_argument(
        "--foundational-prerequisite-summary",
        action="append",
        type=Path,
        default=[],
        help=(
            "Exactly one existing signed foundational prerequisite summary for "
            "SFM-1, SF-1, SF-2/SF-2c, SF-3, SF-4, SF-5b, SF-6, and SF-8a."
        ),
    )
    parser.add_argument("--now-unix", type=positive_int_arg)
    parser.add_argument(
        "--max-summary-artifact-age-secs",
        type=non_negative_int_arg,
        default=DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
    )
    parser.add_argument(
        "--deployment-id",
        help=(
            "Required final deployment id shared by every required lane summary "
            "before aggregate production readiness can run."
        ),
    )
    parser.add_argument(
        "--environment",
        help=(
            "Required final prod/production environment shared by every required "
            "lane summary before aggregate production readiness can run."
        ),
    )
    parser.add_argument(
        "--foundational-prerequisite-signer-public-key-hex",
        dest="foundational_signer_public_key_hex",
        help="Required operator-trusted 32-byte Ed25519 public key.",
    )
    parser.add_argument(
        "--foundational-prerequisite-release-sequence",
        dest="foundational_release_sequence",
        type=positive_int_arg,
        help="Required operator-reviewed monotonic foundational release sequence.",
    )
    parser.add_argument(
        "--foundational-prerequisite-previous-envelope-sha256",
        dest="foundational_previous_envelope_sha256",
        help="Required operator-reviewed predecessor envelope SHA-256.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the aggregate verifier command plan without executing it.",
    )
    raw_args = sys.argv[1:] if argv is None else argv
    try:
        expanded_args = expand_response_args(raw_args, parser)
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
    args = parser.parse_args(expanded_args)
    try:
        args.required_gates = parse_required_gates(
            args.require_gate,
            allowed_kinds=GATE_BY_NAME,
            default_required=DEFAULT_REQUIRED_GATES,
        )
    except ValueError as error:
        emit_runner_exception(error)
        raise SystemExit(2) from error
    return args


def main(argv: list[str] | None = None) -> int:
    """Run the aggregate SoraFS production-readiness plan."""

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
        render_errors = write_runner_plan(rendered_plan)
        if render_errors:
            emit_runner_error_lines(render_errors)
            return 2
        return 0
    manifest_errors: list[str] = []
    manifest_exists = inspect_runner_path_exists(
        replay_manifest_path(args),
        manifest_errors,
        label="deterministic replay manifest output",
    )
    if manifest_exists:
        manifest_errors.append(
            "deterministic replay manifest output must not already exist"
        )
    if manifest_errors:
        emit_runner_error_lines(manifest_errors)
        return 2
    artifact_errors = validate_command_plan_artifacts(
        plan,
        reserved_output_paths=(
            args.out_dir,
            replay_manifest_path(args),
        ),
    )
    if artifact_errors:
        emit_runner_error_lines(artifact_errors)
        return 2
    return execute_deterministic_replay(args, plan)


if __name__ == "__main__":
    raise SystemExit(main())
