#!/usr/bin/env python3
"""Compare Grafana dashboard exports for Android vs Rust telemetry parity."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, Iterable, Iterator, Mapping, Sequence


MetricId = str


@dataclass(frozen=True)
class PanelInfo:
    """Normalized representation of a Grafana panel we care about."""

    metric_id: MetricId
    title: str
    panel_type: str
    unit: str | None
    target_count: int
    expr_hash: str


@dataclass
class PanelDiff:
    """Diff summary for two dashboards."""

    missing_in_android: list[MetricId] = field(default_factory=list)
    missing_in_rust: list[MetricId] = field(default_factory=list)
    unit_mismatches: list[dict[str, Any]] = field(default_factory=list)
    target_count_mismatches: list[dict[str, Any]] = field(default_factory=list)
    expr_mismatches: list[dict[str, Any]] = field(default_factory=list)

    @property
    def ok(self) -> bool:  # pragma: no cover - trivial property
        return (
            not self.missing_in_android
            and not self.missing_in_rust
            and not self.unit_mismatches
            and not self.target_count_mismatches
            and not self.expr_mismatches
        )


@dataclass
class Allowances:
    """Allowlist of known/intentional differences."""

    missing_in_android: set[MetricId] = field(default_factory=set)
    missing_in_rust: set[MetricId] = field(default_factory=set)
    unit_diff: set[MetricId] = field(default_factory=set)
    target_diff: set[MetricId] = field(default_factory=set)
    expr_diff: set[MetricId] = field(default_factory=set)

    def merge(self, other: Allowances) -> None:
        """Merge another allowance set into this one."""

        self.missing_in_android |= other.missing_in_android
        self.missing_in_rust |= other.missing_in_rust
        self.unit_diff |= other.unit_diff
        self.target_diff |= other.target_diff
        self.expr_diff |= other.expr_diff

    @classmethod
    def from_path(cls, path: Path) -> Allowances:
        """Load allowances from a JSON file.

        Expected shape:

        {
            "missing_in_android": ["metric_id"],
            "missing_in_rust": [],
            "unit_diff": [],
            "target_diff": [],
            "expr_diff": []
        }
        """

        data = json.loads(path.read_text(encoding="utf-8"))
        return cls(
            missing_in_android=set(data.get("missing_in_android", [])),
            missing_in_rust=set(data.get("missing_in_rust", [])),
            unit_diff=set(data.get("unit_diff", [])),
            target_diff=set(data.get("target_diff", [])),
            expr_diff=set(data.get("expr_diff", [])),
        )


def _load_dashboard(path: Path) -> Mapping[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:  # pragma: no cover - defensive guard
        raise SystemExit(f"Dashboard not found: {path}") from exc


def _iter_panels(nodes: Iterable[Mapping[str, Any]]) -> Iterator[Mapping[str, Any]]:
    """Yield every panel, recursing into nested panel arrays."""

    for node in nodes:
        yield node
        for child_key in ("panels", "cards"):
            child_nodes = node.get(child_key)
            if isinstance(child_nodes, list):
                yield from _iter_panels(child_nodes)


def _normalize_expr(expr: str) -> str:
    return " ".join(expr.strip().split())


def _hash_targets(targets: Sequence[str]) -> str:
    if not targets:
        return "none"
    digest = hashlib.sha256()
    for expr in sorted(targets):
        digest.update(expr.encode("utf-8"))
        digest.update(b"\0")
    return digest.hexdigest()


def _extract_targets(panel: Mapping[str, Any]) -> list[str]:
    normalized: list[str] = []
    for target in panel.get("targets", []):
        if not isinstance(target, Mapping):
            continue
        expr = (
            target.get("expr")
            or target.get("expression")
            or target.get("query")
            or target.get("rawSql")
        )
        if isinstance(expr, str) and expr.strip():
            normalized.append(_normalize_expr(expr))
    return normalized


def _metric_id(panel: Mapping[str, Any]) -> MetricId:
    field_cfg = panel.get("fieldConfig") or {}
    defaults = field_cfg.get("defaults") or {}
    custom = defaults.get("custom") or {}
    metric_id = custom.get("metric_id")
    if isinstance(metric_id, str) and metric_id.strip():
        return metric_id.strip()
    title = panel.get("title")
    if isinstance(title, str) and title.strip():
        return title.strip()
    raise ValueError("Panel missing both metric_id and title")


def collect_panel_info(tree: Mapping[str, Any]) -> Dict[MetricId, PanelInfo]:
    """Build a mapping of metric id → panel info from a Grafana dashboard."""

    panels = tree.get("panels")
    if not isinstance(panels, list):
        return {}

    info: Dict[MetricId, PanelInfo] = {}
    for panel in _iter_panels(panels):
        if panel.get("type") == "row":
            continue
        try:
            metric_id = _metric_id(panel)
        except ValueError:
            continue  # Skip structural rows lacking a title/metric id.
        targets = _extract_targets(panel)
        expr_hash = _hash_targets(targets)
        defaults = (panel.get("fieldConfig") or {}).get("defaults") or {}
        unit = defaults.get("unit")
        panel_type = panel.get("type", "unknown")
        if metric_id in info:
            raise ValueError(f"Duplicate metric_id '{metric_id}' in dashboard")
        info[metric_id] = PanelInfo(
            metric_id=metric_id,
            title=panel.get("title", metric_id),
            panel_type=panel_type,
            unit=unit if isinstance(unit, str) else None,
            target_count=len(targets),
            expr_hash=expr_hash,
        )
    return info


def compare_dashboards(
    android: Mapping[str, Any],
    rust: Mapping[str, Any],
    allowances: Allowances | None = None,
) -> PanelDiff:
    allowances = allowances or Allowances()

    android_map = collect_panel_info(android)
    rust_map = collect_panel_info(rust)

    diff = PanelDiff()
    android_keys = set(android_map)
    rust_keys = set(rust_map)

    diff.missing_in_android = sorted(
        k for k in (rust_keys - android_keys) if k not in allowances.missing_in_android
    )
    diff.missing_in_rust = sorted(
        k for k in (android_keys - rust_keys) if k not in allowances.missing_in_rust
    )

    for metric_id in sorted(android_keys & rust_keys):
        android_panel = android_map[metric_id]
        rust_panel = rust_map[metric_id]
        if (
            android_panel.unit != rust_panel.unit
            and metric_id not in allowances.unit_diff
        ):
            diff.unit_mismatches.append(
                {
                    "metric_id": metric_id,
                    "android_unit": android_panel.unit,
                    "rust_unit": rust_panel.unit,
                }
            )
        if (
            android_panel.target_count != rust_panel.target_count
            and metric_id not in allowances.target_diff
        ):
            diff.target_count_mismatches.append(
                {
                    "metric_id": metric_id,
                    "android_targets": android_panel.target_count,
                    "rust_targets": rust_panel.target_count,
                }
            )
        if (
            android_panel.expr_hash != rust_panel.expr_hash
            and metric_id not in allowances.expr_diff
        ):
            diff.expr_mismatches.append(
                {
                    "metric_id": metric_id,
                    "android_expr_hash": android_panel.expr_hash,
                    "rust_expr_hash": rust_panel.expr_hash,
                }
            )
    return diff


def _parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare Android vs Rust telemetry dashboards for parity.",
    )
    parser.add_argument("--android", required=True, type=Path, help="Android dashboard JSON export.")
    parser.add_argument("--rust", required=True, type=Path, help="Rust dashboard JSON export.")
    parser.add_argument("--output", type=Path, help="Optional path to write the diff summary JSON.")
    parser.add_argument(
        "--allow-file",
        action="append",
        type=Path,
        default=[],
        help="JSON file describing known differences (see script header).",
    )
    parser.add_argument(
        "--allow-missing-android",
        action="append",
        default=[],
        help="Metric IDs allowed to be missing from the Android dashboard.",
    )
    parser.add_argument(
        "--allow-missing-rust",
        action="append",
        default=[],
        help="Metric IDs allowed to be missing from the Rust dashboard.",
    )
    parser.add_argument(
        "--allow-unit-diff",
        action="append",
        default=[],
        help="Metric IDs allowed to have differing units.",
    )
    parser.add_argument(
        "--allow-target-diff",
        action="append",
        default=[],
        help="Metric IDs allowed to have differing target counts.",
    )
    parser.add_argument(
        "--allow-expr-diff",
        action="append",
        default=[],
        help="Metric IDs allowed to have differing PromQL expressions.",
    )
    return parser.parse_args(argv)


def _build_allowances(args: argparse.Namespace) -> Allowances:
    allowances = Allowances()
    for allow_path in args.allow_file:
        allowances.merge(Allowances.from_path(allow_path))
    allowances.missing_in_android |= set(args.allow_missing_android)
    allowances.missing_in_rust |= set(args.allow_missing_rust)
    allowances.unit_diff |= set(args.allow_unit_diff)
    allowances.target_diff |= set(args.allow_target_diff)
    allowances.expr_diff |= set(args.allow_expr_diff)
    return allowances


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv or sys.argv[1:])
    allowances = _build_allowances(args)
    android_dashboard = _load_dashboard(args.android)
    rust_dashboard = _load_dashboard(args.rust)
    diff = compare_dashboards(android_dashboard, rust_dashboard, allowances)

    summary = {
        "android_dashboard": str(args.android),
        "rust_dashboard": str(args.rust),
        "missing_in_android": diff.missing_in_android,
        "missing_in_rust": diff.missing_in_rust,
        "unit_mismatches": diff.unit_mismatches,
        "target_count_mismatches": diff.target_count_mismatches,
        "expr_mismatches": diff.expr_mismatches,
        "ok": diff.ok,
    }

    if args.output:
        args.output.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

    print(
        f"Compared {args.android.name} vs {args.rust.name}: "
        f"{'OK' if diff.ok else 'differences found'}"
    )
    if diff.missing_in_android:
        print(f"  Missing in Android : {', '.join(diff.missing_in_android)}")
    if diff.missing_in_rust:
        print(f"  Missing in Rust    : {', '.join(diff.missing_in_rust)}")
    if diff.unit_mismatches:
        print(f"  Unit mismatches    : {len(diff.unit_mismatches)}")
    if diff.target_count_mismatches:
        print(f"  Target mismatches  : {len(diff.target_count_mismatches)}")
    if diff.expr_mismatches:
        print(f"  Expr mismatches    : {len(diff.expr_mismatches)}")

    return 0 if diff.ok else 1


if __name__ == "__main__":  # pragma: no cover - CLI entry
    raise SystemExit(main())
