"""Tests for scripts/sync_norito_fixture_families.py."""

from __future__ import annotations

import base64
import importlib.util
import json
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[1] / "sync_norito_fixture_families.py"
SPEC = importlib.util.spec_from_file_location("sync_norito_fixture_families", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _entry(name: str, payload: bytes, signed: bytes) -> dict[str, object]:
    return {
        "authority": f"authority-{name}",
        "chain": "00000002",
        "creation_time_ms": 100,
        "encoded_file": f"{name}.norito",
        "encoded_len": len(payload),
        "name": name,
        "nonce": None,
        "payload_base64": base64.b64encode(payload).decode("ascii"),
        "payload_hash": MODULE._iroha_hash(payload),
        "signed_base64": base64.b64encode(signed).decode("ascii"),
        "signed_hash": MODULE._signed_transaction_hash(signed),
        "signed_len": len(signed),
        "time_to_live_ms": None,
    }


def _sized_field(value: bytes) -> bytes:
    return MODULE._compact_length(len(value)) + value


def _signed_envelope(payload: bytes, signature: bytes = b"signature") -> bytes:
    return _sized_field(signature) + _sized_field(payload) + _sized_field(b"\x00")


def _write_source(root: Path) -> Path:
    source = root / "source"
    source.mkdir()
    entries = [
        _entry("alpha", b"\x00alpha", _signed_envelope(b"\x00alpha")),
        _entry("beta", b"\x00beta", _signed_envelope(b"\x00beta")),
    ]
    (source / MODULE.MANIFEST_NAME).write_text(
        json.dumps({"fixtures": entries}, indent=2) + "\n",
        encoding="utf-8",
    )
    payloads = [
        {
            "name": entry["name"],
            "authority": entry["authority"],
            "chain": entry["chain"],
            "creation_time_ms": entry["creation_time_ms"],
            "encoded": entry["payload_base64"],
            "nonce": entry["nonce"],
            "payload_base64": entry["payload_base64"],
            "payload_hash": entry["payload_hash"],
            "signed_base64": entry["signed_base64"],
            "signed_hash": entry["signed_hash"],
            "time_to_live_ms": entry["time_to_live_ms"],
            "payload": {
                "chain": entry["chain"],
                "authority": entry["authority"],
                "creation_time_ms": entry["creation_time_ms"],
                "executable": {"Instructions": []},
                "fee_payment": {
                    "payer": "authority",
                    "value": {"charge_limits": []},
                },
                "metadata": {},
                "time_to_live_ms": entry["time_to_live_ms"],
                "nonce": entry["nonce"],
            },
        }
        for entry in entries
    ]
    (source / MODULE.PAYLOADS_NAME).write_text(
        json.dumps(payloads, indent=2) + "\n",
        encoding="utf-8",
    )
    for entry in entries:
        (source / str(entry["encoded_file"])).write_bytes(
            base64.b64decode(str(entry["payload_base64"]))
        )
    return source


def test_signed_hash_commits_to_payload_but_not_authorization_proof() -> None:
    payload = b"canonical-payload"
    first = _signed_envelope(payload, b"first-signature")
    second = _signed_envelope(payload, b"replacement-signature")
    changed_payload = _signed_envelope(payload + b"-changed", b"first-signature")

    assert MODULE._signed_transaction_hash(first) == MODULE._signed_transaction_hash(second)
    assert MODULE._signed_transaction_hash(first) != MODULE._signed_transaction_hash(
        changed_payload
    )


@pytest.mark.parametrize(
    "signed",
    (
        b"",
        b"\x80",
        b"\x81\x00x" + _sized_field(b"payload") + _sized_field(b"\x00"),
        _sized_field(b"signature") + b"\xff" * 10,
        _signed_envelope(b"payload") + b"trailing",
    ),
)
def test_signed_hash_rejects_malformed_or_noncanonical_envelopes(signed: bytes) -> None:
    with pytest.raises(MODULE.FixtureSyncError):
        MODULE._signed_transaction_hash(signed)


def test_sync_is_exact_and_preserves_only_swift_specific_norito(tmp_path: Path) -> None:
    source = _write_source(tmp_path)
    targets = {
        "canonical": tmp_path / "canonical",
        "python": tmp_path / "python",
        "swift": tmp_path / "swift",
    }
    for target in targets.values():
        target.mkdir()
        (target / "stale.norito").write_bytes(b"stale")
        (target / "unrelated.json").write_text("{}\n", encoding="utf-8")
    (targets["swift"] / "swift_special.norito").write_bytes(b"swift-only")
    (targets["swift"] / "transaction_payload.json").write_text(
        '{"swift":"only"}\n', encoding="utf-8"
    )

    family = MODULE.load_fixture_family(source, expected_count=2)
    MODULE.sync_targets(family, targets)
    MODULE.sync_targets(family, targets, check=True)

    for target in targets.values():
        assert not (target / "stale.norito").exists()
        assert (target / "unrelated.json").exists()
        for name, payload in family.files.items():
            assert (target / name).read_bytes() == payload
    assert (targets["swift"] / "swift_special.norito").read_bytes() == b"swift-only"
    assert (targets["swift"] / "transaction_payload.json").exists()


def test_check_fails_closed_without_rewriting_drift(tmp_path: Path) -> None:
    source = _write_source(tmp_path)
    target = tmp_path / "target"
    family = MODULE.load_fixture_family(source, expected_count=2)
    MODULE.sync_targets(family, {"canonical": target})
    manifest = target / MODULE.MANIFEST_NAME
    manifest.write_bytes(b"tampered")

    with pytest.raises(MODULE.FixtureSyncError, match="content differs"):
        MODULE.sync_targets(family, {"canonical": target}, check=True)
    assert manifest.read_bytes() == b"tampered"


def test_source_rejects_wrong_reviewed_count(tmp_path: Path) -> None:
    source = _write_source(tmp_path)
    with pytest.raises(MODULE.FixtureSyncError, match="fixture count is 2, expected 3"):
        MODULE.load_fixture_family(source, expected_count=3)


def test_source_rejects_duplicate_names(tmp_path: Path) -> None:
    source = _write_source(tmp_path)
    path = source / MODULE.MANIFEST_NAME
    manifest = json.loads(path.read_text(encoding="utf-8"))
    manifest["fixtures"][1]["name"] = "alpha"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    with pytest.raises(MODULE.FixtureSyncError, match="duplicate fixture name"):
        MODULE.load_fixture_family(source, expected_count=2)


def test_source_rejects_path_traversal(tmp_path: Path) -> None:
    source = _write_source(tmp_path)
    path = source / MODULE.MANIFEST_NAME
    manifest = json.loads(path.read_text(encoding="utf-8"))
    manifest["fixtures"][0]["encoded_file"] = "../alpha.norito"
    path.write_text(json.dumps(manifest), encoding="utf-8")
    with pytest.raises(MODULE.FixtureSyncError, match="encoded_file must be exactly"):
        MODULE.load_fixture_family(source, expected_count=2)


def test_source_rejects_symlinked_fixture(tmp_path: Path) -> None:
    source = _write_source(tmp_path)
    fixture = source / "alpha.norito"
    payload = fixture.read_bytes()
    fixture.unlink()
    outside = tmp_path / "outside.norito"
    outside.write_bytes(payload)
    fixture.symlink_to(outside)
    with pytest.raises(MODULE.FixtureSyncError, match="must not be a symlink"):
        MODULE.load_fixture_family(source, expected_count=2)


def test_source_rejects_payload_catalogue_metadata_drift(tmp_path: Path) -> None:
    source = _write_source(tmp_path)
    path = source / MODULE.PAYLOADS_NAME
    payloads = json.loads(path.read_text(encoding="utf-8"))
    payloads[0]["payload"]["authority"] = "wrong-authority"
    path.write_text(json.dumps(payloads), encoding="utf-8")
    with pytest.raises(MODULE.FixtureSyncError, match="metadata does not match"):
        MODULE.load_fixture_family(source, expected_count=2)


def test_source_rejects_unmanifested_norito(tmp_path: Path) -> None:
    source = _write_source(tmp_path)
    (source / "injected.norito").write_bytes(b"injected")
    with pytest.raises(MODULE.FixtureSyncError, match="inventory differs"):
        MODULE.load_fixture_family(source, expected_count=2)


@pytest.mark.parametrize(
    "location",
    ("manifest_root", "manifest_fixture", "payload_entry", "payload_spec"),
)
def test_source_rejects_unknown_json_keys(tmp_path: Path, location: str) -> None:
    source = _write_source(tmp_path)
    if location.startswith("manifest"):
        path = source / MODULE.MANIFEST_NAME
        document = json.loads(path.read_text(encoding="utf-8"))
        target = document if location == "manifest_root" else document["fixtures"][0]
    else:
        path = source / MODULE.PAYLOADS_NAME
        document = json.loads(path.read_text(encoding="utf-8"))
        target = document[0] if location == "payload_entry" else document[0]["payload"]
    target["injected_unknown_field"] = "must-fail-closed"
    path.write_text(json.dumps(document), encoding="utf-8")

    with pytest.raises(
        MODULE.FixtureSyncError,
        match=r"key inventory differs; .*extra=\['injected_unknown_field'\]",
    ):
        MODULE.load_fixture_family(source, expected_count=2)


def test_regular_file_read_rejects_lstat_open_symlink_swap(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    victim = tmp_path / "victim.norito"
    parked = tmp_path / "original.norito"
    outside = tmp_path / "outside.norito"
    victim.write_bytes(b"reviewed")
    outside.write_bytes(b"attacker")
    original_open = MODULE.os.open
    swapped = False

    def swapping_open(path: object, flags: int, *args: object) -> int:
        nonlocal swapped
        if Path(path) == victim and not swapped:
            swapped = True
            victim.rename(parked)
            victim.symlink_to(outside)
        return original_open(path, flags, *args)

    monkeypatch.setattr(MODULE.os, "open", swapping_open)
    with pytest.raises(
        MODULE.FixtureSyncError,
        match=r"changed during read|became a symlink",
    ):
        MODULE._read_regular_file(victim, "adversarial fixture")
    assert swapped
