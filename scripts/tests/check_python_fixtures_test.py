"""Tests for the Python Norito RPC fixture mirror policy."""

from __future__ import annotations

import base64
import hashlib
import importlib.util
import json
import struct
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_python_fixtures.py"
SPEC = importlib.util.spec_from_file_location("check_python_fixtures", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def write(root: Path, relative: str, contents: str) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")


def test_compare_only_manages_descriptors_and_rejects_redundant_blobs(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(MODULE, "validate_canonical_frames", lambda _path: None)
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "transaction_payloads.json", "payloads")
    write(source, "transaction_fixtures.manifest.json", "manifest")
    write(source, "transfer_asset.norito", "canonical")
    write(target, "transaction_payloads.json", "payloads")
    write(target, "transaction_fixtures.manifest.json", "manifest")
    write(target, "unrelated.json", "{}")

    assert MODULE.compare(source, target) == ([], [], [])

    write(target, "nested/transfer_asset.norito", "redundant")
    assert MODULE.compare(source, target) == (
        [],
        [Path("nested/transfer_asset.norito")],
        [],
    )


def test_compare_reports_missing_and_content_drift(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(MODULE, "validate_canonical_frames", lambda _path: None)
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "transaction_payloads.json", "source")
    write(source, "transaction_fixtures.manifest.json", "source")
    write(target, "transaction_fixtures.manifest.json", "target")

    missing, extra, diffs = MODULE.compare(source, target)

    assert missing == [Path("transaction_payloads.json")]
    assert extra == []
    assert [
        (src.relative_to(source), dst.relative_to(target)) for src, dst in diffs
    ] == [
        (
            Path("transaction_fixtures.manifest.json"),
            Path("transaction_fixtures.manifest.json"),
        )
    ]


def test_compare_requires_both_canonical_descriptors(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(MODULE, "validate_canonical_frames", lambda _path: None)
    source = tmp_path / "source"
    target = tmp_path / "target"
    write(source, "transaction_payloads.json", "payloads")
    target.mkdir()

    with pytest.raises(FileNotFoundError, match="missing canonical fixture"):
        MODULE.compare(source, target)


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


def _field(payload: bytes) -> bytes:
    assert len(payload) < 128
    return bytes((len(payload),)) + payload


def test_manifest_frame_guard_hashes_the_complete_frame_and_rejects_bare(
    tmp_path: Path,
) -> None:
    payload_bare = b"payload"
    payload_frame = _frame(payload_bare, MODULE.TRANSACTION_PAYLOAD_SCHEMA)
    signed_bare = _field(b"signature") + _field(payload_bare) + _field(b"")
    signed_frame = _frame(signed_bare, MODULE.SIGNED_TRANSACTION_SCHEMA)
    digest = bytearray(hashlib.blake2b(payload_frame, digest_size=32).digest())
    digest[-1] |= 1
    entry = {
        "name": "fixture",
        "payload_base64": base64.b64encode(payload_frame).decode("ascii"),
        "signed_base64": base64.b64encode(signed_frame).decode("ascii"),
        "payload_hash": digest.hex(),
        "signed_hash": MODULE.signed_transaction_entrypoint_hash_hex(signed_bare),
    }
    manifest = tmp_path / "manifest.json"
    manifest.write_text(json.dumps({"fixtures": [entry]}), encoding="utf-8")
    MODULE.validate_canonical_frames(manifest)

    entry["payload_base64"] = base64.b64encode(b"payload").decode("ascii")
    manifest.write_text(json.dumps({"fixtures": [entry]}), encoding="utf-8")
    with pytest.raises(ValueError, match="mandatory Norito header"):
        MODULE.validate_canonical_frames(manifest)


def test_typed_frame_guard_rejects_inserted_or_removed_payload_bytes() -> None:
    frame = _frame(b"payload", MODULE.TRANSACTION_PAYLOAD_SCHEMA)
    inserted = frame[:40] + b"\x00" + frame[40:]
    with pytest.raises(ValueError, match="exact canonical padding"):
        MODULE.decode_canonical_norito_frame(
            inserted,
            "inserted padding",
            expected_schema=MODULE.TRANSACTION_PAYLOAD_SCHEMA,
        )

    removed = frame[:40] + frame[41:]
    with pytest.raises(ValueError, match="exact canonical padding"):
        MODULE.decode_canonical_norito_frame(
            removed,
            "removed payload byte",
            expected_schema=MODULE.TRANSACTION_PAYLOAD_SCHEMA,
        )


def test_manifest_frame_guard_checks_signed_hash_and_embedded_payload(
    tmp_path: Path,
) -> None:
    payload_bare = b"payload"
    payload_frame = _frame(payload_bare, MODULE.TRANSACTION_PAYLOAD_SCHEMA)
    signed_bare = _field(b"signature") + _field(payload_bare) + _field(b"")
    signed_frame = _frame(signed_bare, MODULE.SIGNED_TRANSACTION_SCHEMA)
    entry = {
        "name": "fixture",
        "payload_base64": base64.b64encode(payload_frame).decode("ascii"),
        "signed_base64": base64.b64encode(signed_frame).decode("ascii"),
        "payload_hash": MODULE.iroha_hash_hex(payload_frame),
        "signed_hash": MODULE.signed_transaction_entrypoint_hash_hex(signed_bare),
    }
    manifest = tmp_path / "manifest.json"
    manifest.write_text(json.dumps({"fixtures": [entry]}), encoding="utf-8")
    MODULE.validate_canonical_frames(manifest)

    entry["signed_hash"] = "00" * 32
    manifest.write_text(json.dumps({"fixtures": [entry]}), encoding="utf-8")
    with pytest.raises(ValueError, match="compact External semantics"):
        MODULE.validate_canonical_frames(manifest)

    other_payload = b"other"
    other_signed = _field(b"signature") + _field(other_payload) + _field(b"")
    entry["signed_base64"] = base64.b64encode(
        _frame(other_signed, MODULE.SIGNED_TRANSACTION_SCHEMA)
    ).decode("ascii")
    entry["signed_hash"] = MODULE.signed_transaction_entrypoint_hash_hex(other_signed)
    manifest.write_text(json.dumps({"fixtures": [entry]}), encoding="utf-8")
    with pytest.raises(ValueError, match="does not contain its payload"):
        MODULE.validate_canonical_frames(manifest)
