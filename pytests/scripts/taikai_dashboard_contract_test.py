"""Contract tests for Taikai Grafana metric references and rate units."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DASHBOARDS = (
    ROOT / "dashboards" / "grafana" / "taikai_cache.json",
    ROOT / "dashboards" / "grafana" / "taikai_viewer.json",
)
CATALOG = ROOT / "crates" / "iroha_telemetry" / "src" / "metrics" / "catalog_v2.tsv"
TAIKAI_METRIC = re.compile(r"\b(?:taikai_|sorafs_|torii_sorafs_)[a-zA-Z0-9_:]+")


def _registered_metrics() -> set[str]:
    return {
        line.split("\t", 1)[0]
        for line in CATALOG.read_text(encoding="utf-8").splitlines()
        if line and not line.startswith("#")
    }


def _base_metric(metric: str) -> str:
    for suffix in ("_bucket", "_count", "_sum"):
        if metric.endswith(suffix):
            return metric[: -len(suffix)]
    return metric


def test_taikai_dashboards_only_query_registered_metrics() -> None:
    registered = _registered_metrics()
    missing: set[str] = set()
    for path in DASHBOARDS:
        dashboard = json.loads(path.read_text(encoding="utf-8"))
        for metric in TAIKAI_METRIC.findall(json.dumps(dashboard)):
            base = _base_metric(metric)
            if base not in registered:
                missing.add(base)

    assert missing == set()


def test_per_minute_panels_convert_prometheus_rates() -> None:
    for path in DASHBOARDS:
        dashboard = json.loads(path.read_text(encoding="utf-8"))
        for panel in dashboard["panels"]:
            if panel.get("fieldConfig", {}).get("defaults", {}).get("unit") != "1/min":
                continue
            for target in panel.get("targets", []):
                expression = target.get("expr", "")
                assert "60 *" in expression, (path.name, panel["title"], expression)
