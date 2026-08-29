#!/usr/bin/env python3
"""Validate a DOI-backed AtomicPrivateSettlementV1 release evidence bundle.

This tool intentionally validates evidence that already exists. It never
creates placeholder qualification results and it does not turn local unit-test
coverage into real-network, independent-audit, or publication evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import re
import sys
from collections import Counter
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

MANIFEST_VERSION = 1
PROTOCOL = "AtomicPrivateSettlementV1"
REQUIRED_PARTICIPANTS = (2, 3, 4, 8, 16)
REQUIRED_LOSS_PERCENTAGES = (5, 10, 20)
REQUIRED_LOSS_PHASES = ("restricted_da", "prepare", "commit")
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
REQUIRED_AUDIT_SCOPES = (
    "air",
    "dummy_slot_selectors",
    "asset_capsule_bindings",
    "sponsor_reimbursement",
    "hybrid_cryptography",
    "auditor_qc_domains",
    "cross_dataspace_state_machine",
)
REQUIRED_ARTIFACT_KINDS = (
    "audit_attestation",
    "audit_report",
    "auditor_key_custody_report",
    "benchmark_raw",
    "benchmark_report",
    "block_wire_capture",
    "canary_manifest",
    "clippy_report",
    "configuration",
    "configuration_manifest",
    "differential_pair_manifest",
    "event_capture",
    "format_report",
    "formal_model_report",
    "hardware_description",
    "kura_artifact",
    "leakage_report",
    "limitations",
    "merge_artifact",
    "message_count_manifest",
    "operator_log",
    "plot",
    "privacy_release_report",
    "protocol_argument",
    "public_p2p_capture",
    "query_capture",
    "randomized_seed_report",
    "real_network_fault_raw",
    "real_network_fault_report",
    "release_binary",
    "release_inventory_report",
    "reproducible_build_report",
    "restricted_p2p_capture",
    "sanitized_capture",
    "sbom",
    "sdk_test_report",
    "snapshot_artifact",
    "soak_report",
    "source_archive",
    "source_lockfile",
    "source_manifest",
    "telemetry_capture",
    "test_report",
    "threat_model",
    "torii_capture",
    "workspace_test_report",
)
REQUIRED_LEAKAGE_CANARY_NAMES = (
    "account_id",
    "amount",
    "asset_alias",
    "asset_id",
    "capsule",
    "memo",
)
REQUIRED_MESSAGE_COUNT_CHANNELS = (
    "torii_requests",
    "torii_responses",
    "public_p2p_messages",
    "restricted_p2p_messages",
    "block_messages",
    "query_responses",
    "event_records",
    "log_records",
    "telemetry_records",
)
REQUIRED_LEAKAGE_ARTIFACT_KINDS = (
    "block_wire_capture",
    "event_capture",
    "kura_artifact",
    "merge_artifact",
    "message_count_manifest",
    "operator_log",
    "public_p2p_capture",
    "query_capture",
    "restricted_p2p_capture",
    "sanitized_capture",
    "snapshot_artifact",
    "telemetry_capture",
    "torii_capture",
)
REQUIRED_DIFFERENTIAL_ARTIFACT_KINDS = (
    "block_wire_capture",
    "event_capture",
    "kura_artifact",
    "merge_artifact",
    "operator_log",
    "public_p2p_capture",
    "query_capture",
    "restricted_p2p_capture",
    "sanitized_capture",
    "snapshot_artifact",
    "telemetry_capture",
    "torii_capture",
)
_HEX_64 = re.compile(r"[0-9a-f]{64}")
_GIT_COMMIT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_DOI = re.compile(r"10\.\d{4,9}/[-._;()/:a-z0-9]+", re.IGNORECASE)
_UTC_TIMESTAMP = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z")
PASS_REPORT_GATES = {
    "clippy_report": "strict_clippy",
    "format_report": "format_verification",
    "privacy_release_report": "serial_privacy_release",
    "release_inventory_report": "release_inventory",
    "sdk_test_report": "sdk_matrix",
    "test_report": "focused_tests",
    "workspace_test_report": "workspace_tests",
}
REQUIRED_FORMAL_CONFIGURATIONS = (
    ("AtomicPrivateSettlementV1_3.cfg", "pass"),
    ("AtomicPrivateSettlementV1_255.cfg", "pass"),
    ("AtomicPrivateSettlementV1_expiry.cfg", "pass"),
    ("AtomicPrivateSettlementV1_partial_apply_bug.cfg", "safety_violation"),
    ("AtomicPrivateSettlementV1_commit_before_prepare_bug.cfg", "safety_violation"),
    ("AtomicPrivateSettlementV1_drop_stage_on_crash_bug.cfg", "safety_violation"),
)
_BENCHMARK_PROFILES = ("private", "transparent_control")
_BENCHMARK_PRIVATE_STAGES = (
    "proof_generation",
    "restricted_upload_availability",
    "auditor_response",
    "committee_verification",
    "prepare",
    "commit",
    "global_finality",
    "end_to_end",
)
_BENCHMARK_RESOURCE_FIELDS = (
    "throughput_bundles_per_second",
    "cpu_seconds",
    "peak_rss_bytes",
    "network_bytes",
    "proof_bytes",
    "receipt_bytes",
    "storage_growth_bytes",
)


class EvidenceError(ValueError):
    """Raised when release evidence is incomplete, unsafe, or inconsistent."""


@dataclass(frozen=True)
class Artifact:
    """One validated artifact declaration."""

    kind: str
    path: PurePosixPath
    sha256: str
    bytes: int


def _exact_fields(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} must be an object")
    actual = set(value)
    if actual != expected:
        raise EvidenceError(
            f"{label} fields mismatch; missing={sorted(expected - actual)} "
            f"unknown={sorted(actual - expected)}"
        )
    return value


def _nonempty_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise EvidenceError(f"{label} must be a non-empty string")
    return value


def _exact_integer(value: Any, expected: int, label: str) -> None:
    if isinstance(value, bool) or not isinstance(value, int) or value != expected:
        raise EvidenceError(f"{label} must be exactly {expected}")


def _exact_list(value: Any, expected: Sequence[Any], label: str) -> None:
    if not isinstance(value, list) or value != list(expected):
        raise EvidenceError(f"{label} must be exactly {list(expected)}")


def _parse_artifact(value: Any, index: int) -> Artifact:
    label = f"artifacts[{index}]"
    record = _exact_fields(value, {"kind", "path", "sha256", "bytes"}, label)
    kind = _nonempty_string(record["kind"], f"{label}.kind")
    if kind not in REQUIRED_ARTIFACT_KINDS:
        raise EvidenceError(f"{label}.kind is not a recognized release artifact kind")
    raw_path = _nonempty_string(record["path"], f"{label}.path")
    path = PurePosixPath(raw_path)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in ("", ".", "..") for part in path.parts)
    ):
        raise EvidenceError(f"{label}.path must be a normalized relative POSIX path")
    digest = record["sha256"]
    if not isinstance(digest, str) or _HEX_64.fullmatch(digest) is None:
        raise EvidenceError(
            f"{label}.sha256 must be 64 lowercase hexadecimal characters"
        )
    byte_count = record["bytes"]
    if (
        isinstance(byte_count, bool)
        or not isinstance(byte_count, int)
        or byte_count < 0
    ):
        raise EvidenceError(f"{label}.bytes must be a non-negative integer")
    return Artifact(kind=kind, path=path, sha256=digest, bytes=byte_count)


def parse_manifest(document: Any) -> tuple[dict[str, Any], list[Artifact]]:
    """Parse the strict V1 release manifest and enforce policy-level gates."""

    manifest = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "worktree_clean",
            "doi",
            "qualification",
            "independent_audit",
            "artifacts",
        },
        "manifest",
    )
    _exact_integer(manifest["version"], MANIFEST_VERSION, "manifest.version")
    if manifest["protocol"] != PROTOCOL:
        raise EvidenceError(f"manifest.protocol must be {PROTOCOL!r}")
    commit = manifest["commit"]
    if not isinstance(commit, str) or _GIT_COMMIT.fullmatch(commit) is None:
        raise EvidenceError("manifest.commit must be a full lowercase Git object id")
    if manifest["worktree_clean"] is not True:
        raise EvidenceError("manifest.worktree_clean must be true")
    doi = _nonempty_string(manifest["doi"], "manifest.doi")
    normalized_doi = doi.removeprefix("https://doi.org/").removeprefix("doi:")
    if _DOI.fullmatch(normalized_doi) is None:
        raise EvidenceError("manifest.doi must contain a canonical DOI")

    qualification = _exact_fields(
        manifest["qualification"],
        {
            "real_network_participants",
            "validators_per_dataspace",
            "quorum",
            "mandatory_signed_rs16_da_rbc",
            "max_unavailable_per_committee",
            "loss_percentages",
            "crash_boundaries",
            "randomized_seeds",
            "soak_seconds",
            "minimum_warmups",
            "minimum_measured_bundles",
        },
        "manifest.qualification",
    )
    _exact_list(
        qualification["real_network_participants"],
        REQUIRED_PARTICIPANTS,
        "manifest.qualification.real_network_participants",
    )
    _exact_integer(
        qualification["validators_per_dataspace"],
        4,
        "manifest.qualification.validators_per_dataspace",
    )
    if qualification["quorum"] != "3-of-4":
        raise EvidenceError("manifest.qualification.quorum must be '3-of-4'")
    if qualification["mandatory_signed_rs16_da_rbc"] is not True:
        raise EvidenceError(
            "manifest.qualification.mandatory_signed_rs16_da_rbc must be true"
        )
    _exact_integer(
        qualification["max_unavailable_per_committee"],
        1,
        "manifest.qualification.max_unavailable_per_committee",
    )
    _exact_list(
        qualification["loss_percentages"],
        REQUIRED_LOSS_PERCENTAGES,
        "manifest.qualification.loss_percentages",
    )
    _exact_list(
        qualification["crash_boundaries"],
        REQUIRED_CRASH_BOUNDARIES,
        "manifest.qualification.crash_boundaries",
    )
    seeds = qualification["randomized_seeds"]
    if isinstance(seeds, bool) or not isinstance(seeds, int) or seeds < 10:
        raise EvidenceError(
            "manifest.qualification.randomized_seeds must be at least 10"
        )
    soak = qualification["soak_seconds"]
    if isinstance(soak, bool) or not isinstance(soak, int) or soak < 7200:
        raise EvidenceError("manifest.qualification.soak_seconds must be at least 7200")
    warmups = qualification["minimum_warmups"]
    if isinstance(warmups, bool) or not isinstance(warmups, int) or warmups < 5:
        raise EvidenceError("manifest.qualification.minimum_warmups must be at least 5")
    measured = qualification["minimum_measured_bundles"]
    if isinstance(measured, bool) or not isinstance(measured, int) or measured < 30:
        raise EvidenceError(
            "manifest.qualification.minimum_measured_bundles must be at least 30"
        )

    audit = _exact_fields(
        manifest["independent_audit"],
        {"independent", "organization", "conclusion", "scopes", "report_path"},
        "manifest.independent_audit",
    )
    if audit["independent"] is not True:
        raise EvidenceError("manifest.independent_audit.independent must be true")
    _nonempty_string(audit["organization"], "manifest.independent_audit.organization")
    if audit["conclusion"] != "passed":
        raise EvidenceError("manifest.independent_audit.conclusion must be 'passed'")
    _exact_list(
        audit["scopes"], REQUIRED_AUDIT_SCOPES, "manifest.independent_audit.scopes"
    )
    audit_report_path = PurePosixPath(
        _nonempty_string(audit["report_path"], "manifest.independent_audit.report_path")
    )

    raw_artifacts = manifest["artifacts"]
    if not isinstance(raw_artifacts, list) or not raw_artifacts:
        raise EvidenceError("manifest.artifacts must be a non-empty list")
    artifacts = [
        _parse_artifact(value, index) for index, value in enumerate(raw_artifacts)
    ]
    paths = [artifact.path for artifact in artifacts]
    if len(paths) != len(set(paths)):
        raise EvidenceError("manifest.artifacts paths must be unique")
    if paths != sorted(paths, key=str):
        raise EvidenceError("manifest.artifacts must be sorted by path")
    present_kinds = {artifact.kind for artifact in artifacts}
    missing_kinds = set(REQUIRED_ARTIFACT_KINDS) - present_kinds
    if missing_kinds:
        raise EvidenceError(
            f"manifest.artifacts is missing kinds: {sorted(missing_kinds)}"
        )
    audit_artifact = next(
        (artifact for artifact in artifacts if artifact.path == audit_report_path), None
    )
    if audit_artifact is None or audit_artifact.kind != "audit_report":
        raise EvidenceError(
            "manifest.independent_audit.report_path must name an audit_report artifact"
        )
    return manifest, artifacts


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _relative_path(value: Any, label: str) -> PurePosixPath:
    raw = _nonempty_string(value, label)
    path = PurePosixPath(raw)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in ("", ".", "..") for part in path.parts)
    ):
        raise EvidenceError(f"{label} must be a normalized relative POSIX path")
    return path


def _validate_artifact_reference(
    value: Any,
    *,
    label: str,
    expected_kind: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    reference = _exact_fields(value, {"path", "sha256", "bytes"}, label)
    artifact_path = _relative_path(reference["path"], f"{label}.path")
    artifact = artifacts_by_path.get(artifact_path)
    if artifact is None or artifact.kind != expected_kind:
        raise EvidenceError(f"{label} must name a {expected_kind} artifact")
    binding = _parse_file_binding(
        {"sha256": reference["sha256"], "bytes": reference["bytes"]}, label
    )
    if binding != (artifact.sha256, artifact.bytes):
        raise EvidenceError(f"{label} binding does not match archive")
    if artifact.bytes == 0:
        raise EvidenceError(f"{label} must not be empty")
    return artifact_path


def _validate_transcript_binding(
    value: Any,
    *,
    label: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    return _validate_artifact_reference(
        value,
        label=label,
        expected_kind="operator_log",
        artifacts_by_path=artifacts_by_path,
    )


def _validate_pass_report(
    path: Path,
    *,
    artifact_kind: str,
    commit: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    """Validate one successful command gate and its separately bound transcript."""

    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read {artifact_kind}: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "gate",
            "command",
            "exit_code",
            "passed",
            "started_at_utc",
            "duration_seconds",
            "details",
            "transcript",
        },
        artifact_kind,
    )
    if report["version"] != MANIFEST_VERSION or report["protocol"] != PROTOCOL:
        raise EvidenceError(f"{artifact_kind} must be a V1 {PROTOCOL} report")
    if report["commit"] != commit:
        raise EvidenceError(f"{artifact_kind} commit differs from release manifest")
    expected_gate = PASS_REPORT_GATES[artifact_kind]
    if report["gate"] != expected_gate:
        raise EvidenceError(f"{artifact_kind}.gate must be {expected_gate!r}")
    _nonempty_string(report["command"], f"{artifact_kind}.command")
    if report["exit_code"] != 0 or report["passed"] is not True:
        raise EvidenceError(f"{artifact_kind} must record a passing zero-exit command")
    started_at = report["started_at_utc"]
    if not isinstance(started_at, str) or _UTC_TIMESTAMP.fullmatch(started_at) is None:
        raise EvidenceError(f"{artifact_kind}.started_at_utc must be canonical UTC")
    duration = report["duration_seconds"]
    if (
        isinstance(duration, bool)
        or not isinstance(duration, (int, float))
        or not math.isfinite(float(duration))
        or duration <= 0
    ):
        raise EvidenceError(f"{artifact_kind}.duration_seconds must be positive")
    details = report["details"]
    if artifact_kind == "release_inventory_report":
        inventory = _exact_fields(
            details,
            {
                "expected_count",
                "actual_count",
                "missing",
                "unexpected",
                "untracked",
                "incorrect_entries",
            },
            f"{artifact_kind}.details",
        )
        expected_count = inventory["expected_count"]
        actual_count = inventory["actual_count"]
        if (
            isinstance(expected_count, bool)
            or not isinstance(expected_count, int)
            or expected_count <= 0
            or actual_count != expected_count
            or any(
                inventory[field] != []
                for field in ("missing", "unexpected", "untracked", "incorrect_entries")
            )
        ):
            raise EvidenceError(
                "release_inventory_report must prove one exact tracked inventory"
            )
    elif artifact_kind == "sdk_test_report":
        sdk_details = _exact_fields(details, {"sdks"}, f"{artifact_kind}.details")
        sdks = sdk_details["sdks"]
        required_sdks = {
            "rust",
            "cli",
            "kotlin",
            "java",
            "swift",
            "python",
            "javascript",
        }
        if not isinstance(sdks, dict) or set(sdks) != required_sdks:
            raise EvidenceError("sdk_test_report must cover every supported SDK")
        for sdk in sorted(required_sdks):
            result = _exact_fields(
                sdks[sdk],
                {"tests", "failures", "skipped", "package_smoke", "passed"},
                f"sdk_test_report.details.sdks.{sdk}",
            )
            if (
                isinstance(result["tests"], bool)
                or not isinstance(result["tests"], int)
                or result["tests"] <= 0
                or result["failures"] != 0
                or result["skipped"] != 0
                or result["package_smoke"] is not True
                or result["passed"] is not True
            ):
                raise EvidenceError(f"sdk_test_report SDK {sdk!r} is not qualified")
    else:
        gate_details = _exact_fields(
            details,
            {"checks", "failures", "skipped"},
            f"{artifact_kind}.details",
        )
        checks = gate_details["checks"]
        skipped = gate_details["skipped"]
        if (
            isinstance(checks, bool)
            or not isinstance(checks, int)
            or checks <= 0
            or gate_details["failures"] != 0
            or isinstance(skipped, bool)
            or not isinstance(skipped, int)
            or skipped < 0
        ):
            raise EvidenceError(f"{artifact_kind} gate details are not passing")
    return _validate_transcript_binding(
        report["transcript"],
        label=f"{artifact_kind}.transcript",
        artifacts_by_path=artifacts_by_path,
    )


def _validate_randomized_seed_report(
    path: Path,
    *,
    commit: str,
    minimum_seeds: int,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read randomized_seed_report: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "seeds",
            "runs_per_seed",
            "failures",
            "passed",
            "transcript",
        },
        "randomized_seed_report",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or report["failures"] != []
    ):
        raise EvidenceError("randomized seed report is not a passing release report")
    seeds = report["seeds"]
    if (
        not isinstance(seeds, list)
        or any(
            isinstance(seed, bool) or not isinstance(seed, int) or seed < 0
            for seed in seeds
        )
        or seeds != sorted(set(seeds))
        or len(seeds) < minimum_seeds
    ):
        raise EvidenceError(
            "randomized seed report lacks the declared unique seed count"
        )
    runs_per_seed = report["runs_per_seed"]
    if (
        isinstance(runs_per_seed, bool)
        or not isinstance(runs_per_seed, int)
        or runs_per_seed <= 0
    ):
        raise EvidenceError("randomized seed report runs_per_seed must be positive")
    return _validate_transcript_binding(
        report["transcript"],
        label="randomized_seed_report.transcript",
        artifacts_by_path=artifacts_by_path,
    )


def _validate_soak_report(
    path: Path,
    *,
    commit: str,
    minimum_seconds: int,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read soak_report: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "duration_seconds",
            "iterations",
            "seeds",
            "validators_per_dataspace",
            "quorum",
            "mandatory_signed_rs16_da_rbc",
            "max_unavailable_per_committee",
            "partial_visibility_observations",
            "partial_spendable_observations",
            "failures",
            "passed",
            "transcript",
        },
        "soak_report",
    )
    duration = report["duration_seconds"]
    iterations = report["iterations"]
    seeds = report["seeds"]
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or report["failures"] != []
        or isinstance(duration, bool)
        or not isinstance(duration, (int, float))
        or not math.isfinite(float(duration))
        or duration < minimum_seconds
        or isinstance(iterations, bool)
        or not isinstance(iterations, int)
        or iterations <= 0
        or not isinstance(seeds, list)
        or not seeds
        or any(
            isinstance(seed, bool) or not isinstance(seed, int) or seed < 0
            for seed in seeds
        )
        or seeds != sorted(set(seeds))
        or report["validators_per_dataspace"] != 4
        or report["quorum"] != "3-of-4"
        or report["mandatory_signed_rs16_da_rbc"] is not True
        or report["max_unavailable_per_committee"] != 1
        or report["partial_visibility_observations"] != 0
        or report["partial_spendable_observations"] != 0
    ):
        raise EvidenceError(
            "soak report does not prove the required atomic two-hour run"
        )
    return _validate_transcript_binding(
        report["transcript"],
        label="soak_report.transcript",
        artifacts_by_path=artifacts_by_path,
    )


def _validate_formal_model_report(
    path: Path,
    *,
    commit: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read formal_model_report: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "tool",
            "tool_version",
            "tool_sha256",
            "model_sha256",
            "configurations",
            "passed",
            "transcript",
        },
        "formal_model_report",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["tool"] != "TLC"
        or report["passed"] is not True
    ):
        raise EvidenceError("formal model report is not a passing TLC V1 report")
    _nonempty_string(report["tool_version"], "formal_model_report.tool_version")
    for field in ("tool_sha256", "model_sha256"):
        value = report[field]
        if not isinstance(value, str) or _HEX_64.fullmatch(value) is None:
            raise EvidenceError(f"formal_model_report.{field} must be SHA-256")
    configurations = report["configurations"]
    if not isinstance(configurations, list) or len(configurations) != len(
        REQUIRED_FORMAL_CONFIGURATIONS
    ):
        raise EvidenceError("formal model report configuration matrix is incomplete")
    observed: list[tuple[str, str]] = []
    for index, value in enumerate(configurations):
        row = _exact_fields(
            value,
            {
                "name",
                "expected_outcome",
                "observed_outcome",
                "generated_states",
                "distinct_states",
                "depth",
            },
            f"formal_model_report.configurations[{index}]",
        )
        name = row["name"]
        expected = row["expected_outcome"]
        outcome = row["observed_outcome"]
        if (
            not isinstance(name, str)
            or not isinstance(expected, str)
            or outcome != expected
        ):
            raise EvidenceError("formal model report outcome differs from expectation")
        observed.append((name, expected))
        for field in ("generated_states", "distinct_states", "depth"):
            count = row[field]
            if isinstance(count, bool) or not isinstance(count, int) or count <= 0:
                raise EvidenceError(
                    f"formal_model_report.configurations[{index}].{field} must be positive"
                )
    if observed != list(REQUIRED_FORMAL_CONFIGURATIONS):
        raise EvidenceError(
            "formal model report lacks an exact positive/negative matrix"
        )
    return _validate_transcript_binding(
        report["transcript"],
        label="formal_model_report.transcript",
        artifacts_by_path=artifacts_by_path,
    )


def _validate_auditor_key_custody_report(
    path: Path,
    *,
    commit: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(
            f"cannot read auditor_key_custody_report: {error}"
        ) from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "provider",
            "hsm_or_kms_backed",
            "signing_encryption_keys_separate",
            "signing_consensus_keys_separate",
            "encryption_consensus_keys_separate",
            "rotation_tested",
            "retired_key_retention_tested",
            "capsule_rewrap_tested",
            "recovery_tested",
            "retention_period_days",
            "findings",
            "passed",
            "transcript",
        },
        "auditor_key_custody_report",
    )
    retention_days = report["retention_period_days"]
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or report["findings"] != []
        or report["hsm_or_kms_backed"] is not True
        or report["signing_encryption_keys_separate"] is not True
        or report["signing_consensus_keys_separate"] is not True
        or report["encryption_consensus_keys_separate"] is not True
        or report["rotation_tested"] is not True
        or report["recovery_tested"] is not True
        or (
            report["retired_key_retention_tested"] is not True
            and report["capsule_rewrap_tested"] is not True
        )
        or isinstance(retention_days, bool)
        or not isinstance(retention_days, int)
        or retention_days <= 0
    ):
        raise EvidenceError(
            "auditor key custody report does not prove separation, rotation, and retention"
        )
    _nonempty_string(report["provider"], "auditor_key_custody_report.provider")
    return _validate_transcript_binding(
        report["transcript"],
        label="auditor_key_custody_report.transcript",
        artifacts_by_path=artifacts_by_path,
    )


def _parse_reproducible_artifact(
    value: Any, *, label: str, archived: bool
) -> tuple[str, str, str, int, PurePosixPath | None]:
    expected = {"target", "name", "sha256", "bytes"}
    if archived:
        expected.add("path")
    row = _exact_fields(value, expected, label)
    target = _nonempty_string(row["target"], f"{label}.target")
    name = _nonempty_string(row["name"], f"{label}.name")
    digest, byte_count = _parse_file_binding(
        {"sha256": row["sha256"], "bytes": row["bytes"]}, label
    )
    if byte_count == 0:
        raise EvidenceError(f"{label}.bytes must be positive")
    artifact_path = _relative_path(row["path"], f"{label}.path") if archived else None
    return target, name, digest, byte_count, artifact_path


def _validate_reproducible_build_report(
    path: Path,
    *,
    commit: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> list[PurePosixPath]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(
            f"cannot read reproducible_build_report: {error}"
        ) from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "source_date_epoch",
            "targets",
            "archived_artifacts",
            "builds",
            "passed",
        },
        "reproducible_build_report",
    )
    epoch = report["source_date_epoch"]
    targets = report["targets"]
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or isinstance(epoch, bool)
        or not isinstance(epoch, int)
        or epoch <= 0
        or not isinstance(targets, list)
        or not targets
        or any(not isinstance(target, str) or not target for target in targets)
        or targets != sorted(set(targets))
    ):
        raise EvidenceError(
            "reproducible build report identity or target set is invalid"
        )
    raw_archived = report["archived_artifacts"]
    if not isinstance(raw_archived, list) or not raw_archived:
        raise EvidenceError("reproducible build report lacks archived artifacts")
    archived_rows = [
        _parse_reproducible_artifact(
            value,
            label=f"reproducible_build_report.archived_artifacts[{index}]",
            archived=True,
        )
        for index, value in enumerate(raw_archived)
    ]
    if archived_rows != sorted(
        archived_rows, key=lambda row: (row[0], row[1], str(row[4]))
    ):
        raise EvidenceError("reproducible build archived artifacts must be sorted")
    if sorted({row[0] for row in archived_rows}) != targets:
        raise EvidenceError("reproducible build targets differ from archived artifacts")
    archived_paths = [row[4] for row in archived_rows]
    if len(archived_paths) != len(set(archived_paths)):
        raise EvidenceError("reproducible build archived paths must be unique")
    declared_release_binaries = {
        artifact.path: artifact
        for artifact in artifacts_by_path.values()
        if artifact.kind == "release_binary"
    }
    if set(archived_paths) != set(declared_release_binaries):
        raise EvidenceError(
            "reproducible build report does not bind every release binary"
        )
    for _, _, digest, byte_count, artifact_path in archived_rows:
        if artifact_path is None:
            raise EvidenceError("reproducible build archived path is missing")
        artifact = declared_release_binaries[artifact_path]
        if (digest, byte_count) != (artifact.sha256, artifact.bytes):
            raise EvidenceError("reproducible build archived binary binding is invalid")
    expected_build_rows = [
        (target, name, digest, byte_count, None)
        for target, name, digest, byte_count, _ in archived_rows
    ]
    raw_builds = report["builds"]
    if not isinstance(raw_builds, list) or len(raw_builds) < 2:
        raise EvidenceError("reproducible build report requires two independent builds")
    builder_ids: set[str] = set()
    environments: set[str] = set()
    transcripts: list[PurePosixPath] = []
    for index, value in enumerate(raw_builds):
        build = _exact_fields(
            value,
            {"builder_id", "environment_sha256", "artifacts", "transcript"},
            f"reproducible_build_report.builds[{index}]",
        )
        builder_id = _nonempty_string(
            build["builder_id"], f"reproducible_build_report.builds[{index}].builder_id"
        )
        environment = build["environment_sha256"]
        if not isinstance(environment, str) or _HEX_64.fullmatch(environment) is None:
            raise EvidenceError("reproducible build environment digest is invalid")
        if builder_id in builder_ids or environment in environments:
            raise EvidenceError(
                "reproducible builds must use distinct builders and environments"
            )
        builder_ids.add(builder_id)
        environments.add(environment)
        rows = build["artifacts"]
        if not isinstance(rows, list):
            raise EvidenceError("reproducible build artifacts must be a list")
        parsed_rows = [
            _parse_reproducible_artifact(
                row,
                label=f"reproducible_build_report.builds[{index}].artifacts[{row_index}]",
                archived=False,
            )
            for row_index, row in enumerate(rows)
        ]
        if parsed_rows != expected_build_rows:
            raise EvidenceError(
                "independent builds did not produce byte-identical artifacts"
            )
        transcripts.append(
            _validate_transcript_binding(
                build["transcript"],
                label=f"reproducible_build_report.builds[{index}].transcript",
                artifacts_by_path=artifacts_by_path,
            )
        )
    if len(transcripts) != len(set(transcripts)):
        raise EvidenceError("independent builds must use distinct transcripts")
    return transcripts


def _validate_cyclonedx_sbom(
    path: Path, *, commit: str, release_binaries: Sequence[Artifact]
) -> None:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read SBOM: {error}") from error
    if not isinstance(document, dict):
        raise EvidenceError("SBOM must be a CycloneDX JSON object")
    if (
        document.get("bomFormat") != "CycloneDX"
        or document.get("specVersion") not in {"1.5", "1.6"}
        or isinstance(document.get("version"), bool)
        or not isinstance(document.get("version"), int)
        or document["version"] <= 0
        or not isinstance(document.get("serialNumber"), str)
        or not document["serialNumber"].startswith("urn:uuid:")
    ):
        raise EvidenceError("SBOM must be versioned CycloneDX 1.5 or 1.6 JSON")
    metadata = document.get("metadata")
    if not isinstance(metadata, dict) or not isinstance(
        metadata.get("component"), dict
    ):
        raise EvidenceError("SBOM metadata must identify the Iroha component")
    component = metadata["component"]
    if component.get("name") != "iroha" or not isinstance(
        component.get("version"), str
    ):
        raise EvidenceError("SBOM metadata component must be Iroha with a version")
    properties = metadata.get("properties")
    if not isinstance(properties, list) or not any(
        isinstance(item, dict)
        and item.get("name") == "iroha.git.commit"
        and item.get("value") == commit
        for item in properties
    ):
        raise EvidenceError("SBOM does not bind the exact release commit")
    components = document.get("components")
    if not isinstance(components, list) or not components:
        raise EvidenceError("SBOM must contain a non-empty component inventory")
    recorded_hashes: set[str] = set()
    for candidate in [component, *components]:
        if not isinstance(candidate, dict):
            raise EvidenceError("SBOM component entry must be an object")
        hashes = candidate.get("hashes", [])
        if not isinstance(hashes, list):
            raise EvidenceError("SBOM component hashes must be a list")
        for item in hashes:
            if (
                isinstance(item, dict)
                and item.get("alg") == "SHA-256"
                and isinstance(item.get("content"), str)
                and _HEX_64.fullmatch(item["content"]) is not None
            ):
                recorded_hashes.add(item["content"])
    missing_hashes = {
        artifact.sha256 for artifact in release_binaries
    } - recorded_hashes
    if missing_hashes:
        raise EvidenceError("SBOM does not hash every archived release binary")


def _validate_source_manifest(
    path: Path,
    *,
    commit: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> PurePosixPath:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read source_manifest: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "tree",
            "worktree_clean",
            "tracked_file_count",
            "modified",
            "untracked",
            "source_archive",
            "source_lockfile",
            "passed",
            "transcript",
        },
        "source_manifest",
    )
    tree = report["tree"]
    tracked = report["tracked_file_count"]
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or not isinstance(tree, str)
        or _GIT_COMMIT.fullmatch(tree) is None
        or report["worktree_clean"] is not True
        or isinstance(tracked, bool)
        or not isinstance(tracked, int)
        or tracked <= 0
        or report["modified"] != []
        or report["untracked"] != []
        or report["passed"] is not True
    ):
        raise EvidenceError("source manifest does not prove one clean exact Git tree")
    _validate_artifact_reference(
        report["source_archive"],
        label="source_manifest.source_archive",
        expected_kind="source_archive",
        artifacts_by_path=artifacts_by_path,
    )
    _validate_artifact_reference(
        report["source_lockfile"],
        label="source_manifest.source_lockfile",
        expected_kind="source_lockfile",
        artifacts_by_path=artifacts_by_path,
    )
    return _validate_transcript_binding(
        report["transcript"],
        label="source_manifest.transcript",
        artifacts_by_path=artifacts_by_path,
    )


def _validate_audit_attestation(
    path: Path,
    *,
    commit: str,
    audit_manifest: dict[str, Any],
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> None:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read audit_attestation: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "independent",
            "organization",
            "conclusion",
            "scopes",
            "issued_at_utc",
            "report_identifier",
            "report",
            "open_critical_findings",
            "open_high_findings",
            "passed",
        },
        "audit_attestation",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["independent"] is not True
        or report["organization"] != audit_manifest["organization"]
        or report["conclusion"] != "passed"
        or report["conclusion"] != audit_manifest["conclusion"]
        or report["scopes"] != audit_manifest["scopes"]
        or report["open_critical_findings"] != 0
        or report["open_high_findings"] != 0
        or report["passed"] is not True
    ):
        raise EvidenceError(
            "audit attestation does not match the independent passing audit declaration"
        )
    issued_at = report["issued_at_utc"]
    if not isinstance(issued_at, str) or _UTC_TIMESTAMP.fullmatch(issued_at) is None:
        raise EvidenceError("audit_attestation.issued_at_utc must be canonical UTC")
    _nonempty_string(report["report_identifier"], "audit_attestation.report_identifier")
    report_path = _validate_artifact_reference(
        report["report"],
        label="audit_attestation.report",
        expected_kind="audit_report",
        artifacts_by_path=artifacts_by_path,
    )
    if report_path != PurePosixPath(audit_manifest["report_path"]):
        raise EvidenceError("audit attestation binds a different audit report")


def _validate_hardware_description(path: Path, *, commit: str) -> None:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read hardware_description: {error}") from error
    report = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "collected_at_utc",
            "host_id",
            "operating_system",
            "kernel",
            "architecture",
            "cpu_model",
            "physical_cores",
            "logical_cores",
            "memory_bytes",
            "storage_model",
            "network_description",
            "clock_policy",
            "power_profile",
            "virtualized",
            "passed",
        },
        "hardware_description",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
        or not isinstance(report["virtualized"], bool)
    ):
        raise EvidenceError("hardware description does not bind the release candidate")
    collected = report["collected_at_utc"]
    if not isinstance(collected, str) or _UTC_TIMESTAMP.fullmatch(collected) is None:
        raise EvidenceError(
            "hardware_description.collected_at_utc must be canonical UTC"
        )
    for field in (
        "host_id",
        "operating_system",
        "kernel",
        "architecture",
        "cpu_model",
        "storage_model",
        "network_description",
        "clock_policy",
        "power_profile",
    ):
        _nonempty_string(report[field], f"hardware_description.{field}")
    physical = report["physical_cores"]
    logical = report["logical_cores"]
    memory = report["memory_bytes"]
    if (
        isinstance(physical, bool)
        or not isinstance(physical, int)
        or physical <= 0
        or isinstance(logical, bool)
        or not isinstance(logical, int)
        or logical < physical
        or isinstance(memory, bool)
        or not isinstance(memory, int)
        or memory <= 0
    ):
        raise EvidenceError("hardware description resource counts are invalid")


def _validate_configuration_manifest(
    path: Path,
    *,
    commit: str,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> dict[int, str]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read configuration_manifest: {error}") from error
    report = _exact_fields(
        document,
        {"version", "protocol", "commit", "configurations", "passed"},
        "configuration_manifest",
    )
    if (
        report["version"] != MANIFEST_VERSION
        or report["protocol"] != PROTOCOL
        or report["commit"] != commit
        or report["passed"] is not True
    ):
        raise EvidenceError(
            "configuration manifest does not bind the release candidate"
        )
    rows = report["configurations"]
    if not isinstance(rows, list) or len(rows) != len(REQUIRED_PARTICIPANTS):
        raise EvidenceError(
            "configuration manifest must cover every real participant count"
        )
    participants_seen: list[int] = []
    configuration_digests: dict[int, str] = {}
    paths: list[PurePosixPath] = []
    for index, value in enumerate(rows):
        row = _exact_fields(
            value,
            {
                "participants",
                "validators_per_dataspace",
                "quorum",
                "mandatory_signed_rs16_da_rbc",
                "path",
                "sha256",
                "bytes",
            },
            f"configuration_manifest.configurations[{index}]",
        )
        participants = row["participants"]
        if (
            participants not in REQUIRED_PARTICIPANTS
            or row["validators_per_dataspace"] != 4
            or row["quorum"] != "3-of-4"
            or row["mandatory_signed_rs16_da_rbc"] is not True
        ):
            raise EvidenceError(
                "configuration manifest contains an invalid network profile"
            )
        reference = {
            "path": row["path"],
            "sha256": row["sha256"],
            "bytes": row["bytes"],
        }
        artifact_path = _validate_artifact_reference(
            reference,
            label=f"configuration_manifest.configurations[{index}]",
            expected_kind="configuration",
            artifacts_by_path=artifacts_by_path,
        )
        participants_seen.append(participants)
        paths.append(artifact_path)
        configuration_digests[participants] = row["sha256"]
    if participants_seen != list(REQUIRED_PARTICIPANTS) or len(paths) != len(
        set(paths)
    ):
        raise EvidenceError(
            "configuration manifest matrix must be canonical and unique"
        )
    declared_configurations = {
        artifact.path
        for artifact in artifacts_by_path.values()
        if artifact.kind == "configuration"
    }
    if set(paths) != declared_configurations:
        raise EvidenceError(
            "configuration manifest does not bind every archived configuration"
        )
    return configuration_digests


def _regenerate_fault_report(raw_paths: Sequence[Path]) -> dict[str, Any]:
    reporter_path = Path(__file__).with_name("private_settlement_fault_report.py")
    spec = importlib.util.spec_from_file_location(
        "_private_settlement_fault_report_for_release", reporter_path
    )
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the strict fault-matrix reporter")
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
        return module.build_report(
            module.load_runs(raw_paths), module.input_bindings(raw_paths)
        )
    except Exception as error:
        raise EvidenceError(
            f"real-network fault raw evidence is invalid: {error}"
        ) from error


def _validate_fault_report(
    path: Path,
    *,
    raw_artifacts: Sequence[Artifact],
    root: Path,
    commit: str,
    hardware_sha256: str,
    configuration_sha256_by_participants: dict[int, str],
) -> None:
    """Bind the release manifest to a passing strict fault-matrix summary."""

    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(
            f"cannot read real-network fault report: {error}"
        ) from error
    record = _exact_fields(
        report,
        {
            "version",
            "protocol",
            "commit",
            "raw_inputs",
            "environment",
            "requirements",
            "matrix",
            "passed",
        },
        "real_network_fault_report",
    )
    if (
        record["version"] != MANIFEST_VERSION
        or record["protocol"] != PROTOCOL
        or record["passed"] is not True
        or record["commit"] != commit
    ):
        raise EvidenceError("real-network fault report must be a passing V1 report")
    raw_inputs = record["raw_inputs"]
    if not isinstance(raw_inputs, list) or not raw_inputs:
        raise EvidenceError("real-network fault report must bind raw JSONL inputs")
    parsed_bindings = [
        _parse_file_binding(value, f"real_network_fault_report.raw_inputs[{index}]")
        for index, value in enumerate(raw_inputs)
    ]
    if parsed_bindings != sorted(parsed_bindings) or Counter(
        parsed_bindings
    ) != Counter((artifact.sha256, artifact.bytes) for artifact in raw_artifacts):
        raise EvidenceError(
            "real-network fault report raw bindings do not match archive"
        )
    environment = _exact_fields(
        record["environment"],
        {"hardware_sha256", "configuration_sha256_by_participants"},
        "real_network_fault_report.environment",
    )
    expected_configurations = {
        str(participants): configuration_sha256_by_participants[participants]
        for participants in REQUIRED_PARTICIPANTS
    }
    if (
        environment["hardware_sha256"] != hardware_sha256
        or environment["configuration_sha256_by_participants"]
        != expected_configurations
    ):
        raise EvidenceError(
            "real-network fault report used different hardware or configs"
        )
    requirements = _exact_fields(
        record["requirements"],
        {
            "participants",
            "minimum_seeds_per_participant",
            "validators_per_dataspace",
            "quorum",
            "loss_phases",
            "loss_percentages",
            "phase_cuts",
            "crash_boundaries",
        },
        "real_network_fault_report.requirements",
    )
    _exact_list(
        requirements["participants"],
        REQUIRED_PARTICIPANTS,
        "real_network_fault_report.requirements.participants",
    )
    _exact_integer(
        requirements["minimum_seeds_per_participant"],
        10,
        "real_network_fault_report.requirements.minimum_seeds_per_participant",
    )
    _exact_integer(
        requirements["validators_per_dataspace"],
        4,
        "real_network_fault_report.requirements.validators_per_dataspace",
    )
    if requirements["quorum"] != "3-of-4":
        raise EvidenceError("real-network fault report quorum must be '3-of-4'")
    _exact_list(
        requirements["loss_phases"],
        ("restricted_da", "prepare", "commit"),
        "real_network_fault_report.requirements.loss_phases",
    )
    _exact_list(
        requirements["loss_percentages"],
        REQUIRED_LOSS_PERCENTAGES,
        "real_network_fault_report.requirements.loss_percentages",
    )
    _exact_list(
        requirements["phase_cuts"],
        (
            "da_before_availability_qc",
            "prepare_before_complete_barrier",
            "commit_before_complete_barrier",
            "carrier_before_global_finality",
        ),
        "real_network_fault_report.requirements.phase_cuts",
    )
    _exact_list(
        requirements["crash_boundaries"],
        REQUIRED_CRASH_BOUNDARIES,
        "real_network_fault_report.requirements.crash_boundaries",
    )
    matrix = record["matrix"]
    expected_keys = {str(participants) for participants in REQUIRED_PARTICIPANTS}
    if not isinstance(matrix, dict) or set(matrix) != expected_keys:
        raise EvidenceError("real-network fault report matrix is incomplete")
    for participants in REQUIRED_PARTICIPANTS:
        bucket = _exact_fields(
            matrix[str(participants)],
            {"runs", "seeds"},
            f"real_network_fault_report.matrix.{participants}",
        )
        runs = bucket["runs"]
        seeds = bucket["seeds"]
        if (
            isinstance(runs, bool)
            or not isinstance(runs, int)
            or runs < 10
            or not isinstance(seeds, list)
            or len(seeds) < 10
            or any(
                isinstance(seed, bool) or not isinstance(seed, int) for seed in seeds
            )
            or seeds != sorted(set(seeds))
        ):
            raise EvidenceError(
                f"real-network fault report N={participants} lacks ten unique seeds"
            )
    raw_paths = [root.joinpath(*artifact.path.parts) for artifact in raw_artifacts]
    if _regenerate_fault_report(raw_paths) != report:
        raise EvidenceError(
            "real-network fault report does not match archived raw runs"
        )


def _parse_file_binding(value: Any, label: str) -> tuple[str, int]:
    record = _exact_fields(value, {"sha256", "bytes"}, label)
    digest = record["sha256"]
    byte_count = record["bytes"]
    if not isinstance(digest, str) or _HEX_64.fullmatch(digest) is None:
        raise EvidenceError(f"{label}.sha256 must be lowercase SHA-256")
    if (
        isinstance(byte_count, bool)
        or not isinstance(byte_count, int)
        or byte_count < 0
    ):
        raise EvidenceError(f"{label}.bytes must be a non-negative integer")
    return digest, byte_count


def _load_canary_names(path: Path) -> list[str]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read canary manifest: {error}") from error
    manifest = _exact_fields(document, {"version", "canaries"}, "canary_manifest")
    if manifest["version"] != MANIFEST_VERSION:
        raise EvidenceError("canary manifest version must be 1")
    entries = manifest["canaries"]
    if not isinstance(entries, list) or not entries:
        raise EvidenceError("canary manifest must contain canaries")
    names: list[str] = []
    for index, value in enumerate(entries):
        entry = _exact_fields(
            value, {"name", "kind", "value"}, f"canary_manifest.canaries[{index}]"
        )
        name = _nonempty_string(
            entry["name"], f"canary_manifest.canaries[{index}].name"
        )
        kind = entry["kind"]
        secret = entry["value"]
        if kind == "text":
            _nonempty_string(secret, f"canary_manifest.canaries[{index}].value")
        elif kind == "integer":
            if isinstance(secret, bool) or not isinstance(secret, int) or secret < 0:
                raise EvidenceError(
                    f"canary_manifest.canaries[{index}].value must be non-negative"
                )
        elif kind == "binary_base64":
            _nonempty_string(secret, f"canary_manifest.canaries[{index}].value")
        else:
            raise EvidenceError(
                f"canary_manifest.canaries[{index}].kind is unsupported"
            )
        names.append(name)
    if names != sorted(set(names)):
        raise EvidenceError("canary manifest names must be unique and sorted")
    if not set(REQUIRED_LEAKAGE_CANARY_NAMES).issubset(names):
        raise EvidenceError("canary manifest lacks a required secret class")
    return names


def _load_message_count_manifest(path: Path) -> dict[str, int]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read message-count manifest: {error}") from error
    manifest = _exact_fields(
        document, {"version", "channels"}, "message_count_manifest"
    )
    if manifest["version"] != MANIFEST_VERSION:
        raise EvidenceError("message-count manifest version must be 1")
    channels = manifest["channels"]
    if not isinstance(channels, dict) or set(channels) != set(
        REQUIRED_MESSAGE_COUNT_CHANNELS
    ):
        raise EvidenceError("message-count manifest channels are incomplete")
    for channel, count in channels.items():
        if isinstance(count, bool) or not isinstance(count, int) or count < 0:
            raise EvidenceError(
                f"message-count manifest channel {channel!r} is invalid"
            )
    return {channel: channels[channel] for channel in REQUIRED_MESSAGE_COUNT_CHANNELS}


def _verify_archived_canary_scan(
    canary_manifest: Path,
    artifacts: Sequence[Artifact],
    root: Path,
) -> None:
    scanner_path = Path(__file__).with_name("private_settlement_leakage_audit.py")
    module_name = "_private_settlement_leakage_audit_for_release"
    spec = importlib.util.spec_from_file_location(module_name, scanner_path)
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the strict leakage scanner")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(module_name)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
        canaries = module.load_canaries(canary_manifest)
        for artifact in artifacts:
            if artifact.kind not in REQUIRED_LEAKAGE_ARTIFACT_KINDS:
                continue
            if module.scan_file(root.joinpath(*artifact.path.parts), canaries):
                raise EvidenceError(
                    "an archived privacy surface contains a planted secret canary"
                )
    except EvidenceError:
        raise
    except Exception as error:
        raise EvidenceError(f"archived canary scan is invalid: {error}") from error
    finally:
        if previous is None:
            del sys.modules[module_name]
        else:
            sys.modules[module_name] = previous


def _public_json_shape(value: Any) -> Any:
    if isinstance(value, dict):
        if any(not isinstance(key, str) for key in value):
            raise EvidenceError("differential JSON object key is not text")
        return {key: _public_json_shape(value[key]) for key in sorted(value)}
    if isinstance(value, list):
        return [_public_json_shape(item) for item in value]
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, int):
        return "integer"
    if isinstance(value, float):
        if not math.isfinite(value):
            raise EvidenceError("differential JSON number is not finite")
        return "number"
    if isinstance(value, str):
        return "string"
    raise EvidenceError("differential JSON contains an unsupported value")


def _validate_differential_pair_manifest(
    path: Path,
    *,
    commit: str,
    root: Path,
    artifacts_by_path: dict[PurePosixPath, Artifact],
) -> list[tuple[str, int]]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(
            f"cannot read differential_pair_manifest: {error}"
        ) from error
    manifest = _exact_fields(
        document,
        {
            "version",
            "protocol",
            "commit",
            "left_root",
            "right_root",
            "pairs",
            "passed",
        },
        "differential_pair_manifest",
    )
    if (
        manifest["version"] != MANIFEST_VERSION
        or manifest["protocol"] != PROTOCOL
        or manifest["commit"] != commit
        or manifest["passed"] is not True
    ):
        raise EvidenceError(
            "differential pair manifest does not bind the release candidate"
        )
    pairs = manifest["pairs"]
    if not isinstance(pairs, list) or not pairs:
        raise EvidenceError("differential pair manifest must contain artifact pairs")
    left_root = _relative_path(
        manifest["left_root"], "differential_pair_manifest.left_root"
    )
    right_root = _relative_path(
        manifest["right_root"], "differential_pair_manifest.right_root"
    )
    if (
        left_root == right_root
        or left_root.is_relative_to(right_root)
        or right_root.is_relative_to(left_root)
    ):
        raise EvidenceError("differential roots must be distinct and non-overlapping")
    for label, relative_root in (("left", left_root), ("right", right_root)):
        directory = root.joinpath(*relative_root.parts)
        if directory.is_symlink() or not directory.is_dir():
            raise EvidenceError(f"differential {label} root is not a real directory")
    ordering: list[tuple[str, str]] = []
    surfaces: set[str] = set()
    referenced_paths: set[PurePosixPath] = set()
    bindings: list[tuple[str, int]] = []
    for index, value in enumerate(pairs):
        label = f"differential_pair_manifest.pairs[{index}]"
        pair = _exact_fields(
            value,
            {"surface", "relative_name", "left", "right"},
            label,
        )
        surface = pair["surface"]
        if surface not in REQUIRED_DIFFERENTIAL_ARTIFACT_KINDS:
            raise EvidenceError(f"{label}.surface is not a privacy surface")
        relative_name = _relative_path(pair["relative_name"], f"{label}.relative_name")
        order_key = (surface, relative_name.as_posix())
        ordering.append(order_key)
        surfaces.add(surface)
        left_path = _validate_artifact_reference(
            pair["left"],
            label=f"{label}.left",
            expected_kind=surface,
            artifacts_by_path=artifacts_by_path,
        )
        right_path = _validate_artifact_reference(
            pair["right"],
            label=f"{label}.right",
            expected_kind=surface,
            artifacts_by_path=artifacts_by_path,
        )
        if (
            left_path != left_root / relative_name
            or right_path != right_root / relative_name
        ):
            raise EvidenceError(
                "differential pair paths do not match their declared roots and relative name"
            )
        if (
            left_path == right_path
            or left_path in referenced_paths
            or right_path in referenced_paths
        ):
            raise EvidenceError(
                "differential pair paths must be distinct and single-use"
            )
        referenced_paths.update((left_path, right_path))
        left = artifacts_by_path[left_path]
        right = artifacts_by_path[right_path]
        if left.bytes != right.bytes:
            raise EvidenceError("differential pair byte sizes differ")
        bindings.extend(((left.sha256, left.bytes), (right.sha256, right.bytes)))
        if relative_name.suffix.lower() == ".json":
            try:
                left_json = json.loads(
                    root.joinpath(*left_path.parts).read_text(encoding="utf-8")
                )
                right_json = json.loads(
                    root.joinpath(*right_path.parts).read_text(encoding="utf-8")
                )
            except (OSError, UnicodeError, json.JSONDecodeError) as error:
                raise EvidenceError(
                    f"cannot parse differential JSON pair: {error}"
                ) from error
            if _public_json_shape(left_json) != _public_json_shape(right_json):
                raise EvidenceError("differential pair JSON public shapes differ")
    if ordering != sorted(ordering) or len(ordering) != len(set(ordering)):
        raise EvidenceError("differential pairs must be canonically ordered and unique")
    if surfaces != set(REQUIRED_DIFFERENTIAL_ARTIFACT_KINDS):
        raise EvidenceError(
            "differential pair manifest does not cover every privacy surface"
        )
    rooted_artifacts = {
        artifact.path
        for artifact in artifacts_by_path.values()
        if artifact.path.is_relative_to(left_root)
        or artifact.path.is_relative_to(right_root)
    }
    if rooted_artifacts != referenced_paths:
        raise EvidenceError(
            "differential roots contain an unpaired or undeclared archive artifact"
        )
    return bindings


def _validate_leakage_report(
    path: Path, artifacts: Sequence[Artifact], root: Path, commit: str
) -> None:
    """Require a clean differential bound to every archived capture byte."""

    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read leakage report: {error}") from error
    record = _exact_fields(
        report,
        {
            "version",
            "passed",
            "canary_manifest",
            "scanned_artifacts",
            "scanned_files",
            "scanned_bytes",
            "canary_names",
            "findings",
            "differential",
            "message_count_manifests",
            "message_count_mismatches",
        },
        "leakage_report",
    )
    if record["version"] != MANIFEST_VERSION or record["passed"] is not True:
        raise EvidenceError("leakage report must be a passing V1 report")
    scanned_files = record["scanned_files"]
    scanned_bytes = record["scanned_bytes"]
    if (
        isinstance(scanned_files, bool)
        or not isinstance(scanned_files, int)
        or scanned_files <= 0
        or isinstance(scanned_bytes, bool)
        or not isinstance(scanned_bytes, int)
        or scanned_bytes <= 0
    ):
        raise EvidenceError("leakage report must scan a non-empty artifact set")
    canary_names = record["canary_names"]
    if (
        not isinstance(canary_names, list)
        or any(not isinstance(name, str) for name in canary_names)
        or canary_names != sorted(set(canary_names))
        or not set(REQUIRED_LEAKAGE_CANARY_NAMES).issubset(canary_names)
    ):
        raise EvidenceError(
            "leakage report lacks account, asset, alias, amount, memo, or capsule canaries"
        )
    if record["findings"] != [] or record["message_count_mismatches"] != []:
        raise EvidenceError("leakage report contains a canary or message-count finding")

    declared_bindings = Counter(
        (artifact.sha256, artifact.bytes)
        for artifact in artifacts
        if artifact.kind != "leakage_report"
    )
    raw_scanned = record["scanned_artifacts"]
    if not isinstance(raw_scanned, list) or not raw_scanned:
        raise EvidenceError("leakage report must bind its scanned artifacts")
    scanned = [
        _parse_file_binding(value, f"leakage_report.scanned_artifacts[{index}]")
        for index, value in enumerate(raw_scanned)
    ]
    if scanned != sorted(scanned):
        raise EvidenceError("leakage report scanned bindings must be sorted")
    scanned_bindings = Counter(scanned)
    if scanned_bindings - declared_bindings:
        raise EvidenceError("leakage report scanned an unarchived artifact")
    if scanned_files != len(scanned) or scanned_bytes != sum(
        byte_count for _, byte_count in scanned
    ):
        raise EvidenceError("leakage report scan totals do not match its bindings")
    required_bindings = Counter(
        (artifact.sha256, artifact.bytes)
        for artifact in artifacts
        if artifact.kind in REQUIRED_LEAKAGE_ARTIFACT_KINDS
    )
    if required_bindings - scanned_bindings:
        raise EvidenceError(
            "leakage report did not scan every archived privacy surface"
        )

    pair_manifests = [
        artifact
        for artifact in artifacts
        if artifact.kind == "differential_pair_manifest"
    ]
    if len(pair_manifests) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one differential pair manifest"
        )
    pair_bindings = Counter(
        _validate_differential_pair_manifest(
            root.joinpath(*pair_manifests[0].path.parts),
            commit=commit,
            root=root,
            artifacts_by_path={artifact.path: artifact for artifact in artifacts},
        )
    )
    if pair_bindings - scanned_bindings:
        raise EvidenceError(
            "differential pair manifest references an unscanned artifact"
        )

    canary_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "canary_manifest"
    ]
    if len(canary_artifacts) != 1:
        raise EvidenceError("evidence bundle must contain exactly one canary manifest")
    canary_binding = _parse_file_binding(
        record["canary_manifest"], "leakage_report.canary_manifest"
    )
    expected_canary = (canary_artifacts[0].sha256, canary_artifacts[0].bytes)
    if canary_binding != expected_canary:
        raise EvidenceError("leakage report used a different canary manifest")
    archived_canary_names = _load_canary_names(
        root.joinpath(*canary_artifacts[0].path.parts)
    )
    if canary_names != archived_canary_names:
        raise EvidenceError("leakage report canary names do not match its manifest")
    _verify_archived_canary_scan(
        root.joinpath(*canary_artifacts[0].path.parts), artifacts, root
    )

    count_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "message_count_manifest"
    ]
    if len(count_artifacts) != 2:
        raise EvidenceError(
            "evidence bundle must contain exactly two message-count manifests"
        )
    raw_count_bindings = record["message_count_manifests"]
    if not isinstance(raw_count_bindings, list) or len(raw_count_bindings) != 2:
        raise EvidenceError("leakage report must bind two message-count manifests")
    count_bindings = [
        _parse_file_binding(value, f"leakage_report.message_count_manifests[{index}]")
        for index, value in enumerate(raw_count_bindings)
    ]
    if count_bindings != sorted(count_bindings) or Counter(count_bindings) != Counter(
        (artifact.sha256, artifact.bytes) for artifact in count_artifacts
    ):
        raise EvidenceError(
            "leakage report message-count bindings do not match archive"
        )
    archived_counts = [
        _load_message_count_manifest(root.joinpath(*artifact.path.parts))
        for artifact in count_artifacts
    ]
    if archived_counts[0] != archived_counts[1]:
        raise EvidenceError("archived differential message counts do not match")

    differential = _exact_fields(
        record["differential"],
        {"left_only", "right_only", "size_mismatches", "json_shape_mismatches"},
        "leakage_report.differential",
    )
    if any(differential.values()):
        raise EvidenceError("leakage report contains a public shape or size finding")


def _finite_nonnegative(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise EvidenceError(f"{label} must be numeric")
    rendered = float(value)
    if not math.isfinite(rendered) or rendered < 0:
        raise EvidenceError(f"{label} must be finite and non-negative")
    return rendered


def _load_benchmark_raw(
    paths: Sequence[Path],
    commit: str,
    hardware_sha256: str,
    configuration_sha256_by_participants: dict[int, str],
) -> dict[tuple[str, int], dict[str, Any]]:
    """Validate the raw benchmark matrix retained in the publication bundle."""

    expected_fields = {
        "version",
        "protocol",
        "commit",
        "hardware_sha256",
        "configuration_sha256",
        "profile",
        "participants",
        "seed",
        "run",
        "warmup",
        "stages_ms",
        *_BENCHMARK_RESOURCE_FIELDS,
    }
    buckets: dict[tuple[str, int], dict[str, Any]] = {}
    identities: set[tuple[str, int, int, int, bool]] = set()
    for path in paths:
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            raise EvidenceError(
                f"cannot read benchmark raw evidence: {error}"
            ) from error
        for line_number, line in enumerate(lines, 1):
            if not line.strip():
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError as error:
                raise EvidenceError(
                    f"benchmark raw {path}:{line_number} is invalid JSON: {error}"
                ) from error
            row = _exact_fields(record, expected_fields, f"benchmark_raw:{line_number}")
            if (
                row["version"] != MANIFEST_VERSION
                or row["protocol"] != PROTOCOL
                or row["commit"] != commit
            ):
                raise EvidenceError(
                    "benchmark raw sample must bind the release protocol and commit"
                )
            profile = row["profile"]
            participants = row["participants"]
            seed = row["seed"]
            run = row["run"]
            warmup = row["warmup"]
            if (
                profile not in _BENCHMARK_PROFILES
                or participants not in REQUIRED_PARTICIPANTS
            ):
                raise EvidenceError(
                    "benchmark raw sample has unsupported profile or participants"
                )
            if (
                row["hardware_sha256"] != hardware_sha256
                or row["configuration_sha256"]
                != configuration_sha256_by_participants[participants]
            ):
                raise EvidenceError(
                    "benchmark raw sample used different hardware or configuration"
                )
            if (
                isinstance(seed, bool)
                or not isinstance(seed, int)
                or seed < 0
                or isinstance(run, bool)
                or not isinstance(run, int)
                or run < 0
                or not isinstance(warmup, bool)
            ):
                raise EvidenceError("benchmark raw sample identity is invalid")
            identity = (profile, participants, seed, run, warmup)
            if identity in identities:
                raise EvidenceError(f"duplicate benchmark raw identity {identity}")
            identities.add(identity)
            stages = row["stages_ms"]
            required_stages = (
                _BENCHMARK_PRIVATE_STAGES
                if profile == "private"
                else ("global_finality", "end_to_end")
            )
            if not isinstance(stages, dict) or set(stages) != set(required_stages):
                raise EvidenceError("benchmark raw stage set is invalid")
            for stage, value in stages.items():
                _finite_nonnegative(value, f"benchmark_raw.stages_ms.{stage}")
            for field in _BENCHMARK_RESOURCE_FIELDS:
                _finite_nonnegative(row[field], f"benchmark_raw.{field}")
            bucket = buckets.setdefault(
                (profile, participants), {"warmups": 0, "measured": 0, "seeds": set()}
            )
            if warmup:
                bucket["warmups"] += 1
            else:
                bucket["measured"] += 1
                bucket["seeds"].add(seed)
    expected_buckets = {
        (profile, participants)
        for profile in _BENCHMARK_PROFILES
        for participants in REQUIRED_PARTICIPANTS
    }
    if set(buckets) != expected_buckets:
        raise EvidenceError("benchmark raw matrix is incomplete")
    for key, bucket in buckets.items():
        if bucket["warmups"] < 5 or bucket["measured"] < 30 or len(bucket["seeds"]) < 2:
            raise EvidenceError(f"benchmark raw bucket {key} lacks required samples")
    return buckets


def _validate_statistical_summary(value: Any, expected_count: int, label: str) -> None:
    summary = _exact_fields(
        value,
        {"count", "mad", "p50", "p50_ci95", "p95", "p95_ci95", "p99", "p99_ci95"},
        label,
    )
    if summary["count"] != expected_count:
        raise EvidenceError(f"{label}.count does not match raw measured runs")
    for field in ("mad", "p50", "p95", "p99"):
        _finite_nonnegative(summary[field], f"{label}.{field}")
    for field in ("p50_ci95", "p95_ci95", "p99_ci95"):
        interval = summary[field]
        if not isinstance(interval, list) or len(interval) != 2:
            raise EvidenceError(f"{label}.{field} must be a two-value interval")
        low = _finite_nonnegative(interval[0], f"{label}.{field}[0]")
        high = _finite_nonnegative(interval[1], f"{label}.{field}[1]")
        if low > high:
            raise EvidenceError(f"{label}.{field} is reversed")


def _regenerate_benchmark_report(
    raw_paths: Sequence[Path], bootstrap_iterations: int
) -> dict[str, Any]:
    reporter_path = Path(__file__).with_name("private_settlement_benchmark_report.py")
    module_name = "_private_settlement_benchmark_report_for_release"
    spec = importlib.util.spec_from_file_location(module_name, reporter_path)
    if spec is None or spec.loader is None:
        raise EvidenceError("cannot load the strict benchmark reporter")
    module = importlib.util.module_from_spec(spec)
    previous = sys.modules.get(module_name)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
        return module.build_report(module.load_jsonl(raw_paths), bootstrap_iterations)
    except Exception as error:
        raise EvidenceError(f"benchmark raw evidence is invalid: {error}") from error
    finally:
        if previous is None:
            del sys.modules[module_name]
        else:
            sys.modules[module_name] = previous


def _validate_benchmark_report(
    path: Path,
    raw: dict[tuple[str, int], dict[str, Any]],
    raw_paths: Sequence[Path],
    commit: str,
    hardware_sha256: str,
    configuration_sha256_by_participants: dict[int, str],
) -> None:
    """Require a passing report whose sample identities match retained raw data."""

    try:
        report = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read benchmark report: {error}") from error
    record = _exact_fields(
        report,
        {
            "version",
            "protocol",
            "commit",
            "environment",
            "requirements",
            "profiles",
            "regressions",
            "passed",
        },
        "benchmark_report",
    )
    if (
        record["version"] != MANIFEST_VERSION
        or record["protocol"] != PROTOCOL
        or record["commit"] != commit
        or record["passed"] is not True
    ):
        raise EvidenceError("benchmark report must be a passing V1 report")
    if record["regressions"] != []:
        raise EvidenceError("benchmark report contains release regressions")
    environment = _exact_fields(
        record["environment"],
        {"hardware_sha256", "configuration_sha256_by_participants"},
        "benchmark_report.environment",
    )
    expected_configurations = {
        str(participants): configuration_sha256_by_participants[participants]
        for participants in REQUIRED_PARTICIPANTS
    }
    if (
        environment["hardware_sha256"] != hardware_sha256
        or environment["configuration_sha256_by_participants"]
        != expected_configurations
    ):
        raise EvidenceError("benchmark report used different hardware or configs")
    requirements = _exact_fields(
        record["requirements"],
        {
            "participants",
            "minimum_warmups",
            "minimum_measured",
            "minimum_seeds",
            "bootstrap_iterations",
        },
        "benchmark_report.requirements",
    )
    _exact_list(
        requirements["participants"],
        REQUIRED_PARTICIPANTS,
        "benchmark_report.requirements.participants",
    )
    for field, minimum in (
        ("minimum_warmups", 5),
        ("minimum_measured", 30),
        ("minimum_seeds", 2),
        ("bootstrap_iterations", 100),
    ):
        value = requirements[field]
        if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
            raise EvidenceError(f"benchmark_report.requirements.{field} is too small")
    profiles = record["profiles"]
    if not isinstance(profiles, dict) or set(profiles) != set(_BENCHMARK_PROFILES):
        raise EvidenceError("benchmark report profiles are incomplete")
    for profile in _BENCHMARK_PROFILES:
        participant_rows = profiles[profile]
        expected_participants = {str(value) for value in REQUIRED_PARTICIPANTS}
        if (
            not isinstance(participant_rows, dict)
            or set(participant_rows) != expected_participants
        ):
            raise EvidenceError(
                f"benchmark report {profile} participant matrix is incomplete"
            )
        for participants in REQUIRED_PARTICIPANTS:
            label = f"benchmark_report.profiles.{profile}.{participants}"
            bucket = _exact_fields(
                participant_rows[str(participants)],
                {"measured_runs", "seeds", "stages_ms", "resources"},
                label,
            )
            raw_bucket = raw[(profile, participants)]
            if bucket["measured_runs"] != raw_bucket["measured"]:
                raise EvidenceError(
                    f"{label}.measured_runs does not match raw evidence"
                )
            expected_seeds = sorted(raw_bucket["seeds"])
            if bucket["seeds"] != expected_seeds:
                raise EvidenceError(f"{label}.seeds does not match raw evidence")
            stages = bucket["stages_ms"]
            required_stages = (
                _BENCHMARK_PRIVATE_STAGES
                if profile == "private"
                else ("global_finality", "end_to_end")
            )
            if not isinstance(stages, dict) or set(stages) != set(required_stages):
                raise EvidenceError(f"{label}.stages_ms is incomplete")
            for stage in required_stages:
                _validate_statistical_summary(
                    stages[stage], raw_bucket["measured"], f"{label}.stages_ms.{stage}"
                )
            resources = bucket["resources"]
            if not isinstance(resources, dict) or set(resources) != set(
                _BENCHMARK_RESOURCE_FIELDS
            ):
                raise EvidenceError(f"{label}.resources is incomplete")
            for field in _BENCHMARK_RESOURCE_FIELDS:
                _validate_statistical_summary(
                    resources[field],
                    raw_bucket["measured"],
                    f"{label}.resources.{field}",
                )

    regenerated = _regenerate_benchmark_report(
        raw_paths, requirements["bootstrap_iterations"]
    )
    for field in (
        "version",
        "protocol",
        "commit",
        "environment",
        "requirements",
        "profiles",
    ):
        if record[field] != regenerated[field]:
            raise EvidenceError(
                "benchmark report statistics do not match archived raw samples"
            )


def verify_bundle(manifest_path: Path) -> dict[str, Any]:
    """Verify a manifest, every declared artifact, and the exact file inventory."""

    if manifest_path.is_symlink() or not manifest_path.is_file():
        raise EvidenceError("manifest path must be a regular non-symlink file")
    root = manifest_path.parent.resolve(strict=True)
    try:
        document = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read release manifest: {error}") from error
    manifest, artifacts = parse_manifest(document)

    declared = {artifact.path.as_posix() for artifact in artifacts}
    actual: set[str] = set()
    total_bytes = 0
    for path in root.rglob("*"):
        if path.is_symlink():
            raise EvidenceError(f"evidence bundle must not contain symlinks: {path}")
        if path.is_file() and path.resolve() != manifest_path.resolve():
            actual.add(path.relative_to(root).as_posix())
    if actual != declared:
        raise EvidenceError(
            f"evidence file inventory mismatch; missing={sorted(declared - actual)} "
            f"unlisted={sorted(actual - declared)}"
        )

    for artifact in artifacts:
        path = root.joinpath(*artifact.path.parts)
        resolved = path.resolve(strict=True)
        if not resolved.is_relative_to(root) or not resolved.is_file():
            raise EvidenceError(
                f"artifact escapes bundle root or is not a file: {artifact.path}"
            )
        byte_count = resolved.stat().st_size
        if byte_count != artifact.bytes:
            raise EvidenceError(
                f"artifact byte count mismatch for {artifact.path}: "
                f"expected {artifact.bytes}, got {byte_count}"
            )
        digest = _sha256(resolved)
        if digest != artifact.sha256:
            raise EvidenceError(f"artifact SHA-256 mismatch for {artifact.path}")
        total_bytes += byte_count

    artifacts_by_path = {artifact.path: artifact for artifact in artifacts}
    hardware_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "hardware_description"
    ]
    if len(hardware_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one hardware description"
        )
    _validate_hardware_description(
        root.joinpath(*hardware_artifacts[0].path.parts), commit=manifest["commit"]
    )
    configuration_manifests = [
        artifact for artifact in artifacts if artifact.kind == "configuration_manifest"
    ]
    if len(configuration_manifests) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one configuration manifest"
        )
    configuration_digests = _validate_configuration_manifest(
        root.joinpath(*configuration_manifests[0].path.parts),
        commit=manifest["commit"],
        artifacts_by_path=artifacts_by_path,
    )

    fault_reports = [
        artifact
        for artifact in artifacts
        if artifact.kind == "real_network_fault_report"
    ]
    fault_raw = [
        artifact for artifact in artifacts if artifact.kind == "real_network_fault_raw"
    ]
    if len(fault_reports) != 1 or not fault_raw:
        raise EvidenceError(
            "evidence bundle must contain one fault report and non-empty raw fault evidence"
        )
    _validate_fault_report(
        root.joinpath(*fault_reports[0].path.parts),
        raw_artifacts=fault_raw,
        root=root,
        commit=manifest["commit"],
        hardware_sha256=hardware_artifacts[0].sha256,
        configuration_sha256_by_participants=configuration_digests,
    )

    leakage_report_paths = [
        root.joinpath(*artifact.path.parts)
        for artifact in artifacts
        if artifact.kind == "leakage_report"
    ]
    if len(leakage_report_paths) != 1:
        raise EvidenceError("evidence bundle must contain exactly one leakage report")
    _validate_leakage_report(
        leakage_report_paths[0], artifacts, root, manifest["commit"]
    )

    gate_transcripts: list[PurePosixPath] = []
    for artifact_kind in PASS_REPORT_GATES:
        gate_artifacts = [
            artifact for artifact in artifacts if artifact.kind == artifact_kind
        ]
        if len(gate_artifacts) != 1:
            raise EvidenceError(
                f"evidence bundle must contain exactly one {artifact_kind}"
            )
        artifact = gate_artifacts[0]
        gate_transcripts.append(
            _validate_pass_report(
                root.joinpath(*artifact.path.parts),
                artifact_kind=artifact_kind,
                commit=manifest["commit"],
                artifacts_by_path=artifacts_by_path,
            )
        )
    randomized_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "randomized_seed_report"
    ]
    if len(randomized_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one randomized seed report"
        )
    gate_transcripts.append(
        _validate_randomized_seed_report(
            root.joinpath(*randomized_artifacts[0].path.parts),
            commit=manifest["commit"],
            minimum_seeds=manifest["qualification"]["randomized_seeds"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    soak_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "soak_report"
    ]
    if len(soak_artifacts) != 1:
        raise EvidenceError("evidence bundle must contain exactly one soak report")
    gate_transcripts.append(
        _validate_soak_report(
            root.joinpath(*soak_artifacts[0].path.parts),
            commit=manifest["commit"],
            minimum_seconds=manifest["qualification"]["soak_seconds"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    formal_artifacts = [
        artifact for artifact in artifacts if artifact.kind == "formal_model_report"
    ]
    if len(formal_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one formal model report"
        )
    gate_transcripts.append(
        _validate_formal_model_report(
            root.joinpath(*formal_artifacts[0].path.parts),
            commit=manifest["commit"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    custody_artifacts = [
        artifact
        for artifact in artifacts
        if artifact.kind == "auditor_key_custody_report"
    ]
    if len(custody_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one auditor key custody report"
        )
    gate_transcripts.append(
        _validate_auditor_key_custody_report(
            root.joinpath(*custody_artifacts[0].path.parts),
            commit=manifest["commit"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    reproducible_artifacts = [
        artifact
        for artifact in artifacts
        if artifact.kind == "reproducible_build_report"
    ]
    if len(reproducible_artifacts) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one reproducible build report"
        )
    gate_transcripts.extend(
        _validate_reproducible_build_report(
            root.joinpath(*reproducible_artifacts[0].path.parts),
            commit=manifest["commit"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    release_binaries = [
        artifact for artifact in artifacts if artifact.kind == "release_binary"
    ]
    sbom_artifacts = [artifact for artifact in artifacts if artifact.kind == "sbom"]
    if len(sbom_artifacts) != 1:
        raise EvidenceError("evidence bundle must contain exactly one SBOM")
    _validate_cyclonedx_sbom(
        root.joinpath(*sbom_artifacts[0].path.parts),
        commit=manifest["commit"],
        release_binaries=release_binaries,
    )
    source_manifests = [
        artifact for artifact in artifacts if artifact.kind == "source_manifest"
    ]
    if len(source_manifests) != 1:
        raise EvidenceError("evidence bundle must contain exactly one source manifest")
    gate_transcripts.append(
        _validate_source_manifest(
            root.joinpath(*source_manifests[0].path.parts),
            commit=manifest["commit"],
            artifacts_by_path=artifacts_by_path,
        )
    )
    audit_attestations = [
        artifact for artifact in artifacts if artifact.kind == "audit_attestation"
    ]
    if len(audit_attestations) != 1:
        raise EvidenceError(
            "evidence bundle must contain exactly one audit attestation"
        )
    _validate_audit_attestation(
        root.joinpath(*audit_attestations[0].path.parts),
        commit=manifest["commit"],
        audit_manifest=manifest["independent_audit"],
        artifacts_by_path=artifacts_by_path,
    )
    if len(gate_transcripts) != len(set(gate_transcripts)):
        raise EvidenceError("release command gates must use distinct transcripts")

    benchmark_raw_paths = [
        root.joinpath(*artifact.path.parts)
        for artifact in artifacts
        if artifact.kind == "benchmark_raw"
    ]
    benchmark_report_paths = [
        root.joinpath(*artifact.path.parts)
        for artifact in artifacts
        if artifact.kind == "benchmark_report"
    ]
    if len(benchmark_report_paths) != 1:
        raise EvidenceError("evidence bundle must contain exactly one benchmark report")
    _validate_benchmark_report(
        benchmark_report_paths[0],
        _load_benchmark_raw(
            benchmark_raw_paths,
            manifest["commit"],
            hardware_artifacts[0].sha256,
            configuration_digests,
        ),
        benchmark_raw_paths,
        manifest["commit"],
        hardware_artifacts[0].sha256,
        configuration_digests,
    )

    canonical_manifest = json.dumps(
        manifest, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    bundle_binding = hashlib.sha256(
        canonical_manifest
        + b"\n"
        + b"\n".join(
            f"{artifact.path}\t{artifact.bytes}\t{artifact.sha256}".encode()
            for artifact in artifacts
        )
    ).hexdigest()
    return {
        "version": MANIFEST_VERSION,
        "protocol": PROTOCOL,
        "commit": manifest["commit"],
        "doi": manifest["doi"],
        "artifact_count": len(artifacts),
        "artifact_bytes": total_bytes,
        "bundle_binding_sha256": bundle_binding,
        "passed": True,
    }


def _write_report(report: dict[str, Any], output: Path | None) -> None:
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if output is None:
        sys.stdout.write(rendered)
    else:
        output.write_text(rendered, encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    """Run the release evidence validator."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path, help="path to release-manifest-v1.json")
    parser.add_argument("--output", type=Path, help="optional validation report path")
    args = parser.parse_args(argv)
    try:
        report = verify_bundle(args.manifest)
        _write_report(report, args.output)
    except (EvidenceError, OSError) as error:
        print(f"atomic private settlement evidence rejected: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
