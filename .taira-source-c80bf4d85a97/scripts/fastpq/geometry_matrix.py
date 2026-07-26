#!/usr/bin/env python3
"""
Summarise FASTPQ Metal geometry sweep outputs into stability artefacts.

The geometry sweep helper (`launch_geometry_sweep.py`) writes a `summary.json`
bundle alongside the raw benchmark artefacts. This script ingests that summary,
labels every run as either *stable* (GPU timings captured for FFT/LDE/Poseidon)
or *unstable* (missing timings, timeouts, or other failures), and emits both a
Markdown table and optional JSON matrices that CI dashboards can ingest. Use the
host and environment summaries to understand which launch geometries stay
stable, which hosts produce the artefacts, and which reasons prevent GPU runs
from qualifying for Stage7 evidence.
"""

from __future__ import annotations

import argparse
import json
import pathlib
from typing import Any, Dict, Iterable, List, Sequence, Tuple

ENV_COLUMNS: Sequence[tuple[str, str]] = (
    ("FASTPQ_METAL_FFT_COLUMNS", "FFT"),
    ("FASTPQ_METAL_LDE_COLUMNS", "LDE"),
    ("FASTPQ_METAL_QUEUE_FANOUT", "Fanout"),
    ("FASTPQ_POSEIDON_PIPE_COLUMNS", "Pipe"),
    ("FASTPQ_METAL_POSEIDON_LANES", "Lanes"),
    ("FASTPQ_METAL_POSEIDON_BATCH", "Batch"),
)

REQUIRED_OPERATIONS = ("fft", "lde", "poseidon_hash_columns")

HOST_HEADERS: Sequence[tuple[str, str]] = (
    ("host_label", "Host"),
    ("host_name", "Hostname"),
    ("host_machine", "Machine"),
    ("host_platform", "Platform"),
    ("device_class", "Device"),
    ("gpu_label", "GPU"),
)

RUN_HEADERS: Sequence[tuple[str, str]] = (
    ("execution_mode", "Mode"),
    ("gpu_backend", "Backend"),
    ("operation", "Operation"),
)


def classify_entry(entry: Dict[str, Any]) -> tuple[str, List[str]]:
    """Return classification plus reasons (non-empty for unstable)."""

    reasons: List[str] = []
    # CPU fallback or missing accelerator should be treated as unstable so matrix
    # consumers can quickly spot hosts that never executed on the GPU.
    if entry.get("gpu_available") is False:
        reasons.append("gpu_unavailable")
    backend = entry.get("gpu_backend")
    if isinstance(backend, str) and backend.lower() != "metal":
        reasons.append(f"backend={backend}")

    status = entry.get("status")
    if status != "ok":
        reasons.append(f"status={status or 'unknown'}")

    operations = entry.get("operations") or {}
    if not isinstance(operations, dict):
        reasons.append("missing_operations")
    else:
        for name in REQUIRED_OPERATIONS:
            report = operations.get(name)
            if not isinstance(report, dict):
                reasons.append(f"missing_{name}")
                continue
            gpu_mean = report.get("gpu_mean_ms")
            if not isinstance(gpu_mean, (int, float)) or gpu_mean <= 0:
                reasons.append(f"{name}_missing_gpu")

    classification = "stable" if not reasons else "unstable"
    return classification, reasons


def _classification_from_entry(entry: Dict[str, Any]) -> tuple[str, List[str]]:
    """Prefer an explicit classification block, falling back to heuristics."""

    block = entry.get("classification")
    reasons: List[str] = []
    if isinstance(block, dict):
        raw_reasons = block.get("reasons")
        if isinstance(raw_reasons, list):
            reasons = [str(reason) for reason in raw_reasons if str(reason)]
        stable_flag = block.get("stable")
        if isinstance(stable_flag, bool):
            return ("stable" if stable_flag else "unstable"), reasons

    return classify_entry(entry)


def _warnings_from_entry(entry: Dict[str, Any]) -> List[str]:
    warnings = entry.get("warnings")
    if not isinstance(warnings, list):
        return []
    return [str(item) for item in warnings if str(item)]


def _host_label(host: Dict[str, Any] | None) -> str:
    if not isinstance(host, dict):
        return "—"
    for key in ("label", "host"):
        value = host.get(key)
        if isinstance(value, str) and value.strip():
            return value
    return "—"


def _host_name(host: Dict[str, Any] | None) -> str:
    if not isinstance(host, dict):
        return "—"
    value = host.get("host")
    if isinstance(value, str) and value.strip():
        return value
    return "—"


def _host_machine(host: Dict[str, Any] | None) -> str:
    if not isinstance(host, dict):
        return "—"
    value = host.get("machine")
    if isinstance(value, str) and value.strip():
        return value
    return "—"


def _host_platform(host: Dict[str, Any] | None) -> str:
    if not isinstance(host, dict):
        return "—"
    value = host.get("platform")
    if isinstance(value, str) and value.strip():
        return value
    return "—"


def _device_class(host: Dict[str, Any] | None) -> str:
    labels = host.get("labels") if isinstance(host, dict) else None
    if isinstance(labels, dict):
        value = labels.get("device_class") or labels.get("chip_type")
        if isinstance(value, str) and value:
            return value
    machine = host.get("machine") if isinstance(host, dict) else None
    if isinstance(machine, str) and machine:
        return machine
    return "—"


def _gpu_label(host: Dict[str, Any] | None) -> str:
    labels = host.get("labels") if isinstance(host, dict) else None
    if not isinstance(labels, dict):
        return "—"
    vendor = labels.get("gpu_vendor")
    model = labels.get("gpu_model")
    if isinstance(model, str) and model:
        if isinstance(vendor, str) and vendor:
            return f"{vendor}:{model}"
        return model
    if isinstance(vendor, str) and vendor:
        return vendor
    return "—"


def _host_label_value(host: Dict[str, Any] | None, key: str) -> str:
    if not isinstance(host, dict):
        return "—"
    labels = host.get("labels")
    if not isinstance(labels, dict):
        return "—"
    value = labels.get(key)
    if isinstance(value, (int, float, bool)):
        return str(value)
    if isinstance(value, str) and value.strip():
        return value
    return "—"


def build_matrix_entries(
    entries: Iterable[Dict[str, Any]],
    extra_host_labels: Sequence[tuple[str, str]] | None = None,
) -> List[Dict[str, Any]]:
    """Extract the env/metric slice we care about for Markdown/JSON export."""

    label_specs = extra_host_labels or ()
    matrix: List[Dict[str, Any]] = []
    for entry in entries:
        env = entry.get("env") or {}
        if not isinstance(env, dict):
            env = {}

        operations = entry.get("operations") or {}
        source = entry.get("_summary_source")
        host = entry.get("host") if isinstance(entry.get("host"), dict) else None
        classification, reasons = _classification_from_entry(entry)
        warnings = _warnings_from_entry(entry)
        row = {
            "env": {env_key: env.get(env_key, "—") for env_key, _ in ENV_COLUMNS},
            "status": entry.get("status", "unknown"),
            "classification": classification,
            "classification_reasons": reasons,
            "warnings": warnings,
            "duration_seconds": entry.get("duration_seconds"),
            "started_at": entry.get("started_at"),
            "completed_at": entry.get("completed_at"),
            "metrics": {
                op: (operations.get(op) or {}).get("gpu_mean_ms")
                for op in REQUIRED_OPERATIONS
            },
            "host": host or {},
            "source": source,
            "execution_mode": entry.get("execution_mode"),
            "gpu_backend": entry.get("gpu_backend"),
            "operation": entry.get("operation", "all"),
            "rows": entry.get("rows"),
            "iterations": entry.get("iterations"),
            "warmups": entry.get("warmups"),
            "host_labels": {
                key: _host_label_value(host, key) for key, _ in label_specs
            },
        }
        matrix.append(row)

    matrix.sort(
        key=lambda item: (
            item["classification"] != "stable",
            _host_label(item.get("host")),
            tuple(item["env"].get(key, "") for key, _ in ENV_COLUMNS),
        )
    )
    return matrix


def build_host_summary(
    matrix: Iterable[Dict[str, Any]],
    extra_host_labels: Sequence[tuple[str, str]] | None = None,
) -> List[Dict[str, Any]]:
    """Aggregate geometry runs per host for Stage7 evidence bundles."""

    label_specs = extra_host_labels or ()
    summary: Dict[Tuple[str, str, str], Dict[str, Any]] = {}
    for row in matrix:
        host = row.get("host") or {}
        label = _host_label(host)
        device = _device_class(host)
        gpu = _gpu_label(host)
        key = (label, device, gpu)
        entry = summary.get(key)
        if entry is None:
            entry = {
                "host_label": label,
                "host_name": _host_name(host),
                "host_machine": _host_machine(host),
                "host_platform": _host_platform(host),
                "device_class": device,
                "gpu": gpu,
                "execution_modes": set(),
                "gpu_backends": set(),
                "classification_counts": {"stable": 0, "unstable": 0, "unknown": 0},
                "total_runs": 0,
                "unstable_reasons": set(),
                "warnings": set(),
                "sources": set(),
                "host_labels": {
                    key: _host_label_value(host, key) for key, _ in label_specs
                },
            }
            summary[key] = entry

        entry["total_runs"] += 1
        classification = row.get("classification", "unknown")
        entry["classification_counts"].setdefault(classification, 0)
        entry["classification_counts"][classification] += 1

        execution_mode = row.get("execution_mode")
        if isinstance(execution_mode, str) and execution_mode.strip():
            entry["execution_modes"].add(execution_mode)
        gpu_backend = row.get("gpu_backend")
        if isinstance(gpu_backend, str) and gpu_backend.strip():
            entry["gpu_backends"].add(gpu_backend)

        for key, _ in label_specs:
            value = _host_label_value(host, key)
            if entry["host_labels"].get(key) in (None, "—") and value != "—":
                entry["host_labels"][key] = value

        for reason in row.get("classification_reasons") or []:
            if reason:
                entry["unstable_reasons"].add(str(reason))
        for warning in row.get("warnings") or []:
            if warning:
                entry["warnings"].add(str(warning))

        source = row.get("source")
        if source:
            entry["sources"].add(str(source))

    summary_rows: List[Dict[str, Any]] = []
    for entry in summary.values():
        summary_rows.append(
            {
                "host_label": entry["host_label"],
                "host_name": entry["host_name"],
                "host_machine": entry["host_machine"],
                "host_platform": entry["host_platform"],
                "device_class": entry["device_class"],
                "gpu": entry["gpu"],
                "execution_modes": sorted(entry["execution_modes"]),
                "gpu_backends": sorted(entry["gpu_backends"]),
                "total_runs": entry["total_runs"],
                "classification_counts": entry["classification_counts"],
                "unstable_reasons": sorted(entry["unstable_reasons"]),
                "warnings": sorted(entry["warnings"]),
                "sources": sorted(entry["sources"]),
                "host_labels": entry["host_labels"],
            }
        )

    summary_rows.sort(
        key=lambda item: (
            item["host_label"],
            item["device_class"],
            item["gpu"],
        )
    )
    return summary_rows


def build_env_summary(matrix: Iterable[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Aggregate stability metrics per launch-geometry configuration."""

    summary: Dict[Tuple[Tuple[str, Any], ...], Dict[str, Any]] = {}

    def env_key(row: Dict[str, Any]) -> Tuple[Tuple[str, Any], ...]:
        env = row.get("env") or {}
        if not isinstance(env, dict):
            env = {}
        return tuple((env_key, env.get(env_key, "—")) for env_key, _ in ENV_COLUMNS)

    for row in matrix:
        key = env_key(row)
        entry = summary.get(key)
        if entry is None:
            entry = {
                "env": {env_key: value for env_key, value in key},
                "classification_counts": {"stable": 0, "unstable": 0, "unknown": 0},
                "total_runs": 0,
                "duration_sum": 0.0,
                "duration_samples": 0,
                "metric_sums": {op: 0.0 for op in REQUIRED_OPERATIONS},
                "metric_samples": {op: 0 for op in REQUIRED_OPERATIONS},
                "warnings": set(),
                "unstable_reasons": set(),
                "hosts": set(),
                "sources": set(),
            }
            summary[key] = entry

        classification = row.get("classification", "unknown")
        entry["total_runs"] += 1
        entry["classification_counts"].setdefault(classification, 0)
        entry["classification_counts"][classification] += 1

        duration = row.get("duration_seconds")
        if isinstance(duration, (int, float)):
            entry["duration_sum"] += float(duration)
            entry["duration_samples"] += 1

        metrics = row.get("metrics") or {}
        if not isinstance(metrics, dict):
            metrics = {}
        for op in REQUIRED_OPERATIONS:
            value = metrics.get(op)
            if isinstance(value, (int, float)):
                entry["metric_sums"][op] += float(value)
                entry["metric_samples"][op] += 1

        host_label = _host_label(row.get("host"))
        if host_label != "—":
            entry["hosts"].add(host_label)

        source = row.get("source")
        if source:
            entry["sources"].add(str(source))

        for reason in row.get("classification_reasons") or []:
            if reason:
                entry["unstable_reasons"].add(str(reason))
        for warning in row.get("warnings") or []:
            if warning:
                entry["warnings"].add(str(warning))

    summary_rows: List[Dict[str, Any]] = []
    for entry in summary.values():
        total = entry["total_runs"] or 1
        stable = entry["classification_counts"].get("stable", 0)
        summary_rows.append(
            {
                "env": entry["env"],
                "total_runs": entry["total_runs"],
                "classification_counts": entry["classification_counts"],
                "stable_ratio": stable / total,
                "average_duration_seconds": (
                    entry["duration_sum"] / entry["duration_samples"]
                    if entry["duration_samples"]
                    else None
                ),
                "average_metrics_ms": {
                    op: (
                        entry["metric_sums"][op] / entry["metric_samples"][op]
                        if entry["metric_samples"][op]
                        else None
                    )
                    for op in REQUIRED_OPERATIONS
                },
                "hosts": sorted(entry["hosts"]),
                "sources": sorted(entry["sources"]),
                "unstable_reasons": sorted(entry["unstable_reasons"]),
                "warnings": sorted(entry["warnings"]),
            }
        )

    summary_rows.sort(
        key=lambda item: tuple(item["env"].get(env_key, "—") for env_key, _ in ENV_COLUMNS)
    )
    return summary_rows


def build_source_summary(matrix: Iterable[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Summarise runs per summary source for Stage7 evidence bundles."""

    summary: Dict[str, Dict[str, Any]] = {}
    for row in matrix:
        source = str(row.get("source") or "unknown")
        entry = summary.get(source)
        if entry is None:
            entry = {
                "source": source,
                "total_runs": 0,
                "classification_counts": {"stable": 0, "unstable": 0, "unknown": 0},
                "execution_modes": set(),
                "gpu_backends": set(),
                "hosts": set(),
                "geometries": set(),
                "unstable_reasons": set(),
                "warnings": set(),
            }
            summary[source] = entry

        entry["total_runs"] += 1
        classification = row.get("classification", "unknown")
        entry["classification_counts"].setdefault(classification, 0)
        entry["classification_counts"][classification] += 1

        execution_mode = row.get("execution_mode")
        if isinstance(execution_mode, str) and execution_mode.strip():
            entry["execution_modes"].add(execution_mode)
        gpu_backend = row.get("gpu_backend")
        if isinstance(gpu_backend, str) and gpu_backend.strip():
            entry["gpu_backends"].add(gpu_backend)

        host_label = _host_label(row.get("host"))
        if host_label != "—":
            entry["hosts"].add(host_label)

        env = row.get("env") or {}
        if isinstance(env, dict):
            env_tuple = tuple(
                (key, _format_env_value(env.get(key, "—"))) for key, _ in ENV_COLUMNS
            )
            entry["geometries"].add(env_tuple)

        for reason in row.get("classification_reasons") or []:
            if reason:
                entry["unstable_reasons"].add(str(reason))
        for warning in row.get("warnings") or []:
            if warning:
                entry["warnings"].add(str(warning))

    summary_rows: List[Dict[str, Any]] = []
    for entry in summary.values():
        summary_rows.append(
            {
                "source": entry["source"],
                "total_runs": entry["total_runs"],
                "classification_counts": entry["classification_counts"],
                "execution_modes": sorted(entry["execution_modes"]),
                "gpu_backends": sorted(entry["gpu_backends"]),
                "hosts": sorted(entry["hosts"]),
                "geometries": [
                    {key: value for key, value in geometry} for geometry in sorted(entry["geometries"])
                ],
                "unstable_reasons": sorted(entry["unstable_reasons"]),
                "warnings": sorted(entry["warnings"]),
            }
        )

    summary_rows.sort(key=lambda item: item["source"])
    return summary_rows


def render_markdown(
    matrix: Sequence[Dict[str, Any]],
    extra_host_labels: Sequence[tuple[str, str]] | None = None,
) -> str:
    """Return a Markdown table with the geometry + stability summary."""

    label_specs = extra_host_labels or ()
    headers = [label for _, label in ENV_COLUMNS]
    headers.extend(label for _, label in HOST_HEADERS)
    headers.extend(label for _, label in label_specs)
    headers.extend(label for _, label in RUN_HEADERS)
    headers.extend(
        [
            "Status",
            "Started",
            "Completed",
            "Classification",
            "Reasons",
            "Duration (s)",
            "FFT ms",
            "LDE ms",
            "Poseidon ms",
        ]
    )

    lines = ["| " + " | ".join(headers) + " |", "| " + " | ".join(["---"] * len(headers)) + " |"]
    for row in matrix:
        env_values = [_format_env_value(row["env"].get(key)) for key, _ in ENV_COLUMNS]
        metrics = row["metrics"]
        host = row.get("host") or {}
        host_labels = row.get("host_labels") or {}
        reason_text = _format_notes(
            row.get("classification_reasons") or [], row.get("warnings") or []
        )
        line = env_values + [
            _host_label(host),
            _host_name(host),
            _host_machine(host),
            _host_platform(host),
            _device_class(host),
            _gpu_label(host),
        ]
        for key, _ in label_specs:
            line.append(_format_env_value(host_labels.get(key)))
        line.extend(
            [
                _format_env_value(row.get("execution_mode")),
                _format_env_value(row.get("gpu_backend")),
                _format_env_value(row.get("operation")),
                str(row.get("status", "unknown")),
                _format_env_value(row.get("started_at")),
                _format_env_value(row.get("completed_at")),
                row.get("classification", "unknown"),
                reason_text,
                _format_number(row.get("duration_seconds"), decimals=3),
                _format_number(metrics.get("fft")),
                _format_number(metrics.get("lde")),
                _format_number(metrics.get("poseidon_hash_columns")),
            ]
        )
        lines.append("| " + " | ".join(line) + " |")

    stable = sum(1 for row in matrix if row.get("classification") == "stable")
    summary = f"Stable entries: {stable}/{len(matrix)}"
    return "\n".join(lines + ["", summary]) + "\n"


def build_reason_summary(matrix: Iterable[Dict[str, Any]]) -> Dict[str, Dict[str, Any]]:
    """Aggregate instability reasons and warnings for Stage7-2 triage."""

    buckets: Dict[str, Dict[str, Any]] = {
        "classification_reasons": {},
        "warnings": {},
    }

    def record(bucket: Dict[str, Any], key: str, host: str, source: str | None) -> None:
        entry = bucket.setdefault(key, {"count": 0, "hosts": set(), "sources": set()})
        entry["count"] += 1
        if host != "—":
            entry["hosts"].add(host)
        if source:
            entry["sources"].add(source)

    for row in matrix:
        host_label = _host_label(row.get("host"))
        source = row.get("source")
        for reason in row.get("classification_reasons") or []:
            record(buckets["classification_reasons"], str(reason), host_label, source)
        for warning in row.get("warnings") or []:
            record(buckets["warnings"], str(warning), host_label, source)

    for bucket in buckets.values():
        for entry in bucket.values():
            entry["hosts"] = sorted(entry["hosts"])
            entry["sources"] = sorted(entry["sources"])

    return buckets


def _format_env_value(value: Any) -> str:
    if value is None:
        return "—"
    return str(value)


def _format_number(value: Any, *, decimals: int = 1) -> str:
    if not isinstance(value, (int, float)):
        return "—"
    if isinstance(value, int):
        return str(value)
    return f"{value:.{decimals}f}"


def _format_notes(reasons: List[str], warnings: List[str]) -> str:
    notes: List[str] = []
    if reasons:
        notes.append("reasons:" + ";".join(reasons))
    if warnings:
        notes.append("warnings:" + ";".join(warnings))
    if not notes:
        return "—"
    return " | ".join(notes)


def load_summary(path: pathlib.Path) -> Tuple[Dict[str, Any] | None, List[Dict[str, Any]]]:
    with path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    if isinstance(payload, list):
        return None, payload
    if isinstance(payload, dict):
        runs = payload.get("runs")
        if runs is None:
            raise ValueError(f"summary payload at {path} missing runs array")
        if not isinstance(runs, list):
            raise ValueError(f"summary payload at {path} has invalid runs array")
        host = payload.get("host")
        if isinstance(host, dict):
            return host, runs
        return None, runs
    raise ValueError(f"summary payload at {path} must be a list or object")


def gather_summary_entries(summary_paths: Sequence[pathlib.Path]) -> List[Dict[str, Any]]:
    aggregated: List[Dict[str, Any]] = []
    for path in summary_paths:
        host_meta, runs = load_summary(path)
        for run in runs:
            entry = dict(run)
            if host_meta and "host" not in entry:
                entry["host"] = host_meta
            entry["_summary_source"] = str(path)
            aggregated.append(entry)
    return aggregated


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Summarise FASTPQ Metal geometry sweeps into a stability matrix."
    )
    parser.add_argument(
        "--summary",
        action="append",
        default=None,
        help=(
            "Path to a summary.json emitted by launch_geometry_sweep.py. "
            "Repeat to merge multiple hosts (default: ./summary.json)."
        ),
    )
    parser.add_argument(
        "--markdown-out",
        default=None,
        help="Optional path for the Markdown table (default: <summary_dir>/geometry_matrix.md).",
    )
    parser.add_argument(
        "--json-out",
        default=None,
        help="Optional path for a machine-readable JSON matrix (default: disabled).",
    )
    parser.add_argument(
        "--host-summary-out",
        default=None,
        help="Optional path for per-host stability summary JSON (default: disabled).",
    )
    parser.add_argument(
        "--env-summary-out",
        default=None,
        help=(
            "Optional path for per-geometry stability summary JSON "
            "(default: disabled)."
        ),
    )
    parser.add_argument(
        "--reason-summary-out",
        default=None,
        help="Optional path for aggregated instability reason JSON (default: disabled).",
    )
    parser.add_argument(
        "--source-summary-out",
        default=None,
        help="Optional path for per-summary stability JSON (default: disabled).",
    )
    parser.add_argument(
        "--host-label",
        action="append",
        dest="host_labels",
        default=None,
        metavar="KEY[:HEADER]",
        help=(
            "Record extra host.labels entries (format KEY or KEY:Header). "
            "Repeat for multiple labels."
        ),
    )
    parser.add_argument(
        "--print",
        dest="print_table",
        action="store_true",
        help="Echo the Markdown table to stdout in addition to writing the file.",
    )
    args = parser.parse_args()

    summary_inputs = args.summary or ["summary.json"]
    summary_paths = [pathlib.Path(path) for path in summary_inputs]
    for summary_path in summary_paths:
        if not summary_path.exists():
            parser.error(f"summary file {summary_path} does not exist")

    label_specs: List[tuple[str, str]] = []
    for raw in args.host_labels or []:
        if not raw:
            continue
        if ":" in raw:
            key, header = raw.split(":", 1)
        elif "=" in raw:
            key, header = raw.split("=", 1)
        else:
            key, header = raw, raw.replace("_", " ").title()
        key = key.strip()
        header = header.strip() or key.replace("_", " ").title()
        if not key:
            continue
        label_specs.append((key, header))

    entries = gather_summary_entries(summary_paths)
    matrix_entries = build_matrix_entries(entries, label_specs)
    markdown = render_markdown(matrix_entries, label_specs)

    markdown_path = (
        pathlib.Path(args.markdown_out)
        if args.markdown_out
        else summary_paths[0].with_name("geometry_matrix.md")
    )
    markdown_path.write_text(markdown, encoding="utf-8")

    if args.print_table:
        print(markdown, end="")

    if args.json_out:
        json_path = pathlib.Path(args.json_out)
        json_path.write_text(json.dumps(matrix_entries, indent=2), encoding="utf-8")

    if args.host_summary_out:
        host_summary = build_host_summary(matrix_entries, label_specs)
        host_json_path = pathlib.Path(args.host_summary_out)
        host_json_path.write_text(json.dumps(host_summary, indent=2), encoding="utf-8")

    if args.env_summary_out:
        env_summary = build_env_summary(matrix_entries)
        env_json_path = pathlib.Path(args.env_summary_out)
        env_json_path.write_text(json.dumps(env_summary, indent=2), encoding="utf-8")

    if args.source_summary_out:
        source_summary = build_source_summary(matrix_entries)
        source_json_path = pathlib.Path(args.source_summary_out)
        source_json_path.write_text(json.dumps(source_summary, indent=2), encoding="utf-8")

    if args.reason_summary_out:
        reason_summary = build_reason_summary(matrix_entries)
        reasons_path = pathlib.Path(args.reason_summary_out)
        reasons_path.write_text(json.dumps(reason_summary, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
