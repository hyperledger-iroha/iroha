"""Tests for scripts/norito_fixture_alignment.py."""

from __future__ import annotations

import base64
import importlib.util
import json
import struct
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


def _crc64_xz(data: bytes) -> int:
    crc = 0xFFFF_FFFF_FFFF_FFFF
    for value in data:
        crc ^= value
        for _ in range(8):
            crc = (crc >> 1) ^ 0xC96C_5795_D787_0F42 if crc & 1 else crc >> 1
    return crc ^ 0xFFFF_FFFF_FFFF_FFFF


def _frame(payload: bytes, schema: bytes) -> bytes:
    return b"".join(
        (
            b"NRT0\x00\x00",
            schema,
            b"\x00",
            struct.pack("<Q", len(payload)),
            struct.pack("<Q", _crc64_xz(payload)),
            b"\x02",
            payload,
        )
    )


def _current_transaction_fields(
    network_id: str = TEST_NETWORK_ID,
    creation_time_ms: int = 1,
    domain_tag: int = 0,
) -> list[bytes]:
    identity = bytes.fromhex(network_id[5:69])
    domain = domain_tag.to_bytes(4, "little")
    if domain_tag == 0:
        domain += _field(identity)
    charge_limits = (0).to_bytes(8, "little")
    authority_payment = _field(charge_limits) + _field(b"\x00")
    return [
        domain,
        b"authority",
        creation_time_ms.to_bytes(8, "little"),
        (0).to_bytes(4, "little") + (0).to_bytes(8, "little"),
        b"\x01" + _field((100_000).to_bytes(8, "little")),
        b"\x00",
        (0).to_bytes(4, "little") + _field(authority_payment),
        (0).to_bytes(4, "little"),
        (0).to_bytes(8, "little"),
        b"\x00",
    ]


def _transaction_payload(
    network_id: str = TEST_NETWORK_ID,
    creation_time_ms: int = 1,
    domain_tag: int = 0,
) -> bytes:
    return b"".join(
        _field(value)
        for value in _current_transaction_fields(
            network_id=network_id,
            creation_time_ms=creation_time_ms,
            domain_tag=domain_tag,
        )
    )


def _encode_transaction_fields(fields: list[bytes]) -> bytes:
    return b"".join(_field(value) for value in fields)


def _fixture_entry(
    creation_time_ms: int, network_id: str = TEST_NETWORK_ID
) -> dict:
    payload_bare = _transaction_payload(
        network_id=network_id, creation_time_ms=creation_time_ms
    )
    signed_bare = _signed_transaction(payload_bare)
    payload = _frame(payload_bare, MODULE.TRANSACTION_PAYLOAD_SCHEMA)
    signed = _frame(signed_bare, MODULE.SIGNED_TRANSACTION_SCHEMA)
    return {
        "name": "alpha",
        "encoded_file": "alpha.norito",
        "network_id": network_id,
        "authority": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        "payload_base64": base64.b64encode(payload).decode("ascii"),
        "payload_hash": MODULE.iroha_hash_hex(payload),
        "signed_base64": base64.b64encode(signed).decode("ascii"),
        "signed_hash": MODULE.signed_transaction_entrypoint_hash_hex(signed_bare),
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
    genesis_bare = _transaction_payload(domain_tag=1)
    signed_bare = _signed_transaction(genesis_bare)
    genesis_payload = _frame(genesis_bare, MODULE.TRANSACTION_PAYLOAD_SCHEMA)
    signed = _frame(signed_bare, MODULE.SIGNED_TRANSACTION_SCHEMA)
    entry = document["fixtures"][0]
    entry["payload_base64"] = base64.b64encode(genesis_payload).decode("ascii")
    entry["payload_hash"] = MODULE.iroha_hash_hex(genesis_payload)
    entry["encoded_len"] = len(genesis_payload)
    entry["signed_base64"] = base64.b64encode(signed).decode("ascii")
    entry["signed_hash"] = MODULE.signed_transaction_entrypoint_hash_hex(signed_bare)
    entry["signed_len"] = len(signed)
    genesis.write_text(json.dumps(document, indent=2), encoding="utf-8")
    with pytest.raises(SystemExit, match="genesis-only"):
        MODULE.load_manifest(genesis)


def test_transaction_payload_layout_accepts_current_domain_first_shape() -> None:
    payload = _transaction_payload()

    assert MODULE._transaction_payload_network_id(payload, "payload") == bytes.fromhex(
        TEST_NETWORK_ID[5:69]
    )


def test_transaction_payload_layout_rejects_malformed_final_field() -> None:
    with pytest.raises(ValueError, match="truncated payload.attachments"):
        MODULE._transaction_payload_network_id(_transaction_payload()[:-1], "payload")


def test_transaction_payload_layout_rejects_reordered_domain_and_authority() -> None:
    fields = _current_transaction_fields()
    fields[0], fields[1] = fields[1], fields[0]

    with pytest.raises(ValueError, match="unknown transaction domain tag"):
        MODULE._transaction_payload_network_id(
            _encode_transaction_fields(fields), "payload"
        )


@pytest.mark.parametrize(
    "payload",
    [
        _field(_current_transaction_fields()[0]),
        _transaction_payload() + _field(b"legacy"),
    ],
    ids=["domain-only", "trailing-field"],
)
def test_transaction_payload_layout_rejects_legacy_field_sets(payload: bytes) -> None:
    with pytest.raises(ValueError, match="(?:missing required field|legacy fields)"):
        MODULE._transaction_payload_network_id(payload, "payload")


def test_transaction_payload_layout_rejects_legacy_fee_without_gas_limit() -> None:
    fields = _current_transaction_fields()
    charge_limits = (0).to_bytes(8, "little")
    legacy_payment = _field(charge_limits)
    fields[6] = (0).to_bytes(4, "little") + _field(legacy_payment)

    with pytest.raises(ValueError, match="missing required field gas_limit"):
        MODULE._transaction_payload_network_id(
            _encode_transaction_fields(fields), "payload"
        )


@pytest.mark.parametrize(
    ("case", "message"),
    [
        ("bare", "mandatory Norito header"),
        ("header", "canonical NRT0 V1 frame"),
        ("schema", "required canonical schema hash"),
        ("compression", "forbidden compression"),
        ("checksum", "canonical length or CRC64 check"),
        ("flags", "canonical fixture flags"),
        ("padding", "exact canonical padding"),
    ],
)
def test_transaction_payload_frame_rejects_noncanonical_archives(
    case: str, message: str
) -> None:
    bare = _transaction_payload()
    frame = bytearray(_frame(bare, MODULE.TRANSACTION_PAYLOAD_SCHEMA))
    if case == "bare":
        candidate = b"bare"
    elif case == "header":
        frame[4] = 1
        candidate = bytes(frame)
    elif case == "schema":
        frame[6] ^= 1
        candidate = bytes(frame)
    elif case == "compression":
        frame[22] = 1
        candidate = bytes(frame)
    elif case == "checksum":
        frame[31] ^= 1
        candidate = bytes(frame)
    elif case == "flags":
        frame[39] = 0
        candidate = bytes(frame)
    else:
        candidate = bytes(frame[:40] + b"\x00" + frame[40:])

    with pytest.raises(ValueError, match=message):
        MODULE.decode_canonical_norito_frame(
            candidate,
            "payload",
            expected_schema=MODULE.TRANSACTION_PAYLOAD_SCHEMA,
        )


@pytest.mark.parametrize("archive", ["payload", "signed"])
def test_load_manifest_rejects_legacy_unframed_archives(
    tmp_path: Path, archive: str
) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    document = json.loads(path.read_text(encoding="utf-8"))
    payload_bare = _transaction_payload(creation_time_ms=100)
    bare = payload_bare if archive == "payload" else _signed_transaction(payload_bare)
    document["fixtures"][0][f"{archive}_base64"] = base64.b64encode(bare).decode(
        "ascii"
    )
    path.write_text(json.dumps(document, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match="canonical NRT0 V1 frame"):
        MODULE.load_manifest(path)


def test_load_manifest_requires_signed_transaction_schema(tmp_path: Path) -> None:
    path = _write_manifest(tmp_path / "manifest.json", creation_time_ms=100)
    document = json.loads(path.read_text(encoding="utf-8"))
    payload_bare = _transaction_payload(creation_time_ms=100)
    signed_bare = _signed_transaction(payload_bare)
    wrong_schema = _frame(signed_bare, MODULE.TRANSACTION_PAYLOAD_SCHEMA)
    document["fixtures"][0]["signed_base64"] = base64.b64encode(
        wrong_schema
    ).decode("ascii")
    path.write_text(json.dumps(document, indent=2), encoding="utf-8")

    with pytest.raises(SystemExit, match="required canonical schema hash"):
        MODULE.load_manifest(path)


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
    alternate_bare = _transaction_payload(creation_time_ms=2)
    alternate_payload = _frame(alternate_bare, MODULE.TRANSACTION_PAYLOAD_SCHEMA)
    payload["fixtures"][0]["payload_base64"] = base64.b64encode(
        alternate_payload
    ).decode("ascii")
    payload["fixtures"][0]["payload_hash"] = MODULE.iroha_hash_hex(alternate_payload)
    payload["fixtures"][0]["encoded_len"] = len(alternate_payload)
    alternate_signed_bare = _signed_transaction(alternate_bare)
    alternate_signed = _frame(alternate_signed_bare, MODULE.SIGNED_TRANSACTION_SCHEMA)
    payload["fixtures"][0]["signed_base64"] = base64.b64encode(
        alternate_signed
    ).decode("ascii")
    payload["fixtures"][0]["signed_hash"] = (
        MODULE.signed_transaction_entrypoint_hash_hex(alternate_signed_bare)
    )
    payload["fixtures"][0]["signed_len"] = len(alternate_signed)
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
