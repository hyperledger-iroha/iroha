#!/usr/bin/env python3
"""Emit Prometheus metrics for the Swift sample smoke summary."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Dict, Iterable, List, Mapping, MutableMapping, Optional


def _load_summary(path: Path) -> Mapping[str, object]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:  # pragma: no cover - surfaced to CLI
        raise RuntimeError(f"summary JSON not found: {path}") from exc
    except json.JSONDecodeError as exc:  # pragma: no cover - surfaced to CLI
        raise RuntimeError(f"invalid JSON in {path}: {exc}") from exc


def _escape_label(value: str) -> str:
    return (
        value.replace("\\", "\\\\")
        .replace("\n", "\\n")
        .replace('"', '\\"')
    )


def _format_labels(labels: Mapping[str, str]) -> str:
    parts = [f'{key}="{_escape_label(str(val))}"' for key, val in labels.items()]
    return "{" + ",".join(parts) + "}" if parts else ""


def _metric_line(name: str, labels: Mapping[str, str], value: float) -> str:
    return f"{name}{_format_labels(labels)} {value}"


def _extract_samples(
    summary: Mapping[str, object],
    base_labels: Mapping[str, str],
) -> List[str]:
    samples_block = summary.get("samples")
    if not isinstance(samples_block, list):
        return []

    lines: List[str] = []
    for entry in samples_block:
        if not isinstance(entry, MutableMapping):
            continue
        sample_name = str(entry.get("sample", "unknown"))
        status = str(entry.get("status", "unknown")).lower()
        duration_raw = entry.get("duration_seconds", 0)
        try:
            duration_value = float(duration_raw)
        except (TypeError, ValueError):
            duration_value = 0.0
        reason = str(entry.get("reason", "") or "")

        status_labels: Dict[str, str] = {
            **base_labels,
            "sample": sample_name,
            "status": status or "unknown",
        }
        lines.append(_metric_line("swift_samples_sample_status", status_labels, 1))

        duration_labels: Dict[str, str] = {
            **base_labels,
            "sample": sample_name,
        }
        lines.append(
            _metric_line(
                "swift_samples_sample_duration_seconds",
                duration_labels,
                duration_value,
            )
        )

        if reason:
            reason_labels: Dict[str, str] = {
                **base_labels,
                "sample": sample_name,
                "status": status or "unknown",
                "reason": reason,
            }
            lines.append(
                _metric_line("swift_samples_sample_reason_info", reason_labels, 1)
            )

    return lines


def build_metrics(summary: Mapping[str, object], cluster: str) -> List[str]:
    base_labels = {"cluster": cluster}
    lines: List[str] = []

    destination = str(summary.get("destination", "") or "unknown")
    overall = str(summary.get("overall_status", "") or "unknown").lower()

    info_labels = {**base_labels, "destination": destination, "status": overall}
    lines.append(_metric_line("swift_samples_report_info", info_labels, 1))

    derived_base = str(summary.get("derived_data_base", "") or "")
    if derived_base:
        lines.append(
            _metric_line(
                "swift_samples_derived_data_path_info",
                {**base_labels, "path": derived_base},
                1,
            )
        )

    lines.extend(_extract_samples(summary, base_labels))
    return lines


def parse_args(argv: Optional[Iterable[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Convert Swift sample smoke summaries into Prometheus metrics."
    )
    parser.add_argument(
        "--summary",
        type=Path,
        required=True,
        help="Path to scripts/check_swift_samples.sh summary JSON.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="Destination Prometheus textfile path.",
    )
    parser.add_argument(
        "--cluster",
        default="default",
        help="Cluster label to include in metrics (default: default).",
    )
    return parser.parse_args(list(argv) if argv is not None else None)


def main(argv: Optional[Iterable[str]] = None) -> int:
    args = parse_args(argv)
    summary = _load_summary(args.summary)
    metrics = build_metrics(summary, args.cluster)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(metrics) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entry
    raise SystemExit(main())
