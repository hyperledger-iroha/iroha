#!/usr/bin/env python3
"""Aggregate Poseidon microbench summaries into a single manifest."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--input",
        type=Path,
        default=Path("benchmarks/poseidon"),
        help="Directory containing poseidon_microbench_*.json files (default: benchmarks/poseidon)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("benchmarks/poseidon/manifest.json"),
        help="Destination path for the aggregated manifest (default: benchmarks/poseidon/manifest.json)",
    )
    return parser.parse_args()


def normalize_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(Path.cwd().resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def summarize_entry(path: Path) -> dict[str, Any] | None:
    try:
        data = json.loads(path.read_text())
    except Exception as err:  # pragma: no cover - aid debugging
        raise SystemExit(f"failed to read {path}: {err}") from err

    summary = data.get("poseidon_microbench")
    if not isinstance(summary, dict):
        return None

    def sample_mean(key: str) -> float | None:
        sample = summary.get(key)
        if isinstance(sample, dict):
            value = sample.get("mean_ms")
            if isinstance(value, (int, float)):
                return float(value)
        return None

    return {
        "file": normalize_path(path),
        "bundle": data.get("bundle"),
        "capture_timestamp": data.get("capture_timestamp") or data.get("generated_at"),
        "execution_mode": data.get("execution_mode"),
        "gpu_backend": data.get("gpu_backend"),
        "host": data.get("host"),
        "default_mean_ms": sample_mean("default"),
        "scalar_mean_ms": sample_mean("scalar_lane"),
        "speedup_vs_scalar": summary.get("speedup_vs_scalar"),
    }


def main() -> None:
    args = parse_args()
    entries = []
    if not args.input.exists():
        raise SystemExit(f"input directory {args.input} does not exist")

    for path in sorted(args.input.glob("poseidon_microbench*.json")):
        record = summarize_entry(path)
        if record:
            entries.append(record)

    entries.sort(key=lambda item: item.get("capture_timestamp") or "", reverse=True)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({"entries": entries}, indent=2) + "\n")
    print(f"Poseidon microbench manifest written to {args.output}")


if __name__ == "__main__":
    main()
