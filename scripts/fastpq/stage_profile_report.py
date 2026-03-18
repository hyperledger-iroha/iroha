#!/usr/bin/env python3
"""Summarise FASTPQ stage-profile bundles."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple


def _load_summary(path: Path) -> Dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
        raise SystemExit(f"[error] failed to parse JSON from {path}: {exc}") from exc


def _fmt_ms(value: Optional[float]) -> str:
    if value is None:
        return "-"
    return f"{value:.3f}"


def _fmt_ratio(value: Optional[float]) -> str:
    if value is None:
        return "-"
    return f"{value:.3f}x"


def _fmt_bool(value: bool, ok_label: str = "yes", missing_label: str = "no") -> str:
    return ok_label if value else missing_label


def _zero_or_none(payload: Optional[Dict[str, Any]]) -> bool:
    if not isinstance(payload, dict):
        return True
    return all(value in (None, 0, 0.0, []) for value in payload.values())


@dataclass
class StageStat:
    mean_ms: Optional[float]
    min_ms: Optional[float]
    max_ms: Optional[float]


@dataclass
class StageReport:
    stage: str
    operation: str
    cpu: StageStat
    gpu: Optional[StageStat]
    speedup_ratio: Optional[float]
    speedup_delta_ms: Optional[float]
    columns: Optional[int]
    input_len: Optional[int]
    trace_present: bool
    queue_present: bool
    staging_present: bool
    issues: List[str]


@dataclass
class ProfileReport:
    generated_at: str
    rows: int
    warmups: int
    iterations: int
    release_build: bool
    capture_trace: bool
    stages: List[StageReport]


def _extract_stage_stats(payload: Dict[str, Any], label: str) -> Tuple[StageStat, Optional[StageStat], List[str]]:
    issues: List[str] = []
    cpu_block = payload.get("cpu")
    if not isinstance(cpu_block, dict):
        issues.append("missing CPU stats")
        cpu = StageStat(None, None, None)
    else:
        cpu = StageStat(
            mean_ms=cpu_block.get("mean_ms"),
            min_ms=cpu_block.get("min_ms"),
            max_ms=cpu_block.get("max_ms"),
        )
    gpu = None
    gpu_block = payload.get("gpu")
    if isinstance(gpu_block, dict):
        gpu = StageStat(
            mean_ms=gpu_block.get("mean_ms"),
            min_ms=gpu_block.get("min_ms"),
            max_ms=gpu_block.get("max_ms"),
        )
    else:
        issues.append(f"{label} missing GPU stats")
    return cpu, gpu, issues


def _summarize_stage(stage: Dict[str, Any], capture_trace: bool) -> StageReport:
    label = stage.get("stage") or "unknown"
    stats_payload = stage.get("stats")
    issues: List[str] = []
    if not isinstance(stats_payload, dict):
        issues.append("stats block missing")
        cpu = StageStat(None, None, None)
        gpu = None
        speedup_ratio = None
        speedup_delta = None
        columns = None
        input_len = None
    else:
        cpu, gpu, stat_issues = _extract_stage_stats(stats_payload, label)
        issues.extend(stat_issues)
        speedup_ratio = stats_payload.get("speedup_ratio")
        speedup_delta = stats_payload.get("speedup_delta_ms")
        columns = stats_payload.get("columns")
        input_len = stats_payload.get("input_len")

    trace_artifact = stage.get("trace_artifact")
    trace_present = bool(trace_artifact)
    if capture_trace and not trace_present:
        issues.append("trace missing")

    queue_present = not _zero_or_none(stage.get("metal_dispatch_queue"))
    if not queue_present:
        issues.append("queue telemetry missing")
    staging_present = not _zero_or_none(stage.get("column_staging"))
    if not staging_present:
        issues.append("column staging telemetry missing")

    return StageReport(
        stage=str(label),
        operation=str(stage.get("operation") or "unknown"),
        cpu=cpu,
        gpu=gpu,
        speedup_ratio=speedup_ratio,
        speedup_delta_ms=speedup_delta,
        columns=columns if isinstance(columns, int) else None,
        input_len=input_len if isinstance(input_len, int) else None,
        trace_present=trace_present,
        queue_present=queue_present,
        staging_present=staging_present,
        issues=issues,
    )


def load_profile_report(path: Path) -> ProfileReport:
    payload = _load_summary(path)
    stages = payload.get("stages")
    if not isinstance(stages, list):
        raise SystemExit(f"[error] summary file {path} missing stages array")
    capture_trace = bool(payload.get("capture_trace"))
    stage_reports = [_summarize_stage(stage, capture_trace) for stage in stages if isinstance(stage, dict)]
    if not stage_reports:
        raise SystemExit(f"[error] {path} contains no valid stage entries")
    return ProfileReport(
        generated_at=str(payload.get("generated_at") or "unknown"),
        rows=int(payload.get("rows") or 0),
        warmups=int(payload.get("warmups") or 0),
        iterations=int(payload.get("iterations") or 0),
        release_build=bool(payload.get("release_build")),
        capture_trace=capture_trace,
        stages=stage_reports,
    )


def render_markdown(report: ProfileReport, *, label: Optional[str]) -> str:
    title_label = label or ""
    header = f"Stage profile summary — {title_label}".strip(" —")
    metadata = (
        f"rows={report.rows}, warmups={report.warmups}, iterations={report.iterations}, "
        f"release={'yes' if report.release_build else 'no'}, trace={'yes' if report.capture_trace else 'no'}"
    )
    headers = [
        "Stage",
        "Op",
        "GPU mean (ms)",
        "CPU mean (ms)",
        "Speedup",
        "ΔCPU (ms)",
        "Columns",
        "Input",
        "Trace",
        "Queue",
        "Issues",
    ]
    lines = [header, metadata, ""]
    table = ["| " + " | ".join(headers) + " |", "|" + "---|" * len(headers)]
    for stage in report.stages:
        issues = "; ".join(stage.issues) if stage.issues else "—"
        row = [
            stage.stage,
            stage.operation,
            _fmt_ms(stage.gpu.mean_ms if stage.gpu else None),
            _fmt_ms(stage.cpu.mean_ms),
            _fmt_ratio(stage.speedup_ratio),
            _fmt_ms(stage.speedup_delta_ms),
            str(stage.columns or "-"),
            str(stage.input_len or "-"),
            _fmt_bool(stage.trace_present),
            _fmt_bool(stage.queue_present and stage.staging_present, ok_label="ok", missing_label="missing"),
            issues,
        ]
        table.append("| " + " | ".join(row) + " |")
    lines.extend(table)
    return "\n".join(lines)


def build_json_summary(report: ProfileReport, *, label: Optional[str]) -> Dict[str, Any]:
    stages = []
    all_issues: List[str] = []
    for stage in report.stages:
        stage_entry = {
            "stage": stage.stage,
            "operation": stage.operation,
            "cpu_mean_ms": stage.cpu.mean_ms,
            "gpu_mean_ms": stage.gpu.mean_ms if stage.gpu else None,
            "speedup_ratio": stage.speedup_ratio,
            "speedup_delta_ms": stage.speedup_delta_ms,
            "columns": stage.columns,
            "input_len": stage.input_len,
            "trace_present": stage.trace_present,
            "queue_telemetry": stage.queue_present,
            "column_staging": stage.staging_present,
            "issues": stage.issues,
        }
        stages.append(stage_entry)
        all_issues.extend(f"{stage.stage}: {issue}" for issue in stage.issues)
    return {
        "label": label,
        "generated_at": report.generated_at,
        "rows": report.rows,
        "warmups": report.warmups,
        "iterations": report.iterations,
        "release_build": report.release_build,
        "capture_trace": report.capture_trace,
        "stages": stages,
        "issues": all_issues,
    }


def _write_output(path: Optional[Path], content: str) -> None:
    if path is None:
        print(content)
        return
    path.write_text(content + "\n", encoding="utf-8")


def parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Render Markdown/JSON summaries from fastpq-stage-profile outputs."
    )
    parser.add_argument("summary", help="Path to stage_profile_summary.json")
    parser.add_argument(
        "--label",
        default=None,
        help="Optional label inserted into the summary header.",
    )
    parser.add_argument(
        "--markdown-out",
        dest="markdown_out",
        default=None,
        help="Optional path to write the Markdown table (stdout when omitted).",
    )
    parser.add_argument(
        "--json-out",
        dest="json_out",
        default=None,
        help="Optional path to write the JSON summary.",
    )
    return parser.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> None:
    args = parse_args(argv)
    path = Path(args.summary)
    if not path.exists():
        raise SystemExit(f"[error] stage profile summary not found: {path}")
    report = load_profile_report(path)
    markdown = render_markdown(report, label=args.label)
    _write_output(Path(args.markdown_out) if args.markdown_out else None, markdown)
    if args.json_out:
        summary = build_json_summary(report, label=args.label)
        Path(args.json_out).write_text(
            json.dumps(summary, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    main()
