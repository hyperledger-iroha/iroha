#!/usr/bin/env python3
"""Summarize signed FASTPQ rollout manifests into reviewer-friendly artefacts."""
from __future__ import annotations

import argparse
import datetime
import json
import sys
from pathlib import Path
from typing import Any, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent.parent
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.fastpq import wrap_benchmark


def _now_iso() -> str:
    return (
        datetime.datetime.now(datetime.timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z")
    )


def relative_path(path: Path, root: Path) -> str:
    """Render a path relative to a preferred root when possible."""

    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def resolve_bench_path(
    path_value: str | None,
    *,
    bundle_dir: Path,
    repo_root: Path,
) -> Path | None:
    """Resolve a bench path from a signed manifest into a readable local file."""

    if not isinstance(path_value, str) or not path_value.strip():
        return None

    raw = Path(path_value)
    candidates: list[Path] = []
    if raw.is_absolute():
        candidates.append(raw)
    else:
        candidates.append(bundle_dir / raw)
        candidates.append(repo_root / raw)
    candidates.append(bundle_dir / raw.name)

    seen: set[Path] = set()
    for candidate in candidates:
        try:
            resolved = candidate.resolve()
        except OSError:
            continue
        if resolved in seen:
            continue
        seen.add(resolved)
        if resolved.is_file():
            return resolved

    matches = sorted(bundle_dir.rglob(raw.name), key=lambda entry: entry.as_posix())
    if len(matches) == 1:
        return matches[0].resolve()
    return None


def summarize_poseidon_microbench(entry: dict[str, Any] | None) -> dict[str, Any] | None:
    """Keep only the reviewer-facing Poseidon microbench fields."""

    if not isinstance(entry, dict):
        return None
    default = entry.get("default") or {}
    scalar = entry.get("scalar_lane") or {}
    summary = {
        "speedup_vs_scalar": entry.get("speedup_vs_scalar"),
        "default_mean_ms": default.get("mean_ms"),
        "scalar_mean_ms": scalar.get("mean_ms"),
        "columns": default.get("columns") or scalar.get("columns"),
    }
    if all(value is None for value in summary.values()):
        return None
    return summary


def summarize_bench_entry(
    entry: dict[str, Any],
    *,
    bundle_dir: Path,
    repo_root: Path,
) -> dict[str, Any]:
    """Summarize one bench entry from a signed FASTPQ rollout manifest."""

    manifest_path_value = entry.get("path")
    resolved_path = resolve_bench_path(manifest_path_value, bundle_dir=bundle_dir, repo_root=repo_root)
    bench_blob: dict[str, Any] | None = None
    normalized_report: dict[str, Any] | None = None
    if resolved_path is not None:
        bench_blob = json.loads(resolved_path.read_text(encoding="utf-8"))
        normalized_report = wrap_benchmark.normalize_report(bench_blob)

    bundle_metadata = bench_blob.get("metadata") if isinstance(bench_blob, dict) else {}
    labels = bundle_metadata.get("labels") if isinstance(bundle_metadata, dict) else {}
    if not isinstance(labels, dict):
        labels = {}
    benchmarks = bench_blob.get("benchmarks") if isinstance(bench_blob, dict) else {}
    if not isinstance(benchmarks, dict):
        benchmarks = {}

    report_operations = (
        normalized_report.get("operations")
        if isinstance(normalized_report, dict)
        else None
    )
    benchmark_operations = benchmarks.get("operations")
    operations = report_operations if isinstance(report_operations, list) else benchmark_operations
    available_operations = sorted(
        {
            operation.get("operation")
            for operation in operations or []
            if isinstance(operation, dict) and isinstance(operation.get("operation"), str)
        }
    )

    column_count = None
    if isinstance(normalized_report, dict):
        raw_column_count = normalized_report.get("column_count")
        if isinstance(raw_column_count, int):
            column_count = raw_column_count
    if column_count is None:
        raw_column_count = benchmarks.get("column_count")
        if isinstance(raw_column_count, int):
            column_count = raw_column_count

    manifest_metadata = entry.get("metadata") if isinstance(entry.get("metadata"), dict) else {}
    source_command = None
    if isinstance(bundle_metadata, dict):
        source_command = bundle_metadata.get("command")
    if not isinstance(source_command, str) or not source_command.strip():
        source_command = manifest_metadata.get("command")

    summary = {
        "label": entry.get("label"),
        "bench_path": manifest_path_value,
        "resolved_bench_path": (
            relative_path(resolved_path, bundle_dir) if resolved_path is not None else None
        ),
        "rows": entry.get("rows"),
        "padded_rows": entry.get("padded_rows"),
        "iterations": entry.get("iterations"),
        "warmups": entry.get("warmups"),
        "gpu_backend": entry.get("gpu_backend"),
        "gpu_available": entry.get("gpu_available"),
        "operation_filter": entry.get("operation_filter"),
        "matrix_operation_filters": entry.get("matrix_operation_filters"),
        "available_operations": available_operations,
        "column_count": column_count,
        "device_class": labels.get("device_class"),
        "gpu_kind": labels.get("gpu_kind"),
        "chip_type": labels.get("chip_type"),
        "gpu_model": labels.get("gpu_model"),
        "host": manifest_metadata.get("host"),
        "platform": manifest_metadata.get("platform"),
        "machine": manifest_metadata.get("machine"),
        "source_command": source_command,
        "poseidon_microbench": summarize_poseidon_microbench(entry.get("poseidon_microbench")),
    }
    if resolved_path is None:
        summary["load_warning"] = "bench file could not be resolved from the archived bundle"
    return summary


def build_rollout_summary(
    manifest_path: Path,
    *,
    bundle_dir: Path | None = None,
    repo_root: Path | None = None,
) -> dict[str, Any]:
    """Build a compact summary for a signed FASTPQ rollout manifest."""

    bundle_root = bundle_dir.resolve() if bundle_dir is not None else manifest_path.parent.resolve()
    repo = repo_root.resolve() if repo_root is not None else REPO_ROOT.resolve()
    signed = json.loads(manifest_path.read_text(encoding="utf-8"))
    payload = signed.get("payload")
    if not isinstance(payload, dict):
        raise ValueError(f"manifest missing payload: {manifest_path}")
    benches = payload.get("benches")
    if not isinstance(benches, list):
        raise ValueError(f"manifest missing benches: {manifest_path}")

    summarized_benches = [
        summarize_bench_entry(entry, bundle_dir=bundle_root, repo_root=repo)
        for entry in benches
        if isinstance(entry, dict)
    ]
    constraints = payload.get("constraints")
    if not isinstance(constraints, dict):
        constraints = {}

    return {
        "generated_at": _now_iso(),
        "manifest": relative_path(manifest_path.resolve(), bundle_root),
        "generator_commit": payload.get("generator_commit"),
        "generated_unix_ms": payload.get("generated_unix_ms"),
        "signature_present": signed.get("signature") is not None,
        "bench_labels": [
            bench.get("label")
            for bench in summarized_benches
            if isinstance(bench.get("label"), str)
        ],
        "constraints": constraints,
        "benches": summarized_benches,
    }


def render_constraints(constraints: dict[str, Any]) -> list[str]:
    """Render the manifest constraints into Markdown bullets."""

    lines: list[str] = []
    require_rows = constraints.get("require_rows")
    if isinstance(require_rows, int):
        lines.append(f"- Require rows: **{require_rows:,}**")

    max_ops = constraints.get("max_operation_ms")
    if isinstance(max_ops, dict) and max_ops:
        rendered = ", ".join(f"`{name}` <= {limit}" for name, limit in sorted(max_ops.items()))
        lines.append(f"- Max operation ms: {rendered}")

    min_speed = constraints.get("min_operation_speedup")
    if isinstance(min_speed, dict) and min_speed:
        rendered = ", ".join(f"`{name}` >= {limit}" for name, limit in sorted(min_speed.items()))
        lines.append(f"- Min operation speedup: {rendered}")
    return lines


def render_markdown(summary: dict[str, Any]) -> str:
    """Render a compact Markdown view of a rollout manifest summary."""

    lines = ["# FASTPQ Rollout Summary", ""]
    lines.append(f"- Manifest: `{summary.get('manifest', 'fastpq_bench_manifest.json')}`")
    signature = "present" if summary.get("signature_present") else "absent"
    lines.append(f"- Signature: **{signature}**")
    labels = summary.get("bench_labels") or []
    if labels:
        rendered_labels = ", ".join(f"`{label}`" for label in labels if isinstance(label, str))
        lines.append(f"- Bench labels: {rendered_labels}")
    generator_commit = summary.get("generator_commit")
    if isinstance(generator_commit, str) and generator_commit:
        lines.append(f"- Generator commit: `{generator_commit}`")
    lines.extend(render_constraints(summary.get("constraints") or {}))

    for bench in summary.get("benches") or []:
        label = bench.get("label") or "unknown"
        lines.extend(["", f"## {str(label).upper()}", ""])
        backend = bench.get("gpu_backend") or "n/a"
        lines.append(f"- Backend: `{backend}`")
        device_parts = [
            value
            for value in (
                bench.get("device_class"),
                bench.get("gpu_kind"),
                bench.get("gpu_model"),
            )
            if isinstance(value, str) and value
        ]
        if device_parts:
            lines.append(f"- Device: {' / '.join(f'`{value}`' for value in device_parts)}")
        rows = bench.get("rows")
        padded = bench.get("padded_rows")
        if isinstance(rows, int):
            padded_suffix = f", padded **{padded:,}**" if isinstance(padded, int) else ""
            lines.append(f"- Rows: **{rows:,}**{padded_suffix}")
        operation_filter = bench.get("operation_filter")
        if isinstance(operation_filter, str) and operation_filter:
            suffix = " (focused capture)" if operation_filter != "all" else ""
            lines.append(f"- Operation filter: `{operation_filter}`{suffix}")
        matrix_filters = bench.get("matrix_operation_filters")
        if isinstance(matrix_filters, list) and matrix_filters:
            rendered = ", ".join(f"`{item}`" for item in matrix_filters if isinstance(item, str))
            lines.append(f"- Matrix filters: {rendered}")
        operations = bench.get("available_operations")
        if isinstance(operations, list) and operations:
            rendered = ", ".join(f"`{item}`" for item in operations if isinstance(item, str))
            lines.append(f"- Operations present: {rendered}")
        column_count = bench.get("column_count")
        if isinstance(column_count, int):
            lines.append(f"- Column count: **{column_count}**")
        bench_path = bench.get("resolved_bench_path") or bench.get("bench_path")
        if isinstance(bench_path, str) and bench_path:
            lines.append(f"- Bench file: `{bench_path}`")
        source_command = bench.get("source_command")
        if isinstance(source_command, str) and source_command:
            lines.append(f"- Command: `{source_command}`")
        poseidon = bench.get("poseidon_microbench")
        if isinstance(poseidon, dict):
            speedup = poseidon.get("speedup_vs_scalar")
            default_mean = poseidon.get("default_mean_ms")
            scalar_mean = poseidon.get("scalar_mean_ms")
            fragments: list[str] = []
            if isinstance(default_mean, (int, float)):
                fragments.append(f"default {default_mean:.3f} ms")
            if isinstance(scalar_mean, (int, float)):
                fragments.append(f"scalar {scalar_mean:.3f} ms")
            if isinstance(speedup, (int, float)):
                fragments.append(f"speedup x{speedup:.3f}")
            if fragments:
                lines.append("- Poseidon microbench: " + ", ".join(fragments))
        warning = bench.get("load_warning")
        if isinstance(warning, str) and warning:
            lines.append(f"- Note: {warning}")
    return "\n".join(lines) + "\n"


def write_summary(summary: dict[str, Any], *, json_out: Path, markdown_out: Path) -> None:
    """Write JSON and Markdown rollout summaries."""

    json_out.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    markdown_out.write_text(render_markdown(summary), encoding="utf-8")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse CLI arguments."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        required=True,
        type=Path,
        help="Path to the signed fastpq_bench_manifest.json file.",
    )
    parser.add_argument(
        "--bundle-dir",
        type=Path,
        default=None,
        help="Bundle root used for resolving and relativizing bench paths (default: manifest directory).",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=REPO_ROOT,
        help="Repository root used when manifest bench paths are repo-relative.",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        default=None,
        help="Destination JSON summary path (default: <manifest_dir>/fastpq_rollout_summary.json).",
    )
    parser.add_argument(
        "--markdown-out",
        type=Path,
        default=None,
        help="Destination Markdown summary path (default: <manifest_dir>/fastpq_rollout_summary.md).",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entrypoint."""

    args = parse_args(argv)
    manifest_path = args.manifest.resolve()
    if not manifest_path.is_file():
        print(f"[fastpq] manifest not found: {manifest_path}", file=sys.stderr)
        return 1

    bundle_dir = args.bundle_dir.resolve() if args.bundle_dir is not None else manifest_path.parent
    json_out = args.json_out or manifest_path.parent / "fastpq_rollout_summary.json"
    markdown_out = args.markdown_out or manifest_path.parent / "fastpq_rollout_summary.md"

    try:
        summary = build_rollout_summary(
            manifest_path,
            bundle_dir=bundle_dir,
            repo_root=args.repo_root,
        )
    except (json.JSONDecodeError, OSError, ValueError) as exc:
        print(f"[fastpq] failed to summarize rollout manifest: {exc}", file=sys.stderr)
        return 1

    write_summary(summary, json_out=json_out, markdown_out=markdown_out)
    print(f"[fastpq] wrote rollout summary: {json_out}")
    print(f"[fastpq] wrote rollout markdown: {markdown_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
