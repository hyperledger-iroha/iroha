#!/usr/bin/env python3
"""Summarise FASTPQ Metal queue/staging telemetry."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional


def _timestamp() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00", "Z"
    )


def _round(value: Optional[float]) -> Optional[float]:
    if value is None:
        return None
    return round(value, 3)


def _percent(value: Optional[float]) -> Optional[str]:
    if value is None:
        return None
    return f"{value * 100:.1f}%"


def _phase_wait_ratio(stats: Optional[Dict[str, Any]]) -> Optional[float]:
    if not isinstance(stats, dict):
        return None
    ratio = stats.get("wait_ratio")
    if isinstance(ratio, (int, float)):
        return float(ratio)
    flatten = stats.get("flatten_ms")
    wait = stats.get("wait_ms")
    if isinstance(flatten, (int, float)) and isinstance(wait, (int, float)):
        total = float(flatten) + float(wait)
        if total > 0:
            return float(wait) / total
    return None


def _max_wait_ratio(samples: Any) -> Optional[float]:
    if not isinstance(samples, list):
        return None
    max_ratio: Optional[float] = None
    for entry in samples:
        if not isinstance(entry, dict):
            continue
        ratio = entry.get("wait_ratio")
        if not isinstance(ratio, (int, float)):
            flatten = entry.get("flatten_ms")
            wait = entry.get("wait_ms")
            if isinstance(flatten, (int, float)) and isinstance(wait, (int, float)):
                total = float(flatten) + float(wait)
                if total > 0:
                    ratio = float(wait) / total
        if isinstance(ratio, (int, float)):
            ratio = float(ratio)
            if max_ratio is None or ratio > max_ratio:
                max_ratio = ratio
    return max_ratio


def _load_json(path: Path) -> Dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
        raise SystemExit(f"[error] failed to parse JSON from {path}: {exc}") from exc


def _extract_phase(payload: Dict[str, Any], phase: str) -> Dict[str, Any]:
    entry = payload.get("phases", {}).get(phase) if isinstance(payload, dict) else None
    if not isinstance(entry, dict):
        return {}
    return {
        "batches": entry.get("batches"),
        "flatten_ms": entry.get("flatten_ms"),
        "wait_ms": entry.get("wait_ms"),
        "wait_ratio": entry.get("wait_ratio"),
    }


def _extract_queue(payload: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    if not isinstance(payload, dict):
        return None
    queue = {
        "limit": payload.get("limit"),
        "dispatch_count": payload.get("dispatch_count"),
        "max_in_flight": payload.get("max_in_flight"),
        "busy_ms": payload.get("busy_ms"),
        "overlap_ms": payload.get("overlap_ms"),
        "busy_ratio": payload.get("busy_ratio"),
        "overlap_ratio": payload.get("overlap_ratio"),
        "window_ms": payload.get("window_ms"),
    }
    if all(value in (None, 0) for value in queue.values()):
        return queue
    return queue


def _extract_pipeline(payload: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    if not isinstance(payload, dict):
        return None
    pipeline = {
        "batches": payload.get("batches"),
        "chunk_columns": payload.get("chunk_columns"),
        "enabled": payload.get("enabled"),
        "fallbacks": payload.get("fallbacks"),
        "pipe_depth": payload.get("pipe_depth"),
    }
    if any(value not in (None, 0) for value in pipeline.values()):
        return pipeline
    return None


@dataclass
class ReportSummary:
    label: str
    source: str
    queue: Optional[Dict[str, Any]]
    column_staging: Optional[Dict[str, Any]]
    poseidon_queue: Optional[Dict[str, Any]]
    poseidon_pipeline: Optional[Dict[str, Any]]
    run_status: Optional[Dict[str, Any]]
    phase_metrics: Dict[str, Dict[str, Any]]
    phase_max_wait_ratio: Dict[str, Optional[float]]
    issues: List[str]


def summarize_report(
    *,
    path: Path,
    label: str,
    min_dispatch: int,
    min_batches: int,
    max_wait_ratio: Optional[float],
) -> ReportSummary:
    payload = _load_json(path)
    issues: List[str] = []
    run_status = payload.get("run_status") if isinstance(payload, dict) else None
    queue = _extract_queue(payload.get("metal_dispatch_queue"))
    column_staging = payload.get("column_staging")
    poseidon_queue = _extract_queue(
        payload.get("metal_dispatch_queue", {}).get("poseidon")
    )
    poseidon_pipeline = _extract_pipeline(
        payload.get("metal_dispatch_queue", {}).get("poseidon_pipeline")
    )

    if queue is None:
        issues.append("missing metal_dispatch_queue block")
    else:
        dispatch_count = queue.get("dispatch_count")
        if isinstance(dispatch_count, (int, float)):
            if int(dispatch_count) < min_dispatch:
                issues.append(
                    f"dispatch_count<{min_dispatch} (queue telemetry missing or CPU fallback)"
                )
        else:
            issues.append("dispatch_count missing in queue telemetry")

    if column_staging is None:
        issues.append("missing column_staging block")
    else:
        batches = column_staging.get("batches")
        if isinstance(batches, (int, float)):
            if int(batches) < min_batches:
                issues.append(
                    f"column staging batches<{min_batches} (host staging telemetry missing)"
                )
        else:
            issues.append("column_staging.batches missing")
        if not isinstance(column_staging.get("phases"), dict):
            issues.append("column_staging phases missing")

    phase_metrics: Dict[str, Dict[str, Any]] = {}
    phase_max_wait_ratio: Dict[str, Optional[float]] = {}
    samples_block = column_staging.get("samples") if isinstance(column_staging, dict) else None
    for phase in ("fft", "lde", "poseidon"):
        metrics = _extract_phase(column_staging, phase) if column_staging else {}
        phase_metrics[phase] = metrics
        sample_entries: Any = None
        if isinstance(samples_block, dict):
            sample_entries = samples_block.get(phase)
        phase_max = _max_wait_ratio(sample_entries)
        if phase_max is None:
            phase_max = _phase_wait_ratio(metrics)
        phase_max_wait_ratio[phase] = phase_max
        if max_wait_ratio is not None and phase_max is not None and phase_max > max_wait_ratio:
            issues.append(
                f"{phase} wait ratio {phase_max:.3f} exceeds threshold {max_wait_ratio:.3f}"
            )

    if isinstance(run_status, dict):
        state = run_status.get("state")
        if isinstance(state, str) and state.lower() != "ok":
            reason_list = [
                reason for reason in run_status.get("reasons", []) if isinstance(reason, str)
            ]
            suffix = f" ({', '.join(reason_list)})" if reason_list else ""
            issues.append(f"run_status={state}{suffix}")

    return ReportSummary(
        label=label,
        source=str(path),
        queue=queue,
        column_staging=column_staging,
        poseidon_queue=poseidon_queue,
        poseidon_pipeline=poseidon_pipeline,
        run_status=run_status if isinstance(run_status, dict) else None,
        phase_metrics=phase_metrics,
        phase_max_wait_ratio=phase_max_wait_ratio,
        issues=issues,
    )


def _render_markdown(summaries: Iterable[ReportSummary]) -> str:
    headers = [
        "Label",
        "Run status",
        "Dispatch",
        "Busy",
        "Overlap",
        "Max Depth",
        "FFT flatten",
        "FFT wait",
        "FFT wait %",
        "LDE flatten",
        "LDE wait",
        "LDE wait %",
        "Poseidon flatten",
        "Poseidon wait",
        "Poseidon wait %",
        "Pipe depth",
        "Pipe batches",
        "Pipe fallbacks",
    ]
    lines = ["| " + " | ".join(headers) + " |", "|" + "---|" * len(headers)]
    for summary in summaries:
        queue = summary.queue or {}
        staging = summary.column_staging or {}
        fft = _extract_phase(staging, "fft")
        lde = _extract_phase(staging, "lde")
        poseidon = _extract_phase(staging, "poseidon")
        pipeline = summary.poseidon_pipeline or {}
        fft_ratio = _percent(_phase_wait_ratio(fft)) or "-"
        lde_ratio = _percent(_phase_wait_ratio(lde)) or "-"
        poseidon_ratio = _percent(_phase_wait_ratio(poseidon)) or "-"
        status = "-"
        if isinstance(summary.run_status, dict):
            state = summary.run_status.get("state")
            if isinstance(state, str):
                status = state
        row = [
            summary.label,
            status,
            str(queue.get("dispatch_count", "-")),
            _percent(queue.get("busy_ratio")) or "-",
            _percent(queue.get("overlap_ratio")) or "-",
            str(queue.get("max_in_flight", "-")),
            str(_round(fft.get("flatten_ms")) or "-"),
            str(_round(fft.get("wait_ms")) or "-"),
            fft_ratio,
            str(_round(lde.get("flatten_ms")) or "-"),
            str(_round(lde.get("wait_ms")) or "-"),
            lde_ratio,
            str(_round(poseidon.get("flatten_ms")) or "-"),
            str(_round(poseidon.get("wait_ms")) or "-"),
            poseidon_ratio,
            str(pipeline.get("pipe_depth", "-")),
            str(pipeline.get("batches", "-")),
            str(pipeline.get("fallbacks", "-")),
        ]
        lines.append("| " + " | ".join(row) + " |")
    return "\n".join(lines)


def _parse_report_args(values: List[str]) -> List[tuple[Path, str]]:
    reports: List[tuple[Path, str]] = []
    for token in values:
        if "=" in token:
            path_part, label = token.split("=", 1)
        else:
            path_part, label = token, Path(token).stem
        reports.append((Path(path_part), label.strip() or Path(path_part).stem))
    return reports


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Summarise queue depth / staging telemetry from fastpq_metal_bench JSON captures."
        )
    )
    parser.add_argument(
        "reports",
        nargs="+",
        help="fastpq_metal_bench JSON paths; append '=label' to override the name",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Optional path for a machine-readable summary JSON file.",
    )
    parser.add_argument(
        "--markdown-out",
        type=Path,
        help="Optional path for a Markdown table summary.",
    )
    parser.add_argument(
        "--min-dispatch",
        type=int,
        default=1,
        help="Minimum queue dispatch count required before a run is considered valid",
    )
    parser.add_argument(
        "--min-batches",
        type=int,
        default=1,
        help="Minimum column staging batches required before telemetry is considered valid",
    )
    parser.add_argument(
        "--max-wait-ratio",
        type=float,
        default=None,
        help="Optional threshold (0-1) for per-phase host wait ratios; records an issue when exceeded.",
    )
    return parser


def main(argv: Optional[Iterable[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(list(argv) if argv is not None else None)
    report_args = _parse_report_args(list(args.reports))
    summaries = [
        summarize_report(
            path=path,
            label=label,
            min_dispatch=args.min_dispatch,
            min_batches=args.min_batches,
            max_wait_ratio=args.max_wait_ratio,
        )
        for path, label in report_args
    ]

    for summary in summaries:
        print(f"== {summary.label}")
        if summary.queue:
            queue = summary.queue
            busy = _percent(queue.get("busy_ratio")) or "n/a"
            overlap = _percent(queue.get("overlap_ratio")) or "n/a"
            max_depth = queue.get("max_in_flight")
            dispatch = queue.get("dispatch_count")
            print(
                f"Queue dispatch={dispatch} limit={queue.get('limit')} max_depth={max_depth} busy={busy} overlap={overlap}"
            )
        else:
            print("Queue telemetry: missing")
        if summary.column_staging:
            total = summary.column_staging
            print(
                "Staging total batches="
                f"{total.get('batches')} flatten={_round(total.get('flatten_ms'))}ms "
                f"wait={_round(total.get('wait_ms'))}ms"
            )
            for phase, metrics in summary.phase_metrics.items():
                ratio = summary.phase_max_wait_ratio.get(phase)
                ratio_str = f"{ratio * 100:.1f}%" if isinstance(ratio, float) else "n/a"
                print(
                    f"  {phase}: flatten={_round(metrics.get('flatten_ms'))}ms "
                    f"wait={_round(metrics.get('wait_ms'))}ms wait_ratio={ratio_str}"
                )
        else:
            print("Staging telemetry: missing")
        if summary.poseidon_pipeline:
            pipeline = summary.poseidon_pipeline
            status = "enabled" if pipeline.get("enabled") else "disabled"
            batches = pipeline.get("batches")
            depth = pipeline.get("pipe_depth")
            fallbacks = pipeline.get("fallbacks")
            chunk_cols = pipeline.get("chunk_columns")
            print(
                "Poseidon pipeline "
                f"status={status} depth={depth} batches={batches} "
                f"chunk_cols={chunk_cols} fallbacks={fallbacks}"
            )
        else:
            print("Poseidon pipeline: missing")
        if isinstance(summary.run_status, dict):
            state = summary.run_status.get("state")
            reasons = summary.run_status.get("reasons", [])
            formatted_reasons = (
                ", ".join(reason for reason in reasons if isinstance(reason, str))
                if isinstance(reasons, list)
                else ""
            )
            if isinstance(state, str) and state.lower() != "ok":
                suffix = f" [{formatted_reasons}]" if formatted_reasons else ""
                print(f"Run status: {state}{suffix}")
        if summary.issues:
            print("Issues:")
            for issue in summary.issues:
                print(f"  - {issue}")
        else:
            print("Issues: none")
        print()

    if args.markdown_out:
        markdown = _render_markdown(summaries)
        args.markdown_out.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_out.write_text(markdown + "\n", encoding="utf-8")

    if args.json_out:
        payload = {
            "generated_at": _timestamp(),
            "reports": [summary.__dict__ for summary in summaries],
        }
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    raise SystemExit(main())
