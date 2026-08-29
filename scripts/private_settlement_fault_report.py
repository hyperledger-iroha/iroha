#!/usr/bin/env python3
"""Validate AtomicPrivateSettlementV1 real-process fault-matrix JSONL evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import defaultdict
from collections.abc import Sequence
from pathlib import Path
from typing import Any

REPORT_VERSION = 1
PROTOCOL = "AtomicPrivateSettlementV1"
REQUIRED_PARTICIPANTS = (2, 3, 4, 8, 16)
REQUIRED_SEEDS_PER_PARTICIPANT = 10
REQUIRED_LOSS_PHASES = ("restricted_da", "prepare", "commit")
REQUIRED_LOSS_PERCENTAGES = (5, 10, 20)
REQUIRED_PHASE_CUTS = (
    "da_before_availability_qc",
    "prepare_before_complete_barrier",
    "commit_before_complete_barrier",
    "carrier_before_global_finality",
)
REQUIRED_CRASH_BOUNDARIES = (
    "sidecar_fsync",
    "staged_delta_fsync",
    "prepare_qc",
    "commit_qc",
    "kura_append",
    "wsv_application",
    "receipt_publication",
)
_GIT_COMMIT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_HEX_64 = re.compile(r"[0-9a-f]{64}")


class FaultEvidenceError(ValueError):
    """Raised when fault evidence is incomplete or unsafe."""


def _exact_fields(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise FaultEvidenceError(f"{label} must be an object")
    actual = set(value)
    if actual != expected:
        raise FaultEvidenceError(
            f"{label} fields mismatch; missing={sorted(expected - actual)} "
            f"unknown={sorted(actual - expected)}"
        )
    return value


def _nonnegative_integer(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise FaultEvidenceError(f"{label} must be a non-negative integer")
    return value


def _require_true(value: Any, label: str) -> None:
    if value is not True:
        raise FaultEvidenceError(f"{label} must be true")


def _parse_loss_trials(value: Any, label: str) -> None:
    if not isinstance(value, list):
        raise FaultEvidenceError(f"{label} must be a list")
    expected_pairs = [
        (phase, percentage)
        for phase in REQUIRED_LOSS_PHASES
        for percentage in REQUIRED_LOSS_PERCENTAGES
    ]
    actual_pairs: list[tuple[str, int]] = []
    for index, item in enumerate(value):
        trial = _exact_fields(
            item,
            {
                "phase",
                "loss_percent",
                "control_acknowledged",
                "healed",
                "converged",
                "partial_visibility_observed",
            },
            f"{label}[{index}]",
        )
        phase = trial["phase"]
        percentage = trial["loss_percent"]
        if not isinstance(phase, str):
            raise FaultEvidenceError(f"{label}[{index}].phase must be a string")
        if isinstance(percentage, bool) or not isinstance(percentage, int):
            raise FaultEvidenceError(
                f"{label}[{index}].loss_percent must be an integer"
            )
        actual_pairs.append((phase, percentage))
        _require_true(
            trial["control_acknowledged"], f"{label}[{index}].control_acknowledged"
        )
        _require_true(trial["healed"], f"{label}[{index}].healed")
        _require_true(trial["converged"], f"{label}[{index}].converged")
        if trial["partial_visibility_observed"] is not False:
            raise FaultEvidenceError(
                f"{label}[{index}].partial_visibility_observed must be false"
            )
    if actual_pairs != expected_pairs:
        raise FaultEvidenceError(f"{label} must cover the exact ordered loss matrix")


def _parse_phase_cuts(value: Any, label: str) -> None:
    if not isinstance(value, list):
        raise FaultEvidenceError(f"{label} must be a list")
    names: list[str] = []
    for index, item in enumerate(value):
        cut = _exact_fields(
            item,
            {
                "cut",
                "control_acknowledged",
                "delayed_delivery",
                "healed",
                "converged",
                "partial_visibility_observed",
            },
            f"{label}[{index}]",
        )
        name = cut["cut"]
        if not isinstance(name, str):
            raise FaultEvidenceError(f"{label}[{index}].cut must be a string")
        names.append(name)
        for field in (
            "control_acknowledged",
            "delayed_delivery",
            "healed",
            "converged",
        ):
            _require_true(cut[field], f"{label}[{index}].{field}")
        if cut["partial_visibility_observed"] is not False:
            raise FaultEvidenceError(
                f"{label}[{index}].partial_visibility_observed must be false"
            )
    if names != list(REQUIRED_PHASE_CUTS):
        raise FaultEvidenceError(f"{label} must cover the exact ordered phase cuts")


def _parse_crash_recoveries(value: Any, label: str) -> None:
    if not isinstance(value, list):
        raise FaultEvidenceError(f"{label} must be a list")
    names: list[str] = []
    for index, item in enumerate(value):
        recovery = _exact_fields(
            item,
            {
                "boundary",
                "process_restarted",
                "durable_state_reconciled",
                "converged",
                "partial_visibility_observed",
            },
            f"{label}[{index}]",
        )
        boundary = recovery["boundary"]
        if not isinstance(boundary, str):
            raise FaultEvidenceError(f"{label}[{index}].boundary must be a string")
        names.append(boundary)
        for field in ("process_restarted", "durable_state_reconciled", "converged"):
            _require_true(recovery[field], f"{label}[{index}].{field}")
        if recovery["partial_visibility_observed"] is not False:
            raise FaultEvidenceError(
                f"{label}[{index}].partial_visibility_observed must be false"
            )
    if names != list(REQUIRED_CRASH_BOUNDARIES):
        raise FaultEvidenceError(
            f"{label} must cover the exact ordered persistence boundaries"
        )


def _parse_atomicity(value: Any, participants: int, label: str) -> None:
    atomicity = _exact_fields(
        value,
        {
            "continuous_checks",
            "partial_visible_observations",
            "partial_spendable_observations",
            "aborted_private_state_changes",
            "successful_leg_applications",
            "each_leg_applied_exactly_once",
            "invalid_leg_state_byte_identical",
            "replay_rejected",
        },
        label,
    )
    checks = _nonnegative_integer(
        atomicity["continuous_checks"], f"{label}.continuous_checks"
    )
    if checks == 0:
        raise FaultEvidenceError(f"{label}.continuous_checks must be positive")
    for field in (
        "partial_visible_observations",
        "partial_spendable_observations",
        "aborted_private_state_changes",
    ):
        if _nonnegative_integer(atomicity[field], f"{label}.{field}") != 0:
            raise FaultEvidenceError(f"{label}.{field} must be zero")
    if atomicity["successful_leg_applications"] != participants:
        raise FaultEvidenceError(
            f"{label}.successful_leg_applications must equal participants"
        )
    for field in (
        "each_leg_applied_exactly_once",
        "invalid_leg_state_byte_identical",
        "replay_rejected",
    ):
        _require_true(atomicity[field], f"{label}.{field}")


def parse_run(value: Any, source: str) -> tuple[int, int, int, str, str, str]:
    """Validate one complete real-process fault run."""

    record = _exact_fields(
        value,
        {
            "version",
            "protocol",
            "commit",
            "hardware_sha256",
            "configuration_sha256",
            "participants",
            "seed",
            "run",
            "validators_per_dataspace",
            "quorum",
            "mandatory_signed_rs16_da_rbc",
            "authenticated_message_control",
            "committee_validator_restarts",
            "maximum_simultaneously_unavailable_per_committee",
            "quorum_progress_with_one_unavailable",
            "coordinator_restarted",
            "global_node_restarted",
            "loss_trials",
            "phase_cut_partitions",
            "crash_recoveries",
            "atomicity",
            "all_nodes_converged",
        },
        source,
    )
    if record["version"] != REPORT_VERSION or record["protocol"] != PROTOCOL:
        raise FaultEvidenceError(f"{source}: unsupported report version or protocol")
    commit = record["commit"]
    if not isinstance(commit, str) or _GIT_COMMIT.fullmatch(commit) is None:
        raise FaultEvidenceError(f"{source}.commit must be a full Git object id")
    hardware_sha256 = record["hardware_sha256"]
    configuration_sha256 = record["configuration_sha256"]
    if (
        not isinstance(hardware_sha256, str)
        or _HEX_64.fullmatch(hardware_sha256) is None
        or not isinstance(configuration_sha256, str)
        or _HEX_64.fullmatch(configuration_sha256) is None
    ):
        raise FaultEvidenceError(f"{source}: environment digests must be SHA-256")
    participants = record["participants"]
    if participants not in REQUIRED_PARTICIPANTS:
        raise FaultEvidenceError(
            f"{source}.participants is not a real-network matrix size"
        )
    seed = _nonnegative_integer(record["seed"], f"{source}.seed")
    run = _nonnegative_integer(record["run"], f"{source}.run")
    if record["validators_per_dataspace"] != 4 or record["quorum"] != "3-of-4":
        raise FaultEvidenceError(
            f"{source}: committee must be exact four-validator 3-of-4"
        )
    _require_true(
        record["mandatory_signed_rs16_da_rbc"],
        f"{source}.mandatory_signed_rs16_da_rbc",
    )
    _require_true(
        record["authenticated_message_control"],
        f"{source}.authenticated_message_control",
    )
    expected_restarts = list(range(participants))
    if record["committee_validator_restarts"] != expected_restarts:
        raise FaultEvidenceError(
            f"{source}.committee_validator_restarts must be exactly {expected_restarts}"
        )
    if record["maximum_simultaneously_unavailable_per_committee"] != 1:
        raise FaultEvidenceError(
            f"{source}.maximum_simultaneously_unavailable_per_committee must be 1"
        )
    _require_true(
        record["quorum_progress_with_one_unavailable"],
        f"{source}.quorum_progress_with_one_unavailable",
    )
    _require_true(record["coordinator_restarted"], f"{source}.coordinator_restarted")
    _require_true(record["global_node_restarted"], f"{source}.global_node_restarted")
    _parse_loss_trials(record["loss_trials"], f"{source}.loss_trials")
    _parse_phase_cuts(record["phase_cut_partitions"], f"{source}.phase_cut_partitions")
    _parse_crash_recoveries(record["crash_recoveries"], f"{source}.crash_recoveries")
    _parse_atomicity(record["atomicity"], participants, f"{source}.atomicity")
    _require_true(record["all_nodes_converged"], f"{source}.all_nodes_converged")
    return participants, seed, run, commit, hardware_sha256, configuration_sha256


def load_runs(paths: Sequence[Path]) -> list[tuple[int, int, int, str, str, str]]:
    """Load bounded JSONL inputs and reject duplicate run identities."""

    runs: list[tuple[int, int, int, str, str, str]] = []
    seen: set[tuple[int, int, int]] = set()
    for path in paths:
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            raise FaultEvidenceError(f"cannot read {path}: {error}") from error
        for line_number, line in enumerate(lines, 1):
            if not line.strip():
                continue
            source = f"{path}:{line_number}"
            try:
                value = json.loads(line)
            except json.JSONDecodeError as error:
                raise FaultEvidenceError(f"{source}: invalid JSON: {error}") from error
            parsed = parse_run(value, source)
            identity = parsed[:3]
            if identity in seen:
                raise FaultEvidenceError(f"{source}: duplicate run identity {identity}")
            seen.add(identity)
            runs.append(parsed)
    if not runs:
        raise FaultEvidenceError("fault evidence is empty")
    return runs


def build_report(
    runs: Sequence[tuple[int, int, int, str, str, str]],
    raw_inputs: Sequence[dict[str, Any]] = (),
) -> dict[str, Any]:
    """Require the complete participant and ten-seed matrix and summarize it."""

    buckets: dict[int, list[tuple[int, int]]] = defaultdict(list)
    commits = {commit for _, _, _, commit, _, _ in runs}
    if len(commits) != 1:
        raise FaultEvidenceError("fault evidence must use one exact source commit")
    hardware_digests = {hardware for _, _, _, _, hardware, _ in runs}
    if len(hardware_digests) != 1:
        raise FaultEvidenceError(
            "fault evidence must use one pinned hardware description"
        )
    configuration_digests: dict[int, str] = {}
    for participants in REQUIRED_PARTICIPANTS:
        digests = {
            configuration
            for candidate, _, _, _, _, configuration in runs
            if candidate == participants
        }
        if len(digests) != 1:
            raise FaultEvidenceError(
                f"fault evidence N={participants} must use one pinned configuration"
            )
        configuration_digests[participants] = next(iter(digests))
    for participants, seed, run, _, _, _ in runs:
        buckets[participants].append((seed, run))
    if set(buckets) != set(REQUIRED_PARTICIPANTS):
        raise FaultEvidenceError(
            "fault evidence must contain exactly the N=2,3,4,8,16 participant buckets"
        )
    matrix: dict[str, Any] = {}
    for participants in REQUIRED_PARTICIPANTS:
        bucket = buckets[participants]
        seeds = sorted({seed for seed, _ in bucket})
        if len(seeds) < REQUIRED_SEEDS_PER_PARTICIPANT:
            raise FaultEvidenceError(
                f"N={participants} requires at least {REQUIRED_SEEDS_PER_PARTICIPANT} seeds"
            )
        matrix[str(participants)] = {
            "runs": len(bucket),
            "seeds": seeds,
        }
    return {
        "version": REPORT_VERSION,
        "protocol": PROTOCOL,
        "commit": next(iter(commits)),
        "raw_inputs": list(raw_inputs),
        "environment": {
            "hardware_sha256": next(iter(hardware_digests)),
            "configuration_sha256_by_participants": {
                str(participants): configuration_digests[participants]
                for participants in REQUIRED_PARTICIPANTS
            },
        },
        "requirements": {
            "participants": list(REQUIRED_PARTICIPANTS),
            "minimum_seeds_per_participant": REQUIRED_SEEDS_PER_PARTICIPANT,
            "validators_per_dataspace": 4,
            "quorum": "3-of-4",
            "loss_phases": list(REQUIRED_LOSS_PHASES),
            "loss_percentages": list(REQUIRED_LOSS_PERCENTAGES),
            "phase_cuts": list(REQUIRED_PHASE_CUTS),
            "crash_boundaries": list(REQUIRED_CRASH_BOUNDARIES),
        },
        "matrix": matrix,
        "passed": True,
    }


def input_bindings(paths: Sequence[Path]) -> list[dict[str, Any]]:
    """Hash every raw JSONL input in canonical digest/length order."""

    bindings: list[dict[str, Any]] = []
    for path in paths:
        digest = hashlib.sha256()
        try:
            with path.open("rb") as stream:
                while chunk := stream.read(1024 * 1024):
                    digest.update(chunk)
            byte_count = path.stat().st_size
        except OSError as error:
            raise FaultEvidenceError(f"cannot hash {path}: {error}") from error
        bindings.append({"sha256": digest.hexdigest(), "bytes": byte_count})
    bindings.sort(key=lambda item: (item["sha256"], item["bytes"]))
    if any(_HEX_64.fullmatch(item["sha256"]) is None for item in bindings):
        raise FaultEvidenceError("fault raw input digest is invalid")
    return bindings


def main(argv: Sequence[str] | None = None) -> int:
    """Run the fault-evidence reporter."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", action="append", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        report = build_report(load_runs(args.input), input_bindings(args.input))
        args.output.write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    except (FaultEvidenceError, OSError) as error:
        print(f"private-settlement fault evidence error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
