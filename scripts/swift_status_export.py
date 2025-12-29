#!/usr/bin/env python3
"""Render Swift parity/CI dashboard feeds or manage telemetry overrides."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from contextlib import contextmanager
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from types import ModuleType
from typing import Iterable, Iterator, List, Optional, Sequence, Tuple


def _parse_iso8601(value: Optional[str]) -> Optional[datetime]:
    if not value:
        return None
    try:
        if value.endswith("Z"):
            value = value[:-1] + "+00:00"
        return datetime.fromisoformat(value).astimezone(timezone.utc)
    except ValueError as exc:  # pragma: no cover - defensive guard
        raise RuntimeError(f"invalid ISO-8601 timestamp: {value}") from exc


def _isoformat(dt: Optional[datetime]) -> Optional[str]:
    if dt is None:
        return None
    return dt.astimezone(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


@dataclass(frozen=True)
class FixtureDetail:
    instruction: str
    age_hours: float
    owner: str


@dataclass(frozen=True)
class PipelineTestSummary:
    name: str
    duration_seconds: float


@dataclass(frozen=True)
class PipelineMetadataSummary:
    job_name: Optional[str]
    duration_seconds: Optional[float]
    tests: List[PipelineTestSummary]


@dataclass(frozen=True)
class TelemetrySnapshot:
    salt_epoch: Optional[str]
    salt_rotation_age_hours: Optional[float]
    overrides_open: Optional[int]
    device_profile_alignment: Optional[str]
    schema_version: Optional[str]
    schema_policy_violation_count: Optional[int]
    schema_policy_violations: List[str]
    notes: List[str]
    connect_metrics_present: Optional[bool]


@dataclass(frozen=True)
class ParitySnapshot:
    generated_at: datetime
    outstanding_diffs: int
    oldest_diff_hours: float
    details: List[FixtureDetail]
    pipeline_status: str
    pipeline_last_run: Optional[datetime]
    pipeline_failed_tests: List[str]
    pipeline_metadata: Optional[PipelineMetadataSummary]
    pipeline_metadata_source: Optional[str]
    regen_last_success: Optional[datetime]
    regen_hours_since_success: float
    regen_breach: bool
    alerts: List[str]
    acceleration: List[str]
    telemetry: Optional[TelemetrySnapshot]


@dataclass(frozen=True)
class LaneSummary:
    name: str
    success_rate: float
    last_failure: Optional[datetime]
    flake_count: int
    mttr_hours: float
    device_tag: Optional[str]


@dataclass(frozen=True)
class CISnapshot:
    generated_at: datetime
    lanes: List[LaneSummary]
    queue_depth: Optional[int]
    average_runtime_minutes: Optional[float]
    emulator_passes: Optional[int]
    emulator_failures: Optional[int]
    strongbox_passes: Optional[int]
    strongbox_failures: Optional[int]
    consecutive_failures: Optional[int]
    open_incidents: List[str]
    acceleration_notes: List[str]


@dataclass(frozen=True)
class ReadinessDoc:
    title: str
    path: str
    summary: Optional[str]
    sha256: str
    last_modified: Optional[str]
    last_commit: Optional[str]
    size_bytes: int


@dataclass(frozen=True)
class HealthThresholds:
    parity_diff_warn: int = 1
    parity_diff_critical: int = 3
    parity_regen_warn_hours: float = 24.0
    ci_queue_warn: int = 4
    ci_queue_critical: int = 10
    ci_failure_warn: int = 1
    ci_failure_critical: int = 3
    ci_lane_warn: float = 0.95
    ci_lane_critical: float = 0.85


@dataclass(frozen=True)
class ComponentHealth:
    status: str
    reasons: List[str]

    def to_dict(self) -> dict:
        return {"status": self.status, "reasons": self.reasons}


@dataclass
class _HealthAccumulator:
    status: str = "ok"
    reasons: List[str] = field(default_factory=list)

    def escalate(self, level: str, reason: str) -> None:
        if reason:
            self.reasons.append(reason)
        if _SEVERITY_ORDER[level] > _SEVERITY_ORDER[self.status]:
            self.status = level


DEFAULT_HEALTH_THRESHOLDS = HealthThresholds()
_SEVERITY_ORDER = {"ok": 0, "warn": 1, "critical": 2}
DEFAULT_READINESS_DOCS = [
    Path("docs/source/sdk/swift/reproducibility_checklist.md"),
    Path("docs/source/sdk/swift/support_playbook.md"),
]


def _load_json(path: Path) -> dict:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise RuntimeError(f"unable to read {path}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid JSON in {path}: {exc}") from exc
    return payload


def _format_relative_path(path: Path, repo_root: Path) -> str:
    try:
        return str(path.relative_to(repo_root))
    except ValueError:
        return str(path)


def _git_doc_metadata(repo_root: Path, path: Path) -> Tuple[Optional[str], Optional[str]]:
    try:
        rel = path.relative_to(repo_root)
    except ValueError:
        rel = path
    try:
        result = subprocess.run(
            [
                "git",
                "-C",
                str(repo_root),
                "log",
                "-1",
                "--format=%H,%cI",
                "--",
                str(rel),
            ],
            capture_output=True,
            check=True,
            text=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError):  # pragma: no cover - best effort
        return None, None
    output = result.stdout.strip()
    if not output:
        return None, None
    commit, *rest = output.split(",", 1)
    timestamp = rest[0] if rest else None
    return commit or None, timestamp or None


def _skip_leading_comments(lines: Sequence[str], idx: int) -> int:
    while idx < len(lines):
        stripped = lines[idx].strip()
        if not stripped:
            idx += 1
            continue
        if stripped.startswith("<!--"):
            if stripped.endswith("-->"):
                idx += 1
                continue
            idx += 1
            while idx < len(lines) and "-->" not in lines[idx]:
                idx += 1
            if idx < len(lines):
                idx += 1
            continue
        break
    return idx


def _parse_front_matter_block(lines: Sequence[str]) -> Tuple[dict, int]:
    idx = _skip_leading_comments(lines, 0)
    if idx >= len(lines) or lines[idx].strip() != "---":
        return {}, idx
    idx += 1
    data: dict = {}
    while idx < len(lines) and lines[idx].strip() != "---":
        stripped = lines[idx].strip()
        if stripped and not stripped.startswith("#") and ":" in stripped:
            key, value = stripped.split(":", 1)
            data[key.strip()] = value.strip()
        idx += 1
    if idx < len(lines) and lines[idx].strip() == "---":
        idx += 1
    return data, idx


def _extract_first_heading(lines: Sequence[str], start: int) -> Optional[str]:
    idx = start
    while idx < len(lines):
        stripped = lines[idx].strip()
        idx += 1
        if not stripped:
            continue
        if stripped.startswith("<!--"):
            if stripped.endswith("-->"):
                continue
            while idx < len(lines) and "-->" not in lines[idx]:
                idx += 1
            if idx < len(lines):
                idx += 1
            continue
        if stripped.startswith("#"):
            return stripped.lstrip("#").strip()
    return None


def _normalize_doc_path(path: Path, repo_root: Path) -> Path:
    if not path.is_absolute():
        return (repo_root / path).resolve()
    return path.resolve()


def _load_readiness_doc(path: Path, repo_root: Path) -> ReadinessDoc:
    if not path.is_file():
        raise RuntimeError(f"readiness doc not found: {path}")
    raw_bytes = path.read_bytes()
    text = raw_bytes.decode("utf-8")
    lines = text.splitlines()
    front_matter, body_idx = _parse_front_matter_block(lines)
    title = front_matter.get("title") or _extract_first_heading(lines, body_idx) or path.name
    summary = front_matter.get("summary")
    sha256_hex = hashlib.sha256(raw_bytes).hexdigest()
    commit, timestamp = _git_doc_metadata(repo_root, path)
    return ReadinessDoc(
        title=title,
        path=_format_relative_path(path, repo_root),
        summary=summary,
        sha256=sha256_hex,
        last_modified=timestamp,
        last_commit=commit,
        size_bytes=len(raw_bytes),
    )


def collect_readiness_docs(paths: Sequence[Path], repo_root: Path) -> List[ReadinessDoc]:
    docs: List[ReadinessDoc] = []
    seen: set[Path] = set()
    for raw in paths:
        resolved = _normalize_doc_path(Path(raw), repo_root)
        if resolved in seen:
            continue
        seen.add(resolved)
        docs.append(_load_readiness_doc(resolved, repo_root))
    return docs


def _parse_parity_snapshot(data: dict) -> ParitySnapshot:
    fixtures = data.get("fixtures", {})
    details = [
        FixtureDetail(
            instruction=item.get("instruction", "unknown"),
            age_hours=float(item.get("age_hours", 0.0)),
            owner=item.get("owner", "unassigned"),
        )
        for item in fixtures.get("details", [])
    ]
    regen = data.get("regen_sla", {})
    pipeline = data.get("pipeline", {})
    metadata_block = pipeline.get("metadata")
    metadata_summary: Optional[PipelineMetadataSummary] = None
    if isinstance(metadata_block, dict):
        job_raw = metadata_block.get("job_name")
        duration_raw = metadata_block.get("duration_seconds")
        tests_summary: List[PipelineTestSummary] = []
        for entry in metadata_block.get("tests") or []:
            if not isinstance(entry, dict):
                continue
            name = str(entry.get("name", "unknown"))
            duration_value = entry.get("duration_seconds")
            duration = float(duration_value) if duration_value is not None else 0.0
            tests_summary.append(PipelineTestSummary(name=name, duration_seconds=duration))
        metadata_summary = PipelineMetadataSummary(
            job_name=str(job_raw) if job_raw is not None else None,
            duration_seconds=float(duration_raw) if duration_raw is not None else None,
            tests=tests_summary,
        )
    metadata_source_raw = pipeline.get("metadata_source")
    metadata_source = (
        str(metadata_source_raw) if metadata_source_raw is not None else None
    )
    alerts_raw = data.get("alerts", [])
    alerts = []
    for entry in alerts_raw:
        if isinstance(entry, dict):
            message = entry.get("message")
            severity = entry.get("severity")
            if message:
                if severity:
                    alerts.append(f"[{severity.upper()}] {message}")
                else:
                    alerts.append(message)
        elif isinstance(entry, str):
            alerts.append(entry)
    acceleration = []
    accel = data.get("acceleration") or {}
    for label in ("metal", "neon", "strongbox"):
        entry = accel.get(label)
        if not isinstance(entry, dict):
            continue
        enabled = entry.get("enabled")
        parity = entry.get("parity")
        delta = entry.get("perf_delta_pct")
        fragment = f"{label}: enabled={enabled}"
        if parity is not None:
            fragment += f", parity={parity}"
        if delta is not None:
            fragment += f", perf_delta={delta:+.1f}%"
        acceleration.append(fragment)

    telemetry_block = data.get("telemetry")
    telemetry = None
    if isinstance(telemetry_block, dict):
        notes_raw = telemetry_block.get("notes") or []
        notes = [str(item) for item in notes_raw if isinstance(item, (str, int, float))]
        connect_metrics_present = telemetry_block.get("connect_metrics_present")
        if connect_metrics_present is not None:
            connect_metrics_present = bool(connect_metrics_present)
        telemetry = TelemetrySnapshot(
            salt_epoch=str(telemetry_block.get("salt_epoch")) if telemetry_block.get("salt_epoch") is not None else None,
            salt_rotation_age_hours=float(telemetry_block.get("salt_rotation_age_hours", 0.0))
            if telemetry_block.get("salt_rotation_age_hours") is not None
            else None,
            overrides_open=int(telemetry_block.get("overrides_open"))
            if telemetry_block.get("overrides_open") is not None
            else None,
            device_profile_alignment=telemetry_block.get("device_profile_alignment"),
            schema_version=telemetry_block.get("schema_version"),
            schema_policy_violation_count=int(telemetry_block.get("schema_policy_violation_count"))
            if telemetry_block.get("schema_policy_violation_count") is not None
            else None,
            schema_policy_violations=[
                str(entry) for entry in telemetry_block.get("schema_policy_violations", []) or []
            ],
            notes=notes,
            connect_metrics_present=connect_metrics_present,
        )

    return ParitySnapshot(
        generated_at=_parse_iso8601(data.get("generated_at")) or datetime.now(timezone.utc),
        outstanding_diffs=int(fixtures.get("outstanding_diffs", 0)),
        oldest_diff_hours=float(fixtures.get("oldest_diff_hours", 0.0)),
        details=details,
        pipeline_status=pipeline.get("status", "unknown"),
        pipeline_last_run=_parse_iso8601(pipeline.get("last_run")),
        pipeline_failed_tests=[str(item) for item in pipeline.get("failed_tests", [])],
        pipeline_metadata=metadata_summary,
        pipeline_metadata_source=metadata_source,
        regen_last_success=_parse_iso8601(regen.get("last_success")),
        regen_hours_since_success=float(regen.get("hours_since_success", 0.0)),
        regen_breach=bool(regen.get("breach", False)),
        alerts=alerts,
        acceleration=acceleration,
        telemetry=telemetry,
    )


def load_parity_snapshot(path: Path) -> ParitySnapshot:
    return _parse_parity_snapshot(_load_json(path))


def _parse_ci_snapshot(data: dict) -> CISnapshot:
    buildkite = data.get("buildkite", {})
    lanes = []
    for lane in buildkite.get("lanes", []):
        lanes.append(
            LaneSummary(
                name=str(lane.get("name", "unknown")),
                success_rate=float(lane.get("success_rate_14_runs", 0.0)),
                last_failure=_parse_iso8601(lane.get("last_failure")),
                flake_count=int(lane.get("flake_count", 0)),
                mttr_hours=float(lane.get("mttr_hours", 0.0)),
                device_tag=lane.get("device_tag"),
            )
        )

    devices = data.get("devices", {})
    emulators = devices.get("emulators", {})
    strongbox = devices.get("strongbox_capable", {})
    alert_state = data.get("alert_state", {})
    acceleration = []
    accel = data.get("acceleration_bench") or {}
    merkle = accel.get("metal_vs_cpu_merkle_ms")
    if isinstance(merkle, dict):
        cpu = merkle.get("cpu")
        metal = merkle.get("metal")
        if cpu is not None and metal is not None and cpu != 0:
            delta_pct = (metal - cpu) / cpu * 100.0
            acceleration.append(
                f"metal_vs_cpu_merkle_ms: cpu={cpu:.1f} metal={metal:.1f} (delta={delta_pct:+.1f}%)"
            )
    neon_crc = accel.get("neon_crc64_throughput_mb_s")
    if neon_crc is not None:
        acceleration.append(f"neon_crc64_throughput_mb_s={float(neon_crc):.1f}")

    return CISnapshot(
        generated_at=_parse_iso8601(data.get("generated_at")) or datetime.now(timezone.utc),
        lanes=lanes,
        queue_depth=buildkite.get("queue_depth"),
        average_runtime_minutes=buildkite.get("average_runtime_minutes"),
        emulator_passes=emulators.get("passes"),
        emulator_failures=emulators.get("failures"),
        strongbox_passes=strongbox.get("passes"),
        strongbox_failures=strongbox.get("failures"),
        consecutive_failures=alert_state.get("consecutive_failures"),
        open_incidents=[str(item) for item in alert_state.get("open_incidents", [])],
        acceleration_notes=acceleration,
    )


def load_ci_snapshot(path: Path) -> CISnapshot:
    return _parse_ci_snapshot(_load_json(path))


def evaluate_health(
    parity: ParitySnapshot,
    ci: CISnapshot,
    *,
    thresholds: HealthThresholds = DEFAULT_HEALTH_THRESHOLDS,
) -> tuple[ComponentHealth, ComponentHealth]:
    parity_acc = _HealthAccumulator()
    if parity.outstanding_diffs >= thresholds.parity_diff_critical:
        parity_acc.escalate("critical", f"{parity.outstanding_diffs} outstanding diffs")
    elif parity.outstanding_diffs >= thresholds.parity_diff_warn:
        parity_acc.escalate("warn", f"{parity.outstanding_diffs} outstanding diffs")

    if parity.regen_breach:
        parity_acc.escalate("critical", "regen SLA breached")
    elif parity.regen_hours_since_success >= thresholds.parity_regen_warn_hours:
        parity_acc.escalate(
            "warn", f"regen idle for {parity.regen_hours_since_success:.1f}h"
        )

    if parity.pipeline_failed_tests:
        parity_acc.escalate(
            "warn", f"{len(parity.pipeline_failed_tests)} pipeline tests failing"
        )

    pipeline_status = parity.pipeline_status.lower()
    if pipeline_status not in {"green", "ok", "passing"}:
        level = "warn"
        if any(token in pipeline_status for token in ("red", "fail", "error", "crit")):
            level = "critical"
        parity_acc.escalate(level, f"pipeline status={parity.pipeline_status}")

    if parity.telemetry and parity.telemetry.overrides_open:
        parity_acc.escalate(
            "warn", f"{parity.telemetry.overrides_open} telemetry overrides open"
        )
    if parity.telemetry is None:
        parity_acc.escalate("warn", "telemetry snapshot missing (exporter dark)")
    elif parity.telemetry.connect_metrics_present is not True:
        parity_acc.escalate("warn", "Connect telemetry dark or not reported")

    ci_acc = _HealthAccumulator()
    if ci.queue_depth is not None:
        if ci.queue_depth >= thresholds.ci_queue_critical:
            ci_acc.escalate("critical", f"queue depth {ci.queue_depth}")
        elif ci.queue_depth >= thresholds.ci_queue_warn:
            ci_acc.escalate("warn", f"queue depth {ci.queue_depth}")

    if ci.consecutive_failures is not None:
        if ci.consecutive_failures >= thresholds.ci_failure_critical:
            ci_acc.escalate("critical", f"{ci.consecutive_failures} consecutive failures")
        elif ci.consecutive_failures >= thresholds.ci_failure_warn:
            ci_acc.escalate("warn", f"{ci.consecutive_failures} consecutive failures")

    if ci.open_incidents:
        ci_acc.escalate("warn", f"{len(ci.open_incidents)} incident(s) open")

    for lane in ci.lanes:
        rate = lane.success_rate
        if rate <= thresholds.ci_lane_critical:
            ci_acc.escalate(
                "critical", f"{lane.name} success={rate * 100:.0f}%"
            )
        elif rate <= thresholds.ci_lane_warn:
            ci_acc.escalate("warn", f"{lane.name} success={rate * 100:.0f}%")

    if ci.strongbox_failures:
        ci_acc.escalate("warn", f"{ci.strongbox_failures} strongbox failures")
    if ci.emulator_failures:
        ci_acc.escalate("warn", f"{ci.emulator_failures} emulator failures")

    parity_health = ComponentHealth(parity_acc.status, parity_acc.reasons)
    ci_health = ComponentHealth(ci_acc.status, ci_acc.reasons)
    return parity_health, ci_health


def build_health_report(
    parity: ParitySnapshot,
    ci: CISnapshot,
    *,
    thresholds: HealthThresholds = DEFAULT_HEALTH_THRESHOLDS,
) -> dict:
    parity_health, ci_health = evaluate_health(parity, ci, thresholds=thresholds)
    latest = max(parity.generated_at, ci.generated_at)
    parity_rank = _SEVERITY_ORDER[parity_health.status]
    ci_rank = _SEVERITY_ORDER[ci_health.status]
    if parity_rank > ci_rank:
        overall_status = parity_health.status
        overall_reasons = parity_health.reasons
    elif ci_rank > parity_rank:
        overall_status = ci_health.status
        overall_reasons = ci_health.reasons
    else:
        overall_status = parity_health.status
        overall_reasons = parity_health.reasons + ci_health.reasons
    return {
        "generated_at": _isoformat(latest),
        "parity": parity_health.to_dict(),
        "ci": ci_health.to_dict(),
        "overall": {
            "status": overall_status,
            "reasons": overall_reasons,
        },
    }


_OVERRIDE_MODULE: Optional[ModuleType] = None


def _load_override_module() -> ModuleType:
    """Load the telemetry override helper module on demand."""
    global _OVERRIDE_MODULE
    if _OVERRIDE_MODULE is not None:
        return _OVERRIDE_MODULE
    try:
        import swift_telemetry_override as override_mod  # type: ignore[import-not-found]
    except ModuleNotFoundError:
        module_path = Path(__file__).resolve().parent / "swift_telemetry_override.py"
        spec = importlib.util.spec_from_file_location("swift_telemetry_override", module_path)
        if spec is None or spec.loader is None:
            raise RuntimeError(f"unable to load telemetry override helper: {module_path}")
        override_mod = importlib.util.module_from_spec(spec)
        sys.modules.setdefault(spec.name, override_mod)
        spec.loader.exec_module(override_mod)  # type: ignore[attr-defined]
    _OVERRIDE_MODULE = override_mod
    return override_mod


def _run_telemetry_override_command(argv: Sequence[str]) -> int:
    """Delegate telemetry override commands to the helper module."""
    override_mod = _load_override_module()
    args = list(argv)
    if not args:
        return override_mod.main(args)

    # Allow callers to pass the global --store flag either before or after the subcommand.
    store_args: List[str] = []
    filtered: List[str] = []
    idx = 0
    while idx < len(args):
        token = args[idx]
        if token == "--store":
            if idx + 1 >= len(args):
                raise SystemExit("--store requires a path argument")
            store_args.extend([token, args[idx + 1]])
            idx += 2
            continue
        filtered.append(token)
        idx += 1

    return override_mod.main(store_args + filtered)


@dataclass
class MetricsState:
    parity_success_total: int = 0
    parity_failure_total: int = 0
    updated_at: Optional[str] = None


def _parity_success(parity: ParitySnapshot) -> bool:
    """Return True when parity is considered successful for telemetry purposes."""
    return (
        parity.outstanding_diffs == 0
        and not parity.regen_breach
        and not parity.pipeline_failed_tests
    )


def _load_metrics_state(path: Path) -> MetricsState:
    if not path.exists():
        return MetricsState()
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - surfaced to caller
        raise RuntimeError(f"invalid metrics state JSON in {path}: {exc}") from exc
    return MetricsState(
        parity_success_total=int(data.get("parity_success_total", 0)),
        parity_failure_total=int(data.get("parity_failure_total", 0)),
        updated_at=data.get("updated_at"),
    )


def _store_metrics_state(path: Path, state: MetricsState) -> None:
    payload = {
        "parity_success_total": state.parity_success_total,
        "parity_failure_total": state.parity_failure_total,
        "updated_at": state.updated_at,
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def _write_metrics(
    metrics_path: Path,
    parity: ParitySnapshot,
    state: MetricsState,
    success: bool,
) -> None:
    """Emit Prometheus textfile metrics for the latest parity snapshot."""
    lines = [
        "# HELP swift_parity_success_total Count of Swift parity digests with zero outstanding diffs and no SLA breach.",
        "# TYPE swift_parity_success_total counter",
        f"swift_parity_success_total {state.parity_success_total}",
        "# HELP swift_parity_failure_total Count of Swift parity digests where diffs remain or the SLA is breached.",
        "# TYPE swift_parity_failure_total counter",
        f"swift_parity_failure_total {state.parity_failure_total}",
        "# HELP swift_parity_status Latest Swift parity status (1=success, 0=failure).",
        "# TYPE swift_parity_status gauge",
        f"swift_parity_status {1 if success else 0}",
        "# HELP swift_parity_outstanding_diffs Outstanding Swift Norito parity diffs in the latest snapshot.",
        "# TYPE swift_parity_outstanding_diffs gauge",
        f"swift_parity_outstanding_diffs {parity.outstanding_diffs}",
        "# HELP swift_parity_oldest_diff_hours Oldest Swift parity diff age in hours.",
        "# TYPE swift_parity_oldest_diff_hours gauge",
        f"swift_parity_oldest_diff_hours {parity.oldest_diff_hours:.6f}",
        "# HELP swift_parity_regen_hours_since_success Hours since the last successful Swift fixture regeneration.",
        "# TYPE swift_parity_regen_hours_since_success gauge",
        f"swift_parity_regen_hours_since_success {parity.regen_hours_since_success:.6f}",
    ]
    telemetry = parity.telemetry
    connect_metrics_flag: Optional[int] = None
    lines.extend(
        [
            "# HELP swift_telemetry_snapshot_present Whether a telemetry block was present in the latest snapshot (1=yes, 0=no).",
            "# TYPE swift_telemetry_snapshot_present gauge",
            f"swift_telemetry_snapshot_present {1 if telemetry else 0}",
        ]
    )
    if telemetry:
        if telemetry.overrides_open is not None:
            lines.extend(
                [
                    "# HELP swift_telemetry_overrides_open Active Swift telemetry redaction overrides in the latest snapshot.",
                    "# TYPE swift_telemetry_overrides_open gauge",
                    f"swift_telemetry_overrides_open {telemetry.overrides_open}",
                ]
            )
        if telemetry.salt_rotation_age_hours is not None:
            lines.extend(
                [
                    "# HELP swift_telemetry_salt_rotation_age_hours Hours since the current Swift telemetry salt rotated.",
                    "# TYPE swift_telemetry_salt_rotation_age_hours gauge",
                    f"swift_telemetry_salt_rotation_age_hours {telemetry.salt_rotation_age_hours:.6f}",
                ]
            )
        connect_metrics = telemetry.connect_metrics_present
        if connect_metrics is not None:
            connect_metrics_flag = 1 if connect_metrics else 0
    if connect_metrics_flag is None:
        connect_metrics_flag = 0
    lines.extend(
        [
            "# HELP swift_telemetry_connect_metrics_present Whether Connect telemetry counters were reported in the latest snapshot (1=reported, 0=dark).",
            "# TYPE swift_telemetry_connect_metrics_present gauge",
            f"swift_telemetry_connect_metrics_present {connect_metrics_flag}",
        ]
    )
    metrics_path.parent.mkdir(parents=True, exist_ok=True)
    metrics_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _read_remote(url: str, *, retries: int = 3, backoff: float = 2.0) -> str:
    attempts = max(1, retries)
    delay = max(0.0, backoff)
    last_exc: Optional[Exception] = None
    for attempt in range(1, attempts + 1):
        try:
            with urllib.request.urlopen(url) as response:  # nosec: B310 - trusted tooling
                charset = response.headers.get_content_charset() or "utf-8"
                data = response.read()
                return data.decode(charset)
        except Exception as exc:  # pragma: no cover - network errors surfaced to caller
            last_exc = exc
            if attempt == attempts:
                break
            if delay:
                time.sleep(delay)
                delay *= 2
    raise RuntimeError(f"failed to download feed from {url}: {last_exc}") from last_exc


@contextmanager
def _temp_feed(contents: str) -> Iterator[Path]:
    from tempfile import NamedTemporaryFile

    with NamedTemporaryFile("w", encoding="utf-8", suffix=".json", delete=False) as tmp:
        tmp.write(contents)
        tmp_path = Path(tmp.name)
    try:
        yield tmp_path
    finally:
        try:
            tmp_path.unlink()
        except FileNotFoundError:  # pragma: no cover - already removed
            pass


def _format_lane(lane: LaneSummary) -> str:
    success_pct = lane.success_rate * 100.0
    last_failure = _isoformat(lane.last_failure) or "n/a"
    device = f" ({lane.device_tag})" if lane.device_tag else ""
    return (
        f"- {lane.name}{device}: success={success_pct:.0f}% "
        f"| last_failure={last_failure} | flakes={lane.flake_count} | mttr={lane.mttr_hours:.1f}h"
    )


def _find_lane(ci: CISnapshot, lane_name: str) -> Optional[LaneSummary]:
    for lane in ci.lanes:
        if lane.name == lane_name:
            return lane
    return None


def render_markdown(
    parity: ParitySnapshot,
    ci: CISnapshot,
    *,
    heading_level: int = 2,
    metrics: Optional[MetricsState] = None,
    signal_lane_names: Optional[List[str]] = None,
    thresholds: HealthThresholds = DEFAULT_HEALTH_THRESHOLDS,
    health: Optional[dict] = None,
    readiness_docs: Optional[List[ReadinessDoc]] = None,
) -> str:
    signal_lane_names = [lane for lane in (signal_lane_names or []) if lane]
    prefix = "#" * max(1, heading_level)
    subprefix = "#" * max(1, heading_level + 1)
    health_report = health or build_health_report(parity, ci, thresholds=thresholds)
    parity_health = health_report["parity"]
    ci_health = health_report["ci"]

    lines = [
        f"{prefix} Swift Norito Parity Snapshot ({_isoformat(parity.generated_at)})",
        "",
        f"{subprefix} Health Summary",
        "",
        f"- Parity: {parity_health['status'].upper()} "
        f"({'; '.join(parity_health['reasons']) if parity_health['reasons'] else 'all clear'})",
        f"- CI: {ci_health['status'].upper()} "
        f"({'; '.join(ci_health['reasons']) if ci_health['reasons'] else 'all clear'})",
        "",
        f"- Outstanding diffs: {parity.outstanding_diffs} (oldest {parity.oldest_diff_hours:.1f}h)",
        f"- Regen SLA: last success {_isoformat(parity.regen_last_success) or 'n/a'}; "
        f"{parity.regen_hours_since_success:.1f}h since success; breach={'yes' if parity.regen_breach else 'no'}",
        f"- Pipeline: status={parity.pipeline_status}; "
        f"last_run={_isoformat(parity.pipeline_last_run) or 'n/a'}; "
        f"failed_tests={', '.join(parity.pipeline_failed_tests) if parity.pipeline_failed_tests else 'none'}",
    ]
    if parity.pipeline_metadata:
        meta = parity.pipeline_metadata
        job = meta.job_name or "n/a"
        duration = (
            f"{meta.duration_seconds:.1f}s"
            if meta.duration_seconds is not None
            else "n/a"
        )
        lines.append(f"  Pipeline metadata: job={job}; duration={duration}")
        if meta.tests:
            lines.append("    Tests:")
            for test in meta.tests:
                lines.append(f"      - {test.name}: {test.duration_seconds:.1f}s")
    if parity.pipeline_metadata_source:
        lines.append(f"  Buildkite metadata artifact: {parity.pipeline_metadata_source}")

    if parity.acceleration:
        lines.append(f"- Acceleration: {'; '.join(parity.acceleration)}")
    if parity.telemetry:
        telemetry = parity.telemetry
        salt_epoch = telemetry.salt_epoch or "n/a"
        rotation = (
            f"{telemetry.salt_rotation_age_hours:.1f}h"
            if telemetry.salt_rotation_age_hours is not None
            else "n/a"
        )
        overrides = telemetry.overrides_open if telemetry.overrides_open is not None else "n/a"
        alignment = telemetry.device_profile_alignment or "n/a"
        schema = telemetry.schema_version or "n/a"
        violations = telemetry.schema_policy_violation_count
        if violations is None:
            violations = len(telemetry.schema_policy_violations)
        connect_metrics = telemetry.connect_metrics_present
        if connect_metrics is True:
            connect_metrics_status = "ok"
        elif connect_metrics is False:
            connect_metrics_status = "dark"
        else:
            connect_metrics_status = "unknown"
        lines.append(
            f"- Telemetry: salt_epoch={salt_epoch}; rotation_age={rotation}; overrides={overrides}; "
            f"profile_alignment={alignment}; schema={schema}; policy_violations={violations}; "
            f"connect_metrics={connect_metrics_status}"
        )
        if telemetry.schema_policy_violations:
            lines.append("  Policy violations:")
            lines.extend(f"    - {entry}" for entry in telemetry.schema_policy_violations)
        if telemetry.notes:
            lines.append("  Notes:")
            lines.extend(f"    - {note}" for note in telemetry.notes)
        if connect_metrics is not True:
            lines.append("  Telemetry gap: Connect metrics unavailable or dark.")
    else:
        lines.append("- Telemetry: unavailable (exporter missing?)")
    if parity.alerts:
        lines.append("- Alerts:")
        lines.extend([f"  - {entry}" for entry in parity.alerts])
    else:
        lines.append("- Alerts: none")
    if parity.details:
        lines.extend(
            [
                "",
                f"{subprefix} Fixture Drift Summary",
                "",
                "| Instruction | Age (h) | Owner |",
                "| --- | ---: | --- |",
            ]
        )
        for detail in parity.details:
            lines.append(f"| {detail.instruction} | {detail.age_hours:.1f} | {detail.owner} |")
    if metrics is not None or signal_lane_names:
        lines.extend(
            [
                "",
                f"{subprefix} CI Signals",
                "",
            ]
        )
        if metrics is not None:
            lines.append(
                "- swift_parity_success_total="
                f"{metrics.parity_success_total} | swift_parity_failure_total={metrics.parity_failure_total} | "
                f"metrics_updated_at={metrics.updated_at or 'n/a'}"
            )
        else:
            lines.append("- swift_parity_success_total: metrics state unavailable")
        for lane_name in signal_lane_names:
            lane = _find_lane(ci, lane_name)
            if lane:
                lines.append(_format_lane(lane))
            else:
                lines.append(f"- {lane_name}: no data in feed")
    doc_entries = readiness_docs or []
    if doc_entries:
        lines.extend(
            [
                "",
                f"{subprefix} IOS8 Readiness Docs",
                "",
            ]
        )
        for doc in doc_entries:
            size_kib = doc.size_bytes / 1024 if doc.size_bytes else 0.0
            commit = doc.last_commit or "n/a"
            updated = doc.last_modified or "n/a"
            summary = f" — {doc.summary}" if doc.summary else ""
            lines.append(
                f"- {doc.title} ({doc.path}; size={size_kib:.1f} KiB) — "
                f"sha256={doc.sha256}; commit={commit}; updated={updated}{summary}"
            )
    lines.extend(
        [
            "",
            f"{subprefix} Swift CI Snapshot ({_isoformat(ci.generated_at)})",
            "",
        ]
    )
    if ci.queue_depth is not None or ci.average_runtime_minutes is not None:
        lines.append(
            f"- Queue depth: {ci.queue_depth if ci.queue_depth is not None else 'n/a'}; "
            f"avg runtime: {ci.average_runtime_minutes:.1f} min"
            if ci.average_runtime_minutes is not None
            else f"- Queue depth: {ci.queue_depth if ci.queue_depth is not None else 'n/a'}"
        )
    if ci.consecutive_failures is not None:
        incident_summary = ", ".join(ci.open_incidents) if ci.open_incidents else "none"
        lines.append(f"- Consecutive failures: {ci.consecutive_failures}; incidents: {incident_summary}")
    if ci.acceleration_notes:
        lines.append(f"- Acceleration bench: {'; '.join(ci.acceleration_notes)}")
    if ci.emulator_passes is not None or ci.strongbox_passes is not None:
        emu = (
            f"{ci.emulator_passes}/{ci.emulator_failures or 0}"
            if ci.emulator_passes is not None
            else "n/a"
        )
        strongbox = (
            f"{ci.strongbox_passes}/{ci.strongbox_failures or 0}"
            if ci.strongbox_passes is not None
            else "n/a"
        )
        lines.append(f"- Device results: emulators {emu} | strongbox {strongbox}")
    if ci.lanes:
        lines.append("- Lane summary (last 14 runs):")
        lines.extend(_format_lane(lane) for lane in ci.lanes)
    else:
        lines.append("- Lane summary: none")
    return "\n".join(lines) + "\n"


def build_summary(
    parity: ParitySnapshot,
    ci: CISnapshot,
    *,
    thresholds: HealthThresholds = DEFAULT_HEALTH_THRESHOLDS,
    health: Optional[dict] = None,
    readiness_docs: Optional[List[ReadinessDoc]] = None,
) -> dict:
    health_report = health or build_health_report(parity, ci, thresholds=thresholds)
    summary = {
        "parity": {
            "generated_at": _isoformat(parity.generated_at),
            "outstanding_diffs": parity.outstanding_diffs,
            "oldest_diff_hours": parity.oldest_diff_hours,
            "fixture_details": [
                {
                    "instruction": detail.instruction,
                    "age_hours": detail.age_hours,
                    "owner": detail.owner,
                }
                for detail in parity.details
            ],
            "regen": {
                "last_success": _isoformat(parity.regen_last_success),
                "hours_since_success": parity.regen_hours_since_success,
                "breach": parity.regen_breach,
            },
            "pipeline": {
                "status": parity.pipeline_status,
                "last_run": _isoformat(parity.pipeline_last_run),
                "failed_tests": parity.pipeline_failed_tests,
                "metadata": {
                    "job_name": parity.pipeline_metadata.job_name,
                    "duration_seconds": parity.pipeline_metadata.duration_seconds,
                    "tests": [
                        {
                            "name": test.name,
                            "duration_seconds": test.duration_seconds,
                        }
                        for test in parity.pipeline_metadata.tests
                    ],
                }
                if parity.pipeline_metadata
                else None,
                "metadata_source": parity.pipeline_metadata_source,
            },
            "alerts": parity.alerts,
            "acceleration": parity.acceleration,
            "telemetry": {
                "salt_epoch": parity.telemetry.salt_epoch if parity.telemetry else None,
                "salt_rotation_age_hours": parity.telemetry.salt_rotation_age_hours if parity.telemetry else None,
                "overrides_open": parity.telemetry.overrides_open if parity.telemetry else None,
                "device_profile_alignment": parity.telemetry.device_profile_alignment if parity.telemetry else None,
                "schema_version": parity.telemetry.schema_version if parity.telemetry else None,
                "schema_policy_violation_count": parity.telemetry.schema_policy_violation_count
                if parity.telemetry
                else None,
                "schema_policy_violations": parity.telemetry.schema_policy_violations
                if parity.telemetry
                else [],
                "notes": parity.telemetry.notes if parity.telemetry else [],
                "connect_metrics_present": parity.telemetry.connect_metrics_present if parity.telemetry else None,
            }
            if parity.telemetry
            else None,
        },
        "ci": {
            "generated_at": _isoformat(ci.generated_at),
            "queue_depth": ci.queue_depth,
            "average_runtime_minutes": ci.average_runtime_minutes,
            "consecutive_failures": ci.consecutive_failures,
            "open_incidents": ci.open_incidents,
            "acceleration": ci.acceleration_notes,
            "devices": {
                "emulators": {
                    "passes": ci.emulator_passes,
                    "failures": ci.emulator_failures,
                },
                "strongbox": {
                    "passes": ci.strongbox_passes,
                    "failures": ci.strongbox_failures,
                },
            },
            "lanes": [
                {
                    "name": lane.name,
                    "device_tag": lane.device_tag,
                    "success_rate_14_runs": lane.success_rate,
                    "last_failure": _isoformat(lane.last_failure),
                    "flake_count": lane.flake_count,
                    "mttr_hours": lane.mttr_hours,
                }
                for lane in ci.lanes
            ],
        },
    }
    summary["readiness_docs"] = [
        {
            "title": doc.title,
            "path": doc.path,
            "summary": doc.summary,
            "sha256": doc.sha256,
            "last_modified": doc.last_modified,
            "last_commit": doc.last_commit,
            "size_bytes": doc.size_bytes,
        }
        for doc in (readiness_docs or [])
    ]
    summary["health"] = health_report
    return summary


def _resolve_feed(
    *,
    path: Optional[Path],
    url: Optional[str],
    default: Path,
    parser,
    retries: int,
    backoff: float,
):
    if path and url:
        raise RuntimeError("specify either a local path or a URL for the feed, not both")
    if url:
        contents = _read_remote(url, retries=retries, backoff=backoff)
        with _temp_feed(contents) as tmp:
            return parser(tmp)
    chosen = path or default
    return parser(chosen)


def _format_hours(value: Optional[float]) -> str:
    if value is None:
        return "n/a"
    return f"{float(value):.1f}h"


def _build_slack_payload(
    parity: ParitySnapshot,
    ci: CISnapshot,
    digest_markdown: str,
    *,
    preview_lines: int,
) -> dict:
    headline_parts = [
        f"diffs={parity.outstanding_diffs}",
        f"oldest={_format_hours(parity.oldest_diff_hours)}",
        f"regen_breach={'yes' if parity.regen_breach else 'no'}",
    ]
    if ci.queue_depth is not None:
        headline_parts.append(f"ci_queue={ci.queue_depth}")

    build_number = os.getenv("BUILDKITE_BUILD_NUMBER")
    build_url = os.getenv("BUILDKITE_BUILD_URL")
    build_fragment = ""
    if build_number:
        build_fragment = f"Build #{build_number}"
    if build_url:
        link = f"<{build_url}|Open in Buildkite>"
        build_fragment = f"{build_fragment} {link}".strip()

    text = "*Swift status digest ready*"
    if build_fragment:
        text = f"{text} — {build_fragment}"
    text = f"{text}\n{', '.join(headline_parts)}"

    preview = ""
    if digest_markdown.strip():
        lines = digest_markdown.strip().splitlines()
        if preview_lines >= 0:
            lines = lines[:preview_lines]
        preview = "\n".join(lines)

    blocks: List[dict] = [{"type": "section", "text": {"type": "mrkdwn", "text": text}}]
    if preview:
        blocks.append(
            {
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": f"```{preview}```",
                },
            }
        )

    return {"text": text, "blocks": blocks}


def post_slack_notification(
    webhook: str,
    parity: ParitySnapshot,
    ci: CISnapshot,
    digest_markdown: str,
    *,
    preview_lines: int = 15,
) -> None:
    payload = _build_slack_payload(
        parity,
        ci,
        digest_markdown,
        preview_lines=max(0, preview_lines),
    )

    parsed = urllib.parse.urlparse(webhook)
    if parsed.scheme == "file":
        target = Path(urllib.request.url2pathname(parsed.path))
        target.write_text(json.dumps(payload), encoding="utf-8")
        return

    request = urllib.request.Request(
        webhook,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request) as response:
            status = getattr(response, "status", 200)
            if status >= 400:
                raise RuntimeError(f"Slack webhook returned status {status}")
    except urllib.error.HTTPError as exc:  # pragma: no cover - surfaced to caller
        raise RuntimeError(f"Slack webhook error: {exc.status} {exc.reason}") from exc
    except urllib.error.URLError as exc:  # pragma: no cover - surfaced to caller
        raise RuntimeError(f"Slack webhook unreachable: {exc.reason}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    raw_args = list(argv if argv is not None else sys.argv[1:])
    if raw_args:
        first = raw_args[0]
        if first in {"telemetry-override", "telemetry"}:
            remainder = raw_args[1:]
            if first == "telemetry":
                if not remainder or remainder[0] != "override":
                    raise SystemExit("usage: telemetry override <args>")
                remainder = remainder[1:]
            return _run_telemetry_override_command(remainder)

    parser = argparse.ArgumentParser(
        description="Generate Swift parity/CI weekly status exports from dashboard feeds."
    )
    repo_root = Path(__file__).resolve().parents[1]
    default_parity = repo_root / "dashboards" / "data" / "mobile_parity.sample.json"
    default_ci = repo_root / "dashboards" / "data" / "mobile_ci.sample.json"
    parser.add_argument("--parity", type=Path, help="Path to mobile parity feed JSON")
    parser.add_argument("--parity-url", help="URL to mobile parity feed JSON (https or s3)")
    parser.add_argument("--ci", type=Path, help="Path to mobile CI feed JSON")
    parser.add_argument("--ci-url", help="URL to mobile CI feed JSON (https or s3)")
    parser.add_argument(
        "--format",
        choices=("markdown", "json"),
        default="markdown",
        help="Output format (default: markdown)",
    )
    parser.add_argument(
        "--heading-level",
        type=int,
        default=2,
        help="Heading level for markdown export (default: 2)",
    )
    parser.add_argument(
        "--retry-attempts",
        type=int,
        default=3,
        help="Number of attempts for downloading remote feeds (default: 3)",
    )
    parser.add_argument(
        "--retry-backoff",
        type=float,
        default=2.0,
        help="Initial backoff in seconds between remote feed retries (default: 2.0)",
    )
    parser.add_argument(
        "--slack-webhook",
        help="Slack webhook URL for digest notifications (use file://... for dry-run payload capture)",
    )
    parser.add_argument(
        "--slack-preview-lines",
        type=int,
        default=15,
        help="Number of digest lines to include in Slack preview (default: 15)",
    )
    parser.add_argument(
        "--metrics-path",
        type=Path,
        help="Optional Prometheus textfile output for parity metrics.",
    )
    parser.add_argument(
        "--metrics-state",
        type=Path,
        help="Optional JSON file tracking cumulative parity metric counters.",
    )
    parser.add_argument(
        "--ci-signal-lane",
        action="append",
        dest="ci_signal_lanes",
        help="Lane name to highlight in the CI Signals section (repeat for multiple lanes).",
    )
    parser.add_argument(
        "--readiness-doc",
        dest="readiness_docs",
        action="append",
        help="Additional readiness document path to include in the digest (repeatable).",
    )
    parser.add_argument(
        "--skip-default-readiness-docs",
        action="store_true",
        help="Skip the default IOS8 readiness documents when rendering the digest.",
    )
    parser.add_argument(
        "--health-output",
        type=Path,
        help="Optional JSON file to write the health summary for dashboards.",
    )
    parser.add_argument(
        "--health-diff-warn",
        type=int,
        default=DEFAULT_HEALTH_THRESHOLDS.parity_diff_warn,
        help="Warn when outstanding diffs reach this count (default: %(default)s).",
    )
    parser.add_argument(
        "--health-diff-critical",
        type=int,
        default=DEFAULT_HEALTH_THRESHOLDS.parity_diff_critical,
        help="Critical when outstanding diffs reach this count (default: %(default)s).",
    )
    parser.add_argument(
        "--health-regen-warn-hours",
        type=float,
        default=DEFAULT_HEALTH_THRESHOLDS.parity_regen_warn_hours,
        help="Warn when regen idle time exceeds this many hours (default: %(default)s).",
    )
    parser.add_argument(
        "--health-ci-queue-warn",
        type=int,
        default=DEFAULT_HEALTH_THRESHOLDS.ci_queue_warn,
        help="Warn when CI queue depth reaches this value (default: %(default)s).",
    )
    parser.add_argument(
        "--health-ci-queue-critical",
        type=int,
        default=DEFAULT_HEALTH_THRESHOLDS.ci_queue_critical,
        help="Critical when CI queue depth reaches this value (default: %(default)s).",
    )
    parser.add_argument(
        "--health-ci-failure-warn",
        type=int,
        default=DEFAULT_HEALTH_THRESHOLDS.ci_failure_warn,
        help="Warn when CI consecutive failures reach this value (default: %(default)s).",
    )
    parser.add_argument(
        "--health-ci-failure-critical",
        type=int,
        default=DEFAULT_HEALTH_THRESHOLDS.ci_failure_critical,
        help="Critical when CI consecutive failures reach this value (default: %(default)s).",
    )
    parser.add_argument(
        "--health-ci-lane-warn",
        type=float,
        default=DEFAULT_HEALTH_THRESHOLDS.ci_lane_warn,
        help="Warn when a lane success rate falls below this ratio (default: %(default)s).",
    )
    parser.add_argument(
        "--health-ci-lane-critical",
        type=float,
        default=DEFAULT_HEALTH_THRESHOLDS.ci_lane_critical,
        help="Critical when a lane success rate falls below this ratio (default: %(default)s).",
    )

    args = parser.parse_args(raw_args or None)
    doc_paths: List[Path] = []
    if not args.skip_default_readiness_docs:
        doc_paths.extend(DEFAULT_READINESS_DOCS)
    if args.readiness_docs:
        doc_paths.extend(Path(entry) for entry in args.readiness_docs if entry)
    readiness_docs = collect_readiness_docs(doc_paths, repo_root) if doc_paths else []
    ci_signal_lanes = [lane for lane in (args.ci_signal_lanes or ["ci/xcode-swift-parity"]) if lane]
    thresholds = HealthThresholds(
        parity_diff_warn=max(0, args.health_diff_warn),
        parity_diff_critical=max(args.health_diff_warn, args.health_diff_critical),
        parity_regen_warn_hours=max(0.0, args.health_regen_warn_hours),
        ci_queue_warn=max(0, args.health_ci_queue_warn),
        ci_queue_critical=max(args.health_ci_queue_warn, args.health_ci_queue_critical),
        ci_failure_warn=max(0, args.health_ci_failure_warn),
        ci_failure_critical=max(args.health_ci_failure_warn, args.health_ci_failure_critical),
        ci_lane_warn=min(1.0, max(0.0, args.health_ci_lane_warn)),
        ci_lane_critical=min(args.health_ci_lane_warn, max(0.0, args.health_ci_lane_critical)),
    )

    parity = _resolve_feed(
        path=args.parity,
        url=args.parity_url,
        default=default_parity,
        parser=load_parity_snapshot,
        retries=args.retry_attempts,
        backoff=args.retry_backoff,
    )
    ci = _resolve_feed(
        path=args.ci,
        url=args.ci_url,
        default=default_ci,
        parser=load_ci_snapshot,
        retries=args.retry_attempts,
        backoff=args.retry_backoff,
    )

    metrics_state_path = args.metrics_state or (repo_root / "artifacts" / "swift_status_metrics_state.json")
    metrics_state: Optional[MetricsState] = None
    if args.metrics_path or metrics_state_path.exists():
        metrics_state = _load_metrics_state(metrics_state_path)

    if args.metrics_path:
        success = _parity_success(parity)
        if metrics_state is None:
            metrics_state = MetricsState()
        if success:
            metrics_state.parity_success_total += 1
        else:
            metrics_state.parity_failure_total += 1
        metrics_state.updated_at = _isoformat(datetime.now(timezone.utc))
        _store_metrics_state(metrics_state_path, metrics_state)
        _write_metrics(args.metrics_path, parity, metrics_state, success)

    health_report = build_health_report(parity, ci, thresholds=thresholds)
    digest_markdown = render_markdown(
        parity,
        ci,
        heading_level=args.heading_level,
        metrics=metrics_state,
        signal_lane_names=ci_signal_lanes,
        thresholds=thresholds,
        health=health_report,
        readiness_docs=readiness_docs,
    )

    if args.format == "markdown":
        output = digest_markdown
    else:
        summary = build_summary(
            parity,
            ci,
            thresholds=thresholds,
            health=health_report,
            readiness_docs=readiness_docs,
        )
        output = json.dumps(summary, indent=2)

    print(output)
    digest_preview = digest_markdown

    if args.health_output:
        args.health_output.parent.mkdir(parents=True, exist_ok=True)
        args.health_output.write_text(json.dumps(health_report, indent=2) + "\n", encoding="utf-8")

    if args.slack_webhook:
        try:
            post_slack_notification(
                args.slack_webhook,
                parity,
                ci,
                digest_preview,
                preview_lines=args.slack_preview_lines,
            )
        except RuntimeError as exc:
            raise SystemExit(f"failed to post Slack notification: {exc}") from exc

    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    raise SystemExit(main())
