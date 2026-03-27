#!/usr/bin/env python3
"""Summarise stage-specific FASTPQ Metal benchmark JSON outputs."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Optional


@dataclass
class StageSample:
    path: Path
    bundle_filter: str
    operation: str
    gpu_ms: Optional[float]
    cpu_ms: Optional[float]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", nargs="+", help="Paths to fastpq_metal_bench JSON reports")
    parser.add_argument(
        "--operation",
        choices=["fft", "ifft", "lde", "poseidon_hash_columns"],
        default=None,
        help="Filter by operation (default: include all operations)",
    )
    return parser.parse_args()


def load_samples(paths: Iterable[str], operation_filter: Optional[str]) -> List[StageSample]:
    samples: List[StageSample] = []
    for raw_path in paths:
        path = Path(raw_path)
        data = normalize_report(json.loads(path.read_text()))
        bundle_filter = format_operation_filter(data)
        operations = data.get("operations") or []
        for op in operations:
            name = op.get("operation")
            if not isinstance(name, str):
                continue
            if operation_filter and name != operation_filter:
                continue
            gpu_mean = (op.get("gpu") or {}).get("mean_ms")
            cpu_mean = (op.get("cpu") or {}).get("mean_ms")
            samples.append(
                StageSample(
                    path=path,
                    bundle_filter=bundle_filter,
                    operation=name,
                    gpu_ms=float(gpu_mean) if gpu_mean is not None else None,
                    cpu_ms=float(cpu_mean) if cpu_mean is not None else None,
                )
            )
    return samples


def normalize_report(payload: dict) -> dict:
    nested = payload.get("report")
    if not isinstance(nested, dict):
        return payload
    report = dict(nested)
    benchmarks = payload.get("benchmarks")
    if isinstance(benchmarks, dict):
        if report.get("operation_filter") is None and benchmarks.get("operation_filter") is not None:
            report["operation_filter"] = benchmarks.get("operation_filter")
        if "operations" not in report and benchmarks.get("operations") is not None:
            report["operations"] = benchmarks.get("operations")
    return report


def format_operation_filter(report: dict) -> str:
    raw = report.get("operation_filter")
    if isinstance(raw, str) and raw.strip():
        return raw
    operations = report.get("operations")
    if isinstance(operations, list) and len(operations) == 1:
        operation = operations[0].get("operation")
        if isinstance(operation, str) and operation:
            return operation
    if isinstance(operations, list) and operations:
        return "all"
    return "—"


def render_table(samples: List[StageSample]) -> str:
    header = "| Report | Filter | Operation | GPU mean (ms) | CPU mean (ms) | Speedup |"
    divider = "|--------|--------|-----------|---------------|---------------|---------|"
    lines = [header, divider]
    for sample in samples:
        gpu = f"{sample.gpu_ms:.3f}" if sample.gpu_ms is not None else "n/a"
        cpu = f"{sample.cpu_ms:.3f}" if sample.cpu_ms is not None else "n/a"
        if sample.gpu_ms is not None and sample.cpu_ms is not None and sample.gpu_ms > 0:
            ratio = sample.cpu_ms / sample.gpu_ms
            speedup = f"{ratio:.3f}"
        else:
            speedup = "n/a"
        lines.append(
            f"| `{sample.path.name}` | {sample.bundle_filter} | {sample.operation} | {gpu} | {cpu} | {speedup} |"
        )
    return "\n".join(lines)


def main() -> None:
    args = parse_args()
    samples = load_samples(args.reports, args.operation)
    if not samples:
        raise SystemExit("no matching operations found in provided reports")
    print(render_table(samples))


if __name__ == "__main__":
    main()
