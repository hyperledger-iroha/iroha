"""Tests for scripts/norito_fixture_alignment.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

MODULE_PATH = Path(__file__).resolve().parents[1] / "norito_fixture_alignment.py"
SPEC = importlib.util.spec_from_file_location("norito_fixture_alignment", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _fixture_entry(creation_time_ms: int) -> dict:
    return {
        "name": "alpha",
        "chain": "00000002",
        "authority": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
        "payload_hash": "payload-hash",
        "signed_hash": "signed-hash",
        "encoded_len": 10,
        "signed_len": 20,
        "creation_time_ms": creation_time_ms,
        "time_to_live_ms": None,
        "nonce": None,
    }


def _write_manifest(path: Path, creation_time_ms: int) -> Path:
    payload = {"fixtures": [_fixture_entry(creation_time_ms)]}
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return path


def test_compare_manifests_flags_creation_time_drift(tmp_path: Path) -> None:
    canonical_path = _write_manifest(tmp_path / "canonical.json", creation_time_ms=100)
    target_path = _write_manifest(tmp_path / "target.json", creation_time_ms=200)

    canonical = MODULE.load_manifest(canonical_path)
    target = MODULE.load_manifest(target_path)
    result = MODULE.compare_manifests("target", canonical, target)

    assert not result.ok
    assert result.mismatched
    assert result.mismatched[0].name == "alpha"
    assert "creation_time_ms" in result.mismatched[0].differences


def test_compare_manifests_flags_chain_drift(tmp_path: Path) -> None:
    canonical_path = _write_manifest(tmp_path / "canonical.json", creation_time_ms=100)
    target_path = _write_manifest(tmp_path / "target.json", creation_time_ms=100)

    payload = json.loads(target_path.read_text(encoding="utf-8"))
    payload["fixtures"][0]["chain"] = "00000003"
    target_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    canonical = MODULE.load_manifest(canonical_path)
    target = MODULE.load_manifest(target_path)
    result = MODULE.compare_manifests("target", canonical, target)

    assert not result.ok
    assert result.mismatched
    assert "chain" in result.mismatched[0].differences


def test_compare_manifests_flags_authority_drift(tmp_path: Path) -> None:
    canonical_path = _write_manifest(tmp_path / "canonical.json", creation_time_ms=100)
    target_path = _write_manifest(tmp_path / "target.json", creation_time_ms=100)

    payload = json.loads(target_path.read_text(encoding="utf-8"))
    payload["fixtures"][0]["authority"] = "sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"
    target_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    canonical = MODULE.load_manifest(canonical_path)
    target = MODULE.load_manifest(target_path)
    result = MODULE.compare_manifests("target", canonical, target)

    assert not result.ok
    assert result.mismatched
    assert "authority" in result.mismatched[0].differences


def test_compare_manifests_flags_ttl_drift(tmp_path: Path) -> None:
    canonical_path = _write_manifest(tmp_path / "canonical.json", creation_time_ms=100)
    target_path = _write_manifest(tmp_path / "target.json", creation_time_ms=100)

    payload = json.loads(target_path.read_text(encoding="utf-8"))
    payload["fixtures"][0]["time_to_live_ms"] = 5000
    target_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    canonical = MODULE.load_manifest(canonical_path)
    target = MODULE.load_manifest(target_path)
    result = MODULE.compare_manifests("target", canonical, target)

    assert not result.ok
    assert result.mismatched
    assert "time_to_live_ms" in result.mismatched[0].differences


def test_compare_manifests_flags_nonce_drift(tmp_path: Path) -> None:
    canonical_path = _write_manifest(tmp_path / "canonical.json", creation_time_ms=100)
    target_path = _write_manifest(tmp_path / "target.json", creation_time_ms=100)

    payload = json.loads(target_path.read_text(encoding="utf-8"))
    payload["fixtures"][0]["nonce"] = 7
    target_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    canonical = MODULE.load_manifest(canonical_path)
    target = MODULE.load_manifest(target_path)
    result = MODULE.compare_manifests("target", canonical, target)

    assert not result.ok
    assert result.mismatched
    assert "nonce" in result.mismatched[0].differences


def test_load_manifest_treats_missing_optional_fields_as_none(tmp_path: Path) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["fixtures"][0].pop("time_to_live_ms")
    payload["fixtures"][0].pop("nonce")
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    manifest = MODULE.load_manifest(path)

    fixture = manifest.fixtures["alpha"]
    assert fixture.time_to_live_ms is None
    assert fixture.nonce is None
