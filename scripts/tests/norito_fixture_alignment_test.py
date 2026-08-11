"""Tests for scripts/norito_fixture_alignment.py."""

from __future__ import annotations

import base64
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

TEST_NETWORK_ID = (
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
)
TEST_OTHER_NETWORK_ID = (
    "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"
)


def _field(value: bytes) -> bytes:
    return MODULE._compact_length(len(value)) + value


def _signed_transaction(payload: bytes, signature: bytes = b"signature") -> bytes:
    return _field(signature) + _field(payload) + _field(b"\x00")


def _transaction_payload(
    suffix: bytes = b"", network_id: str = TEST_NETWORK_ID
) -> bytes:
    identity = bytes.fromhex(network_id[5:69])
    domain = (0).to_bytes(4, "little") + _field(identity)
    return _field(domain) + suffix


def _fixture_entry(
    creation_time_ms: int, network_id: str = TEST_NETWORK_ID
) -> dict:
    payload = _transaction_payload(network_id=network_id)
    signed = _signed_transaction(payload)
    return {
        "name": "alpha",
        "encoded_file": "alpha.norito",
        "network_id": network_id,
        "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        "payload_base64": base64.b64encode(payload).decode("ascii"),
        "payload_hash": MODULE._iroha_hash(payload),
        "signed_base64": base64.b64encode(signed).decode("ascii"),
        "signed_hash": MODULE._signed_transaction_hash(signed),
        "encoded_len": len(payload),
        "signed_len": len(signed),
        "creation_time_ms": creation_time_ms,
        "time_to_live_ms": 100_000,
        "nonce": None,
    }


def _write_manifest(
    path: Path,
    creation_time_ms: int,
    network_id: str = TEST_NETWORK_ID,
) -> Path:
    payload = {"fixtures": [_fixture_entry(creation_time_ms, network_id)]}
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


def test_compare_manifests_flags_network_id_drift(tmp_path: Path) -> None:
    canonical_path = _write_manifest(tmp_path / "canonical.json", creation_time_ms=100)
    target_path = _write_manifest(
        tmp_path / "target.json",
        creation_time_ms=100,
        network_id=TEST_OTHER_NETWORK_ID,
    )

    canonical = MODULE.load_manifest(canonical_path)
    target = MODULE.load_manifest(target_path)
    result = MODULE.compare_manifests("target", canonical, target)

    assert not result.ok
    assert result.mismatched
    assert "network_id" in result.mismatched[0].differences


def test_load_manifest_rejects_mismatched_or_genesis_transaction_domain(
    tmp_path: Path,
) -> None:
    mismatched = _write_manifest(
        tmp_path / "mismatched.json",
        creation_time_ms=100,
    )
    payload = json.loads(mismatched.read_text(encoding="utf-8"))
    payload["fixtures"][0]["network_id"] = TEST_OTHER_NETWORK_ID
    mismatched.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    with pytest.raises(SystemExit, match="network_id does not match"):
        MODULE.load_manifest(mismatched)

    genesis = _write_manifest(tmp_path / "genesis.json", creation_time_ms=100)
    document = json.loads(genesis.read_text(encoding="utf-8"))
    genesis_payload = _field((1).to_bytes(4, "little"))
    signed = _signed_transaction(genesis_payload)
    entry = document["fixtures"][0]
    entry["payload_base64"] = base64.b64encode(genesis_payload).decode("ascii")
    entry["payload_hash"] = MODULE._iroha_hash(genesis_payload)
    entry["encoded_len"] = len(genesis_payload)
    entry["signed_base64"] = base64.b64encode(signed).decode("ascii")
    entry["signed_hash"] = MODULE._signed_transaction_hash(signed)
    entry["signed_len"] = len(signed)
    genesis.write_text(json.dumps(document, indent=2), encoding="utf-8")
    with pytest.raises(SystemExit, match="genesis-only"):
        MODULE.load_manifest(genesis)


def test_compare_manifests_flags_authority_drift(tmp_path: Path) -> None:
    canonical_path = _write_manifest(tmp_path / "canonical.json", creation_time_ms=100)
    target_path = _write_manifest(tmp_path / "target.json", creation_time_ms=100)

    payload = json.loads(target_path.read_text(encoding="utf-8"))
    payload["fixtures"][0]["authority"] = (
        "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"
    )
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


def test_load_manifest_rejects_missing_explicit_nonce(tmp_path: Path) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["fixtures"][0].pop("nonce")
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match=r"missing=\['nonce'\]"):
        MODULE.load_manifest(path)


def test_load_manifest_rejects_missing_ttl(tmp_path: Path) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["fixtures"][0].pop("time_to_live_ms")
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match="time_to_live_ms"):
        MODULE.load_manifest(path)


@pytest.mark.parametrize("legacy_field", ["chain", "chainId", "chain_id"])
def test_manifest_rejects_chain_chainId_and_chain_id(
    tmp_path: Path, legacy_field: str
) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    payload = json.loads(path.read_text(encoding="utf-8"))
    entry = payload["fixtures"][0]
    entry[legacy_field] = entry.pop("network_id")
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    with pytest.raises(
        SystemExit,
        match=rf"missing=\['network_id'\], unexpected=\['{legacy_field}'\]",
    ):
        MODULE.load_manifest(path)


@pytest.mark.parametrize("legacy_field", ["chain", "chainId", "chain_id"])
def test_manifest_rejects_legacy_identity_aliases(
    tmp_path: Path, legacy_field: str
) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    payload = json.loads(path.read_text(encoding="utf-8"))
    entry = payload["fixtures"][0]
    entry[legacy_field] = TEST_NETWORK_ID
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    with pytest.raises(
        SystemExit,
        match=rf"missing=\[\], unexpected=\['{legacy_field}'\]",
    ):
        MODULE.load_manifest(path)


@pytest.mark.parametrize(
    "network_id",
    [
        "00000002",
        TEST_NETWORK_ID.lower(),
        f"{TEST_NETWORK_ID[:-4]}0000",
    ],
    ids=["chain-label", "lowercase", "bad-checksum"],
)
def test_load_manifest_rejects_noncanonical_network_id(
    tmp_path: Path, network_id: str
) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["fixtures"][0]["network_id"] = network_id
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match="canonical network_id"):
        MODULE.load_manifest(path)


@pytest.mark.parametrize(
    "ttl",
    [None, 0, -1, True, False, 1.5, "100000"],
    ids=["null", "zero", "negative", "true", "false", "float", "string"],
)
def test_load_manifest_rejects_non_positive_integer_ttl(
    tmp_path: Path, ttl: object
) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    payload = json.loads(path.read_text(encoding="utf-8"))
    payload["fixtures"][0]["time_to_live_ms"] = ttl
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match="time_to_live_ms"):
        MODULE.load_manifest(path)


def test_load_manifest_rejects_duplicate_fixture_names(tmp_path: Path) -> None:
    path = tmp_path / "manifest.json"
    entry = _fixture_entry(creation_time_ms=100)
    path.write_text(
        json.dumps({"fixtures": [entry, entry]}, indent=2), encoding="utf-8"
    )

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
    path.write_text(
        json.dumps({"fixtures": [first, clone]}, indent=2), encoding="utf-8"
    )

    with pytest.raises(SystemExit, match="duplicate payload_hash"):
        MODULE.load_manifest(path)


@pytest.mark.parametrize(
    "encoded",
    ["YQ!!", "Y Q==", "YQ=", "YQ===", "YR=="],
    ids=[
        "invalid-char",
        "whitespace",
        "missing-padding",
        "excess-padding",
        "noncanonical-bits",
    ],
)
def test_load_manifest_rejects_noncanonical_base64(
    tmp_path: Path, encoded: str
) -> None:
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
    alternate_payload = _transaction_payload(b"\x02")
    payload["fixtures"][0]["payload_base64"] = base64.b64encode(
        alternate_payload
    ).decode("ascii")
    payload["fixtures"][0]["payload_hash"] = MODULE._iroha_hash(alternate_payload)
    payload["fixtures"][0]["encoded_len"] = len(alternate_payload)
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
        ("network_id", 2),
        ("authority", []),
        ("payload_base64", 1),
        ("payload_hash", False),
        ("signed_base64", {}),
        ("signed_hash", 7),
        ("encoded_len", 1.0),
        ("signed_len", True),
        ("creation_time_ms", "100"),
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
