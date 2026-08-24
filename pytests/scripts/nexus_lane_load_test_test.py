"""Tests for the exact first-release Nexus lane-load CLI and manifest schema."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/nexus_lane_load_test.py"
SPEC = importlib.util.spec_from_file_location("nexus_lane_load_test", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
LOAD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LOAD)


def test_parse_args_accepts_only_lifecycle_file(tmp_path: Path) -> None:
    args = LOAD.parse_args(
        [
            "--lifecycle-file",
            "lifecycle.json",
            "--metrics-file",
            "metrics.prom",
            "--lane-alias",
            "core",
            "--out-dir",
            str(tmp_path),
        ]
    )
    assert args.lifecycle_file == Path("lifecycle.json")

    with pytest.raises(SystemExit):
        LOAD.parse_args(
            [
                "--status-file",
                "lifecycle.json",
                "--metrics-file",
                "metrics.prom",
                "--lane-alias",
                "core",
                "--out-dir",
                str(tmp_path),
            ]
        )


def test_manifest_uses_exact_lifecycle_input_key(tmp_path: Path) -> None:
    args = SimpleNamespace(
        lane_alias=["core"],
        lifecycle_file=Path("lifecycle.json"),
        metrics_file=Path("metrics.prom"),
        telemetry_file=None,
        alias_migrations=[],
        slot_range=None,
        workload_seed="seed",
        min_da_quorum=0.95,
        max_oracle_staleness=75.0,
        expected_oracle_twap=60.0,
        oracle_twap_tolerance=5.0,
        max_oracle_haircut_bps=100.0,
        min_settlement_buffer=0.25,
        min_block_height=1,
        max_finality_lag=4.0,
        max_settlement_backlog=1.0,
        max_headroom_events=0.0,
        min_teu_capacity=1.0,
        max_teu_slot_commit_ratio=0.9,
        max_teu_deferrals=0.0,
        max_must_serve_truncations=0.0,
        max_slot_p95=1000.0,
        max_slot_p99=1100.0,
        min_slot_samples=10,
        expected_lane_count=None,
        metadata={},
    )

    path = LOAD.write_manifest(args, tmp_path, {})
    payload = json.loads(path.read_text(encoding="utf-8"))
    assert payload["inputs"]["lifecycle_file"] == "lifecycle.json"
    assert "status_file" not in payload["inputs"]
