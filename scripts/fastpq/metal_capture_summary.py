#!/usr/bin/env python3
"""Summarise FASTPQ Metal benchmark captures for WP2-E analysis."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Mapping, MutableMapping, Optional, Sequence


OP_LABELS: Mapping[str, str] = {
    "fft": "FFT",
    "ifft": "IFFT",
    "lde": "LDE",
    "poseidon_hash_columns": "Poseidon",
}


@dataclass
class StageRow:
    """Computed summary for a single benchmark stage."""

    label: str
    columns: Optional[int]
    input_len: Optional[int]
    gpu_mean_ms: Optional[float]
    gpu_min_ms: Optional[float]
    gpu_max_ms: Optional[float]
    gpu_share: Optional[float]
    cpu_mean_ms: Optional[float]
    cpu_min_ms: Optional[float]
    cpu_max_ms: Optional[float]
    speedup_ratio: Optional[float]
    speedup_delta_ms: Optional[float]
    gpu_recorded: bool


def load_capture(path: Path) -> MutableMapping[str, object]:
    """Load a FASTPQ capture JSON file."""

    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if not isinstance(payload, MutableMapping):
        raise ValueError(f"{path} does not contain a JSON object")
    return payload


def build_stage_rows(capture: Mapping[str, object]) -> List[StageRow]:
    """Convert capture operations into structured stage rows."""

    operations: Sequence[Mapping[str, object]] = capture.get("operations") or []
    gpu_total = 0.0
    for entry in operations:
        gpu = entry.get("gpu")
        if isinstance(gpu, Mapping) and entry.get("gpu_recorded", False):
            gpu_total += float(gpu.get("mean_ms") or 0.0)

    rows: List[StageRow] = []
    for entry in operations:
        op_name = str(entry.get("operation") or "unknown")
        label = OP_LABELS.get(op_name, op_name.replace("_", " ").title())
        columns = entry.get("columns")
        input_len = entry.get("input_len")
        gpu = entry.get("gpu") if isinstance(entry.get("gpu"), Mapping) else None
        cpu = entry.get("cpu") if isinstance(entry.get("cpu"), Mapping) else None
        speedup = entry.get("speedup") if isinstance(entry.get("speedup"), Mapping) else None
        gpu_recorded = bool(entry.get("gpu_recorded", False))
        gpu_mean = float(gpu.get("mean_ms")) if gpu and gpu.get("mean_ms") is not None else None
        gpu_share: Optional[float]
        if gpu_recorded and gpu_total > 0 and gpu_mean is not None:
            gpu_share = gpu_mean / gpu_total
        else:
            gpu_share = None

        rows.append(
            StageRow(
                label=label,
                columns=int(columns) if isinstance(columns, int) else None,
                input_len=int(input_len) if isinstance(input_len, int) else None,
                gpu_mean_ms=gpu_mean,
                gpu_min_ms=float(gpu.get("min_ms")) if gpu and gpu.get("min_ms") is not None else None,
                gpu_max_ms=float(gpu.get("max_ms")) if gpu and gpu.get("max_ms") is not None else None,
                gpu_share=gpu_share,
                cpu_mean_ms=float(cpu.get("mean_ms")) if cpu and cpu.get("mean_ms") is not None else None,
                cpu_min_ms=float(cpu.get("min_ms")) if cpu and cpu.get("min_ms") is not None else None,
                cpu_max_ms=float(cpu.get("max_ms")) if cpu and cpu.get("max_ms") is not None else None,
                speedup_ratio=float(speedup.get("ratio"))
                if speedup and speedup.get("ratio") is not None
                else None,
                speedup_delta_ms=float(speedup.get("delta_ms"))
                if speedup and speedup.get("delta_ms") is not None
                else None,
                gpu_recorded=gpu_recorded,
            )
        )
    return rows


def _format_duration(mean: Optional[float], min_val: Optional[float], max_val: Optional[float]) -> str:
    if mean is None:
        return "n/a"
    parts = [f"{mean:.3f} ms"]
    if min_val is not None and max_val is not None:
        parts.append(f"({min_val:.3f}-{max_val:.3f})")
    return " ".join(parts)


def _format_share(share: Optional[float]) -> str:
    if share is None:
        return "n/a"
    return f"{share * 100:.1f}%"


def _format_speedup(ratio: Optional[float]) -> str:
    if ratio is None:
        return "n/a"
    return f"{ratio:.3f}x"


def _format_delta(delta: Optional[float]) -> str:
    if delta is None:
        return "n/a"
    return f"{delta:+.3f}"


def _summarize_heuristics(capture: Mapping[str, object]) -> Optional[str]:
    heuristics = capture.get("metal_heuristics")
    if not isinstance(heuristics, Mapping):
        return None
    parts: List[str] = []
    multiplier = heuristics.get("poseidon_batch_multiplier")
    if isinstance(multiplier, (int, float)):
        parts.append(f"poseidon_batch_multiplier={int(multiplier)}")
    tile_limit = heuristics.get("lde_tile_stage_limit")
    if isinstance(tile_limit, (int, float)):
        parts.append(f"lde_tile_stage_limit={int(tile_limit)}")
    batches = heuristics.get("batch_columns")
    if isinstance(batches, Mapping):
        fft = batches.get("fft") or {}
        lde = batches.get("lde") or {}
        poseidon = batches.get("poseidon") or {}
        summaries = []
        if isinstance(fft, Mapping) and fft.get("columns") is not None:
            summaries.append(f"fft={fft.get('columns')}")
        if isinstance(lde, Mapping) and lde.get("columns") is not None:
            summaries.append(f"lde={lde.get('columns')}")
        if isinstance(poseidon, Mapping) and poseidon.get("columns") is not None:
            summaries.append(f"poseidon={poseidon.get('columns')}")
        if summaries:
            parts.append(f"batch_columns: {', '.join(summaries)}")
    if not parts:
        return None
    return "Heuristics: " + ", ".join(parts)


def render_markdown_table(rows: Sequence[StageRow]) -> str:
    """Build the Markdown table used in the WP2-E report."""

    header = (
        "| Stage | Columns | Input len | GPU mean (ms) | CPU mean (ms) | "
        "GPU share | Speedup | Δ CPU (ms) |\n"
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    )
    lines = [header]
    for row in rows:
        gpu_duration = _format_duration(row.gpu_mean_ms, row.gpu_min_ms, row.gpu_max_ms)
        cpu_duration = _format_duration(row.cpu_mean_ms, row.cpu_min_ms, row.cpu_max_ms)
        lines.append(
            "| {label} | {columns} | {input_len} | {gpu} | {cpu} | {share} | {speedup} | {delta} |".format(
                label=row.label,
                columns=row.columns if row.columns is not None else "n/a",
                input_len=row.input_len if row.input_len is not None else "n/a",
                gpu=gpu_duration,
                cpu=cpu_duration,
                share=_format_share(row.gpu_share),
                speedup=_format_speedup(row.speedup_ratio),
                delta=_format_delta(row.speedup_delta_ms),
            )
        )
    return "\n".join(lines)


def render_summary(capture: Mapping[str, object], rows: Sequence[StageRow]) -> str:
    """Produce a human-readable summary for the capture."""

    gpu_total = sum(row.gpu_mean_ms or 0.0 for row in rows if row.gpu_recorded)
    cpu_total = sum(row.cpu_mean_ms or 0.0 for row in rows if row.cpu_mean_ms is not None)
    execution_mode = capture.get("execution_mode", "unknown")
    gpu_backend = capture.get("gpu_backend", "unknown")
    gpu_available = capture.get("gpu_available")
    lines = [
        f"GPU total mean: {gpu_total:.3f} ms across {len(rows)} stages "
        f"(execution_mode={execution_mode}, backend={gpu_backend}, gpu_available={gpu_available})."
    ]
    if cpu_total > 0:
        lines.append(f"CPU total mean: {cpu_total:.3f} ms.")

    queue = capture.get("metal_dispatch_queue") or {}
    if isinstance(queue, Mapping) and queue:
        lines.append(
            "Metal dispatch queue: dispatch_count={dispatch} overlap_ms={overlap} "
            "max_in_flight={max_in_flight} limit={limit}".format(
                dispatch=queue.get("dispatch_count", "n/a"),
                overlap=queue.get("overlap_ms", "n/a"),
                max_in_flight=queue.get("max_in_flight", "n/a"),
                limit=queue.get("limit", "n/a"),
            )
        )
    heuristics_line = _summarize_heuristics(capture)
    if heuristics_line:
        lines.append(heuristics_line)

    dominant = max(
        (row for row in rows if row.gpu_share is not None),
        key=lambda item: item.gpu_share or 0.0,
        default=None,
    )
    if dominant and dominant.gpu_share is not None:
        lines.append(
            f"Dominant stage: {dominant.label} accounts for {_format_share(dominant.gpu_share)} of GPU time."
        )

    slowest = max(
        (row for row in rows if row.gpu_mean_ms is not None),
        key=lambda item: item.gpu_mean_ms or 0.0,
        default=None,
    )
    if slowest and slowest.speedup_ratio is not None:
        lines.append(
            f"{slowest.label} speedup: {_format_speedup(slowest.speedup_ratio)} "
            f"(Δ={_format_delta(slowest.speedup_delta_ms)})."
        )
    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Summarise FASTPQ Metal benchmark captures as Markdown."
    )
    parser.add_argument(
        "captures",
        nargs="+",
        help="Path(s) to fastpq_metal_bench JSON captures.",
    )
    parser.add_argument(
        "--label",
        action="append",
        default=[],
        help="Optional label per capture (repeat for each path). "
        "Defaults to the file stem.",
    )
    parser.add_argument(
        "--no-summary",
        action="store_true",
        help="Suppress the textual summary and emit the table only.",
    )
    return parser.parse_args()


def iter_labels(args: argparse.Namespace) -> Iterable[str]:
    """Yield labels matching each provided capture path."""

    if args.label:
        if len(args.label) != len(args.captures):
            raise SystemExit(
                f"--label count ({len(args.label)}) must match capture count ({len(args.captures)})"
            )
        for label in args.label:
            yield label
    else:
        for capture in args.captures:
            yield Path(capture).stem


def main() -> None:
    args = parse_args()
    for path_arg, label in zip(args.captures, iter_labels(args)):
        capture_path = Path(path_arg)
        capture = load_capture(capture_path)
        rows = build_stage_rows(capture)
        print(f"## {label}")
        print(render_markdown_table(rows))
        if not args.no_summary:
            print()
            print(render_summary(capture, rows))
        if path_arg != args.captures[-1]:
            print()


if __name__ == "__main__":
    main()
