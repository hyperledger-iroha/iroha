"""Keep fixture analysis on explicit authoring inputs, without a second submission summary."""

import importlib.util
import json
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "fixtures/documentation/sorafs_capacity_simulation/analyze.py"
SPEC = importlib.util.spec_from_file_location("capacity_simulation", SOURCE)
assert SPEC is not None and SPEC.loader is not None
ANALYSIS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ANALYSIS)


def test_quota_analysis_uses_authoring_specs_and_detects_overallocation(tmp_path):
    directory = tmp_path / "quota_negotiation"
    directory.mkdir()
    assignments = []
    for index, name in enumerate(("alpha", "beta", "gamma")):
        provider = f"{index + 1:064x}"
        (directory / f"provider_{name}_declaration_spec.json").write_text(
            json.dumps({"provider_id_hex": provider, "committed_capacity_gib": 100})
        )
        assignments.append({"provider_id_hex": provider, "slice_gib": 101 if index == 0 else 50})
    (directory / "replication_order_summary.json").write_text(
        json.dumps({"assignments": assignments})
    )
    report, warnings = ANALYSIS.quota_analysis(tmp_path)
    assert report["declaration_source"] == "authoring_spec"
    assert report["total_declared_gib"] == 300
    assert report["total_assigned_gib"] == 201
    assert len(warnings) == 1 and "alpha" in warnings[0]


def test_retired_declaration_summary_is_not_an_authoring_spec_fallback(tmp_path):
    directory = tmp_path / "quota_negotiation"
    directory.mkdir()
    (directory / "provider_alpha_declaration_summary.json").write_text(
        '{"provider_id_hex":"retired","committed_capacity_gib":100}'
    )
    with pytest.raises(FileNotFoundError, match="provider_alpha_declaration_spec"):
        ANALYSIS.quota_analysis(tmp_path)
