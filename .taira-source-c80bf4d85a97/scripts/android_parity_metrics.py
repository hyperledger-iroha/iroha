#!/usr/bin/env python3
"""Convert Android parity summaries into Prometheus textfile metrics."""

from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Iterable, List, Mapping, Optional


def _load_summary(path: Path) -> Dict[str, object]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:  # pragma: no cover - surfaced to CLI
        raise RuntimeError(f"summary JSON not found: {path}") from exc
    except json.JSONDecodeError as exc:  # pragma: no cover - surfaced to CLI
        raise RuntimeError(f"invalid JSON in {path}: {exc}") from exc


def _parse_iso8601(value: Optional[str]) -> Optional[datetime]:
    if not value:
        return None
    text = value.strip()
    if not text:
        return None
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        return datetime.fromisoformat(text).astimezone(timezone.utc)
    except ValueError as exc:  # pragma: no cover - surfaced to CLI
        raise RuntimeError(f"invalid ISO-8601 timestamp: {value}") from exc


def _format_labels(labels: Mapping[str, str]) -> str:
    def _escape(value: str) -> str:
        return (
            value.replace("\\", "\\\\")
            .replace("\n", "\\n")
            .replace('"', '\\"')
        )

    items = [f'{key}="{_escape(str(value))}"' for key, value in labels.items()]
    return "{" + ",".join(items) + "}"


def _timestamp_seconds(dt: Optional[datetime]) -> Optional[int]:
    if dt is None:
        return None
    return int(dt.timestamp())


def _metric_line(name: str, labels: Mapping[str, str], value: float) -> str:
    return f"{name}{_format_labels(labels)} {value}"


def _extract_pipeline_metrics(
    pipeline_block: Mapping[str, object],
    cluster: str,
) -> List[str]:
    lines: List[str] = []
    metadata = pipeline_block.get("metadata")
    if isinstance(metadata, dict):
        job_name = metadata.get("job_name")
        job_duration = metadata.get("duration_seconds")
        if isinstance(job_name, str) and isinstance(job_duration, (int, float)):
            lines.append(
                _metric_line(
                    "android_parity_pipeline_job_duration_seconds",
                    {"cluster": cluster, "job_name": job_name},
                    float(job_duration),
                )
            )
        tests = metadata.get("tests")
        if isinstance(tests, list):
            for entry in tests:
                if not isinstance(entry, dict):
                    continue
                test_name = entry.get("name")
                duration = entry.get("duration_seconds")
                if isinstance(test_name, str) and isinstance(duration, (int, float)):
                    lines.append(
                        _metric_line(
                            "android_parity_pipeline_test_duration_seconds",
                            {"cluster": cluster, "test": test_name},
                            float(duration),
                        )
                    )
    return lines


def build_metrics(summary: Mapping[str, object], cluster: str) -> List[str]:
    labels = {"cluster": cluster}
    lines: List[str] = []

    generated_at = _parse_iso8601(summary.get("generated_at"))  # type: ignore[arg-type]
    summary_ts = _timestamp_seconds(generated_at)
    if summary_ts is not None:
        lines.append(_metric_line("android_parity_summary_timestamp_seconds", labels, summary_ts))

    result = summary.get("result", {})
    error_count = 0.0
    result_status = "unknown"
    if isinstance(result, dict):
        raw_count = result.get("error_count", 0)
        try:
            error_count = float(raw_count)
        except (TypeError, ValueError):
            error_count = 0.0
        status_value = result.get("status")
        if isinstance(status_value, str) and status_value.strip():
            result_status = status_value.strip().lower()
    lines.append(_metric_line("android_parity_error_count", labels, error_count))
    lines.append(
        _metric_line(
            "android_parity_summary_status",
            {**labels, "status": result_status},
            1,
        )
    )

    state = summary.get("state")
    if isinstance(state, dict):
        regen_ts = _parse_iso8601(state.get("generated_at"))  # type: ignore[arg-type]
        if regen_ts is not None and generated_at is not None:
            age_hours = (generated_at - regen_ts).total_seconds() / 3600.0
            lines.append(_metric_line("android_parity_regen_hours_since_success", labels, age_hours))
        owner = state.get("rotation_owner")
        if isinstance(owner, str) and owner:
            owner_labels = {"cluster": cluster, "rotation_owner": owner}
            cadence = state.get("cadence")
            if isinstance(cadence, str) and cadence:
                owner_labels["cadence"] = cadence
            lines.append(_metric_line("android_parity_rotation_owner_info", owner_labels, 1))

    pipeline_block = summary.get("pipeline")
    if isinstance(pipeline_block, dict):
        lines.extend(_extract_pipeline_metrics(pipeline_block, cluster))

    return lines


def parse_args(argv: Optional[Iterable[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Convert Android parity summary JSON into Prometheus metrics.")
    parser.add_argument("--summary", type=Path, required=True, help="Path to the parity summary JSON.")
    parser.add_argument("--output", type=Path, required=True, help="Destination path for Prometheus metrics.")
    parser.add_argument("--cluster", default="default", help="Cluster label for emitted metrics (default: default).")
    return parser.parse_args(list(argv) if argv is not None else None)


def main(argv: Optional[Iterable[str]] = None) -> int:
    args = parse_args(argv)
    summary = _load_summary(args.summary)
    lines = build_metrics(summary, args.cluster)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    raise SystemExit(main())
