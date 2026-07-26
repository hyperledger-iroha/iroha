"""Tests for scripts/telemetry/check_slot_duration.py (roadmap NX-18)."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
from typing import List

import pytest

MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "telemetry" / "check_slot_duration.py"
)
SPEC = importlib.util.spec_from_file_location("check_slot_duration", MODULE_PATH)
assert SPEC and SPEC.loader  # pragma: no cover - import guard
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)  # type: ignore[misc]

BUNDLER_PATH = (
    Path(__file__).resolve().parents[1] / "telemetry" / "bundle_slot_artifacts.py"
)
BUNDLER_SPEC = importlib.util.spec_from_file_location("bundle_slot_artifacts", BUNDLER_PATH)
assert BUNDLER_SPEC and BUNDLER_SPEC.loader  # pragma: no cover - import guard
bundler = importlib.util.module_from_spec(BUNDLER_SPEC)
BUNDLER_SPEC.loader.exec_module(bundler)  # type: ignore[misc]


def _write_metrics(path: Path, lines: List[str]) -> None:
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def test_load_slot_stats_computes_quantiles(tmp_path: Path) -> None:
    metrics_path = tmp_path / "metrics.prom"
    _write_metrics(
        metrics_path,
        [
            "# HELP demo histogram for NX-18 acceptance",
            '# TYPE iroha_slot_duration_ms histogram',
            'iroha_slot_duration_ms_bucket{le="500"} 100',
            'iroha_slot_duration_ms_bucket{le="800"} 190',
            'iroha_slot_duration_ms_bucket{le="1000"} 198',
            'iroha_slot_duration_ms_bucket{le="+Inf"} 200',
            "iroha_slot_duration_ms_count 200",
            "iroha_slot_duration_ms_sum 120000",
            "iroha_slot_duration_ms_latest 750",
        ],
    )

    stats = checker.load_slot_stats(metrics_path)
    assert stats.count == pytest.approx(200)
    assert stats.p50_ms == pytest.approx(500.0)
    assert stats.p95_ms == pytest.approx(800.0)
    assert stats.p99_ms == pytest.approx(1000.0)
    assert stats.latest_ms == pytest.approx(750.0)


def test_main_flags_threshold_breach(tmp_path: Path) -> None:
    metrics_path = tmp_path / "breach.prom"
    _write_metrics(
        metrics_path,
        [
            'iroha_slot_duration_ms_bucket{le="500"} 10',
            'iroha_slot_duration_ms_bucket{le="1000"} 20',
            'iroha_slot_duration_ms_bucket{le="+Inf"} 20',
            "iroha_slot_duration_ms_latest 1500",
        ],
    )

    report_path = tmp_path / "report.json"
    exit_code = checker.main(
        [
            "--max-p95-ms",
            "15",  # intentionally tiny threshold
            "--quiet",
            "--json-out",
            str(report_path),
            str(metrics_path),
        ]
    )
    assert exit_code == 2
    payload = json.loads(report_path.read_text())
    assert payload["exit_code"] == 2
    assert payload["metrics_path"] == str(metrics_path)
    assert payload["thresholds"]["max_p95_ms"] == 15.0
    assert payload["samples"] == pytest.approx(20)


def test_parse_labels_rejects_invalid_payload() -> None:
    with pytest.raises(checker.SlotDurationError):
        checker.parse_labels("{invalid}")


def test_bundle_slot_artifacts_writes_manifest(tmp_path: Path) -> None:
    metrics = tmp_path / "metrics.prom"
    summary = tmp_path / "slot_summary.json"
    metrics.write_text('iroha_slot_duration_ms_bucket{le="500"} 1\n', encoding="utf-8")
    summary.write_text(json.dumps({"p95_ms": 250.0}), encoding="utf-8")
    out_dir = tmp_path / "bundle"
    exit_code = bundler.main(
        [
            "--metrics",
            str(metrics),
            "--summary",
            str(summary),
            "--out-dir",
            str(out_dir),
            "--metadata",
            "label=test",
        ]
    )
    assert exit_code == 0
    manifest_path = out_dir / "slot_bundle_manifest.json"
    manifest = json.loads(manifest_path.read_text())
    assert manifest["metadata"]["label"] == "test"
    metrics_copy = out_dir / "slot_metrics.prom"
    summary_copy = out_dir / "slot_summary.json"
    assert metrics_copy.exists()
    assert summary_copy.exists()
    names = {entry["name"] for entry in manifest["artifacts"]}
    assert names == {"metrics", "summary"}
