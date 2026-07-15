#!/usr/bin/env python3
"""Validate durable, source-bound evidence from the Taira v2 release soak."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
from typing import Any


EXPECTED_PROFILE: dict[str, int | float | str] = {
    "seed": "taira-public-sim",
    "target_tps": 5,
    "packet_loss_percent": 10,
    "churn_interval_secs": 300,
    "max_height_skew": 2,
    "max_height_skew_grace_secs": 30,
    "max_transient_height_skew": 32,
    "stall_timeout_secs": 300,
    "max_view_change_rate": 0.2,
    "max_lagged_cycle_ratio": 0.35,
    "min_committed_tps_ratio": 0.6,
    "process_downtime_secs": 5,
}
EXPECTED_DURATION_SECS = 86_400
EXPECTED_VALIDATORS = 4
EXPECTED_PROCESS_CHURN_CYCLES = 287
EXPECTED_MEMBERSHIP_CHURN_CYCLES = 288
MAX_CHURN_PAUSED_RATIO = 0.25
MAX_SOAK_OVERRUN_SECS = 15 * 60
EXPECTED_SUMMARY_FIELDS = frozenset(
    {
        "git_revision",
        "workspace_source_manifest_sha256",
        "localnet_artifact_path",
        "daemon_binary_path",
        "daemon_binary_blake2b_256",
        "kagami_binary_path",
        "kagami_binary_blake2b_256",
        "test_binary_path",
        "test_binary_blake2b_256",
        "generated_config_blake2b_256",
        "seed",
        "duration_secs",
        "target_tps",
        "packet_loss_percent",
        "churn_interval_secs",
        "max_height_skew",
        "max_height_skew_grace_secs",
        "max_transient_height_skew",
        "stall_timeout_secs",
        "max_view_change_rate",
        "max_lagged_cycle_ratio",
        "min_committed_tps_ratio",
        "process_downtime_secs",
        "tx_attempted",
        "tx_sent",
        "tx_submit_errors",
        "process_churn_cycles",
        "expected_process_churn_cycles",
        "process_churn_lagged_cycles",
        "membership_join_cycles",
        "membership_leave_cycles",
        "expected_membership_churn_cycles",
        "membership_cleanup_leave",
        "membership_churn_lagged_cycles",
        "membership_churn_warning_cycles",
        "churn_paused_secs",
        "churn_paused_ratio",
        "soak_overrun_secs",
        "max_height_skew_observed",
        "view_changes_start",
        "view_changes_end",
        "view_change_rate_per_sec",
        "scheduled_tps",
        "submitted_tps",
        "committed_tps",
        "committed_txs_min_delta",
        "saturated_samples",
        "total_samples",
        "initial_status_snapshots",
        "final_status_snapshots",
        "no_progress_intervals",
        "unclassified_no_progress_intervals",
    }
)
NO_PROGRESS_INTERVAL_FIELDS = frozenset(
    {
        "start_elapsed_ms",
        "end_elapsed_ms",
        "classifications",
        "classified",
        "status_snapshots",
    }
)
LIVENESS_CLASSIFICATIONS = frozenset(
    {
        "missing_proposal",
        "body_unavailable",
        "prepare_quorum_missing",
        "commit_quorum_missing",
        "timeout_certificate_missing",
        "scheduler_starvation",
        "application_pending",
    }
)
STATUS_SNAPSHOT_FIELDS = frozenset({"validator_index", "status"})
STATUS_REQUIRED_FIELDS = frozenset(
    {"protocol_version", "restart_required", "height", "view", "leader", "liveness"}
)
LIVENESS_REQUIRED_FIELDS = frozenset(
    {
        "generation",
        "prepare_quorums",
        "commit_quorums",
        "timeout_quorums",
        "outbound_intents",
        "work",
        "queues",
        "no_progress_age_ms",
        "ignore_counts",
    }
)


class EvidenceError(RuntimeError):
    """Raised when release evidence is missing or inconsistent."""


def _iroha_blake2b_256(payload: bytes) -> str:
    digest = bytearray(hashlib.blake2b(payload, digest_size=32).digest())
    digest[-1] |= 1
    return digest.hex()


def _file_digest(path: Path) -> str:
    return _iroha_blake2b_256(path.read_bytes())


def _generated_config_digest(root: Path) -> str:
    paths = []
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if relative.parts and relative.parts[0] == "storage":
            continue
        if not path.is_file() or path.is_symlink():
            continue
        if path.name == "taira_simulation_summary.json":
            continue
        if path.suffix.removeprefix(".") not in {
            "toml",
            "json",
            "to",
            "sh",
            "yaml",
            "yml",
        }:
            continue
        paths.append(path)

    manifest = bytearray(b"iroha-taira-generated-config-v1\0")
    for path in sorted(paths):
        relative = str(path.relative_to(root)).encode()
        payload = path.read_bytes()
        manifest.extend(len(relative).to_bytes(8, "big"))
        manifest.extend(relative)
        manifest.extend(len(payload).to_bytes(8, "big"))
        manifest.extend(payload)
    return _iroha_blake2b_256(bytes(manifest))


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise EvidenceError(message)


def _require_digest(value: Any, name: str) -> str:
    _require(
        isinstance(value, str)
        and len(value) == 64
        and value == value.lower()
        and all(character in "0123456789abcdef" for character in value),
        f"{name} must be a lowercase 64-character digest",
    )
    return value


def _require_number(summary: dict[str, Any], name: str) -> float:
    value = summary.get(name)
    _require(
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value),
        f"{name} must be a finite number",
    )
    return float(value)


def _require_int(summary: dict[str, Any], name: str, *, minimum: int = 0) -> int:
    value = summary.get(name)
    _require(type(value) is int and value >= minimum, f"{name} must be an integer >= {minimum}")
    return value


def _require_close(actual: float, expected: float, name: str) -> None:
    tolerance = max(1e-9, abs(expected) * 1e-9)
    _require(
        math.isclose(actual, expected, rel_tol=1e-9, abs_tol=tolerance),
        f"{name} is inconsistent with its recorded counters",
    )


def _require_under(path: Path, root: Path, name: str) -> None:
    try:
        path.relative_to(root)
    except ValueError as error:
        raise EvidenceError(f"{name} is outside the source-bound build root: {path}") from error


def _validate_status_snapshots(
    snapshots: Any, name: str, *, require_blocker: bool = False
) -> set[str]:
    _require(isinstance(snapshots, list), f"{name} is not a list")
    validator_indices: set[int] = set()
    blockers: set[str] = set()
    for snapshot in snapshots:
        _require(isinstance(snapshot, dict), f"{name} contains a non-object snapshot")
        _require(
            set(snapshot) == STATUS_SNAPSHOT_FIELDS,
            f"{name} snapshot envelope does not match the evidence schema",
        )
        validator_index = snapshot.get("validator_index")
        _require(
            type(validator_index) is int and 0 <= validator_index < EXPECTED_VALIDATORS,
            f"{name} contains an invalid validator index",
        )
        validator_indices.add(validator_index)
        status = snapshot.get("status")
        _require(isinstance(status, dict), f"{name} contains an invalid status payload")
        _require(
            STATUS_REQUIRED_FIELDS <= set(status),
            f"{name} status payload omits required Sumeragi fields",
        )
        _require(status.get("protocol_version") == 3, f"{name} uses the wrong protocol version")
        _require(status.get("restart_required") is False, f"{name} records a fail-stopped validator")
        for field, minimum in (("height", 1), ("view", 0), ("leader", 0)):
            value = status.get(field)
            _require(
                type(value) is int and value >= minimum,
                f"{name} has an invalid status {field}",
            )
        liveness = status.get("liveness")
        _require(isinstance(liveness, dict), f"{name} omits the liveness payload")
        _require(
            LIVENESS_REQUIRED_FIELDS <= set(liveness),
            f"{name} liveness payload omits required fields",
        )
        for field in (
            "prepare_quorums",
            "commit_quorums",
            "timeout_quorums",
            "outbound_intents",
            "queues",
            "ignore_counts",
        ):
            _require(isinstance(liveness.get(field), list), f"{name} has invalid liveness {field}")
        _require(bool(liveness["queues"]), f"{name} has no bounded-queue evidence")
        _require(isinstance(liveness.get("work"), dict), f"{name} has invalid liveness work")
        for field in ("generation", "no_progress_age_ms"):
            value = liveness.get(field)
            _require(
                type(value) is int and value >= 0,
                f"{name} has invalid liveness {field}",
            )
        blocker = liveness.get("blocker")
        if require_blocker:
            _require(isinstance(blocker, dict), f"{name} lacks a watchdog blocker")
        if blocker is not None:
            _require(
                isinstance(blocker, dict)
                and set(blocker) == {"blocker", "details"}
                and blocker.get("details") is None
                and blocker.get("blocker") in LIVENESS_CLASSIFICATIONS,
                f"{name} has an invalid watchdog blocker",
            )
            blockers.add(blocker["blocker"])
    _require(
        len(validator_indices) >= 3,
        f"{name} lacks a valid quorum of distinct validator snapshots",
    )
    return blockers


def validate_evidence(
    summary: dict[str, Any],
    *,
    source_manifest_sha256: str,
    build_root: Path,
    repo_root: Path,
) -> None:
    """Validate one decoded Taira release summary."""

    _require(
        set(summary) == EXPECTED_SUMMARY_FIELDS,
        "summary fields must exactly match the release evidence schema",
    )
    expected_manifest = _require_digest(
        source_manifest_sha256, "expected workspace source manifest"
    )
    _require(
        summary.get("workspace_source_manifest_sha256") == expected_manifest,
        "summary workspace source manifest does not match the release invocation",
    )
    for name, expected in EXPECTED_PROFILE.items():
        _require(summary.get(name) == expected, f"unexpected {name}: {summary.get(name)!r}")
    duration_secs = _require_int(summary, "duration_secs")
    _require(duration_secs >= EXPECTED_DURATION_SECS, "soak did not run for at least 24 wall-clock hours")
    soak_overrun_secs = _require_number(summary, "soak_overrun_secs")
    _require(
        0 <= soak_overrun_secs <= MAX_SOAK_OVERRUN_SECS,
        "soak exceeded the maximum wall-clock overrun",
    )
    elapsed_secs = EXPECTED_DURATION_SECS + soak_overrun_secs
    _require(
        duration_secs == math.floor(elapsed_secs),
        "duration_secs and soak_overrun_secs describe different elapsed times",
    )

    current_revision = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    _require(summary.get("git_revision") == current_revision, "Git revision drifted")

    build_root = build_root.resolve()
    for prefix in ("daemon", "kagami", "test"):
        path_value = summary.get(f"{prefix}_binary_path")
        _require(isinstance(path_value, str), f"missing {prefix} binary path")
        path = Path(path_value).resolve(strict=True)
        _require_under(path, build_root, f"{prefix} binary")
        recorded = _require_digest(
            summary.get(f"{prefix}_binary_blake2b_256"),
            f"{prefix} binary digest",
        )
        _require(_file_digest(path) == recorded, f"{prefix} binary digest mismatch")

    artifact_value = summary.get("localnet_artifact_path")
    _require(isinstance(artifact_value, str), "missing localnet artifact path")
    artifact_root = Path(artifact_value).resolve(strict=True)
    _require(artifact_root.is_dir(), "localnet artifact path is not a directory")
    recorded_config = _require_digest(
        summary.get("generated_config_blake2b_256"), "generated config digest"
    )
    _require(
        _generated_config_digest(artifact_root) == recorded_config,
        "generated localnet configuration digest mismatch",
    )

    _require(
        summary.get("expected_process_churn_cycles") == EXPECTED_PROCESS_CHURN_CYCLES,
        "process-churn schedule evidence is inconsistent",
    )
    _require(
        summary.get("expected_membership_churn_cycles")
        == EXPECTED_MEMBERSHIP_CHURN_CYCLES,
        "membership-churn schedule evidence is inconsistent",
    )
    process_cycles = _require_int(summary, "process_churn_cycles")
    membership_joins = _require_int(summary, "membership_join_cycles")
    membership_leaves = _require_int(summary, "membership_leave_cycles")
    membership_cycles = membership_joins + membership_leaves
    _require(
        process_cycles >= math.ceil(EXPECTED_PROCESS_CHURN_CYCLES * 0.9),
        "insufficient sustained process churn",
    )
    _require(
        type(membership_cycles) is int
        and membership_cycles >= math.ceil(EXPECTED_MEMBERSHIP_CHURN_CYCLES * 0.9),
        "insufficient sustained membership churn",
    )
    _require(membership_joins > 0, "no membership joins")
    _require(membership_leaves > 0, "no membership leaves")
    _require(type(summary.get("membership_cleanup_leave")) is bool, "invalid cleanup leave flag")
    membership_warning_cycles = _require_int(summary, "membership_churn_warning_cycles")
    _require(membership_warning_cycles <= membership_cycles, "invalid membership warning count")
    churn_paused_secs = _require_number(summary, "churn_paused_secs")
    _require(0 <= churn_paused_secs <= elapsed_secs, "invalid churn paused seconds")
    churn_paused_ratio = _require_number(summary, "churn_paused_ratio")
    _require(
        0 <= churn_paused_ratio <= MAX_CHURN_PAUSED_RATIO,
        "churn consumed too much of the wall-clock soak",
    )
    _require_close(churn_paused_ratio, churn_paused_secs / elapsed_secs, "churn_paused_ratio")

    for lagged_name, total in (
        ("process_churn_lagged_cycles", process_cycles),
        ("membership_churn_lagged_cycles", membership_cycles),
    ):
        lagged = _require_int(summary, lagged_name)
        _require(lagged <= total, f"invalid {lagged_name}")
        _require(lagged / total <= 0.35, f"{lagged_name} exceeds the release ratio")

    attempted = _require_int(summary, "tx_attempted")
    sent = _require_int(summary, "tx_sent")
    submit_errors = _require_int(summary, "tx_submit_errors")
    _require(sent > 0, "no accepted transactions")
    _require(attempted == sent + submit_errors, "transaction accounting is inconsistent")
    _require(submit_errors <= attempted // 20 + 1, "too many submit errors")
    scheduled_tps = _require_number(summary, "scheduled_tps")
    submitted_tps = _require_number(summary, "submitted_tps")
    committed_tps = _require_number(summary, "committed_tps")
    committed_delta = _require_int(summary, "committed_txs_min_delta")
    _require_close(scheduled_tps, attempted / elapsed_secs, "scheduled_tps")
    _require_close(submitted_tps, sent / elapsed_secs, "submitted_tps")
    _require_close(committed_tps, committed_delta / elapsed_secs, "committed_tps")
    _require(scheduled_tps >= 4.0, "scheduled TPS below release floor")
    _require(submitted_tps >= 2.75, "submitted TPS below release floor")
    _require(committed_tps >= 3.0, "committed TPS below release floor")
    view_changes_start = _require_int(summary, "view_changes_start")
    view_changes_end = _require_int(summary, "view_changes_end")
    _require(view_changes_end >= view_changes_start, "view-change counter regressed")
    view_change_rate = _require_number(summary, "view_change_rate_per_sec")
    _require_close(
        view_change_rate,
        (view_changes_end - view_changes_start) / elapsed_secs,
        "view_change_rate_per_sec",
    )
    _require(
        _require_int(summary, "max_height_skew_observed")
        <= EXPECTED_PROFILE["max_transient_height_skew"],
        "transient height skew exceeded the release bound",
    )
    _require(
        view_change_rate <= EXPECTED_PROFILE["max_view_change_rate"],
        "view-change rate exceeded the release bound",
    )
    saturated_samples = _require_int(summary, "saturated_samples")
    total_samples = _require_int(summary, "total_samples", minimum=1)
    _require(saturated_samples <= total_samples, "invalid saturation sample accounting")
    _require(
        summary.get("unclassified_no_progress_intervals") == 0,
        "unclassified no-progress interval present",
    )
    intervals = summary.get("no_progress_intervals")
    _require(isinstance(intervals, list), "no-progress interval evidence is missing")
    previous_interval_end_ms = 0
    maximum_interval_end_ms = math.ceil(elapsed_secs * 1_000)
    for interval in intervals:
        _require(isinstance(interval, dict), "no-progress interval must be an object")
        _require(
            set(interval) == NO_PROGRESS_INTERVAL_FIELDS,
            "no-progress interval fields do not match the evidence schema",
        )
        start_ms = interval.get("start_elapsed_ms")
        end_ms = interval.get("end_elapsed_ms")
        _require(type(start_ms) is int and start_ms >= 0, "invalid interval start")
        _require(type(end_ms) is int and end_ms >= start_ms, "invalid interval end")
        _require(start_ms >= previous_interval_end_ms, "no-progress intervals overlap or regress")
        _require(end_ms <= maximum_interval_end_ms, "no-progress interval exceeds soak duration")
        previous_interval_end_ms = end_ms
        classifications = interval.get("classifications")
        _require(
            isinstance(classifications, list)
            and bool(classifications)
            and all(
                isinstance(classification, str)
                and classification in LIVENESS_CLASSIFICATIONS
                for classification in classifications
            ),
            "no-progress interval has an invalid watchdog classification",
        )
        _require(
            classifications == sorted(set(classifications)),
            "no-progress interval classifications are not unique and canonical",
        )
        _require(interval.get("classified") is True, "no-progress interval lacks a watchdog classification")
        interval_snapshots = interval.get("status_snapshots")
        observed_blockers = _validate_status_snapshots(
            interval_snapshots,
            "no-progress interval status snapshots",
            require_blocker=True,
        )
        _require(
            observed_blockers == set(classifications),
            "no-progress interval classifications disagree with retained status snapshots",
        )
    for name in ("initial_status_snapshots", "final_status_snapshots"):
        _validate_status_snapshots(summary.get(name), name)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("--source-manifest", required=True)
    parser.add_argument("--build-root", type=Path, required=True)
    parser.add_argument(
        "--repo-root", type=Path, default=Path(__file__).resolve().parents[1]
    )
    args = parser.parse_args()
    try:
        payload = args.evidence.read_bytes()
        summary = json.loads(payload)
        _require(isinstance(summary, dict), "summary root must be an object")
        validate_evidence(
            summary,
            source_manifest_sha256=args.source_manifest,
            build_root=args.build_root,
            repo_root=args.repo_root,
        )
    except (EvidenceError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"invalid Taira v2 soak evidence: {error}", file=sys.stderr)
        return 1
    print(f"Taira v2 soak evidence verified: sha256={hashlib.sha256(payload).hexdigest()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
