#!/usr/bin/env python3
"""Export benchmark JSON artefacts to CSV for automation dashboards."""

from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path
from typing import Iterable, List, Sequence


POSEIDON_PREFIX = "poseidon_microbench"
POSEIDON_COLUMNS: Sequence[str] = (
    "bundle",
    "capture_timestamp",
    "execution_mode",
    "gpu_backend",
    "host",
    "labels",
    "mode",
    "mean_ms",
    "min_ms",
    "max_ms",
    "columns",
    "states",
    "iterations",
    "trace_log2",
    "warmups",
    "speedup_vs_scalar",
)

MERKLE_COLUMNS: Sequence[str] = (
    "profile",
    "metal_available",
    "leaves",
    "cpu_ms",
    "metal_ms",
    "speedup",
)


def export_poseidon_csv(directory: Path) -> List[Path]:
    """Convert Poseidon microbench JSON exports into CSV summaries."""

    csv_paths: List[Path] = []
    if not directory.exists():
        return csv_paths

    for json_path in sorted(directory.glob(f"{POSEIDON_PREFIX}*.json")):
        if json_path.name == "manifest.json":
            continue

        data = _read_json(json_path)
        microbench = data.get("poseidon_microbench")
        if not isinstance(microbench, dict):
            continue

        rows = _build_poseidon_rows(data, microbench)
        if not rows:
            continue

        csv_path = json_path.with_suffix(".csv")
        _write_csv(csv_path, POSEIDON_COLUMNS, rows)
        csv_paths.append(csv_path)

    return csv_paths


def export_merkle_csv(directory: Path) -> List[Path]:
    """Convert Merkle threshold JSON exports into CSV summaries."""

    csv_paths: List[Path] = []
    if not directory.exists():
        return csv_paths

    for json_path in sorted(directory.glob("*.json")):
        data = _read_json(json_path)
        rows = data.get("rows")
        if not isinstance(rows, list):
            continue

        metal_available = bool(data.get("metal_available"))
        profile = json_path.stem
        formatted_rows = [
            {
                "profile": profile,
                "metal_available": str(metal_available).lower(),
                "leaves": entry.get("leaves"),
                "cpu_ms": entry.get("cpu_ms"),
                "metal_ms": entry.get("metal_ms"),
                "speedup": entry.get("speedup"),
            }
            for entry in rows
            if isinstance(entry, dict)
        ]
        if not formatted_rows:
            continue

        csv_path = json_path.with_suffix(".csv")
        _write_csv(csv_path, MERKLE_COLUMNS, formatted_rows)
        csv_paths.append(csv_path)

    return csv_paths


def _read_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def _write_csv(path: Path, columns: Sequence[str], rows: Iterable[dict]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(columns))
        writer.writeheader()
        for row in rows:
            writer.writerow({column: _format_value(row.get(column)) for column in columns})


def _build_poseidon_rows(data: dict, microbench: dict) -> List[dict]:
    metadata = {
        "bundle": data.get("bundle"),
        "capture_timestamp": data.get("capture_timestamp"),
        "execution_mode": data.get("execution_mode"),
        "gpu_backend": data.get("gpu_backend"),
        "host": data.get("host"),
        "labels": json.dumps(data.get("labels"), sort_keys=True) if data.get("labels") else "",
    }
    result: List[dict] = []
    for key in ("default", "scalar_lane"):
        lane = microbench.get(key)
        if not isinstance(lane, dict):
            continue

        row = {**metadata}
        row.update(
            {
                "mode": lane.get("mode"),
                "mean_ms": lane.get("mean_ms"),
                "min_ms": lane.get("min_ms"),
                "max_ms": lane.get("max_ms"),
                "columns": lane.get("columns"),
                "states": lane.get("states"),
                "iterations": lane.get("iterations"),
                "trace_log2": lane.get("trace_log2"),
                "warmups": lane.get("warmups"),
                "speedup_vs_scalar": microbench.get("speedup_vs_scalar")
                if key == "default"
                else None,
            }
        )
        result.append(row)

    return result


def _format_value(value: object) -> object:
    if value is None:
        return ""
    if isinstance(value, bool):
        return str(value).lower()
    return value


def main(argv: Sequence[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--poseidon-dir",
        type=Path,
        default=Path("benchmarks/poseidon"),
        help="Directory that contains poseidon_microbench JSON exports.",
    )
    parser.add_argument(
        "--merkle-dir",
        type=Path,
        default=Path("benchmarks/merkle_threshold"),
        help="Directory that contains Merkle threshold JSON captures.",
    )
    args = parser.parse_args(argv)

    poseidon_paths = export_poseidon_csv(args.poseidon_dir)
    merkle_paths = export_merkle_csv(args.merkle_dir)

    generated = poseidon_paths + merkle_paths
    if generated:
        for path in generated:
            print(f"wrote {path}")
    else:
        print("no benchmark CSV outputs generated")


if __name__ == "__main__":
    main()
