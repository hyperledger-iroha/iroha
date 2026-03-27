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


def format_operation_filter(
    benchmarks: dict[str, Any] | None,
    report: dict[str, Any] | None,
    bundle: dict[str, Any],
) -> str | None:
    for source in (benchmarks, report, bundle):
        if not isinstance(source, dict):
            continue
        raw = source.get("operation_filter")
        if isinstance(raw, str) and raw.strip():
            return raw
    for source in (benchmarks, report, bundle):
        if not isinstance(source, dict):
            continue
        operations = source.get("operations")
        if not isinstance(operations, list):
            continue
        names = sorted(
            {
                entry.get("operation")
                for entry in operations
                if isinstance(entry, dict) and isinstance(entry.get("operation"), str)
            }
        )
        if len(names) == 1:
            return names[0]
        if names:
            return "all"
    return None


def build_payload(bundle_path: Path, bundle: dict[str, Any], summary: dict[str, Any]) -> dict[str, Any]:
    metadata = bundle.get("metadata") or {}
    labels = metadata.get("labels")
    if not isinstance(labels, dict):
        labels = None
    benchmarks = bundle.get("benchmarks") or {}
    report = bundle.get("report") or {}
    raw_gpu_backend = bundle.get("gpu_backend")
    raw_execution_mode = bundle.get("execution_mode")
    raw_timestamp = iso_timestamp(bundle.get("unix_epoch_secs"))
    return {
        "bundle": str(bundle_path),
        "generated_at": metadata.get("generated_at"),
        "host": metadata.get("host") or (labels or {}).get("host"),
        "labels": labels,
        "execution_mode": (
            benchmarks.get("execution_mode")
            or report.get("execution_mode")
            or raw_execution_mode
        ),
        "gpu_backend": (
            benchmarks.get("gpu_backend")
            or report.get("gpu_backend")
            or raw_gpu_backend
        ),
        "capture_timestamp": metadata.get("generated_at") or raw_timestamp,
        "operation_filter": format_operation_filter(benchmarks, report, bundle),
        "column_count": benchmarks.get("column_count") or report.get("column_count"),
        "source_command": metadata.get("command"),
        "poseidon_microbench": summary,
    }


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
    payload = build_payload(args.bundle, bundle, summary)
    output_path = args.output or default_output_path(args.bundle, metadata)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"Poseidon microbench summary written to {output_path}")


if __name__ == "__main__":
    main()
