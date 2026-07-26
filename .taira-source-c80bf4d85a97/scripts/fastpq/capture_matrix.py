#!/usr/bin/env python3
"""Build the FASTPQ cross-device benchmark matrix manifest."""
from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
ACCEL_HELPERS_DIR = SCRIPT_DIR.parent / "acceleration"
if ACCEL_HELPERS_DIR.is_dir():
    sys.path.append(str(ACCEL_HELPERS_DIR))
try:
    import acceleration_matrix  # type: ignore
except Exception:
    acceleration_matrix = None


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def default_matrix_dir(root: Path) -> Path:
    return root / "artifacts" / "fastpq_benchmarks" / "matrix"


@dataclass
class OperationSummary:
    samples: list[float] = field(default_factory=list)
    speedups: list[float] = field(default_factory=list)

    def add(self, gpu_mean_ms: float | None, speedup_ratio: float | None) -> None:
        if gpu_mean_ms is not None:
            self.samples.append(gpu_mean_ms)
        if speedup_ratio is not None:
            self.speedups.append(speedup_ratio)

    def median_ms(self) -> float | None:
        if not self.samples:
            return None
        return round(statistics.median(sorted(self.samples)), 3)

    def median_speedup(self) -> float | None:
        if not self.speedups:
            return None
        return round(statistics.median(sorted(self.speedups)), 3)

    def sample_count(self) -> int:
        return len(self.samples)


@dataclass
class DeviceSummary:
    label: str
    backend: str | None
    platforms: set[str]
    machines: set[str]
    rows_seen: list[int]
    padded_rows: set[int]
    bench_files: list[Path]
    operation_filters: set[str]
    operations: dict[str, OperationSummary]
    acceleration_state: dict | None

    def to_manifest_entry(
        self,
        repo_root: Path,
        max_ms_slack: float,
        min_speedup_slack: float,
    ) -> dict:
        def rel(path: Path) -> str:
            try:
                return str(path.relative_to(repo_root))
            except ValueError:
                return str(path)

        op_entries = {}
        max_operation_ms: dict[str, float] = {}
        min_operation_speedup: dict[str, float] = {}
        for name, summary in sorted(self.operations.items()):
            median_ms = summary.median_ms()
            median_speedup = summary.median_speedup()
            op_entry = {
                "sample_count": summary.sample_count(),
                "median_gpu_mean_ms": median_ms,
                "median_speedup_ratio": median_speedup,
            }
            op_entries[name] = op_entry
            if median_ms is not None:
                max_operation_ms[name] = round(
                    median_ms * (1.0 + max_ms_slack / 100.0),
                    3,
                )
            if median_speedup is not None and median_speedup > 0.0:
                if median_speedup > 1.0:
                    min_operation_speedup[name] = round(
                        median_speedup * (1.0 - min_speedup_slack / 100.0),
                        3,
                    )
        rows_block = {
            "min": min(self.rows_seen) if self.rows_seen else None,
            "max": max(self.rows_seen) if self.rows_seen else None,
            "padded": sorted(self.padded_rows),
        }
        return {
            "label": self.label,
            "backend": self.backend,
            "platforms": sorted(self.platforms),
            "machines": sorted(self.machines),
            "rows": rows_block,
            "bench_files": [rel(path) for path in sorted(self.bench_files)],
            "operation_filters": sorted(self.operation_filters),
            "operations": op_entries,
            "max_operation_ms": max_operation_ms,
            "min_operation_speedup": min_operation_speedup,
            "acceleration_state": self.acceleration_state,
        }


def parse_args() -> argparse.Namespace:
    root = repo_root()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=root,
        help="Repository root (auto-detected by default).",
    )
    parser.add_argument(
        "--matrix-dir",
        type=Path,
        default=default_matrix_dir(root),
        help="Directory containing matrix device lists and manifest output.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Manifest output path (defaults to <matrix-dir>/matrix_manifest.json).",
    )
    parser.add_argument(
        "--accel-matrix-json",
        type=Path,
        help="Optional path for the acceleration-state matrix JSON (default: <matrix-dir>/acceleration_matrix.json).",
    )
    parser.add_argument(
        "--accel-matrix-markdown",
        type=Path,
        help="Optional path for the acceleration-state Markdown table (default: <matrix-dir>/acceleration_matrix.md).",
    )
    parser.add_argument(
        "--skip-acceleration-matrix",
        action="store_true",
        help="Disable generation of the acceleration-state matrix even when acceleration states are present.",
    )
    parser.add_argument(
        "--device",
        action="append",
        dest="devices",
        help="Specific device label to include (defaults to every entry under matrix-dir/devices).",
    )
    parser.add_argument(
        "--require-rows",
        type=int,
        default=20000,
        help="Row floor enforced by the manifest.",
    )
    parser.add_argument(
        "--max-ms-slack",
        type=float,
        default=5.0,
        help="Percent slack applied to per-operation medians when deriving max_operation_ms.",
    )
    parser.add_argument(
        "--min-speedup-slack",
        type=float,
        default=5.0,
        help="Percent slack applied to median speedups when deriving min_operation_speedup.",
    )
    return parser.parse_args()


def read_device_paths(list_path: Path, repo_root: Path) -> list[Path]:
    if not list_path.is_file():
        raise FileNotFoundError(f"device list not found: {list_path}")
    bench_paths: list[Path] = []
    for raw_line in list_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        candidate = Path(line)
        if not candidate.is_absolute():
            candidate = (repo_root / candidate).resolve()
        bench_paths.append(candidate)
    if not bench_paths:
        raise ValueError(f"{list_path} contained no bench paths")
    return bench_paths


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def load_acceleration_state_from_bench(bench_path: Path, data: dict) -> dict | None:
    meta = data.get("metadata") or {}
    state = meta.get("acceleration_state")
    if isinstance(state, dict):
        return state
    companion = bench_path.with_suffix(bench_path.suffix + ".accel.json")
    if companion.is_file():
        try:
            return json.loads(companion.read_text(encoding="utf-8"))
        except json.JSONDecodeError as exc:
            print(f"[fastpq] invalid acceleration-state JSON at {companion}: {exc}", file=sys.stderr)
    return None


def format_operation_filter(benchmarks: dict) -> str | None:
    raw = benchmarks.get("operation_filter")
    if isinstance(raw, str) and raw.strip():
        return raw
    operations = benchmarks.get("operations")
    if not isinstance(operations, list):
        return None
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


def summarize_device(label: str, bench_paths: Sequence[Path]) -> DeviceSummary:
    backend = None
    platforms: set[str] = set()
    machines: set[str] = set()
    rows_seen: list[int] = []
    padded_rows: set[int] = set()
    operation_filters: set[str] = set()
    operations: dict[str, OperationSummary] = {}
    accel_state: dict | None = None

    for bench_path in bench_paths:
        data = load_json(bench_path)
        meta = data.get("metadata") or {}
        benchmarks = data.get("benchmarks") or {}
        backend_entry = benchmarks.get("gpu_backend")
        if backend_entry:
            backend = backend_entry
        platform = meta.get("platform")
        if platform:
            platforms.add(platform)
        machine = meta.get("machine")
        if machine:
            machines.add(machine)
        rows = benchmarks.get("rows")
        if isinstance(rows, int):
            rows_seen.append(rows)
        padded = benchmarks.get("padded_rows")
        if isinstance(padded, int):
            padded_rows.add(padded)
        operation_filter = format_operation_filter(benchmarks)
        if operation_filter:
            operation_filters.add(operation_filter)
        for entry in benchmarks.get("operations", []):
            name = entry.get("operation")
            if not isinstance(name, str):
                continue
            gpu_mean_ms = entry.get("gpu_mean_ms")
            if gpu_mean_ms is not None:
                gpu_mean_ms = float(gpu_mean_ms)
            speedup_ratio = entry.get("speedup_ratio")
            if speedup_ratio is not None:
                speedup_ratio = float(speedup_ratio)
            operations.setdefault(name, OperationSummary()).add(
                gpu_mean_ms,
                speedup_ratio,
            )
        if accel_state is None:
            accel_state = load_acceleration_state_from_bench(bench_path, data)

    return DeviceSummary(
        label=label,
        backend=backend,
        platforms=platforms,
        machines=machines,
        rows_seen=rows_seen,
        padded_rows=padded_rows,
        bench_files=list(bench_paths),
        operation_filters=operation_filters,
        operations=operations,
        acceleration_state=accel_state,
    )


def build_global_limits(
    devices: Iterable[dict],
) -> tuple[dict[str, float], dict[str, float]]:
    global_max: dict[str, float] = {}
    global_min_speedup: dict[str, float] = {}
    for device in devices:
        for op, value in device.get("max_operation_ms", {}).items():
            current = global_max.get(op)
            if current is None or value > current:
                global_max[op] = value
        for op, value in device.get("min_operation_speedup", {}).items():
            current = global_min_speedup.get(op)
            if current is None or value < current:
                global_min_speedup[op] = value
    return global_max, global_min_speedup


def iso_timestamp() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def write_acceleration_matrix(
    states: list[tuple[str, dict]],
    json_out: Path | None,
    markdown_out: Path | None,
) -> None:
    if not states or acceleration_matrix is None:
        return
    matrix = acceleration_matrix.build_matrix(states)
    if json_out:
        json_out.parent.mkdir(parents=True, exist_ok=True)
        json_out.write_text(acceleration_matrix.render_json(matrix), encoding="utf-8")
    if markdown_out:
        markdown_out.parent.mkdir(parents=True, exist_ok=True)
        markdown_out.write_text(acceleration_matrix.render_markdown(matrix), encoding="utf-8")


def regenerate_history(repo_root: Path) -> None:
    script = repo_root / "scripts" / "fastpq" / "update_benchmark_history.py"
    if not script.is_file():
        print(f"[fastpq] history script not found at {script}", file=sys.stderr)
        return
    try:
        subprocess.run(
            ["python3", str(script)],
            check=True,
            cwd=str(repo_root),
        )
        rel = str(script.relative_to(repo_root))
        print(f"[fastpq] refreshed GPU benchmark history via {rel}")
    except subprocess.CalledProcessError as exc:
        print(
            f"[fastpq] warning: failed to regenerate benchmark history ({exc})",
            file=sys.stderr,
        )


def main() -> None:
    args = parse_args()
    repo = args.repo_root.resolve()
    matrix_dir = args.matrix_dir.resolve()
    devices_dir = matrix_dir / "devices"
    if not devices_dir.is_dir():
        raise SystemExit(f"matrix devices directory not found: {devices_dir}")

    if args.devices:
        labels = args.devices
    else:
        labels = sorted(path.stem for path in devices_dir.glob("*.txt"))
    if not labels:
        raise SystemExit(f"no device definitions found under {devices_dir}")

    max_ms_slack = args.max_ms_slack
    min_speedup_slack = args.min_speedup_slack

    manifest_devices = []
    source_lists = {}
    acceleration_states: list[tuple[str, dict]] = []
    for label in labels:
        list_path = devices_dir / f"{label}.txt"
        bench_paths = read_device_paths(list_path, repo)
        summary = summarize_device(label, bench_paths)
        manifest_entry = summary.to_manifest_entry(
            repo,
            max_ms_slack=max_ms_slack,
            min_speedup_slack=min_speedup_slack,
        )
        manifest_devices.append(manifest_entry)
        if summary.acceleration_state:
            acceleration_states.append((label, summary.acceleration_state))
        try:
            source_lists[label] = str(list_path.relative_to(repo))
        except ValueError:
            source_lists[label] = str(list_path)

    manifest_devices.sort(key=lambda entry: entry["label"])
    global_max, global_min_speedup = build_global_limits(manifest_devices)

    output_path = (
        args.output.resolve()
        if args.output
        else (matrix_dir / "matrix_manifest.json").resolve()
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    manifest = {
        "version": 1,
        "generated_at": iso_timestamp(),
        "require_rows": args.require_rows,
        "slack": {
            "max_operation_ms_pct": max_ms_slack,
            "min_operation_speedup_pct": min_speedup_slack,
        },
        "devices": manifest_devices,
        "max_operation_ms": global_max,
        "min_operation_speedup": global_min_speedup,
        "source_lists": source_lists,
    }
    output_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    try:
        rel_output = str(output_path.relative_to(repo))
    except ValueError:
        rel_output = str(output_path)
    print(f"[fastpq] wrote matrix manifest to {rel_output}")
    if acceleration_states and not args.skip_acceleration_matrix:
        accel_json_out = (
            args.accel_matrix_json.resolve()
            if args.accel_matrix_json
            else (matrix_dir / "acceleration_matrix.json").resolve()
        )
        accel_md_out = (
            args.accel_matrix_markdown.resolve()
            if args.accel_matrix_markdown
            else (matrix_dir / "acceleration_matrix.md").resolve()
        )
        write_acceleration_matrix(acceleration_states, accel_json_out, accel_md_out)
        try:
            rel_json = str(accel_json_out.relative_to(repo))
        except ValueError:
            rel_json = str(accel_json_out)
        print(f"[fastpq] wrote acceleration matrix to {rel_json}")
    regenerate_history(repo)


if __name__ == "__main__":
    main()
