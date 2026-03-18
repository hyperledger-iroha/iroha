"""Tests for the Nexus telemetry pack validator."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.telemetry.validate_nexus_telemetry_pack import (
    build_manifest,
    normalize_workload_seed,
    parse_slot_range,
)


def _create_artifact(path: Path, name: str, payload: bytes) -> None:
    target = path / name
    target.write_bytes(payload)


def test_parse_slot_range_accepts_valid_bounds() -> None:
    canonical, start, end = parse_slot_range("820-860")
    assert canonical == "820-860"
    assert start == 820
    assert end == 860


@pytest.mark.parametrize("value", ["", "foo", "10", "-1-5", "30-20"])
def test_parse_slot_range_rejects_invalid_values(value: str) -> None:
    with pytest.raises(ValueError):
        parse_slot_range(value)


def test_normalize_workload_seed_strips_whitespace() -> None:
    assert normalize_workload_seed("  NEXUS-REH-2026Q1  ") == "NEXUS-REH-2026Q1"
    with pytest.raises(ValueError):
        normalize_workload_seed("   ")


def test_build_manifest_preserves_slot_metadata(tmp_path: Path) -> None:
    pack = tmp_path / "pack"
    pack.mkdir()
    _create_artifact(pack, "prometheus.tgz", b"metrics")
    _create_artifact(pack, "otlp.ndjson", b"{}")
    metadata = {}
    canonical, start, end = parse_slot_range("1-2")
    metadata["slot_range"] = canonical
    metadata["slot_range_start"] = start
    metadata["slot_range_end"] = end
    manifest = build_manifest(pack, ["prometheus.tgz", "otlp.ndjson"], metadata)
    meta_section = manifest["metadata"]
    assert isinstance(meta_section, dict)
    assert meta_section["slot_range"] == "1-2"
    assert meta_section["slot_range_start"] == 1
    assert meta_section["slot_range_end"] == 2
