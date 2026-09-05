#!/usr/bin/env python3
"""Revalidate FASTPQ rollout captures against their manifest and release limits.

Requires Python 3.10+ and the BLAKE3 package from scripts/requirements.txt.
Paths in a manifest are absolute or relative to --repo-root. This read-only
consistency helper does not authenticate a signer; ci/check_fastpq_rollout.sh
also invokes the Rust signature verifier with an independently trusted key.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any, Sequence

import blake3

CANONICAL_OPERATIONS = {
    "fft", "ifft", "lde", "poseidon_hash_columns", "poseidon_merkle_pairs",
    "bn254_poseidon_words",
}


def object_value(value: Any, context: str) -> dict[str, Any]:
    """Require an object before reading evidence fields."""
    if not isinstance(value, dict):
        raise ValueError(f"{context} must be an object")
    return value


def number(value: Any, context: str, *, positive: bool = False) -> float:
    """Reject booleans, strings, negative values and nonfinite measurements."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{context} must be a finite number")
    try:
        valid = math.isfinite(value) and (value > 0 if positive else value >= 0)
    except OverflowError:
        valid = False
    if not valid:
        raise ValueError(f"{context} must be finite and {'positive' if positive else 'non-negative'}")
    return value


def integer(value: Any, context: str, *, minimum: int = 0) -> int:
    """Require an integral counter without accepting booleans as integers."""
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        raise ValueError(f"{context} must be an integer >= {minimum}")
    return value


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Reject duplicate JSON fields instead of silently choosing one claim."""
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON field {key!r}")
        result[key] = value
    return result


def load_json(content: bytes) -> dict[str, Any]:
    """Read a JSON object with unambiguous member names."""
    return object_value(json.loads(content, object_pairs_hook=unique_object), "JSON root")


def validate_operations(benchmarks: dict[str, Any], constraints: dict[str, Any]) -> None:
    """Check actual GPU measurements and every declared performance threshold."""
    entries = benchmarks.get("operations")
    if not isinstance(entries, list) or not entries:
        raise ValueError("benchmarks.operations must contain measured operations")
    operations = {}
    for raw in entries:
        entry = object_value(raw, "operation")
        name = entry.get("operation")
        if not isinstance(name, str) or name not in CANONICAL_OPERATIONS:
            raise ValueError("benchmark contains an unknown operation")
        if name in operations:
            raise ValueError(f"duplicate operation {name}")
        operations[name] = entry
        for field in ("cpu_mean_ms", "gpu_mean_ms", "speedup_ratio"):
            number(entry.get(field), f"{name}.{field}", positive=True)
        # Captures round timing and speedup fields to three decimals. Compare
        # intervals rather than demanding equality of their rounded quotient.
        cpu = entry["cpu_mean_ms"]
        gpu = entry["gpu_mean_ms"]
        speedup = entry["speedup_ratio"]
        lower = max(0, cpu - 0.0005) / (gpu + 0.0005)
        upper = (cpu + 0.0005) / max(gpu - 0.0005, 1e-300)
        if speedup + 0.0005 < lower or speedup - 0.0005 > upper:
            raise ValueError(f"{name}.speedup_ratio disagrees with CPU/GPU timings")
    missing = CANONICAL_OPERATIONS - operations.keys()
    if missing or benchmarks.get("operation_filter", "all") != "all":
        raise ValueError("rollout capture must measure every canonical operation")
    for key, field, is_max in (
        ("max_operation_ms", "gpu_mean_ms", True),
        ("min_operation_speedup", "speedup_ratio", False),
    ):
        for operation, limit in constraints[key].items():
            number(limit, f"constraints.{key}.{operation}", positive=True)
            if operation not in operations:
                raise ValueError(f"missing constrained operation {operation}")
            value = operations[operation][field]
            if (is_max and value > limit) or (not is_max and value < limit):
                raise ValueError(f"{operation}.{field}={value} violates {key}={limit}")


def validate_metal_telemetry(benchmarks: dict[str, Any]) -> None:
    """Require real dispatches, queue headroom and bounded LDE zero-fill costs."""
    queue = object_value(benchmarks.get("metal_dispatch_queue"), "metal_dispatch_queue")
    limit = integer(queue.get("limit"), "queue.limit", minimum=1)
    maximum = integer(queue.get("max_in_flight"), "queue.max_in_flight", minimum=1)
    integer(queue.get("dispatch_count"), "queue.dispatch_count", minimum=1)
    if limit - maximum < 1:
        raise ValueError("Metal queue headroom must be at least one slot")
    hotspots = benchmarks.get("zero_fill_hotspots")
    if not isinstance(hotspots, list) or not hotspots:
        raise ValueError("missing zero_fill_hotspots telemetry")
    lde_entries = [entry for entry in hotspots if isinstance(entry, dict) and entry.get("operation") == "lde"]
    if not lde_entries:
        raise ValueError("zero_fill_hotspots lacks LDE entries")
    for entry in lde_entries:
        mean = number(entry.get("mean_ms"), "LDE zero-fill mean_ms")
        if mean > 0.40:
            raise ValueError("LDE zero-fill mean_ms exceeds 0.40 ms")


def validate_manifest(manifest_path: Path, repo_root: Path) -> None:
    """Reject mismatched captures, CPU fallbacks and unenforced release limits."""
    signed = load_json(manifest_path.read_bytes())
    payload = object_value(signed.get("payload"), "manifest.payload")
    if type(payload.get("version")) is not int or payload["version"] != 1:
        raise ValueError("unexpected manifest version")
    constraints = object_value(payload.get("constraints"), "constraints")
    required_rows = integer(constraints.get("require_rows"), "require_rows", minimum=20_000)
    maximum = object_value(constraints.get("max_operation_ms"), "max_operation_ms")
    minimum = object_value(constraints.get("min_operation_speedup"), "min_operation_speedup")
    if number(maximum.get("lde"), "LDE limit", positive=True) > 950:
        raise ValueError("manifest must enforce max-operation-ms lde=950 or stricter")
    if number(minimum.get("fft"), "FFT speedup limit", positive=True) < 1:
        raise ValueError("manifest must enforce min-operation-speedup fft=1 or stricter")
    benches = payload.get("benches")
    if not isinstance(benches, list) or len(benches) < 2:
        raise ValueError("manifest must contain Metal and CUDA captures")
    labels = set()
    for raw in benches:
        bench = object_value(raw, "bench")
        label = bench.get("label")
        if not isinstance(label, str) or not label.strip() or label in labels:
            raise ValueError("bench labels must be unique non-empty strings")
        labels.add(label)
        raw_path = bench.get("path")
        if not isinstance(raw_path, str) or not raw_path.strip():
            raise ValueError(f"bench {label} missing path")
        bench_path = Path(raw_path)
        if not bench_path.is_absolute():
            bench_path = repo_root / bench_path
        content = bench_path.read_bytes()
        hashes = object_value(bench.get("hashes"), f"{label}.hashes")
        for name, digest in (
            ("sha256_hex", hashlib.sha256(content).hexdigest()),
            ("blake3_hex", blake3.blake3(content).hexdigest()),
        ):
            if hashes.get(name) != digest:
                raise ValueError(f"{label}.{name} does not match captured benchmark bytes")
        capture = load_json(content)
        metadata = object_value(capture.get("metadata"), "metadata")
        device_labels = object_value(metadata.get("labels"), "metadata.labels")
        for key in ("device_class", "gpu_kind"):
            if not isinstance(device_labels.get(key), str) or not device_labels[key].strip():
                raise ValueError(f"metadata.labels.{key} missing or empty")
        benchmarks = object_value(capture.get("benchmarks"), "benchmarks")
        rows = integer(benchmarks.get("rows"), "benchmarks.rows", minimum=required_rows)
        padded = integer(benchmarks.get("padded_rows"), "padded_rows", minimum=rows)
        if padded != 1 << (rows - 1).bit_length():
            raise ValueError("padded_rows must be the next power of two of rows")
        integer(benchmarks.get("iterations"), "iterations", minimum=1)
        integer(benchmarks.get("warmups"), "warmups")
        for key in ("rows", "padded_rows", "iterations", "warmups", "gpu_backend", "gpu_available"):
            if type(bench.get(key)) is not type(benchmarks.get(key)) or bench.get(key) != benchmarks.get(key):
                raise ValueError(f"{label}.{key} disagrees with captured benchmark")
        backend = benchmarks.get("gpu_backend")
        if benchmarks.get("gpu_available") is not True or backend not in {"metal", "cuda"}:
            raise ValueError(f"{label} must record an available Metal/CUDA GPU")
        if label in {"metal", "cuda"} and backend != label:
            raise ValueError(f"{label} capture used the wrong GPU backend")
        validate_operations(benchmarks, constraints)
        if backend == "metal":
            validate_metal_telemetry(benchmarks)
    if not {"metal", "cuda"}.issubset(labels):
        raise ValueError("manifest missing required Metal/CUDA bench labels")


def main(argv: Sequence[str] | None = None) -> int:
    """Run the read-only rollout consistency gate."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args(argv)
    try:
        validate_manifest(args.manifest, args.repo_root)
    except (OSError, ValueError) as error:
        parser.exit(1, f"[fastpq] {args.manifest}: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
