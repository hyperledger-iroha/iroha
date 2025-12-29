#!/usr/bin/env python3
"""Regenerate the Grafana benchmark panel from a wrapped FASTPQ bundle."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Sequence


OP_LABELS = {
    "fft": "FFT",
    "ifft": "IFFT",
    "lde": "LDE",
    "poseidon_hash_columns": "Poseidon columns",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--bundle",
        required=True,
        type=Path,
        help="Path to the wrapped fastpq_*_bench_*.json bundle.",
    )
    parser.add_argument(
        "--dashboard",
        required=True,
        type=Path,
        help="Grafana dashboard JSON to update (e.g. dashboards/grafana/fastpq_acceleration.json).",
    )
    parser.add_argument(
        "--panel-title",
        default="Latest Benchmark",
        help="Panel title to replace; defaults to 'Latest Benchmark'.",
    )
    return parser.parse_args()


def format_float(value: float | None) -> str:
    if value is None:
        return "n/a"
    return f"{value:.2f}"


def format_operation_entry(entry: dict[str, Any]) -> str:
    name = OP_LABELS.get(entry.get("operation"), entry.get("operation", "unknown"))
    cpu = format_float(entry.get("cpu_mean_ms"))
    gpu = format_float(entry.get("gpu_mean_ms"))
    ratio = entry.get("speedup_ratio")
    ratio_str = f"x{ratio:.2f}" if isinstance(ratio, (int, float)) else "n/a"
    return f"{name} {cpu} / {gpu} ({ratio_str})"


def summarize_operations(entries: Sequence[dict[str, Any]] | None) -> str:
    if not entries:
        return "Mean timings unavailable."
    rendered = [format_operation_entry(entry) for entry in entries]
    return "Mean timings (ms): " + ", ".join(rendered)


def summarize_device(labels: dict[str, Any] | None) -> str | None:
    if not labels:
        return None
    device = labels.get("device_class")
    chip = labels.get("chip_type")
    gpu = labels.get("gpu_model")
    gpu_kind = labels.get("gpu_kind")
    cores = labels.get("gpu_cores")
    parts = []
    if chip:
        parts.append(f"chip `{chip}`")
    if gpu:
        suffix = f", {cores} cores" if cores else ""
        kind = f" ({gpu_kind})" if gpu_kind else ""
        parts.append(f"GPU `{gpu}`{kind}{suffix}")
    detail = "; ".join(parts)
    if device and detail:
        return f"`{device}` ({detail})"
    if device:
        return f"`{device}`"
    return detail or None


def summarize_queue(queue: dict[str, Any] | None) -> str | None:
    if not queue:
        return None
    parts: list[str] = []
    for key in ("limit", "max_in_flight", "dispatch_count", "busy_ms", "overlap_ms", "overlap_ratio"):
        value = queue.get(key)
        if value is None:
            continue
        label = key.replace("_", " ")
        if isinstance(value, float):
            parts.append(f"{label}={value:.2f}")
        else:
            parts.append(f"{label}={value}")
    return ", ".join(parts) if parts else None


def summarize_zero_fill(hotspots: list[dict[str, Any]] | None) -> str:
    if not hotspots:
        return "Zero-fill telemetry: sampler disabled on this capture; Stage 7 SLO allows <=0.40 ms mean when enabled."
    entry = hotspots[0]
    mean_ms = entry.get("mean_ms")
    bandwidth = entry.get("bandwidth_gbps")
    return (
        f"Zero-fill telemetry: mean {mean_ms:.3f} ms (bandwidth {bandwidth:.2f} GB/s) "
        "on the hottest block; Stage 7 SLO requires <=0.40 ms mean."
    )


def summarize_column_staging(entry: dict[str, Any] | None) -> str | None:
    if not entry:
        return None
    batches = entry.get("batches")
    flatten_ms = entry.get("flatten_ms")
    wait_ms = entry.get("wait_ms")
    wait_ratio = entry.get("wait_ratio")
    if batches is None or flatten_ms is None or wait_ms is None or wait_ratio is None:
        return None
    return (
        "Column staging overlap: "
        f"{batches} batches, flatten {flatten_ms:.3f} ms, wait {wait_ms:.3f} ms "
        f"(wait ratio {wait_ratio:.3f})."
    )


def summarize_poseidon_microbench(entry: dict[str, Any] | None) -> str | None:
    if not entry:
        return None

    def sample_string(name: str, sample: dict[str, Any]) -> str:
        mean_ms = sample.get("mean_ms")
        columns = sample.get("columns")
        trace_log = sample.get("trace_log2")
        tuning = sample.get("tuning") or {}
        lanes = tuning.get("threadgroup_lanes")
        batch = tuning.get("states_per_lane")
        parts = [f"{name} {mean_ms:.3f} ms" if mean_ms is not None else f"{name} n/a"]
        if columns and trace_log:
            parts.append(f"{columns} cols @ 2^{trace_log}")
        if lanes and batch:
            parts.append(f"{lanes} lanes x {batch}")
        return " (" .join(parts).replace(" (", " (") if len(parts) > 1 else parts[0]

    default = entry.get("default")
    scalar = entry.get("scalar_lane")
    if not isinstance(default, dict) or not isinstance(scalar, dict):
        return None
    speedup = entry.get("speedup_vs_scalar")
    pieces = [
        sample_string("default", default),
        sample_string("scalar", scalar),
    ]
    if isinstance(speedup, (int, float)):
        pieces.append(f"speedup x{speedup:.3f}")
    return "Poseidon microbench: " + "; ".join(pieces)


def relative_bundle_path(bundle_path: Path) -> str:
    try:
        return bundle_path.resolve().relative_to(Path.cwd().resolve()).as_posix()
    except ValueError:
        return bundle_path.as_posix()


def build_markdown(bundle_path: Path, bundle: dict[str, Any]) -> str:
    metadata = bundle.get("metadata", {})
    benchmarks = bundle.get("benchmarks", {})
    generated_at = metadata.get("generated_at", "")
    date = generated_at.split("T")[0] if isinstance(generated_at, str) and "T" in generated_at else generated_at
    lines: list[str] = [f"### Operator Benchmark ({date})", ""]
    rel_path = relative_bundle_path(bundle_path)
    lines.append(f"- Artefact: `{rel_path}`")
    device = summarize_device(metadata.get("labels"))
    if device:
        lines.append(f"- Device class: {device}")
    rows = benchmarks.get("rows")
    padded = benchmarks.get("padded_rows")
    iterations = benchmarks.get("iterations")
    backend = benchmarks.get("gpu_backend") or bundle.get("report", {}).get("gpu_backend")
    row_desc = (
        f"**{rows:,}** (padded **{padded:,}**), iterations **{iterations}**, backend `{backend or 'n/a'}`"
        if rows and padded and iterations
        else "rows/iterations unavailable"
    )
    lines.append(f"- Rows: {row_desc}")
    operations = summarize_operations(benchmarks.get("operations"))
    lines.append(f"- {operations}")
    queue = summarize_queue(benchmarks.get("metal_dispatch_queue"))
    if queue:
        lines.append(f"- Queue depth snapshot: {queue}")
    zero_fill_summary = summarize_zero_fill(benchmarks.get("zero_fill_hotspots"))
    lines.append(f"- {zero_fill_summary}")
    staging_summary = summarize_column_staging(benchmarks.get("column_staging"))
    if staging_summary:
        lines.append(f"- {staging_summary}")
    poseidon_micro = summarize_poseidon_microbench(benchmarks.get("poseidon_microbench"))
    if poseidon_micro:
        lines.append(f"- {poseidon_micro}")
    lines.append(
        "- Regenerate with `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1)` and "
        "`scripts/fastpq/wrap_benchmark.py` after each macOS nightly run."
    )
    return "\\n".join(lines)


def update_dashboard(path: Path, panel_title: str, markdown: str) -> None:
    data = json.loads(path.read_text())
    for panel in data.get("panels", []):
        if panel.get("title") != panel_title:
            continue
        options = panel.setdefault("options", {})
        options["mode"] = "markdown"
        options["content"] = markdown
        path.write_text(json.dumps(data, indent=2) + "\n")
        return
    raise SystemExit(f"panel titled '{panel_title}' not found in {path}")


def main() -> None:
    args = parse_args()
    bundle = json.loads(args.bundle.read_text())
    markdown = build_markdown(args.bundle, bundle)
    update_dashboard(args.dashboard, args.panel_title, markdown)
    print(f"Updated panel '{args.panel_title}' in {args.dashboard} from {args.bundle}")


if __name__ == "__main__":
    main()
