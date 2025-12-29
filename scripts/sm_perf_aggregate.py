#!/usr/bin/env python3
"""Aggregate sm_perf capture outputs into baseline-ready medians."""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Mapping, MutableMapping, Optional, Sequence


@dataclass(frozen=True)
class Capture:
    """Single sm_perf capture entry."""

    path: Path
    mode: str
    target_arch: str
    target_os: str
    cpu_label: Optional[str]
    generated_unix_ms: Optional[int]
    benchmarks: Dict[str, float]


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Aggregate sm_perf capture outputs and emit per-mode medians."
    )
    parser.add_argument(
        "captures",
        nargs="+",
        type=Path,
        help="One or more JSON files produced via `scripts/sm_perf.sh --capture-json`.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional path to write the aggregated JSON (defaults to stdout).",
    )
    parser.add_argument(
        "--pretty",
        action="store_true",
        help="Pretty-print the aggregated JSON with indentation.",
    )
    parser.add_argument(
        "--mode",
        dest="modes",
        action="append",
        help=(
            "Restrict aggregation to the provided mode (e.g. scalar, auto, neon-force). "
            "Repeat for multiple modes."
        ),
    )
    return parser.parse_args(argv)


def load_capture(path: Path) -> Capture:
    """Load and validate a capture JSON file."""

    payload = _load_json(path)
    if not isinstance(payload, Mapping):
        raise ValueError(f"{path} does not contain a JSON object")

    benchmarks = payload.get("benchmarks")
    if not isinstance(benchmarks, Mapping) or not benchmarks:
        raise ValueError(f"{path} missing 'benchmarks' entries")
    bench_values: Dict[str, float] = {}
    for key, value in benchmarks.items():
        if not isinstance(key, str):
            raise ValueError(f"{path} contains a non-string benchmark key: {key!r}")
        if not isinstance(value, (int, float)):
            raise ValueError(
                f"{path} benchmark '{key}' must be numeric, found {type(value).__name__}"
            )
        bench_values[key] = float(value)

    metadata = payload.get("metadata")
    if not isinstance(metadata, Mapping):
        raise ValueError(f"{path} missing 'metadata' object")
    mode = metadata.get("mode") or metadata.get("label")
    if not isinstance(mode, str) or not mode:
        raise ValueError(f"{path} metadata is missing a 'mode' or 'label' field")
    target_arch = metadata.get("target_arch")
    target_os = metadata.get("target_os")
    if not isinstance(target_arch, str) or not isinstance(target_os, str):
        raise ValueError(f"{path} metadata missing 'target_arch' or 'target_os'")

    cpu_label = metadata.get("cpu_label")
    if cpu_label is not None:
        cpu_label = str(cpu_label)

    generated = metadata.get("generated_unix_ms")
    if generated is not None:
        if not isinstance(generated, (int, float)):
            raise ValueError(
                f"{path} metadata.generated_unix_ms must be numeric when present"
            )
        generated = int(generated)

    return Capture(
        path=path,
        mode=mode,
        target_arch=target_arch,
        target_os=target_os,
        cpu_label=cpu_label,
        generated_unix_ms=generated,
        benchmarks=bench_values,
    )


def aggregate_modes(
    captures: Sequence[Capture], allowed_modes: Sequence[str] | None = None
) -> Dict[str, Dict[str, object]]:
    """Aggregate capture data per mode and compute medians."""

    if not captures:
        raise ValueError("no capture files were provided")

    requested_modes = {mode for mode in allowed_modes or []}
    grouped: Dict[str, List[Capture]] = {}
    for capture in captures:
        if requested_modes and capture.mode not in requested_modes:
            continue
        grouped.setdefault(capture.mode, []).append(capture)

    if not grouped:
        if requested_modes:
            raise ValueError(
                "no captures matched the requested --mode filters: "
                + ", ".join(sorted(requested_modes))
            )
        raise ValueError("no captures matched the aggregation criteria")

    return {mode: summarise_mode(mode, grouped[mode]) for mode in sorted(grouped)}


def summarise_mode(mode: str, captures: Sequence[Capture]) -> Dict[str, object]:
    """Compute per-benchmark medians/statistics for a single mode."""

    arch = _require_same(captures, mode, attr="target_arch")
    os_token = _require_same(captures, mode, attr="target_os")
    benchmark_names = _require_benchmark_set(captures, mode)

    stats_block: Dict[str, MutableMapping[str, object]] = {}
    medians: Dict[str, float] = {}
    for benchmark in benchmark_names:
        values = [capture.benchmarks[benchmark] for capture in captures]
        median = statistics.median(values)
        medians[benchmark] = median
        stats_block[benchmark] = {
            "samples": values,
            "min": min(values),
            "max": max(values),
            "mean": sum(values) / len(values),
            "median": median,
            "pstdev": statistics.pstdev(values) if len(values) > 1 else 0.0,
        }

    return {
        "mode": mode,
        "target_arch": arch,
        "target_os": os_token,
        "sample_count": len(captures),
        "sources": [
            {
                "path": str(capture.path),
                "cpu_label": capture.cpu_label,
                "generated_unix_ms": capture.generated_unix_ms,
            }
            for capture in captures
        ],
        "benchmarks": medians,
        "statistics": stats_block,
    }


def build_output(modes: Mapping[str, Dict[str, object]]) -> Dict[str, object]:
    """Wrap the aggregated payload with metadata."""

    capture_count = sum(
        len(summary.get("sources", [])) for summary in modes.values()
    )
    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "mode_count": len(modes),
        "capture_count": capture_count,
        "modes": modes,
    }


def _load_json(path: Path):
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except FileNotFoundError as exc:
        raise ValueError(f"capture file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in capture {path}: {exc}") from exc


def _require_same(
    captures: Sequence[Capture],
    mode: str,
    attr: str,
) -> str:
    values = {getattr(capture, attr) for capture in captures}
    if len(values) != 1:
        joined = ", ".join(sorted(values))
        raise ValueError(
            f"mode '{mode}' has inconsistent {attr} values: {joined}"
        )
    return values.pop()


def _require_benchmark_set(
    captures: Sequence[Capture], mode: str
) -> List[str]:
    reference = set(captures[0].benchmarks.keys())
    for capture in captures[1:]:
        current = set(capture.benchmarks.keys())
        if current != reference:
            missing = reference - current
            extra = current - reference
            problems: List[str] = []
            if missing:
                problems.append(
                    f"missing benchmarks {sorted(missing)} in {capture.path}"
                )
            if extra:
                problems.append(
                    f"unexpected benchmarks {sorted(extra)} in {capture.path}"
                )
            details = "; ".join(problems)
            raise ValueError(
                f"mode '{mode}' has inconsistent benchmark sets: {details}"
            )
    return sorted(reference)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        captures = [load_capture(path) for path in args.captures]
        aggregated = aggregate_modes(captures, args.modes)
        payload = build_output(aggregated)
    except ValueError as err:
        print(f"[error] {err}", file=sys.stderr)
        return 2

    json_kwargs = {"indent": 2} if args.pretty else {}
    output = json.dumps(payload, **json_kwargs)

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output + "\n", encoding="utf-8")
    else:
        print(output)
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    sys.exit(main())
