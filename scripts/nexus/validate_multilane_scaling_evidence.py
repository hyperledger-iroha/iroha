#!/usr/bin/env python3
"""Validate a Sumeragi V2 one-lane/four-lane horizontal-scaling evidence bundle.

The validator uses only the Python standard library.  It treats the bundle as
untrusted release evidence: JSON must be strict, referenced files must be
regular in-bundle files with matching SHA-256 digests, all ten runs must be
present in canonical pair order, and every recorded value is recomputed before
the scaling thresholds are evaluated.
"""

from __future__ import annotations

import argparse
import errno
import hashlib
import json
import math
import os
import re
import secrets
import stat
import statistics
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any, NoReturn, Sequence


EVIDENCE_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.evidence.v1"
RUN_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.run.v1"
IDENTITY_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.identity.v1"
REPORT_SCHEMA = "iroha.sumeragi_v2.multilane_scaling.validation.v1"
EXPECTED_PAIR_COUNT = 5
MIN_INTERVAL_SAMPLES = 20
MIN_LATENCY_SAMPLES = 100
MIN_THROUGHPUT_RATIO = 1.5
MAX_P95_LATENCY_RATIO = 1.25
MAX_OFFERED_LOAD_DEVIATION_FRACTION = 0.01
SEED_DERIVATION = "sha256(seed_namespace + ':' + decimal_pair_index)"
MAX_BUNDLE_FILE_COUNT = 256
MAX_BUNDLE_FILE_BYTES = 256 * 1024 * 1024
MAX_BUNDLE_TOTAL_BYTES = 2 * 1024 * 1024 * 1024

REQUIRED_TOOLING = (
    ("localnet", "scripts/deploy_localnet.sh"),
    ("load_generator", "scripts/tx_load.py"),
    ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
)

_DIGEST_RE = re.compile(r"^[0-9a-f]{64}$")
_REVISION_RE = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
_SEED_NAMESPACE_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
_SAFE_PATH_COMPONENT_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")

_MANIFEST_FIELDS = {
    "schema",
    "generated_at_utc",
    "pair_count",
    "seed_namespace",
    "seed_derivation",
    "identity",
    "configuration",
    "workload",
    "budgets",
    "observation_scope",
    "thresholds",
    "trial_harness",
    "validator",
    "tooling",
    "runs",
}
_IDENTITY_FIELDS = {"schema", "hardware", "software"}
_HARDWARE_FIELDS = {
    "machine_id",
    "cpu_model",
    "physical_core_count",
    "logical_core_count",
    "memory_bytes",
    "storage_model",
}
_SOFTWARE_FIELDS = {
    "os",
    "kernel",
    "architecture",
    "python_version",
    "rustc_version",
    "source_revision",
    "workspace_source_sha256",
    "nexus_config_sha256",
    "irohad_sha256",
    "iroha_cli_sha256",
}
_WORKLOAD_FIELDS = {
    "offered_load_tps",
    "warmup_seconds",
    "measurement_seconds",
    "min_interval_samples",
    "min_latency_samples",
    "max_offered_load_deviation_fraction",
}
_BUDGET_FIELDS = {
    "queue_depth_max",
    "index_entries_max",
    "memory_bytes_max",
    "disk_bytes_max",
}
_SCOPE_FIELDS = {"queue", "index", "memory", "disk"}
_THRESHOLD_FIELDS = {
    "min_four_lane_throughput_ratio",
    "max_four_lane_p95_latency_ratio",
}
_TOOL_FIELDS = {"role", "source_path", "artifact"}
_RUN_ENTRY_FIELDS = {
    "sequence",
    "pair_index",
    "variant",
    "active_execution_lanes",
    "seed",
    "status",
    "skipped",
    "exit_code",
    "raw_samples",
    "command_log",
}
_RAW_RUN_FIELDS = {
    "schema",
    "pair_index",
    "variant",
    "active_execution_lanes",
    "execution_lane_ids",
    "seed",
    "identity_before",
    "identity_after",
    "workload",
    "status",
    "summary",
    "samples",
    "artifacts",
}
_STATUS_FIELDS = {"outcome", "skipped", "failure"}
_SUMMARY_FIELDS = {
    "offered_count",
    "accepted_count",
    "committed_count",
    "queue_depth_max",
    "index_entries_max",
    "memory_bytes_max",
    "disk_bytes_max",
}
_SAMPLE_FIELDS = {
    "sequence",
    "start_offset_seconds",
    "end_offset_seconds",
    "offered_count",
    "accepted_count",
    "committed_count",
    "commit_latencies_ms",
    "queue_depth",
    "index_entries",
    "memory_bytes",
    "disk_bytes",
}
_FILE_REF_FIELDS = {"path", "sha256"}
_RUN_ARTIFACT_FIELDS = {
    "nexus_load_test_manifest",
    "lifecycle_snapshot",
    "metrics_snapshot",
    "load_generator_log",
}


class EvidenceError(ValueError):
    """The supplied release-evidence bundle violates the G-SCALE contract."""


@dataclass(frozen=True)
class RunMetrics:
    """Metrics recomputed from one raw run."""

    pair_index: int
    variant: str
    lane_ids: tuple[str, ...]
    offered_count: int
    accepted_count: int
    committed_count: int
    throughput_tps: float
    p95_latency_ms: float
    interval_sample_count: int
    latency_sample_count: int
    latencies_ms: tuple[float, ...]
    maxima: dict[str, int]


def _fail(message: str) -> NoReturn:
    raise EvidenceError(message)


def _strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def _reject_constant(value: str) -> NoReturn:
    _fail(f"nonfinite JSON numeric literal is forbidden: {value}")


def load_json(path: Path, label: str) -> Any:
    """Load strict UTF-8 JSON, rejecting duplicate keys and nonfinite literals."""

    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise EvidenceError(f"{label} cannot be read: {path}: {error}") from error
    try:
        return json.loads(
            text,
            object_pairs_hook=_strict_object,
            parse_constant=_reject_constant,
        )
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError(f"{label} is not strict UTF-8 JSON: {path}: {error}") from error


def sha256_file(path: Path) -> str:
    """Return the lowercase SHA-256 digest of a regular file."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def derive_seed(namespace: str, pair_index: int) -> str:
    """Derive the deterministic seed shared by both variants of one pair."""

    return hashlib.sha256(f"{namespace}:{pair_index}".encode("utf-8")).hexdigest()


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        _fail(f"{label} must be a list")
    return value


def _require_exact_fields(value: dict[str, Any], fields: set[str], label: str) -> None:
    actual = set(value)
    if actual != fields:
        missing = sorted(fields - actual)
        extra = sorted(actual - fields)
        _fail(f"{label} fields differ from schema; missing={missing}, extra={extra}")


def _require_text(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or "\x00" in value
        or "\n" in value
        or "\r" in value
    ):
        _fail(f"{label} must be a non-empty, trimmed single-line string")
    return value


def _require_safe_relative_path(value: Any, label: str) -> str:
    relative = _require_text(value, label)
    pure = PurePosixPath(relative)
    if (
        relative != pure.as_posix()
        or pure.is_absolute()
        or not pure.parts
        or any(
            part in {"", ".", ".."}
            or _SAFE_PATH_COMPONENT_RE.fullmatch(part) is None
            for part in pure.parts
        )
    ):
        _fail(f"{label} must be a normalized relative safe in-bundle path")
    return relative


def _scan_bundle(root: Path) -> tuple[set[str], set[str]]:
    """Enumerate a bounded bundle without following links or special files."""

    try:
        root_metadata = root.lstat()
    except OSError as error:
        raise EvidenceError(
            f"evidence bundle root cannot be inspected: {root}: {error}"
        ) from error
    if stat.S_ISLNK(root_metadata.st_mode) or not stat.S_ISDIR(
        root_metadata.st_mode
    ):
        _fail(f"evidence bundle root must be a non-symlink directory: {root}")

    files: set[str] = set()
    directories: set[str] = set()
    inodes: dict[tuple[int, int], str] = {}
    total_bytes = 0

    def visit(directory: Path, prefix: PurePosixPath | None) -> None:
        nonlocal total_bytes
        try:
            entries = sorted(os.scandir(directory), key=lambda entry: entry.name)
        except OSError as error:
            raise EvidenceError(
                f"evidence bundle directory cannot be enumerated: {directory}: {error}"
            ) from error
        for entry in entries:
            component = entry.name
            if _SAFE_PATH_COMPONENT_RE.fullmatch(component) is None:
                _fail(
                    "evidence bundle contains an unsafe path component: "
                    f"{component!r}"
                )
            relative_path = (
                PurePosixPath(component)
                if prefix is None
                else prefix / component
            )
            relative = relative_path.as_posix()
            try:
                metadata = entry.stat(follow_symlinks=False)
            except OSError as error:
                raise EvidenceError(
                    f"evidence bundle entry cannot be inspected: {relative}: {error}"
                ) from error
            if stat.S_ISLNK(metadata.st_mode):
                _fail(f"evidence bundle contains a symlink: {relative}")
            if stat.S_ISDIR(metadata.st_mode):
                directories.add(relative)
                visit(Path(entry.path), relative_path)
                continue
            if not stat.S_ISREG(metadata.st_mode):
                _fail(f"evidence bundle contains a nonregular entry: {relative}")
            if metadata.st_nlink != 1:
                _fail(f"evidence bundle file has a hard-link alias: {relative}")
            inode = (metadata.st_dev, metadata.st_ino)
            alias = inodes.get(inode)
            if alias is not None:
                _fail(
                    "evidence bundle files are hard-link aliases: "
                    f"{alias} and {relative}"
                )
            inodes[inode] = relative
            if metadata.st_size > MAX_BUNDLE_FILE_BYTES:
                _fail(
                    f"evidence bundle file exceeds {MAX_BUNDLE_FILE_BYTES} bytes: "
                    f"{relative}"
                )
            total_bytes += metadata.st_size
            if total_bytes > MAX_BUNDLE_TOTAL_BYTES:
                _fail(
                    "evidence bundle exceeds aggregate size limit "
                    f"{MAX_BUNDLE_TOTAL_BYTES} bytes"
                )
            files.add(relative)
            if len(files) > MAX_BUNDLE_FILE_COUNT:
                _fail(
                    "evidence bundle exceeds file-count limit "
                    f"{MAX_BUNDLE_FILE_COUNT}"
                )

    visit(root, None)
    return files, directories


def _expected_bundle_directories(files: set[str]) -> set[str]:
    directories: set[str] = set()
    for relative in files:
        parent = PurePosixPath(relative).parent
        while parent != PurePosixPath("."):
            directories.add(parent.as_posix())
            parent = parent.parent
    return directories


def _require_int(value: Any, label: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum:
        _fail(f"{label} must be an integer >= {minimum}")
    return value


def _require_number(
    value: Any,
    label: str,
    *,
    minimum: float | None = None,
    strictly_positive: bool = False,
) -> float:
    if (
        not isinstance(value, (int, float))
        or isinstance(value, bool)
        or not math.isfinite(value)
    ):
        _fail(f"{label} must be a finite number")
    number = float(value)
    if strictly_positive and number <= 0:
        _fail(f"{label} must be greater than zero")
    if minimum is not None and number < minimum:
        _fail(f"{label} must be >= {minimum}")
    return number


def _require_digest(value: Any, label: str) -> str:
    digest = _require_text(value, label)
    if _DIGEST_RE.fullmatch(digest) is None:
        _fail(f"{label} must be a lowercase 64-hex SHA-256 digest")
    return digest


def _require_ref(
    value: Any,
    root: Path,
    label: str,
    *,
    referenced_paths: set[str] | None = None,
) -> Path:
    ref = _require_object(value, label)
    _require_exact_fields(ref, _FILE_REF_FIELDS, label)
    relative = _require_safe_relative_path(ref["path"], f"{label}.path")
    expected_digest = _require_digest(ref["sha256"], f"{label}.sha256")
    relative_path = PurePosixPath(relative)

    candidate = root
    for part in relative_path.parts:
        candidate = candidate / part
        try:
            if candidate.is_symlink():
                _fail(f"{label}.path traverses a symlink: {relative}")
        except OSError as error:
            raise EvidenceError(f"{label}.path cannot be inspected: {relative}: {error}") from error
    if not candidate.is_file():
        _fail(f"{label}.path is not a regular file: {relative}")
    actual_digest = sha256_file(candidate)
    if actual_digest != expected_digest:
        _fail(
            f"{label}.sha256 mismatch for {relative}: "
            f"recorded={expected_digest}, actual={actual_digest}"
        )
    if referenced_paths is not None:
        referenced_paths.add(relative)
    return candidate


def _require_timestamp(value: Any, label: str) -> None:
    raw = _require_text(value, label)
    if not raw.endswith("Z"):
        _fail(f"{label} must be an RFC3339 UTC timestamp ending in Z")
    try:
        parsed = datetime.fromisoformat(raw[:-1] + "+00:00")
    except ValueError as error:
        raise EvidenceError(f"{label} is not a valid RFC3339 timestamp") from error
    if parsed.tzinfo != timezone.utc:
        _fail(f"{label} must identify UTC")


def validate_identity(value: Any, label: str = "identity") -> dict[str, Any]:
    """Validate and return a pinned hardware/software identity object."""

    identity = _require_object(value, label)
    _require_exact_fields(identity, _IDENTITY_FIELDS, label)
    if identity["schema"] != IDENTITY_SCHEMA:
        _fail(f"{label}.schema must be {IDENTITY_SCHEMA!r}")

    hardware = _require_object(identity["hardware"], f"{label}.hardware")
    _require_exact_fields(hardware, _HARDWARE_FIELDS, f"{label}.hardware")
    for field in ("machine_id", "cpu_model", "storage_model"):
        _require_text(hardware[field], f"{label}.hardware.{field}")
    physical = _require_int(
        hardware["physical_core_count"],
        f"{label}.hardware.physical_core_count",
        minimum=1,
    )
    logical = _require_int(
        hardware["logical_core_count"],
        f"{label}.hardware.logical_core_count",
        minimum=1,
    )
    if physical > logical:
        _fail(f"{label}.hardware physical cores cannot exceed logical cores")
    _require_int(hardware["memory_bytes"], f"{label}.hardware.memory_bytes", minimum=1)

    software = _require_object(identity["software"], f"{label}.software")
    _require_exact_fields(software, _SOFTWARE_FIELDS, f"{label}.software")
    for field in (
        "os",
        "kernel",
        "architecture",
        "python_version",
        "rustc_version",
    ):
        _require_text(software[field], f"{label}.software.{field}")
    revision = _require_text(software["source_revision"], f"{label}.software.source_revision")
    if _REVISION_RE.fullmatch(revision) is None:
        _fail(f"{label}.software.source_revision must be lowercase 40- or 64-hex")
    for field in (
        "workspace_source_sha256",
        "nexus_config_sha256",
        "irohad_sha256",
        "iroha_cli_sha256",
    ):
        _require_digest(software[field], f"{label}.software.{field}")
    return identity


def _nearest_rank_p95(samples: Sequence[float]) -> float:
    if not samples:
        _fail("cannot compute p95 from an empty latency sample")
    ordered = sorted(samples)
    return ordered[math.ceil(0.95 * len(ordered)) - 1]


def _same_number(actual: float, expected: float) -> bool:
    return math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12)


def _validate_raw_run(
    raw: Any,
    *,
    label: str,
    pair_index: int,
    variant: str,
    active_lanes: int,
    seed: str,
    identity: dict[str, Any],
    workload: dict[str, Any],
    budgets: dict[str, Any],
    evidence_root: Path,
    seen_support_paths: set[Path],
    referenced_paths: set[str],
) -> RunMetrics:
    run = _require_object(raw, label)
    _require_exact_fields(run, _RAW_RUN_FIELDS, label)
    if run["schema"] != RUN_SCHEMA:
        _fail(f"{label}.schema must be {RUN_SCHEMA!r}")
    if run["pair_index"] != pair_index:
        _fail(f"{label}.pair_index does not match its manifest pair")
    if run["variant"] != variant:
        _fail(f"{label}.variant does not match its manifest variant")
    if run["active_execution_lanes"] != active_lanes:
        _fail(f"{label}.active_execution_lanes does not match the required variant")
    if run["seed"] != seed:
        _fail(f"{label}.seed does not match the deterministic pair seed")
    if run["identity_before"] != identity:
        _fail(f"{label}.identity_before drifted from the pinned bundle identity")
    if run["identity_after"] != identity:
        _fail(f"{label}.identity_after drifted from the pinned bundle identity")

    lane_ids_raw = _require_list(run["execution_lane_ids"], f"{label}.execution_lane_ids")
    if len(lane_ids_raw) != active_lanes:
        _fail(
            f"{label}.execution_lane_ids must contain exactly {active_lanes} "
            "active execution lanes"
        )
    lane_ids = tuple(
        _require_text(item, f"{label}.execution_lane_ids[{index}]")
        for index, item in enumerate(lane_ids_raw)
    )
    if len(set(lane_ids)) != len(lane_ids):
        _fail(f"{label}.execution_lane_ids contains a duplicate lane")
    if list(lane_ids) != sorted(lane_ids):
        _fail(f"{label}.execution_lane_ids must use canonical sorted order")

    artifacts = _require_object(run["artifacts"], f"{label}.artifacts")
    _require_exact_fields(artifacts, _RUN_ARTIFACT_FIELDS, f"{label}.artifacts")
    artifact_paths: dict[str, Path] = {}
    for name in sorted(_RUN_ARTIFACT_FIELDS):
        path = _require_ref(
            artifacts[name],
            evidence_root,
            f"{label}.artifacts.{name}",
            referenced_paths=referenced_paths,
        )
        if path in seen_support_paths:
            _fail(f"{label}.artifacts.{name} duplicates support evidence from another run")
        seen_support_paths.add(path)
        artifact_paths[name] = path
    nexus_manifest = _require_object(
        load_json(
            artifact_paths["nexus_load_test_manifest"],
            f"{label} Nexus lane-load manifest",
        ),
        f"{label} Nexus lane-load manifest",
    )
    if nexus_manifest.get("version") != 1:
        _fail(f"{label} Nexus lane-load manifest must have version 1")
    if nexus_manifest.get("lanes") != list(lane_ids):
        _fail(f"{label} Nexus lane-load manifest lanes do not match active execution lanes")
    if nexus_manifest.get("workload_seed") != seed:
        _fail(f"{label} Nexus lane-load manifest workload_seed does not match the pair seed")

    raw_workload = _require_object(run["workload"], f"{label}.workload")
    _require_exact_fields(raw_workload, _WORKLOAD_FIELDS, f"{label}.workload")
    if raw_workload != workload:
        _fail(f"{label}.workload drifted from the pinned bundle workload")

    status = _require_object(run["status"], f"{label}.status")
    _require_exact_fields(status, _STATUS_FIELDS, f"{label}.status")
    if status["outcome"] != "passed":
        _fail(f"{label}.status.outcome must be 'passed'")
    if status["skipped"] is not False:
        _fail(f"{label}.status.skipped must be false")
    if status["failure"] is not None:
        _fail(f"{label}.status.failure must be null for a passed run")

    summary = _require_object(run["summary"], f"{label}.summary")
    _require_exact_fields(summary, _SUMMARY_FIELDS, f"{label}.summary")
    offered_count = _require_int(summary["offered_count"], f"{label}.summary.offered_count", minimum=1)
    accepted_count = _require_int(summary["accepted_count"], f"{label}.summary.accepted_count")
    committed_count = _require_int(
        summary["committed_count"],
        f"{label}.summary.committed_count",
        minimum=1,
    )
    if accepted_count > offered_count:
        _fail(f"{label}.summary.accepted_count exceeds offered_count")
    if committed_count > accepted_count:
        _fail(f"{label}.summary.committed_count exceeds accepted_count")

    samples = _require_list(run["samples"], f"{label}.samples")
    minimum_intervals = workload["min_interval_samples"]
    if len(samples) < minimum_intervals:
        _fail(
            f"{label}.samples has weak interval sample count: "
            f"{len(samples)} < {minimum_intervals}"
        )

    count_totals = {"offered_count": 0, "accepted_count": 0, "committed_count": 0}
    maxima = {
        "queue_depth_max": 0,
        "index_entries_max": 0,
        "memory_bytes_max": 0,
        "disk_bytes_max": 0,
    }
    sample_to_maximum = {
        "queue_depth": "queue_depth_max",
        "index_entries": "index_entries_max",
        "memory_bytes": "memory_bytes_max",
        "disk_bytes": "disk_bytes_max",
    }
    latencies: list[float] = []
    previous_end = 0.0
    for offset, sample_raw in enumerate(samples, start=1):
        sample_label = f"{label}.samples[{offset - 1}]"
        sample = _require_object(sample_raw, sample_label)
        _require_exact_fields(sample, _SAMPLE_FIELDS, sample_label)
        if sample["sequence"] != offset:
            _fail(f"{sample_label}.sequence must be {offset}")
        start = _require_number(
            sample["start_offset_seconds"],
            f"{sample_label}.start_offset_seconds",
            minimum=0,
        )
        end = _require_number(
            sample["end_offset_seconds"],
            f"{sample_label}.end_offset_seconds",
            strictly_positive=True,
        )
        if not _same_number(start, previous_end):
            _fail(f"{sample_label} is unordered or leaves a measurement interval gap")
        if end <= start:
            _fail(f"{sample_label}.end_offset_seconds must exceed its start")
        previous_end = end

        for count_name in count_totals:
            count = _require_int(sample[count_name], f"{sample_label}.{count_name}")
            count_totals[count_name] += count

        latency_values = _require_list(
            sample["commit_latencies_ms"],
            f"{sample_label}.commit_latencies_ms",
        )
        interval_committed = sample["committed_count"]
        if len(latency_values) != interval_committed:
            _fail(
                f"{sample_label}.commit_latencies_ms must contain one latency "
                "for every committed transaction"
            )
        for latency_index, latency_raw in enumerate(latency_values):
            latencies.append(
                _require_number(
                    latency_raw,
                    f"{sample_label}.commit_latencies_ms[{latency_index}]",
                    strictly_positive=True,
                )
            )

        for sample_name, maximum_name in sample_to_maximum.items():
            observed = _require_int(sample[sample_name], f"{sample_label}.{sample_name}")
            budget = budgets[maximum_name]
            if observed > budget:
                _fail(
                    f"{sample_label}.{sample_name} exceeds {maximum_name} budget: "
                    f"{observed} > {budget}"
                )
            maxima[maximum_name] = max(maxima[maximum_name], observed)

    measurement_seconds = workload["measurement_seconds"]
    if not _same_number(previous_end, measurement_seconds):
        _fail(f"{label}.samples do not exactly cover measurement_seconds")
    for count_name, computed in count_totals.items():
        if summary[count_name] != computed:
            _fail(
                f"{label}.summary.{count_name} is inconsistent with raw samples: "
                f"recorded={summary[count_name]}, computed={computed}"
            )
    for maximum_name, computed in maxima.items():
        recorded = _require_int(summary[maximum_name], f"{label}.summary.{maximum_name}")
        if recorded != computed:
            _fail(
                f"{label}.summary.{maximum_name} is inconsistent with raw samples: "
                f"recorded={recorded}, computed={computed}"
            )
        if recorded > budgets[maximum_name]:
            _fail(
                f"{label}.summary.{maximum_name} exceeds its budget: "
                f"{recorded} > {budgets[maximum_name]}"
            )

    minimum_latencies = workload["min_latency_samples"]
    if len(latencies) < minimum_latencies:
        _fail(
            f"{label} has weak latency sample count: "
            f"{len(latencies)} < {minimum_latencies}"
        )
    if len(latencies) != committed_count:
        _fail(f"{label} latency sample count does not equal committed_count")

    actual_offered_tps = offered_count / measurement_seconds
    target_offered_tps = workload["offered_load_tps"]
    deviation = abs(actual_offered_tps - target_offered_tps) / target_offered_tps
    if deviation > workload["max_offered_load_deviation_fraction"]:
        _fail(
            f"{label} actual offered load deviates from target by {deviation:.6f}, "
            f"over {workload['max_offered_load_deviation_fraction']:.6f}"
        )

    return RunMetrics(
        pair_index=pair_index,
        variant=variant,
        lane_ids=lane_ids,
        offered_count=offered_count,
        accepted_count=accepted_count,
        committed_count=committed_count,
        throughput_tps=committed_count / measurement_seconds,
        p95_latency_ms=_nearest_rank_p95(latencies),
        interval_sample_count=len(samples),
        latency_sample_count=len(latencies),
        latencies_ms=tuple(latencies),
        maxima=maxima,
    )


def validate_evidence(
    manifest_path: Path,
    *,
    expected_source_revision: str | None = None,
    expected_workspace_source_sha256: str | None = None,
    expected_validator_sha256: str | None = None,
    expected_trial_harness_sha256: str | None = None,
    expected_configuration_sha256: str | None = None,
    expected_irohad_sha256: str | None = None,
    expected_iroha_cli_sha256: str | None = None,
    expected_repository_root: Path | None = None,
) -> dict[str, Any]:
    """Validate *manifest_path* and return deterministic, recomputed metrics.

    The optional expected values bind an otherwise self-consistent benchmark
    bundle to the exact release candidate, operator-approved harness/config,
    measured binaries, and retained tooling which consume it. They are used by
    the source-sealed release corridor before any long-running network gate
    starts.
    """

    for value, label in (
        (expected_workspace_source_sha256, "expected workspace source"),
        (expected_validator_sha256, "expected validator"),
        (expected_trial_harness_sha256, "expected trial harness"),
        (expected_configuration_sha256, "expected configuration"),
        (expected_irohad_sha256, "expected irohad"),
        (expected_iroha_cli_sha256, "expected iroha CLI"),
    ):
        if value is not None:
            _require_digest(value, label)
    retained_root: Path | None = None
    if expected_repository_root is not None:
        if (
            not expected_repository_root.is_absolute()
            or Path(os.path.abspath(expected_repository_root))
            != expected_repository_root
            or expected_repository_root.is_symlink()
            or not expected_repository_root.is_dir()
            or expected_repository_root.resolve() != expected_repository_root
        ):
            _fail(
                "expected repository root must be one absolute canonical "
                "non-symlink directory"
            )
        retained_root = expected_repository_root

    if manifest_path.is_symlink() or not manifest_path.is_file():
        _fail(f"evidence manifest must be a regular non-symlink file: {manifest_path}")
    manifest_path = manifest_path.resolve()
    if manifest_path.name != "scaling_evidence.json":
        _fail("evidence manifest must use the canonical scaling_evidence.json name")
    root = manifest_path.parent
    bundle_files, bundle_directories = _scan_bundle(root)
    referenced_paths: set[str] = set()
    manifest = _require_object(load_json(manifest_path, "evidence manifest"), "evidence manifest")
    _require_exact_fields(manifest, _MANIFEST_FIELDS, "evidence manifest")
    if manifest["schema"] != EVIDENCE_SCHEMA:
        _fail(f"evidence manifest.schema must be {EVIDENCE_SCHEMA!r}")
    _require_timestamp(manifest["generated_at_utc"], "evidence manifest.generated_at_utc")
    if manifest["pair_count"] != EXPECTED_PAIR_COUNT:
        _fail(f"evidence manifest.pair_count must be exactly {EXPECTED_PAIR_COUNT}")

    namespace = _require_text(manifest["seed_namespace"], "evidence manifest.seed_namespace")
    if _SEED_NAMESPACE_RE.fullmatch(namespace) is None:
        _fail("evidence manifest.seed_namespace has invalid characters or length")
    if manifest["seed_derivation"] != SEED_DERIVATION:
        _fail(f"evidence manifest.seed_derivation must be {SEED_DERIVATION!r}")

    identity_path = _require_ref(
        manifest["identity"],
        root,
        "evidence manifest.identity",
        referenced_paths=referenced_paths,
    )
    identity = validate_identity(load_json(identity_path, "pinned identity"), "pinned identity")
    if (
        expected_source_revision is not None
        and identity["software"]["source_revision"] != expected_source_revision
    ):
        _fail(
            "pinned identity software.source_revision does not match the "
            "expected release source"
        )
    if (
        expected_workspace_source_sha256 is not None
        and identity["software"]["workspace_source_sha256"]
        != expected_workspace_source_sha256
    ):
        _fail(
            "pinned identity software.workspace_source_sha256 does not match "
            "the expected sealed workspace"
        )
    if (
        expected_irohad_sha256 is not None
        and identity["software"]["irohad_sha256"] != expected_irohad_sha256
    ):
        _fail(
            "pinned identity software.irohad_sha256 does not match the "
            "expected measured binary"
        )
    if (
        expected_iroha_cli_sha256 is not None
        and identity["software"]["iroha_cli_sha256"]
        != expected_iroha_cli_sha256
    ):
        _fail(
            "pinned identity software.iroha_cli_sha256 does not match the "
            "expected measured binary"
        )
    config_path = _require_ref(
        manifest["configuration"],
        root,
        "evidence manifest.configuration",
        referenced_paths=referenced_paths,
    )
    if sha256_file(config_path) != identity["software"]["nexus_config_sha256"]:
        _fail("configuration artifact does not match identity.software.nexus_config_sha256")
    if (
        expected_configuration_sha256 is not None
        and sha256_file(config_path) != expected_configuration_sha256
    ):
        _fail(
            "evidence manifest.configuration does not match the expected "
            "release configuration"
        )

    workload = _require_object(manifest["workload"], "evidence manifest.workload")
    _require_exact_fields(workload, _WORKLOAD_FIELDS, "evidence manifest.workload")
    workload_values = {
        "offered_load_tps": _require_number(
            workload["offered_load_tps"],
            "evidence manifest.workload.offered_load_tps",
            strictly_positive=True,
        ),
        "warmup_seconds": _require_number(
            workload["warmup_seconds"],
            "evidence manifest.workload.warmup_seconds",
            minimum=0,
        ),
        "measurement_seconds": _require_number(
            workload["measurement_seconds"],
            "evidence manifest.workload.measurement_seconds",
            strictly_positive=True,
        ),
        "min_interval_samples": _require_int(
            workload["min_interval_samples"],
            "evidence manifest.workload.min_interval_samples",
            minimum=MIN_INTERVAL_SAMPLES,
        ),
        "min_latency_samples": _require_int(
            workload["min_latency_samples"],
            "evidence manifest.workload.min_latency_samples",
            minimum=MIN_LATENCY_SAMPLES,
        ),
        "max_offered_load_deviation_fraction": _require_number(
            workload["max_offered_load_deviation_fraction"],
            "evidence manifest.workload.max_offered_load_deviation_fraction",
            minimum=0,
        ),
    }
    if not _same_number(
        workload_values["max_offered_load_deviation_fraction"],
        MAX_OFFERED_LOAD_DEVIATION_FRACTION,
    ):
        _fail(
            "evidence manifest.workload.max_offered_load_deviation_fraction "
            f"must be exactly {MAX_OFFERED_LOAD_DEVIATION_FRACTION}"
        )

    budgets = _require_object(manifest["budgets"], "evidence manifest.budgets")
    _require_exact_fields(budgets, _BUDGET_FIELDS, "evidence manifest.budgets")
    budget_values = {
        field: _require_int(value, f"evidence manifest.budgets.{field}", minimum=1)
        for field, value in budgets.items()
    }

    scopes = _require_object(manifest["observation_scope"], "evidence manifest.observation_scope")
    _require_exact_fields(scopes, _SCOPE_FIELDS, "evidence manifest.observation_scope")
    for field in sorted(_SCOPE_FIELDS):
        _require_text(scopes[field], f"evidence manifest.observation_scope.{field}")

    thresholds = _require_object(manifest["thresholds"], "evidence manifest.thresholds")
    _require_exact_fields(thresholds, _THRESHOLD_FIELDS, "evidence manifest.thresholds")
    throughput_threshold = _require_number(
        thresholds["min_four_lane_throughput_ratio"],
        "evidence manifest.thresholds.min_four_lane_throughput_ratio",
        strictly_positive=True,
    )
    latency_threshold = _require_number(
        thresholds["max_four_lane_p95_latency_ratio"],
        "evidence manifest.thresholds.max_four_lane_p95_latency_ratio",
        strictly_positive=True,
    )
    if not _same_number(throughput_threshold, MIN_THROUGHPUT_RATIO):
        _fail(
            "evidence manifest cannot weaken or change the throughput threshold; "
            f"expected {MIN_THROUGHPUT_RATIO}"
        )
    if not _same_number(latency_threshold, MAX_P95_LATENCY_RATIO):
        _fail(
            "evidence manifest cannot weaken or change the p95 latency threshold; "
            f"expected {MAX_P95_LATENCY_RATIO}"
        )

    trial_harness_path = _require_ref(
        manifest["trial_harness"],
        root,
        "evidence manifest.trial_harness",
        referenced_paths=referenced_paths,
    )
    if (
        expected_trial_harness_sha256 is not None
        and sha256_file(trial_harness_path) != expected_trial_harness_sha256
    ):
        _fail(
            "evidence manifest.trial_harness does not match the expected "
            "operator-approved harness"
        )
    validator_path = _require_ref(
        manifest["validator"],
        root,
        "evidence manifest.validator",
        referenced_paths=referenced_paths,
    )
    if (
        expected_validator_sha256 is not None
        and sha256_file(validator_path) != expected_validator_sha256
    ):
        _fail(
            "evidence manifest.validator does not match the expected retained "
            "validator"
        )
    tooling = _require_list(manifest["tooling"], "evidence manifest.tooling")
    if len(tooling) != len(REQUIRED_TOOLING):
        _fail("evidence manifest.tooling must contain the three required tooling artifacts")
    for index, (entry_raw, expected) in enumerate(zip(tooling, REQUIRED_TOOLING)):
        role, source_path = expected
        label = f"evidence manifest.tooling[{index}]"
        entry = _require_object(entry_raw, label)
        _require_exact_fields(entry, _TOOL_FIELDS, label)
        if entry["role"] != role or entry["source_path"] != source_path:
            _fail(f"{label} does not identify required tool {role}:{source_path}")
        artifact_path = _require_ref(
            entry["artifact"],
            root,
            f"{label}.artifact",
            referenced_paths=referenced_paths,
        )
        if retained_root is not None:
            retained_path = retained_root.joinpath(*PurePosixPath(source_path).parts)
            if (
                retained_path.is_symlink()
                or not retained_path.is_file()
                or retained_path.resolve() != retained_path
                or sha256_file(artifact_path) != sha256_file(retained_path)
            ):
                _fail(
                    f"{label}.artifact does not match retained repository tool "
                    f"{source_path}"
                )

    runs = _require_list(manifest["runs"], "evidence manifest.runs")
    expected_run_count = EXPECTED_PAIR_COUNT * 2
    if len(runs) != expected_run_count:
        _fail(
            "evidence manifest.runs must contain exactly ten entries "
            "(five complete one-lane/four-lane pairs)"
        )

    expected_order = [
        (pair_index, variant, active_lanes)
        for pair_index in range(1, EXPECTED_PAIR_COUNT + 1)
        for variant, active_lanes in (("one_lane", 1), ("four_lane", 4))
    ]
    seen_raw_paths: set[Path] = set()
    seen_log_paths: set[Path] = set()
    seen_support_paths: set[Path] = set()
    metrics: list[RunMetrics] = []
    for sequence, (entry_raw, expected) in enumerate(
        zip(runs, expected_order),
        start=1,
    ):
        pair_index, variant, active_lanes = expected
        label = f"evidence manifest.runs[{sequence - 1}]"
        entry = _require_object(entry_raw, label)
        _require_exact_fields(entry, _RUN_ENTRY_FIELDS, label)
        actual_identity = (
            entry["pair_index"],
            entry["variant"],
            entry["active_execution_lanes"],
        )
        if actual_identity != expected:
            _fail(
                "evidence manifest.runs has missing, duplicate, or unordered pairs; "
                f"entry {sequence} must be {expected}, got {actual_identity}"
            )
        if entry["sequence"] != sequence:
            _fail(f"{label}.sequence must be {sequence}")
        seed = derive_seed(namespace, pair_index)
        if entry["seed"] != seed:
            _fail(f"{label}.seed does not match deterministic pair derivation")
        if entry["status"] != "passed":
            _fail(f"{label}.status must be 'passed'")
        if entry["skipped"] is not False:
            _fail(f"{label}.skipped must be false")
        if entry["exit_code"] != 0 or type(entry["exit_code"]) is not int:
            _fail(f"{label}.exit_code must be integer zero")

        raw_path = _require_ref(
            entry["raw_samples"],
            root,
            f"{label}.raw_samples",
            referenced_paths=referenced_paths,
        )
        log_path = _require_ref(
            entry["command_log"],
            root,
            f"{label}.command_log",
            referenced_paths=referenced_paths,
        )
        if raw_path in seen_raw_paths:
            _fail(f"{label}.raw_samples duplicates another run path")
        seen_raw_paths.add(raw_path)
        if log_path in seen_log_paths:
            _fail(f"{label}.command_log duplicates another run path")
        seen_log_paths.add(log_path)
        if raw_path == log_path:
            _fail(f"{label} cannot use the raw sample file as its command log")
        raw = load_json(raw_path, f"{label} raw samples")
        metrics.append(
            _validate_raw_run(
                raw,
                label=f"pair {pair_index} {variant}",
                pair_index=pair_index,
                variant=variant,
                active_lanes=active_lanes,
                seed=seed,
                identity=identity,
                workload=workload_values,
                budgets=budget_values,
                evidence_root=root,
                seen_support_paths=seen_support_paths,
                referenced_paths=referenced_paths,
            )
        )

    pairs: list[dict[str, Any]] = []
    one_runs: list[RunMetrics] = []
    four_runs: list[RunMetrics] = []
    baseline_one_lane_ids: tuple[str, ...] | None = None
    baseline_four_lane_ids: tuple[str, ...] | None = None
    baseline_offered_count: int | None = None
    for pair_index in range(1, EXPECTED_PAIR_COUNT + 1):
        one = metrics[(pair_index - 1) * 2]
        four = metrics[(pair_index - 1) * 2 + 1]
        if one.offered_count != four.offered_count:
            _fail(
                f"pair {pair_index} offered load is not matched: "
                f"one_lane={one.offered_count}, four_lane={four.offered_count}"
            )
        if one.lane_ids[0] not in four.lane_ids:
            _fail(f"pair {pair_index} four-lane set does not contain the baseline execution lane")
        if baseline_one_lane_ids is None:
            baseline_one_lane_ids = one.lane_ids
            baseline_four_lane_ids = four.lane_ids
            baseline_offered_count = one.offered_count
        elif one.lane_ids != baseline_one_lane_ids or four.lane_ids != baseline_four_lane_ids:
            _fail(f"pair {pair_index} active execution-lane identity drifted across trials")
        if one.offered_count != baseline_offered_count:
            _fail(
                f"pair {pair_index} offered count drifted across trials: "
                f"{one.offered_count} != {baseline_offered_count}"
            )
        one_runs.append(one)
        four_runs.append(four)
        pairs.append(
            {
                "pair_index": pair_index,
                "seed": derive_seed(namespace, pair_index),
                "offered_count": one.offered_count,
                "one_lane_accepted_count": one.accepted_count,
                "four_lane_accepted_count": four.accepted_count,
                "one_lane_committed_count": one.committed_count,
                "four_lane_committed_count": four.committed_count,
                "one_lane_committed_throughput_tps": one.throughput_tps,
                "four_lane_committed_throughput_tps": four.throughput_tps,
                "one_lane_p95_latency_ms": one.p95_latency_ms,
                "four_lane_p95_latency_ms": four.p95_latency_ms,
                "one_lane_interval_samples": one.interval_sample_count,
                "four_lane_interval_samples": four.interval_sample_count,
                "one_lane_latency_samples": one.latency_sample_count,
                "four_lane_latency_samples": four.latency_sample_count,
                "one_lane_resource_maxima": one.maxima,
                "four_lane_resource_maxima": four.maxima,
            }
        )

    one_median = statistics.median(run.throughput_tps for run in one_runs)
    four_median = statistics.median(run.throughput_tps for run in four_runs)
    throughput_ratio = four_median / one_median
    one_latencies = tuple(value for run in one_runs for value in run.latencies_ms)
    four_latencies = tuple(value for run in four_runs for value in run.latencies_ms)
    one_p95 = _nearest_rank_p95(one_latencies)
    four_p95 = _nearest_rank_p95(four_latencies)
    latency_ratio = four_p95 / one_p95
    one_resource_maxima = {
        field: max(run.maxima[field] for run in one_runs)
        for field in sorted(_BUDGET_FIELDS)
    }
    four_resource_maxima = {
        field: max(run.maxima[field] for run in four_runs)
        for field in sorted(_BUDGET_FIELDS)
    }

    if throughput_ratio < MIN_THROUGHPUT_RATIO:
        _fail(
            "four-lane median committed throughput gate failed: "
            f"ratio={throughput_ratio:.12g} < {MIN_THROUGHPUT_RATIO}"
        )
    if latency_ratio > MAX_P95_LATENCY_RATIO:
        _fail(
            "four-lane pooled p95 commit latency gate failed: "
            f"ratio={latency_ratio:.12g} > {MAX_P95_LATENCY_RATIO}"
        )

    expected_files = referenced_paths | {"scaling_evidence.json"}
    if "validation_report.json" in bundle_files:
        expected_files.add("validation_report.json")
    unexpected_files = sorted(bundle_files - expected_files)
    missing_files = sorted(expected_files - bundle_files)
    if unexpected_files or missing_files:
        _fail(
            "evidence bundle file inventory differs from the canonical manifest; "
            f"missing={missing_files}, unexpected={unexpected_files}"
        )
    expected_directories = _expected_bundle_directories(expected_files)
    unexpected_directories = sorted(bundle_directories - expected_directories)
    missing_directories = sorted(expected_directories - bundle_directories)
    if unexpected_directories or missing_directories:
        _fail(
            "evidence bundle directory inventory differs from canonical paths; "
            f"missing={missing_directories}, unexpected={unexpected_directories}"
        )

    return {
        "pair_count": EXPECTED_PAIR_COUNT,
        "run_count": expected_run_count,
        "one_lane_median_committed_throughput_tps": one_median,
        "four_lane_median_committed_throughput_tps": four_median,
        "four_to_one_median_throughput_ratio": throughput_ratio,
        "one_lane_pooled_p95_commit_latency_ms": one_p95,
        "four_lane_pooled_p95_commit_latency_ms": four_p95,
        "four_to_one_p95_latency_ratio": latency_ratio,
        "one_lane_resource_maxima": one_resource_maxima,
        "four_lane_resource_maxima": four_resource_maxima,
        "minimum_throughput_ratio": MIN_THROUGHPUT_RATIO,
        "maximum_p95_latency_ratio": MAX_P95_LATENCY_RATIO,
        "pairs": pairs,
    }


def _write_report(path: Path, report: dict[str, Any]) -> None:
    """Durably publish deterministic report bytes without replacing a path."""

    final_name = path.name
    if not final_name or final_name in {".", ".."} or "\x00" in final_name:
        raise OSError(errno.EINVAL, "invalid validation report name", str(path))
    data = (json.dumps(report, indent=2, sort_keys=True) + "\n").encode("utf-8")
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        directory_flags |= os.O_NOFOLLOW
    directory_fd = os.open(path.parent, directory_flags)
    temporary_name = f".gscale-report-{secrets.token_hex(16)}"
    temporary_fd: int | None = None
    owned_inode: tuple[int, int] | None = None
    published = False

    def unlink_owned(name: str) -> bool:
        if owned_inode is None:
            return False
        try:
            metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        except FileNotFoundError:
            return False
        if (
            not stat.S_ISREG(metadata.st_mode)
            or (metadata.st_dev, metadata.st_ino) != owned_inode
        ):
            return False
        os.unlink(name, dir_fd=directory_fd)
        return True

    try:
        try:
            os.stat(final_name, dir_fd=directory_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise FileExistsError(
                errno.EEXIST,
                "validation report destination already exists",
                str(path),
            )

        temporary_flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            temporary_flags |= os.O_NOFOLLOW
        temporary_fd = os.open(
            temporary_name,
            temporary_flags,
            0o600,
            dir_fd=directory_fd,
        )
        opened = os.fstat(temporary_fd)
        if stat.S_ISREG(opened.st_mode):
            owned_inode = (opened.st_dev, opened.st_ino)
        if owned_inode is None or opened.st_nlink != 1:
            raise OSError(errno.EIO, "validation report stage is not private")
        os.fchmod(temporary_fd, 0o600)

        remaining = memoryview(data)
        while remaining:
            written = os.write(temporary_fd, remaining)
            if written <= 0:
                raise OSError(errno.EIO, "short write while publishing validation report")
            remaining = remaining[written:]
        os.fsync(temporary_fd)
        completed = os.fstat(temporary_fd)
        if (
            not stat.S_ISREG(completed.st_mode)
            or (completed.st_dev, completed.st_ino) != owned_inode
            or completed.st_nlink != 1
            or completed.st_size != len(data)
            or stat.S_IMODE(completed.st_mode) != 0o600
        ):
            raise OSError(
                errno.EIO,
                "validation report stage changed while it was written",
            )
        os.close(temporary_fd)
        temporary_fd = None

        os.link(
            temporary_name,
            final_name,
            src_dir_fd=directory_fd,
            dst_dir_fd=directory_fd,
            follow_symlinks=False,
        )
        published = True
        os.fsync(directory_fd)
        if not unlink_owned(temporary_name):
            raise OSError(errno.EIO, "validation report stage changed before cleanup")
        os.fsync(directory_fd)

        final = os.stat(final_name, dir_fd=directory_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(final.st_mode)
            or (final.st_dev, final.st_ino) != owned_inode
            or final.st_nlink != 1
            or final.st_size != len(data)
            or stat.S_IMODE(final.st_mode) != 0o600
        ):
            raise OSError(
                errno.EIO,
                "validation report changed during atomic publication",
            )
    except BaseException:
        if temporary_fd is not None:
            try:
                os.close(temporary_fd)
            except OSError:
                pass
        cleaned = False
        for name in (temporary_name, final_name if published else ""):
            if not name:
                continue
            try:
                cleaned = unlink_owned(name) or cleaned
            except OSError:
                pass
        if cleaned:
            try:
                os.fsync(directory_fd)
            except OSError:
                pass
        raise
    finally:
        os.close(directory_fd)


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path, help="Path to scaling_evidence.json.")
    parser.add_argument(
        "--report",
        type=Path,
        help="Write a machine-readable pass/fail validation report.",
    )
    parser.add_argument(
        "--expected-source-revision",
        help="Require the pinned identity to name this exact release revision.",
    )
    parser.add_argument(
        "--expected-workspace-source-sha256",
        help="Require the pinned identity to name this exact sealed workspace digest.",
    )
    parser.add_argument(
        "--expected-validator-sha256",
        help="Require the archived validator to match this exact retained digest.",
    )
    parser.add_argument(
        "--expected-trial-harness-sha256",
        help="Require the archived trial harness to match this approved digest.",
    )
    parser.add_argument(
        "--expected-configuration-sha256",
        help="Require the archived Nexus configuration to match this approved digest.",
    )
    parser.add_argument(
        "--expected-irohad-sha256",
        help="Require the pinned identity to name this measured irohad digest.",
    )
    parser.add_argument(
        "--expected-iroha-cli-sha256",
        help="Require the pinned identity to name this measured iroha CLI digest.",
    )
    parser.add_argument(
        "--expected-repository-root",
        type=Path,
        help="Require archived workload tools to match this retained source root.",
    )
    parser.add_argument("--quiet", action="store_true", help="Suppress the human summary.")
    args = parser.parse_args(argv)
    if (
        args.expected_source_revision is not None
        and _REVISION_RE.fullmatch(args.expected_source_revision) is None
    ):
        parser.error("--expected-source-revision must be lowercase 40- or 64-hex")
    for name in (
        "expected_workspace_source_sha256",
        "expected_validator_sha256",
        "expected_trial_harness_sha256",
        "expected_configuration_sha256",
        "expected_irohad_sha256",
        "expected_iroha_cli_sha256",
    ):
        value = getattr(args, name)
        if value is not None and _DIGEST_RE.fullmatch(value) is None:
            parser.error(f"--{name.replace('_', '-')} must be lowercase SHA-256")
    return args


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    manifest_path = args.manifest
    try:
        metrics = validate_evidence(
            manifest_path,
            expected_source_revision=args.expected_source_revision,
            expected_workspace_source_sha256=args.expected_workspace_source_sha256,
            expected_validator_sha256=args.expected_validator_sha256,
            expected_trial_harness_sha256=args.expected_trial_harness_sha256,
            expected_configuration_sha256=args.expected_configuration_sha256,
            expected_irohad_sha256=args.expected_irohad_sha256,
            expected_iroha_cli_sha256=args.expected_iroha_cli_sha256,
            expected_repository_root=args.expected_repository_root,
        )
    except (EvidenceError, OSError) as error:
        report = {
            "schema": REPORT_SCHEMA,
            "result": "fail",
            "manifest_sha256": (
                sha256_file(manifest_path)
                if manifest_path.is_file() and not manifest_path.is_symlink()
                else None
            ),
            "errors": [str(error)],
            "metrics": None,
        }
        if args.report is not None:
            _write_report(args.report, report)
        if not args.quiet:
            print(f"[g-scale] FAIL: {error}", file=sys.stderr)
        return 1

    report = {
        "schema": REPORT_SCHEMA,
        "result": "pass",
        "manifest_sha256": sha256_file(manifest_path),
        "errors": [],
        "metrics": metrics,
    }
    if args.report is not None:
        _write_report(args.report, report)
    if not args.quiet:
        print(
            "[g-scale] PASS: "
            f"throughput={metrics['four_to_one_median_throughput_ratio']:.3f}x "
            f"(required >= {MIN_THROUGHPUT_RATIO:.2f}x), "
            f"p95-latency={metrics['four_to_one_p95_latency_ratio']:.3f}x "
            f"(required <= {MAX_P95_LATENCY_RATIO:.2f}x)"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
