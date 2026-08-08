from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from scripts import close_taira_qualification_handoff as closer


def _canonical(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")


def _fixture(tmp_path: Path) -> tuple[Path, Path, Path, dict[str, object]]:
    source = {
        "cargo_lock_sha256": "a" * 64,
        "commit": "b" * 40,
        "dpn_validator_release_commit": "d" * 40,
        "workspace_source_manifest_sha256": "c" * 64,
    }
    identity = tmp_path / closer.SOURCE_IDENTITY_NAME
    identity.write_bytes(
        _canonical(
            {
                "source": source,
                "source_date_epoch": 1,
            }
        )
    )
    receipt_value = {
        "artifact_handoff_sha256": "e" * 64,
        "receipt_id": "f" * 64,
        "schema": closer.MACOS_RECEIPT_SCHEMA,
        "schema_version": closer.MACOS_RECEIPT_SCHEMA_VERSION,
        "source": source,
        "validator_binary_sha256": "1" * 64,
    }
    receipt = tmp_path / closer.RECEIPT_NAME
    receipt.write_bytes(closer.canonical_json_bytes(receipt_value))
    privacy_receipt = tmp_path / closer.PRIVACY_PROTOCOL_RECEIPT_NAME
    privacy_receipt.write_bytes(
        closer.canonical_json_bytes(
            {
                "candidate": {
                    "source": source,
                    "validator_binary_sha256": "1" * 64,
                },
                "receipt_id": "9" * 64,
                "schema": closer.PRIVACY_PROTOCOL_RECEIPT_SCHEMA,
                "schema_version": closer.PRIVACY_PROTOCOL_RECEIPT_SCHEMA_VERSION,
            }
        )
    )
    return receipt, privacy_receipt, identity, receipt_value


def test_qualification_handoff_is_root_freezable_and_exactly_closed(
    tmp_path: Path,
) -> None:
    receipt, privacy_receipt, identity, receipt_value = _fixture(tmp_path)
    output = tmp_path / "handoff"

    result = closer.close_handoff(receipt, privacy_receipt, identity, output)

    assert result["receipt_id"] == receipt_value["receipt_id"]
    assert closer.scan_inventory_paths(output) == sorted(
        [
            closer.HANDOFF_MANIFEST,
            closer.PRIVACY_PROTOCOL_RECEIPT_NAME,
            closer.RECEIPT_NAME,
            closer.SOURCE_IDENTITY_NAME,
        ]
    )
    assert stat_mode(output) == 0o555
    assert all(stat_mode(output / name) == 0o444 for name in closer.scan_inventory_paths(output))
    manifest = json.loads((output / closer.HANDOFF_MANIFEST).read_bytes())
    assert [row["path"] for row in manifest["files"]] == [
        closer.RECEIPT_NAME,
        closer.PRIVACY_PROTOCOL_RECEIPT_NAME,
        closer.SOURCE_IDENTITY_NAME,
    ]


def stat_mode(path: Path) -> int:
    return path.stat().st_mode & 0o777


def test_qualification_handoff_rejects_source_substitution(tmp_path: Path) -> None:
    receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    value = json.loads(identity.read_bytes())
    value["source"]["commit"] = "0" * 40
    identity.write_bytes(_canonical(value))

    with pytest.raises(closer.QualificationHandoffError, match="exact source"):
        closer.close_handoff(receipt, privacy_receipt, identity, tmp_path / "handoff")


def test_qualification_handoff_rejects_dpn_only_source_substitution(
    tmp_path: Path,
) -> None:
    receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    value = json.loads(identity.read_bytes())
    value["source"]["dpn_validator_release_commit"] = "e" * 40
    identity.write_bytes(_canonical(value))

    with pytest.raises(closer.QualificationHandoffError, match="exact source"):
        closer.close_handoff(receipt, privacy_receipt, identity, tmp_path / "handoff")


def test_qualification_handoff_rejects_legacy_top_level_dpn_alias(
    tmp_path: Path,
) -> None:
    receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    value = json.loads(identity.read_bytes())
    value["dpn_validator_release_commit"] = value["source"][
        "dpn_validator_release_commit"
    ]
    identity.write_bytes(_canonical(value))

    with pytest.raises(closer.QualificationHandoffError, match="exact first-release"):
        closer.close_handoff(receipt, privacy_receipt, identity, tmp_path / "handoff")


def test_qualification_handoff_rejects_symlink_and_hardlink_inputs(
    tmp_path: Path,
) -> None:
    receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    alias = tmp_path / "receipt-alias"
    alias.symlink_to(receipt)
    with pytest.raises(
        closer.ReleaseArtifactError,
        match="Too many levels|symlink|regular",
    ):
        closer.close_handoff(alias, privacy_receipt, identity, tmp_path / "symlink-output")

    hardlink = tmp_path / "identity-hardlink"
    os.link(identity, hardlink)
    with pytest.raises(closer.ReleaseArtifactError, match="hard link"):
        closer.close_handoff(receipt, privacy_receipt, identity, tmp_path / "hardlink-output")


def test_qualification_handoff_rejects_noncanonical_receipt(tmp_path: Path) -> None:
    receipt, privacy_receipt, identity, receipt_value = _fixture(tmp_path)
    receipt.write_text(json.dumps(receipt_value, indent=2), encoding="ascii")
    with pytest.raises(closer.QualificationHandoffError, match="canonical"):
        closer.close_handoff(receipt, privacy_receipt, identity, tmp_path / "handoff")


def test_qualification_handoff_rejects_fifo_input(tmp_path: Path) -> None:
    receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    receipt.unlink()
    os.mkfifo(receipt, mode=0o600)
    with pytest.raises(closer.ReleaseArtifactError, match="regular file"):
        closer.close_handoff(receipt, privacy_receipt, identity, tmp_path / "handoff")


def test_partial_qualification_handoff_is_never_frozen_or_inventory_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    output = tmp_path / "partial-handoff"
    real_write = closer._write_frozen_at
    calls = 0

    def partial_write(
        directory_fd: int,
        name: str,
        payload: bytes,
        **kwargs,
    ) -> tuple[int, ...]:
        nonlocal calls
        calls += 1
        result = real_write(directory_fd, name, payload, **kwargs)
        if calls == 1:
            raise OSError("injected partial handoff failure")
        return result

    monkeypatch.setattr(closer, "_write_frozen_at", partial_write)
    with pytest.raises(OSError, match="injected partial"):
        closer.close_handoff(receipt, privacy_receipt, identity, output)

    assert output.exists()
    assert stat_mode(output) == 0o700
    assert not (output / closer.HANDOFF_MANIFEST).exists()


def test_post_read_source_replacement_cannot_change_closed_handoff(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    original = receipt.read_bytes()
    real_read = closer.stable_read_path
    replaced = False

    def replacing_read(path: Path, **kwargs):
        nonlocal replaced
        result = real_read(path, **kwargs)
        if path == receipt and not replaced:
            replacement = tmp_path / "replacement"
            replacement.write_bytes(closer.canonical_json_bytes({"hostile": True}))
            os.replace(replacement, receipt)
            replaced = True
        return result

    monkeypatch.setattr(closer, "stable_read_path", replacing_read)
    output = tmp_path / "handoff"
    closer.close_handoff(receipt, privacy_receipt, identity, output)
    assert (output / closer.RECEIPT_NAME).read_bytes() == original


def test_qualification_handoff_rejects_device_input(tmp_path: Path) -> None:
    _receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    device = Path("/dev/null")
    if not device.exists():
        pytest.skip("platform does not expose /dev/null")
    with pytest.raises(closer.ReleaseArtifactError, match="regular file"):
        closer.close_handoff(device, privacy_receipt, identity, tmp_path / "device-output")


def test_qualification_handoff_rejects_same_uid_root_replacement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    output = tmp_path / "handoff"
    displaced = tmp_path / "displaced-handoff"
    real_write = closer._write_frozen_at

    def replacing_write(directory_fd: int, name: str, payload: bytes, **kwargs):
        result = real_write(directory_fd, name, payload, **kwargs)
        if name == closer.HANDOFF_MANIFEST:
            output.rename(displaced)
            output.mkdir(mode=0o755)
        return result

    monkeypatch.setattr(closer, "_write_frozen_at", replacing_write)
    with pytest.raises(closer.QualificationHandoffError, match="handoff root"):
        closer.close_handoff(receipt, privacy_receipt, identity, output)

    assert displaced.exists()
    assert not (output / closer.HANDOFF_MANIFEST).exists()


def test_qualification_handoff_rejects_mutated_completed_output(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    receipt, privacy_receipt, identity, _receipt_value = _fixture(tmp_path)
    output = tmp_path / "handoff"
    real_write = closer._write_frozen_at

    def mutating_write(directory_fd: int, name: str, payload: bytes, **kwargs):
        result = real_write(directory_fd, name, payload, **kwargs)
        if name == closer.HANDOFF_MANIFEST:
            target = output / closer.RECEIPT_NAME
            target.chmod(0o600)
            target.write_bytes(b"same-uid replacement\n")
        return result

    monkeypatch.setattr(closer, "_write_frozen_at", mutating_write)
    with pytest.raises(
        closer.QualificationHandoffError,
        match="output was replaced",
    ):
        closer.close_handoff(receipt, privacy_receipt, identity, output)
