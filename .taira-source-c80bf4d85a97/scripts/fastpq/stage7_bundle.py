#!/usr/bin/env python3
"""
Assemble FASTPQ Stage7 rollout evidence from geometry sweeps and row-usage snapshots.

This helper consumes the `summary.json` files emitted by `launch_geometry_sweep.py`,
builds the cross-host stability matrices, validates `row_usage/*.json` snapshots,
and writes a single bundle JSON suitable for release/runbook evidence.
"""

from __future__ import annotations

import argparse
import datetime
import json
import pathlib
import sys
from typing import Any, Dict, Iterable, List, Sequence, Tuple

from scripts.fastpq import geometry_matrix
from scripts.fastpq import validate_row_usage_snapshot


def _now_iso() -> str:
    return (
        datetime.datetime.now(datetime.timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z")
    )


def _parse_host_labels(raw: Sequence[str] | None) -> List[Tuple[str, str]]:
    labels: List[Tuple[str, str]] = []
    for item in raw or ():
        if not item:
            continue
        if ":" in item:
            key, header = item.split(":", 1)
        elif "=" in item:
            key, header = item.split("=", 1)
        else:
            key, header = item, item.replace("_", " ").title()
        key = key.strip()
        header = header.strip() or key.replace("_", " ").title()
        if key:
            labels.append((key, header))
    return labels


def summarize_row_usage(path: pathlib.Path) -> Dict[str, Any]:
    """Validate and aggregate a single row_usage snapshot."""

    payload = json.loads(path.read_text(encoding="utf-8"))
    validate_row_usage_snapshot.validate_row_usage_snapshot(payload, path)
    counts: Dict[str, int] = {field: 0 for field in validate_row_usage_snapshot.COUNT_FIELDS}
    batches = payload["fastpq_batches"]
    for batch in batches:
        usage = batch["row_usage"]
        for field in counts:
            counts[field] += int(usage[field])

    total_rows = counts["total_rows"]
    transfer_rows = counts["transfer_rows"]
    aggregate_ratio = 0.0 if total_rows == 0 else transfer_rows / total_rows
    return {
        "path": str(path),
        "batches": len(batches),
        "counts": counts,
        "aggregate_transfer_ratio": aggregate_ratio,
    }


def aggregate_row_usage(reports: Iterable[Dict[str, Any]]) -> Dict[str, Any]:
    totals: Dict[str, int] = {field: 0 for field in validate_row_usage_snapshot.COUNT_FIELDS}
    total_batches = 0
    for report in reports:
        total_batches += int(report.get("batches", 0))
        counts = report.get("counts") or {}
        for field in totals:
            value = counts.get(field)
            if isinstance(value, int) and not isinstance(value, bool):
                totals[field] += value
    total_rows = totals["total_rows"]
    transfer_rows = totals["transfer_rows"]
    aggregate_ratio = 0.0 if total_rows == 0 else transfer_rows / total_rows
    return {
        "batches": total_batches,
        "counts": totals,
        "aggregate_transfer_ratio": aggregate_ratio,
    }


def build_stage7_bundle(
    summary_paths: Sequence[pathlib.Path],
    row_usage_paths: Sequence[pathlib.Path] | None = None,
    host_labels: Sequence[Tuple[str, str]] | None = None,
) -> Dict[str, Any]:
    """Return a Stage7 evidence bundle composed of geometry and row-usage summaries."""

    entries = geometry_matrix.gather_summary_entries(summary_paths)
    matrix = geometry_matrix.build_matrix_entries(entries, host_labels or [])
    bundle: Dict[str, Any] = {
        "generated_at": _now_iso(),
        "inputs": {
            "summaries": [str(path) for path in summary_paths],
            "row_usage": [str(path) for path in row_usage_paths or []],
        },
        "geometry": {
            "matrix": matrix,
            "host_summary": geometry_matrix.build_host_summary(matrix, host_labels or []),
            "env_summary": geometry_matrix.build_env_summary(matrix),
            "reason_summary": geometry_matrix.build_reason_summary(matrix),
            "source_summary": geometry_matrix.build_source_summary(matrix),
        },
    }

    if row_usage_paths:
        reports: List[Dict[str, Any]] = []
        for path in row_usage_paths:
            reports.append(summarize_row_usage(path))
        bundle["row_usage"] = {
            "snapshots": reports,
            "aggregate": aggregate_row_usage(reports),
        }

    return bundle


def _write_bundle(
    bundle: Dict[str, Any],
    bundle_path: pathlib.Path,
    markdown_path: pathlib.Path | None,
    host_labels: Sequence[Tuple[str, str]],
) -> None:
    bundle_path.write_text(json.dumps(bundle, indent=2), encoding="utf-8")
    if markdown_path is not None:
        matrix = bundle["geometry"]["matrix"]
        markdown = geometry_matrix.render_markdown(matrix, host_labels)
        markdown_path.write_text(markdown, encoding="utf-8")


def _parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a FASTPQ Stage7 evidence bundle from geometry + row_usage artefacts."
    )
    parser.add_argument(
        "--summary",
        action="append",
        required=True,
        help="Path to a geometry sweep summary.json (repeat for multiple hosts).",
    )
    parser.add_argument(
        "--row-usage",
        action="append",
        dest="row_usage",
        default=None,
        help="Path to a row_usage JSON snapshot (repeatable).",
    )
    parser.add_argument(
        "--host-label",
        action="append",
        dest="host_labels",
        default=None,
        metavar="KEY[:HEADER]",
        help="Record additional host.labels entries in the matrix output.",
    )
    parser.add_argument(
        "--bundle-out",
        default=None,
        help="Target JSON bundle path (default: <summary_dir>/stage7_bundle.json).",
    )
    parser.add_argument(
        "--markdown-out",
        default=None,
        help="Optional Markdown table path (default: <summary_dir>/stage7_geometry.md).",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    summary_paths = [pathlib.Path(path) for path in args.summary]
    for path in summary_paths:
        if not path.exists():
            print(f"[fastpq] summary file not found: {path}", file=sys.stderr)
            return 1

    row_usage_paths = [pathlib.Path(path) for path in (args.row_usage or [])]
    for path in row_usage_paths:
        if not path.exists():
            print(f"[fastpq] row_usage file not found: {path}", file=sys.stderr)
            return 1

    host_labels = _parse_host_labels(args.host_labels)
    try:
        bundle = build_stage7_bundle(summary_paths, row_usage_paths, host_labels)
    except validate_row_usage_snapshot.ValidationError as exc:
        print(f"[fastpq] row_usage validation failed: {exc}", file=sys.stderr)
        return 1
    except (json.JSONDecodeError, ValueError) as exc:  # decode or gather errors
        print(f"[fastpq] failed to load inputs: {exc}", file=sys.stderr)
        return 1

    default_root = summary_paths[0].parent if summary_paths else pathlib.Path(".")
    bundle_path = (
        pathlib.Path(args.bundle_out) if args.bundle_out else default_root / "stage7_bundle.json"
    )
    markdown_path = (
        pathlib.Path(args.markdown_out)
        if args.markdown_out is not None
        else default_root / "stage7_geometry.md"
    )

    _write_bundle(bundle, bundle_path, markdown_path, host_labels)
    print(f"[fastpq] wrote bundle: {bundle_path}")
    if markdown_path is not None:
        print(f"[fastpq] wrote geometry markdown: {markdown_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
