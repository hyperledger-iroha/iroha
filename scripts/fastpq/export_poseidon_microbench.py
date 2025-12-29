#!/usr/bin/env python3
"""Extract the Poseidon microbench summary from a wrapped FASTPQ bundle."""
from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--bundle",
        required=True,
        type=Path,
        help="Path to the wrapped fastpq_*_bench_*.json bundle.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Destination path for the extracted summary "
        "(defaults to benchmarks/poseidon/poseidon_microbench_<timestamp>.json).",
    )
    return parser.parse_args()


def default_output_path(bundle_path: Path, metadata: dict[str, Any]) -> Path:
    base_dir = Path("benchmarks/poseidon")
    base_dir.mkdir(parents=True, exist_ok=True)
    generated = metadata.get("generated_at")
    if isinstance(generated, str):
        ts = generated.replace(":", "").replace("+", "").replace("Z", "")
    else:
        ts = datetime.now().isoformat().replace(":", "").split(".")[0]
    stem = bundle_path.stem
    filename = f"poseidon_microbench_{ts}_{stem}.json"
    return base_dir / filename


def iso_timestamp(epoch_seconds: int | None) -> str | None:
    if epoch_seconds is None:
        return None
    return (
        datetime.fromtimestamp(epoch_seconds, tz=timezone.utc)
        .isoformat()
        .replace("+00:00", "Z")
    )


def main() -> None:
    args = parse_args()
    bundle = json.loads(args.bundle.read_text())
    benchmarks = bundle.get("benchmarks")
    summary = None
    source = "benchmarks.poseidon_microbench"
    if isinstance(benchmarks, dict):
        summary = benchmarks.get("poseidon_microbench")
    if summary is None and isinstance(bundle.get("report"), dict):
        summary = bundle["report"].get("poseidon_microbench")
        source = "report.poseidon_microbench"
    if summary is None:
        summary = bundle.get("poseidon_microbench")
        source = "poseidon_microbench"
    if not isinstance(summary, dict):
        raise SystemExit(
            "Poseidon microbench summary missing in bundle; ensure wrap_benchmark.py "
            "was run with the latest sources and that the capture included Poseidon telemetry "
            "(checked {})".format(source)
        )
    metadata = bundle.get("metadata") or {}
    report = bundle.get("report") or {}
    raw_gpu_backend = bundle.get("gpu_backend")
    raw_execution_mode = bundle.get("execution_mode")
    raw_timestamp = iso_timestamp(bundle.get("unix_epoch_secs"))
    payload = {
        "bundle": str(args.bundle),
        "generated_at": metadata.get("generated_at"),
        "host": metadata.get("host") or metadata.get("labels", {}).get("host"),
        "labels": metadata.get("labels"),
        "execution_mode": (
            (benchmarks or {}).get("execution_mode")
            or report.get("execution_mode")
            or raw_execution_mode
        ),
        "gpu_backend": (
            (benchmarks or {}).get("gpu_backend")
            or report.get("gpu_backend")
            or raw_gpu_backend
        ),
        "capture_timestamp": metadata.get("generated_at") or raw_timestamp,
        "poseidon_microbench": summary,
    }
    output_path = args.output or default_output_path(args.bundle, metadata)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"Poseidon microbench summary written to {output_path}")


if __name__ == "__main__":
    main()
