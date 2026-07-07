#!/usr/bin/env python3
"""
Lightweight validator for Swift dashboard JSON feeds.

Usage:
    scripts/check_swift_dashboard_data.py dashboards/data/mobile_parity.sample.json
    scripts/check_swift_dashboard_data.py dashboards/data/mobile_ci.sample.json

The validator ensures the top-level structure matches the schema documented in
docs/source/references/ios_metrics.md so CI can catch drift early.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional, Sequence, Tuple


def load(path: Path) -> Dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def parse_generated_at(value: Any, path: str) -> datetime:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{path}.generated_at must be a non-empty ISO-8601 timestamp")
    raw = value.strip()
    normalized = raw[:-1] + "+00:00" if raw.endswith("Z") else raw
    try:
        parsed = datetime.fromisoformat(normalized)
    except ValueError as exc:
        raise ValueError(f"{path}.generated_at must be a valid ISO-8601 timestamp") from exc
    if parsed.tzinfo is None:
        raise ValueError(f"{path}.generated_at must include a timezone")
    return parsed.astimezone(timezone.utc)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def checksum_sidecar_path(path: Path) -> Path:
    return path.with_name(path.name + ".sha256")


def read_checksum_sidecar(path: Path) -> str:
    sidecar = checksum_sidecar_path(path)
    try:
        raw = sidecar.read_text(encoding="utf-8").strip()
    except FileNotFoundError as exc:
        raise ValueError(f"{path}: checksum sidecar missing at {sidecar}") from exc
    if not raw:
        raise ValueError(f"{sidecar}: checksum sidecar is empty")
    parts = raw.split()
    digest = parts[0].lower()
    if len(digest) != 64 or any(ch not in "0123456789abcdef" for ch in digest):
        raise ValueError(f"{sidecar}: first field must be a SHA-256 hex digest")
    if len(parts) > 1:
        labelled = parts[-1].lstrip("*")
        if Path(labelled).name != path.name:
            raise ValueError(
                f"{sidecar}: checksum label {labelled!r} does not match {path.name!r}"
            )
    return digest


def verify_checksum_sidecar(path: Path) -> str:
    expected = read_checksum_sidecar(path)
    actual = sha256_file(path)
    if actual != expected:
        raise ValueError(
            f"{path}: SHA-256 mismatch against {checksum_sidecar_path(path)} "
            f"(expected {expected}, got {actual})"
        )
    return actual


def validate_feed_freshness(
    generated_at: datetime,
    path: str,
    max_age_seconds: Optional[float],
    now: datetime,
) -> float:
    age_seconds = (now - generated_at).total_seconds()
    if age_seconds < 0:
        raise ValueError(f"{path}.generated_at is in the future")
    if max_age_seconds is not None and age_seconds > max_age_seconds:
        raise ValueError(
            f"{path}: feed age {age_seconds:.0f}s exceeds maximum {max_age_seconds:.0f}s"
        )
    return age_seconds


def expect_keys(obj: Dict[str, Any], keys: List[str], path: str) -> None:
    missing = [key for key in keys if key not in obj]
    if missing:
        raise ValueError(f"{path} missing keys: {missing}")


def _validate_accel_backend(backend: Any, path: str) -> None:
    if not isinstance(backend, dict):
        raise ValueError(f"{path} must be an object with acceleration metadata")
    expect_keys(backend, ["enabled"], path)
    if not isinstance(backend["enabled"], bool):
        raise ValueError(f"{path}.enabled must be a boolean")
    parity = backend.get("parity")
    if parity is not None and not isinstance(parity, str):
        raise ValueError(f"{path}.parity must be a string when present")
    perf_delta = backend.get("perf_delta_pct")
    if perf_delta is not None and not isinstance(perf_delta, (int, float)):
        raise ValueError(f"{path}.perf_delta_pct must be numeric when present")
    unexpected = set(backend.keys()) - {"enabled", "parity", "perf_delta_pct"}
    if unexpected:
        raise ValueError(f"{path} contains unexpected keys: {sorted(unexpected)}")


def _validate_device_group(group: Any, path: str) -> None:
    if not isinstance(group, dict):
        raise ValueError(f"{path} must be an object with pass/fail counts")
    expect_keys(group, ["passes", "failures"], path)
    for field in ("passes", "failures"):
        value = group.get(field)
        if not isinstance(value, int) or value < 0:
            raise ValueError(f"{path}.{field} must be a non-negative integer")


def _validate_pipeline_metadata(metadata: Any, path: str) -> None:
    if not isinstance(metadata, dict):
        raise ValueError(f"{path} must be an object when present")
    job_name = metadata.get("job_name")
    if job_name is not None and not isinstance(job_name, str):
        raise ValueError(f"{path}.job_name must be a string when present")
    duration = metadata.get("duration_seconds")
    if duration is not None:
        if not isinstance(duration, (int, float)):
            raise ValueError(f"{path}.duration_seconds must be numeric when present")
        if duration < 0:
            raise ValueError(f"{path}.duration_seconds must be non-negative")
    tests = metadata.get("tests")
    if tests is not None:
        if not isinstance(tests, list):
            raise ValueError(f"{path}.tests must be a list when present")
        for idx, test in enumerate(tests):
            test_path = f"{path}.tests[{idx}]"
            if not isinstance(test, dict):
                raise ValueError(f"{test_path} must be an object")
            expect_keys(test, ["name", "duration_seconds"], test_path)
            name = test.get("name")
            if not isinstance(name, str):
                raise ValueError(f"{test_path}.name must be a string")
            duration_value = test.get("duration_seconds")
            if not isinstance(duration_value, (int, float)):
                raise ValueError(f"{test_path}.duration_seconds must be numeric")
            if duration_value < 0:
                raise ValueError(f"{test_path}.duration_seconds must be non-negative")


def _validate_fixture_details(details: Any, path: str) -> None:
    if not isinstance(details, list):
        raise ValueError(f"{path} must be a list")
    for idx, entry in enumerate(details):
        entry_path = f"{path}[{idx}]"
        if not isinstance(entry, dict):
            raise ValueError(f"{entry_path} must be an object")
        instruction = entry.get("instruction")
        if not isinstance(instruction, str) or not instruction.strip():
            raise ValueError(f"{entry_path}.instruction must be a non-empty string")
        age = entry.get("age_hours")
        if not isinstance(age, (int, float)) or isinstance(age, bool):
            raise ValueError(f"{entry_path}.age_hours must be numeric")
        if age < 0:
            raise ValueError(f"{entry_path}.age_hours must be non-negative")
        owner = entry.get("owner")
        if not isinstance(owner, str) or not owner.strip():
            raise ValueError(f"{entry_path}.owner must be a non-empty string")


def validate_parity(data: Dict[str, Any], path: str) -> None:
    expect_keys(data, ["generated_at", "fixtures", "pipeline", "regen_sla", "alerts"], path)
    parse_generated_at(data["generated_at"], path)

    fixtures = data["fixtures"]
    expect_keys(fixtures, ["outstanding_diffs", "oldest_diff_hours", "details"], f"{path}.fixtures")
    _validate_fixture_details(fixtures["details"], f"{path}.fixtures.details")

    pipeline = data["pipeline"]
    expect_keys(pipeline, ["last_run", "status", "failed_tests"], f"{path}.pipeline")
    metadata = pipeline.get("metadata")
    if metadata is not None:
        _validate_pipeline_metadata(metadata, f"{path}.pipeline.metadata")
    metadata_source = pipeline.get("metadata_source")
    if metadata_source is not None and not isinstance(metadata_source, str):
        raise ValueError(f"{path}.pipeline.metadata_source must be a string when present")

    regen = data["regen_sla"]
    expect_keys(regen, ["last_success", "hours_since_success", "breach"], f"{path}.regen_sla")

    if not isinstance(data["alerts"], list):
        raise ValueError(f"{path}.alerts must be a list")

    acceleration = data.get("acceleration")
    if acceleration is not None:
        if not isinstance(acceleration, dict):
            raise ValueError(f"{path}.acceleration must be an object when present")
        unexpected = set(acceleration.keys()) - {"metal", "neon", "strongbox"}
        if unexpected:
            raise ValueError(f"{path}.acceleration has unexpected backends: {sorted(unexpected)}")
        for backend_name, backend in acceleration.items():
            _validate_accel_backend(backend, f"{path}.acceleration.{backend_name}")

    telemetry = data.get("telemetry")
    if telemetry is not None:
        if not isinstance(telemetry, dict):
            raise ValueError(f"{path}.telemetry must be an object when present")
        allowed = {
            "salt_epoch",
            "salt_rotation_age_hours",
            "overrides_open",
            "device_profile_alignment",
            "schema_version",
            "schema_policy_violation_count",
            "schema_policy_violations",
            "notes",
            "connect_metrics_present",
        }
        unexpected = set(telemetry.keys()) - allowed
        if unexpected:
            raise ValueError(f"{path}.telemetry contains unexpected keys: {sorted(unexpected)}")
        notes = telemetry.get("notes")
        if notes is not None:
            if not isinstance(notes, list):
                raise ValueError(f"{path}.telemetry.notes must be a list when present")
            for idx, note in enumerate(notes):
                if not isinstance(note, str):
                    raise ValueError(
                        f"{path}.telemetry.notes[{idx}] must be a string when present"
                    )
        salt_epoch = telemetry.get("salt_epoch")
        if salt_epoch is not None and not isinstance(salt_epoch, str):
            raise ValueError(f"{path}.telemetry.salt_epoch must be a string when present")
        salt_rotation = telemetry.get("salt_rotation_age_hours")
        if salt_rotation is not None:
            if not isinstance(salt_rotation, (int, float)) or isinstance(salt_rotation, bool):
                raise ValueError(
                    f"{path}.telemetry.salt_rotation_age_hours must be numeric when present"
                )
            if salt_rotation < 0:
                raise ValueError(
                    f"{path}.telemetry.salt_rotation_age_hours must be non-negative"
                )
        overrides_open = telemetry.get("overrides_open")
        if overrides_open is not None:
            if not isinstance(overrides_open, int) or isinstance(overrides_open, bool):
                raise ValueError(
                    f"{path}.telemetry.overrides_open must be an integer count when present"
                )
            if overrides_open < 0:
                raise ValueError(
                    f"{path}.telemetry.overrides_open must be non-negative when present"
                )
        device_alignment = telemetry.get("device_profile_alignment")
        if device_alignment is not None and not isinstance(device_alignment, str):
            raise ValueError(
                f"{path}.telemetry.device_profile_alignment must be a string when present"
            )
        schema_version = telemetry.get("schema_version")
        if schema_version is not None and not isinstance(schema_version, str):
            raise ValueError(
                f"{path}.telemetry.schema_version must be a string when present"
            )
        violation_count = telemetry.get("schema_policy_violation_count")
        if violation_count is not None:
            if not isinstance(violation_count, int) or isinstance(violation_count, bool):
                raise ValueError(
                    f"{path}.telemetry.schema_policy_violation_count must be an integer when present"
                )
            if violation_count < 0:
                raise ValueError(
                    f"{path}.telemetry.schema_policy_violation_count must be non-negative when present"
                )
        violations = telemetry.get("schema_policy_violations")
        if violations is not None:
            if not isinstance(violations, list):
                raise ValueError(
                    f"{path}.telemetry.schema_policy_violations must be a list when present"
                )
            for idx, violation in enumerate(violations):
                if not isinstance(violation, str):
                    raise ValueError(
                        f"{path}.telemetry.schema_policy_violations[{idx}] must be a string"
                    )
        connect_metrics = telemetry.get("connect_metrics_present")
        if connect_metrics is not None and not isinstance(connect_metrics, bool):
            raise ValueError(
                f"{path}.telemetry.connect_metrics_present must be a boolean when present"
            )


def validate_ci(data: Dict[str, Any], path: str) -> None:
    expect_keys(data, ["generated_at", "buildkite", "devices", "alert_state"], path)
    parse_generated_at(data["generated_at"], path)

    buildkite = data["buildkite"]
    expect_keys(buildkite, ["lanes", "queue_depth", "average_runtime_minutes"], f"{path}.buildkite")
    if not isinstance(buildkite["lanes"], list):
        raise ValueError(f"{path}.buildkite.lanes must be a list")
    for idx, lane in enumerate(buildkite["lanes"]):
        lane_path = f"{path}.buildkite.lanes[{idx}]"
        if not isinstance(lane, dict):
            raise ValueError(f"{lane_path} must be an object")
        expect_keys(
            lane,
            ["name", "success_rate_14_runs", "last_failure", "flake_count", "mttr_hours"],
            lane_path,
        )
        device_tag = lane.get("device_tag")
        if device_tag is not None and not isinstance(device_tag, str):
            raise ValueError(f"{lane_path}.device_tag must be a string when present")

    devices = data["devices"]
    expect_keys(devices, ["emulators", "strongbox_capable"], f"{path}.devices")
    _validate_device_group(devices["emulators"], f"{path}.devices.emulators")
    _validate_device_group(devices["strongbox_capable"], f"{path}.devices.strongbox_capable")

    alert_state = data["alert_state"]
    expect_keys(alert_state, ["consecutive_failures", "open_incidents"], f"{path}.alert_state")

    accel = data.get("acceleration_bench")
    if accel is not None:
        if not isinstance(accel, dict):
            raise ValueError(f"{path}.acceleration_bench must be an object when present")
        unexpected = set(accel.keys()) - {"metal_vs_cpu_merkle_ms", "neon_crc64_throughput_mb_s"}
        if unexpected:
            raise ValueError(
                f"{path}.acceleration_bench has unexpected fields: {sorted(unexpected)}"
            )
        merkle = accel.get("metal_vs_cpu_merkle_ms")
        if merkle is not None:
            if not isinstance(merkle, dict):
                raise ValueError(
                    f"{path}.acceleration_bench.metal_vs_cpu_merkle_ms must be an object"
                )
            expect_keys(merkle, ["cpu", "metal"], f"{path}.acceleration_bench.metal_vs_cpu_merkle_ms")
            for field in ("cpu", "metal"):
                value = merkle.get(field)
                if not isinstance(value, (int, float)) or value < 0:
                    raise ValueError(
                        f"{path}.acceleration_bench.metal_vs_cpu_merkle_ms.{field} must be a non-negative number"
                    )
        neon_crc = accel.get("neon_crc64_throughput_mb_s")
        if neon_crc is not None:
            if not isinstance(neon_crc, (int, float)):
                raise ValueError(
                    f"{path}.acceleration_bench.neon_crc64_throughput_mb_s must be numeric"
                )
            if neon_crc < 0:
                raise ValueError(
                    f"{path}.acceleration_bench.neon_crc64_throughput_mb_s must be non-negative"
                )


def enforce_parity_sla(
    data: Dict[str, Any],
    path: str,
    max_diffs: int | None,
    max_oldest_hours: float | None,
    max_regen_hours: float | None,
    require_no_breach: bool,
) -> None:
    fixtures = data.get("fixtures", {})
    regen = data.get("regen_sla", {})

    if max_diffs is not None:
        outstanding = fixtures.get("outstanding_diffs")
        if outstanding is None or outstanding > max_diffs:
            raise ValueError(
                f"{path}: outstanding_diffs {outstanding} exceeds allowed maximum {max_diffs}"
            )

    if max_oldest_hours is not None:
        oldest = fixtures.get("oldest_diff_hours")
        if oldest is None or oldest > max_oldest_hours:
            raise ValueError(
                f"{path}: oldest_diff_hours {oldest} exceeds allowed maximum {max_oldest_hours}"
            )

    if max_regen_hours is not None:
        hours = regen.get("hours_since_success")
        if hours is None or hours > max_regen_hours:
            raise ValueError(
                f"{path}: regen_sla.hours_since_success {hours} exceeds allowed maximum {max_regen_hours}"
            )

    if require_no_breach and regen.get("breach"):
        raise ValueError(f"{path}: regen_sla.breach flagged while no-breach policy is enforced")


def parse_backend_requirements(items: Iterable[str], option_name: str) -> Dict[str, str]:
    """Parse CLI backend=value pairs for acceleration requirements."""

    requirements: Dict[str, str] = {}
    for raw in items:
        if "=" not in raw:
            raise ValueError(f"{option_name} entries must use backend=value syntax (got {raw!r})")
        backend, expected = raw.split("=", 1)
        backend = backend.strip()
        if not backend:
            raise ValueError(f"{option_name} entries require a backend name (got {raw!r})")
        expected = expected.strip()
        if not expected:
            raise ValueError(f"{option_name} entries require an expected value (got {raw!r})")
        requirements[backend] = expected
    return requirements


def parse_lane_thresholds(items: Iterable[str]) -> List[Tuple[str, float]]:
    thresholds: List[Tuple[str, float]] = []
    for raw in items:
        if ">=" in raw:
            name, value = raw.split(">=", 1)
        elif "=" in raw:
            name, value = raw.split("=", 1)
        else:
            raise ValueError(
                "Lane thresholds must use 'lane>=value' or 'lane=value' syntax (e.g. ci/xcode-swift-parity>=0.95)."
            )
        name = name.strip()
        if not name:
            raise ValueError("Lane threshold is missing a name")
        try:
            thresholds.append((name, float(value)))
        except ValueError as exc:  # pragma: no cover - ValueError message propagated
            raise ValueError(f"Invalid success rate for lane '{name}': {value}") from exc
    return thresholds


def enforce_parity_acceleration(
    data: Dict[str, Any],
    path: str,
    parity_requirements: Dict[str, str],
    require_enabled: Sequence[str],
) -> None:
    if not parity_requirements and not require_enabled:
        return

    acceleration = data.get("acceleration")
    if not isinstance(acceleration, dict):
        raise ValueError(f"{path}: acceleration data is required when enforcing acceleration policies")

    for backend in require_enabled:
        backend_data = acceleration.get(backend)
        if backend_data is None:
            raise ValueError(f"{path}: acceleration.{backend} missing while --parity-accel-require-enabled is set")
        if backend_data.get("enabled") is not True:
            raise ValueError(f"{path}: acceleration.{backend}.enabled must be true")

    for backend, expected in parity_requirements.items():
        backend_data = acceleration.get(backend)
        if backend_data is None:
            raise ValueError(f"{path}: acceleration.{backend} missing while enforcing parity expectation '{expected}'")
        actual = backend_data.get("parity")
        if actual != expected:
            raise ValueError(
                f"{path}: acceleration.{backend}.parity={actual!r} does not match expected value {expected!r}"
            )


def enforce_ci_sla(
    data: Dict[str, Any],
    path: str,
    lane_thresholds: Sequence[Tuple[str, float]],
    max_consecutive_failures: int | None,
) -> None:
    if not lane_thresholds and max_consecutive_failures is None:
        return

    buildkite = data.get("buildkite", {})
    lanes = buildkite.get("lanes", [])
    lane_by_name = {lane.get("name"): lane for lane in lanes if "name" in lane}

    for name, minimum in lane_thresholds:
        if name not in lane_by_name:
            raise ValueError(f"{path}: missing lane '{name}' required for SLA enforcement")
        lane = lane_by_name[name]
        rate = lane.get("success_rate_14_runs")
        if rate is None or rate < minimum:
            raise ValueError(
                f"{path}: lane '{name}' success_rate_14_runs={rate} is below required minimum {minimum}"
            )

    if max_consecutive_failures is not None:
        consecutive = data.get("alert_state", {}).get("consecutive_failures")
        if consecutive is None or consecutive > max_consecutive_failures:
            raise ValueError(
                f"{path}: alert_state.consecutive_failures {consecutive} exceeds allowed maximum {max_consecutive_failures}"
            )


def enforce_ci_acceleration_benchmarks(
    data: Dict[str, Any],
    path: str,
    metal_min_speedup: float | None,
    neon_min_throughput: float | None,
) -> None:
    if metal_min_speedup is None and neon_min_throughput is None:
        return

    bench = data.get("acceleration_bench")
    if not isinstance(bench, dict):
        raise ValueError(f"{path}: acceleration_bench data is required when enforcing acceleration thresholds")

    if metal_min_speedup is not None:
        merkle = bench.get("metal_vs_cpu_merkle_ms")
        if not isinstance(merkle, dict):
            raise ValueError(f"{path}: acceleration_bench.metal_vs_cpu_merkle_ms missing while enforcing Metal speedup")
        cpu = merkle.get("cpu")
        metal = merkle.get("metal")
        if not isinstance(cpu, (int, float)) or not isinstance(metal, (int, float)):
            raise ValueError(f"{path}: acceleration_bench.metal_vs_cpu_merkle_ms entries must be numeric")
        if metal <= 0:
            raise ValueError(f"{path}: acceleration_bench.metal_vs_cpu_merkle_ms.metal must be positive to compute speedup")
        speedup = cpu / metal
        if speedup < metal_min_speedup:
            raise ValueError(
                f"{path}: Metal speedup {speedup:.2f}x below required minimum {metal_min_speedup:.2f}x"
            )

    if neon_min_throughput is not None:
        throughput = bench.get("neon_crc64_throughput_mb_s")
        if not isinstance(throughput, (int, float)):
            raise ValueError(
                f"{path}: acceleration_bench.neon_crc64_throughput_mb_s missing while enforcing NEON throughput"
            )
        if throughput < neon_min_throughput:
            raise ValueError(
                f"{path}: NEON CRC64 throughput {throughput} MB/s below required minimum {neon_min_throughput} MB/s"
            )


def detect_schema(data: Dict[str, Any], path: str) -> str:
    if isinstance(data, dict):
        if "fixtures" in data and "pipeline" in data:
            return "parity"
        if "buildkite" in data and "devices" in data:
            return "ci"
    raise ValueError(
        f"Unable to infer schema for '{path}'. Expected parity payload "
        "with 'fixtures'/'pipeline' keys or CI payload with 'buildkite'/'devices'."
    )


def build_health_entry(
    *,
    path: Path,
    schema: str,
    payload: Dict[str, Any],
    now: datetime,
    max_feed_age_seconds: Optional[float],
    checksum_verified: bool,
    sha256_hex: str,
) -> Dict[str, Any]:
    generated_at = parse_generated_at(payload.get("generated_at"), str(path))
    age_seconds = validate_feed_freshness(
        generated_at,
        str(path),
        max_feed_age_seconds,
        now,
    )
    return {
        "path": str(path),
        "schema": schema,
        "generated_at": generated_at.isoformat().replace("+00:00", "Z"),
        "age_seconds": age_seconds,
        "fresh": max_feed_age_seconds is None or age_seconds <= max_feed_age_seconds,
        "sha256": sha256_hex,
        "checksum_sidecar": str(checksum_sidecar_path(path)),
        "checksum_verified": checksum_verified,
    }


def write_health_output(path: Path, entries: Sequence[Dict[str, Any]]) -> None:
    payload = {
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "healthy": all(entry["fresh"] and entry["checksum_verified"] for entry in entries),
        "feeds": list(entries),
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate Swift dashboard JSON feeds and optional SLAs.")
    parser.add_argument("files", nargs="+", type=Path, help="JSON files to validate")
    parser.add_argument("--parity-max-diffs", type=int, default=None,
                        help="Maximum allowed outstanding diffs for parity feeds.")
    parser.add_argument("--parity-max-oldest-hours", type=float, default=None,
                        help="Maximum allowed oldest diff age (hours) for parity feeds.")
    parser.add_argument("--parity-max-regen-hours", type=float, default=None,
                        help="Maximum allowed hours since last successful regen for parity feeds.")
    parser.add_argument("--parity-require-no-breach", action="store_true",
                        help="Fail if regen_sla.breach is true for parity feeds.")
    parser.add_argument("--parity-accel-require", action="append", default=[], metavar="backend=value",
                        help="Require acceleration backend parity to match the given value (repeatable).")
    parser.add_argument("--parity-accel-require-enabled", action="append", default=[], metavar="backend",
                        help="Require the acceleration backend to be enabled (repeatable).")
    parser.add_argument("--ci-lane-threshold", action="append", default=[], metavar="lane>=rate",
                        help="Enforce Buildkite lane success rate threshold (repeatable, e.g. --ci-lane-threshold ci/xcode-swift-parity>=0.95).")
    parser.add_argument("--ci-max-consecutive-failures", type=int, default=None,
                        help="Maximum allowed consecutive failure count for CI feeds.")
    parser.add_argument("--ci-metal-min-speedup", type=float, default=None,
                        help="Minimum Metal vs CPU speedup (x) required when acceleration benchmarks are present.")
    parser.add_argument("--ci-neon-min-throughput", type=float, default=None,
                        help="Minimum NEON CRC64 throughput (MB/s) required when acceleration benchmarks are present.")
    parser.add_argument("--require-checksum-sidecars", action="store_true",
                        help="Require and verify a sibling .sha256 sidecar for every feed.")
    parser.add_argument("--max-feed-age-seconds", type=float, default=None,
                        help="Maximum generated_at age for each feed. Also used in --health-output.")
    parser.add_argument("--health-output", type=Path,
                        help="Write a health JSON document containing freshness and checksum status.")
    args = parser.parse_args()

    has_error = False
    health_entries: List[Dict[str, Any]] = []
    now = datetime.now(timezone.utc)
    lane_thresholds: List[Tuple[str, float]] = []
    try:
        lane_thresholds = parse_lane_thresholds(args.ci_lane_threshold)
    except ValueError as exc:
        print(f"[error] ci-lane-threshold: {exc}", file=sys.stderr)
        return 1

    try:
        parity_accel_requirements = parse_backend_requirements(args.parity_accel_require, "--parity-accel-require")
    except ValueError as exc:
        print(f"[error] parity-accel-require: {exc}", file=sys.stderr)
        return 1

    for file_path in args.files:
        try:
            data = load(file_path)
            schema = detect_schema(data, str(file_path))
            if schema == "parity":
                validate_parity(data, str(file_path))
                validate_feed_freshness(
                    parse_generated_at(data["generated_at"], str(file_path)),
                    str(file_path),
                    args.max_feed_age_seconds,
                    now,
                )
                enforce_parity_sla(
                    data,
                    str(file_path),
                    args.parity_max_diffs,
                    args.parity_max_oldest_hours,
                    args.parity_max_regen_hours,
                    args.parity_require_no_breach,
                )
                enforce_parity_acceleration(
                    data,
                    str(file_path),
                    parity_accel_requirements,
                    args.parity_accel_require_enabled,
                )
            else:
                validate_ci(data, str(file_path))
                validate_feed_freshness(
                    parse_generated_at(data["generated_at"], str(file_path)),
                    str(file_path),
                    args.max_feed_age_seconds,
                    now,
                )
                enforce_ci_sla(
                    data,
                    str(file_path),
                    lane_thresholds,
                    args.ci_max_consecutive_failures,
                )
                enforce_ci_acceleration_benchmarks(
                    data,
                    str(file_path),
                    args.ci_metal_min_speedup,
                    args.ci_neon_min_throughput,
                )
            checksum_verified = False
            if args.require_checksum_sidecars:
                sha256_hex = verify_checksum_sidecar(file_path)
                checksum_verified = True
            else:
                sha256_hex = sha256_file(file_path)
                if checksum_sidecar_path(file_path).exists():
                    sha256_hex = verify_checksum_sidecar(file_path)
                    checksum_verified = True
            if args.health_output:
                health_entries.append(
                    build_health_entry(
                        path=file_path,
                        schema=schema,
                        payload=data,
                        now=now,
                        max_feed_age_seconds=args.max_feed_age_seconds,
                        checksum_verified=checksum_verified,
                        sha256_hex=sha256_hex,
                    )
                )
            print(f"[ok] {file_path}")
        except Exception as exc:  # pylint: disable=broad-except
            has_error = True
            print(f"[error] {file_path}: {exc}", file=sys.stderr)

    if args.health_output and not has_error:
        write_health_output(args.health_output, health_entries)

    return 1 if has_error else 0


if __name__ == "__main__":
    sys.exit(main())
