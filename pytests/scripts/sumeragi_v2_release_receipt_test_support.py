"""Shared constants and encoding helpers for release-receipt contract tests."""

from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path


FINAL_MARKER = (
    "Sumeragi v2 formal gate passed: source-bound TLAPS, all registered "
    "adversarial scheduler/readiness/indexed-height/item-carrier/reply-writer/"
    "recovery/ownership mutations, bounded TLC, trace replay, and production Verus"
)
CHAOS_MARKER = (
    "SUMERAGI_V2_CHAOS_COMPLETED permissioned_heights=50000 "
    "npos_heights=50000 total_heights=100000 supplied_commit_qcs=100000 "
    "supplied_tcs=75000 finalized_validators=400000 wal_append_restarts=314 "
    "fetch_restarts=312 store_restarts=312 validation_restarts=312 "
    "application_restarts=312 stale_generation_rejections=1562 "
    "deferred_fetch_completions=400936 deferred_store_completions=400624 "
    "deferred_validation_completions=400312 "
    "deferred_application_completions=400000 duplicate_commit_qcs=3124 "
    "reordered_commit_batches=75000 reordered_tc_batches=75000 "
    "insufficient_dual_qcs=1030 count_only_qcs=0 power_only_qcs=0 "
    "restart_interval=64 duplicate_interval=32 under_quorum_interval=97 "
    "certificate_source=external_fixture"
)
CHAOS_FIELDS = {
    "schema_version": "2",
    "permissioned_heights": "50000",
    "npos_heights": "50000",
    "completed_heights": "100000",
    "supplied_commit_qcs": "100000",
    "supplied_tcs": "75000",
    "finalized_validators": "400000",
    "wal_append_restarts": "314",
    "fetch_restarts": "312",
    "store_restarts": "312",
    "validation_restarts": "312",
    "application_restarts": "312",
    "stale_generation_rejections": "1562",
    "deferred_fetch_completions": "400936",
    "deferred_store_completions": "400624",
    "deferred_validation_completions": "400312",
    "deferred_application_completions": "400000",
    "duplicate_commit_qcs": "3124",
    "reordered_commit_batches": "75000",
    "reordered_tc_batches": "75000",
    "insufficient_dual_qcs": "1030",
    "count_only_qcs": "0",
    "power_only_qcs": "0",
    "restart_interval": "64",
    "duplicate_interval": "32",
    "under_quorum_interval": "97",
    "certificate_source": "external_fixture",
}
SCENARIOS = (
    "authoritative_v2_genesis_commits_on_every_validator",
    "authoritative_v2_finalizes_through_validator_restart",
    "taira_npos_leader_timeout_commits_within_rotation_bound",
    "real_network_same_subject_locked_reproposal_converges_after_ordered_quorum_release",
    "real_network_distinct_subject_prepare_qcs_converge_after_causal_release",
)
SUMMARY_FIELDS = (
    "profile",
    "source_manifest_sha256",
    "scenario",
    "seed",
    "result",
    "cargo_status",
    "tee_status",
    "run_log_sha256",
    "output",
    "localnet",
    "command",
)
SCALING_CONFIGURATION_DATA = b"[nexus]\nenabled = true\n"
SCALING_TRIAL_HARNESS_DATA = b"#!/usr/bin/env bash\nexit 0\n"
SCALING_IROHAD_SHA256 = "c" * 64
SCALING_IROHA_CLI_SHA256 = "d" * 64
CARGO_VERSION_OUTPUT = b"cargo 1.93.1 (083ac5135 2025-12-15)\n"
RUSTC_VERSION_OUTPUT = (
    b"rustc 1.93.1 (01f6ddf75 2026-02-11)\n"
    b"binary: rustc\n"
    b"commit-hash: 01f6ddf7501f6ddf7501f6ddf7501f6ddf7501f6\n"
    b"commit-date: 2026-02-11\n"
    b"host: x86_64-unknown-linux-gnu\n"
    b"release: 1.93.1\n"
    b"LLVM version: 21.1.0\n"
)
PREBUILT_HOST_TRIPLE = "x86_64-unknown-linux-gnu"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical_json(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode("utf-8")


def artifact_metadata(path: Path, mode: int) -> dict[str, object]:
    return {
        "archive_name": path.name,
        "mode": f"{mode:04o}",
        "sha256": sha256(path),
        "size_bytes": path.stat().st_size,
    }


def protected_metadata(
    path: Path, mode: int, protected_sha256: str
) -> dict[str, object]:
    return {
        "archive_name": path.name,
        "mode": f"{mode:04o}",
        "observed_sha256": sha256(path),
        "protected_sha256": protected_sha256,
        "size_bytes": path.stat().st_size,
    }


def command_record(
    argv: list[str],
    replay_argv: list[str],
    status: int,
    stdout: bytes,
    stderr: bytes,
) -> dict[str, object]:
    return {
        "argv": argv,
        "replay_argv": replay_argv,
        "exit_status": status,
        "stdout_base64": base64.b64encode(stdout).decode("ascii"),
        "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
        "stdout_size_bytes": len(stdout),
        "stderr_base64": base64.b64encode(stderr).decode("ascii"),
        "stderr_sha256": hashlib.sha256(stderr).hexdigest(),
        "stderr_size_bytes": len(stderr),
    }


def write_tsv(path: Path, fields: dict[str, str]) -> None:
    path.write_text(
        "".join(f"{name}\t{value}\n" for name, value in fields.items()),
        encoding="utf-8",
    )
