"""Tests for scripts/norito_fixture_alignment.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest

MODULE_PATH = Path(__file__).resolve().parents[1] / "norito_fixture_alignment.py"
SPEC = importlib.util.spec_from_file_location("norito_fixture_alignment", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _fixture_entry(creation_time_ms: int) -> dict:
    payload = b"\x00"
    signed = b"\x01"
    return {
        "name": "alpha",
        "encoded_file": "alpha.norito",
        "chain": "00000002",
        "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        "payload_base64": "AA==",
        "payload_hash": MODULE._iroha_hash(payload),
        "signed_base64": "AQ==",
        "signed_hash": MODULE._signed_transaction_hash(signed),
        "encoded_len": len(payload),
        "signed_len": len(signed),
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
    payload["fixtures"][0]["authority"] = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"
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


def test_load_manifest_rejects_duplicate_fixture_names(tmp_path: Path) -> None:
    path = tmp_path / "manifest.json"
    entry = _fixture_entry(creation_time_ms=100)
    path.write_text(json.dumps({"fixtures": [entry, entry]}, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match="duplicate fixture name 'alpha'"):
        MODULE.load_manifest(path)


def test_load_manifest_rejects_renamed_cloned_hashes(tmp_path: Path) -> None:
    path = tmp_path / "manifest.json"
    first = _fixture_entry(creation_time_ms=100)
    clone = {
        **first,
        "name": "renamed-clone",
        "encoded_file": "renamed-clone.norito",
    }
    path.write_text(json.dumps({"fixtures": [first, clone]}, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match="duplicate payload_hash"):
        MODULE.load_manifest(path)


@pytest.mark.parametrize(
    "encoded",
    ["YQ!!", "Y Q==", "YQ=", "YQ===", "YR=="],
    ids=["invalid-char", "whitespace", "missing-padding", "excess-padding", "noncanonical-bits"],
)
def test_load_manifest_rejects_noncanonical_base64(tmp_path: Path, encoded: str) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["fixtures"][0]["payload_base64"] = encoded
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match="(?:invalid|non-canonical) base64"):
        MODULE.load_manifest(path)


def test_compare_manifests_flags_canonical_payload_byte_drift(tmp_path: Path) -> None:
    canonical_path = _write_manifest(tmp_path / "canonical.json", creation_time_ms=100)
    target_path = _write_manifest(tmp_path / "target.json", creation_time_ms=100)
    payload = json.loads(target_path.read_text(encoding="utf-8"))
    payload["fixtures"][0]["payload_base64"] = "Ag=="
    payload["fixtures"][0]["payload_hash"] = MODULE._iroha_hash(b"\x02")
    target_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    canonical = MODULE.load_manifest(canonical_path)
    target = MODULE.load_manifest(target_path)
    result = MODULE.compare_manifests("target", canonical, target)

    assert not result.ok
    assert "payload_base64" in result.mismatched[0].differences
    assert "payload_hash" in result.mismatched[0].differences


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("name", 1),
        ("encoded_file", None),
        ("chain", 2),
        ("authority", []),
        ("payload_base64", 1),
        ("payload_hash", False),
        ("signed_base64", {}),
        ("signed_hash", 7),
        ("encoded_len", 1.0),
        ("signed_len", True),
        ("creation_time_ms", "100"),
        ("time_to_live_ms", -1),
        ("nonce", 1.0),
    ],
)
def test_load_manifest_rejects_wrong_field_types(
    tmp_path: Path, field: str, value: object
) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["fixtures"][0][field] = value
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match="malformed fixture entry"):
        MODULE.load_manifest(path)


def test_load_manifest_rejects_non_object_root(tmp_path: Path) -> None:
    path = tmp_path / "manifest.json"
    path.write_text("[]", encoding="utf-8")

    with pytest.raises(SystemExit, match="must contain a JSON object"):
        MODULE.load_manifest(path)
