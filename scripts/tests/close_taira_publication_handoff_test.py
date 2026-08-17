from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

import pytest

from scripts import close_taira_publication_handoff as closer

QUALIFICATION_RECEIPT_ID = "1" * 64
SOURCE_COMMIT = "2" * 40
DPN_COMMIT = "3" * 40
CARGO_LOCK_SHA256 = "4" * 64
WORKSPACE_MANIFEST_SHA256 = "5" * 64
PUBLIC_KEY = b"publication-test-key-material!!!"
assert len(PUBLIC_KEY) == 32
SIGNING_FINGERPRINT = hashlib.sha256(PUBLIC_KEY).hexdigest()


def _canonical(value: object) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=True,
            indent=2,
            sort_keys=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def _receipt_value() -> dict[str, object]:
    return {
        "qualification_receipt_id": QUALIFICATION_RECEIPT_ID,
        "schema": "iroha.taira.publication_receipt",
        "schema_version": 1,
        "source": {
            "cargo_lock_sha256": CARGO_LOCK_SHA256,
            "commit": SOURCE_COMMIT,
            "dpn_validator_release_commit": DPN_COMMIT,
            "workspace_source_manifest_sha256": WORKSPACE_MANIFEST_SHA256,
        },
    }


def _write_frozen(path: Path, payload: bytes) -> None:
    path.write_bytes(payload)
    path.chmod(0o444)


def _fixture(tmp_path: Path) -> tuple[Path, Path, dict[str, bytes]]:
    tmp_path.mkdir(parents=True, exist_ok=True)
    source_parent = tmp_path / "authority-scratch"
    handoff_root = tmp_path / "public-handoff"
    source_parent.mkdir(mode=0o700)
    handoff_root.mkdir(mode=0o711)
    source_parent.chmod(0o700)
    handoff_root.chmod(0o711)
    terminal = source_parent / "terminal"
    terminal.mkdir(mode=0o700)

    primary_manifest = b'{"schemaVersion":2,"kind":"primary"}\n'
    receipt_manifest = b'{"schemaVersion":2,"kind":"receipt"}\n'
    payloads = {
        "publication-receipt-v1.json": _canonical(_receipt_value()),
        "publication-receipt-v1.json.pub": PUBLIC_KEY,
        "publication-receipt-v1.json.sig": b"s" * 64,
        "published-primary-digest": (
            f"sha256:{hashlib.sha256(primary_manifest).hexdigest()}\n".encode(
                "ascii"
            )
        ),
        "published-receipt-digest": (
            f"sha256:{hashlib.sha256(receipt_manifest).hexdigest()}\n".encode(
                "ascii"
            )
        ),
        "taira-primary-oci-manifest.json": primary_manifest,
        "taira-publication-receipt-oci-manifest.json": receipt_manifest,
    }
    for name, payload in payloads.items():
        _write_frozen(terminal / name, payload)
    terminal.chmod(0o555)
    return source_parent, handoff_root, payloads


def _close(
    source_parent: Path,
    handoff_root: Path,
    **overrides: object,
) -> dict[str, object]:
    values: dict[str, object] = {
        "expected_authority_uid": os.getuid(),
        "expected_authority_gid": os.getgid(),
        "expected_controller_uid": os.getuid(),
        "expected_controller_gid": os.getgid(),
        "expected_qualification_receipt_id": QUALIFICATION_RECEIPT_ID,
        "expected_signing_fingerprint": SIGNING_FINGERPRINT,
        "expected_source_commit": SOURCE_COMMIT,
        "expected_dpn_validator_release_commit": DPN_COMMIT,
        "expected_cargo_lock_sha256": CARGO_LOCK_SHA256,
        "expected_workspace_source_manifest_sha256": (
            WORKSPACE_MANIFEST_SHA256
        ),
        "_required_controller_uid": os.getuid(),
    }
    values.update(overrides)
    return closer._close_handoff(  # type: ignore[arg-type]
        source_parent, handoff_root, **values
    )


def _thaw_terminal(source_parent: Path) -> Path:
    terminal = source_parent / "terminal"
    terminal.chmod(0o700)
    return terminal


def _refreeze_terminal(terminal: Path) -> None:
    terminal.chmod(0o555)


def _mode(path: Path) -> int:
    return path.stat().st_mode & 0o777


def test_closer_copies_exact_terminal_to_receipt_derived_root_handoff(
    tmp_path: Path,
) -> None:
    source_parent, handoff_root, payloads = _fixture(tmp_path)

    result = _close(source_parent, handoff_root)

    output = handoff_root / f"publication-receipt-{QUALIFICATION_RECEIPT_ID}"
    assert result["output"] == str(output)
    assert result["qualification_receipt_id"] == QUALIFICATION_RECEIPT_ID
    assert sorted(path.name for path in output.iterdir()) == sorted(
        closer.TERMINAL_FILES
    )
    assert _mode(output) == 0o555
    for name, payload in payloads.items():
        assert (output / name).read_bytes() == payload
        assert _mode(output / name) == 0o444
        assert (output / name).stat().st_nlink == 1
    assert _mode(source_parent) == 0o700
    assert _mode(source_parent / "terminal") == 0o555
    assert {
        path.name for path in (source_parent / "terminal").iterdir()
    } == set(closer.TERMINAL_FILES)
    assert not list(handoff_root.glob(".*.pending-*"))


@pytest.mark.parametrize(
    ("name", "mutation", "message"),
    (
        (
            "missing",
            lambda terminal: (terminal / closer.TERMINAL_FILES[0]).unlink(),
            "inventory",
        ),
        (
            "extra",
            lambda terminal: _write_frozen(terminal / "extra", b"extra"),
            "inventory",
        ),
        (
            "bad-mode",
            lambda terminal: (terminal / closer.TERMINAL_FILES[0]).chmod(0o644),
            "identity differs",
        ),
    ),
)
def test_closer_rejects_missing_extra_and_mutable_files(
    tmp_path: Path,
    name: str,
    mutation,
    message: str,
) -> None:
    del name
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    terminal = _thaw_terminal(source_parent)
    mutation(terminal)
    _refreeze_terminal(terminal)

    with pytest.raises(closer.PublicationHandoffError, match=message):
        _close(source_parent, handoff_root)


def test_closer_rejects_symlink_and_hardlink_terminal_files(
    tmp_path: Path,
) -> None:
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    terminal = _thaw_terminal(source_parent)
    target = terminal / "publication-receipt-v1.json.sig"
    target.chmod(0o600)
    target.unlink()
    target.symlink_to("publication-receipt-v1.json.pub")
    _refreeze_terminal(terminal)
    with pytest.raises(
        (closer.PublicationHandoffError, OSError),
        match="identity differs|symbolic link|Too many levels",
    ):
        _close(source_parent, handoff_root)

    source_parent, handoff_root, _payloads = _fixture(tmp_path / "hardlink")
    terminal = _thaw_terminal(source_parent)
    os.link(
        terminal / "publication-receipt-v1.json.sig",
        source_parent / "signature-alias",
    )
    _refreeze_terminal(terminal)
    with pytest.raises(closer.PublicationHandoffError, match="identity differs"):
        _close(source_parent, handoff_root)


@pytest.mark.parametrize(
    ("path_kind", "mode", "message"),
    (
        ("source", 0o755, "source parent ownership"),
        ("terminal", 0o700, "terminal ownership"),
        ("handoff", 0o700, "handoff root ownership"),
    ),
)
def test_closer_rejects_writable_or_inexact_directories(
    tmp_path: Path,
    path_kind: str,
    mode: int,
    message: str,
) -> None:
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    paths = {
        "source": source_parent,
        "terminal": source_parent / "terminal",
        "handoff": handoff_root,
    }
    paths[path_kind].chmod(mode)

    with pytest.raises(closer.PublicationHandoffError, match=message):
        _close(source_parent, handoff_root)


def test_closer_rejects_non_ascii_and_mismatched_oci_digests(
    tmp_path: Path,
) -> None:
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    terminal = _thaw_terminal(source_parent)
    digest = terminal / "published-primary-digest"
    digest.chmod(0o600)
    _write_frozen(digest, b"\xff" * 72)
    _refreeze_terminal(terminal)
    with pytest.raises(closer.PublicationHandoffError, match="noncanonical"):
        _close(source_parent, handoff_root)

    source_parent, handoff_root, _payloads = _fixture(tmp_path / "mismatch")
    terminal = _thaw_terminal(source_parent)
    digest = terminal / "published-primary-digest"
    digest.chmod(0o600)
    _write_frozen(digest, f"sha256:{'0' * 64}\n".encode("ascii"))
    _refreeze_terminal(terminal)
    with pytest.raises(closer.PublicationHandoffError, match="digest differs"):
        _close(source_parent, handoff_root)


@pytest.mark.parametrize(
    ("override", "value", "message"),
    (
        (
            "expected_qualification_receipt_id",
            "9" * 64,
            "source binding differs",
        ),
        ("expected_source_commit", "9" * 40, "source binding differs"),
        (
            "expected_dpn_validator_release_commit",
            "9" * 40,
            "source binding differs",
        ),
        (
            "expected_cargo_lock_sha256",
            "9" * 64,
            "source binding differs",
        ),
        (
            "expected_workspace_source_manifest_sha256",
            "9" * 64,
            "source binding differs",
        ),
        (
            "expected_signing_fingerprint",
            "9" * 64,
            "fingerprint differs",
        ),
    ),
)
def test_closer_rejects_confused_deputy_semantic_bindings(
    tmp_path: Path,
    override: str,
    value: str,
    message: str,
) -> None:
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    with pytest.raises(closer.PublicationHandoffError, match=message):
        _close(source_parent, handoff_root, **{override: value})


def test_closer_rejects_duplicate_receipt_keys(tmp_path: Path) -> None:
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    terminal = _thaw_terminal(source_parent)
    receipt = terminal / "publication-receipt-v1.json"
    receipt.chmod(0o600)
    _write_frozen(receipt, b'{"schema":1,"schema":2}\n')
    _refreeze_terminal(terminal)

    with pytest.raises(closer.PublicationHandoffError, match="strict JSON"):
        _close(source_parent, handoff_root)


def test_closer_rejects_preexisting_receipt_derived_output(tmp_path: Path) -> None:
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    output = handoff_root / f"publication-receipt-{QUALIFICATION_RECEIPT_ID}"
    output.mkdir(mode=0o700)

    with pytest.raises(closer.PublicationHandoffError, match="already exists"):
        _close(source_parent, handoff_root)

    assert output.exists()
    assert not list(handoff_root.glob(".*.pending-*"))


def test_closer_detects_source_replacement_during_copy(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    terminal = source_parent / "terminal"
    target = terminal / "publication-receipt-v1.json.sig"
    real_replay = closer._replay_at
    replaced = False

    def replacing_replay(*args, **kwargs) -> None:
        nonlocal replaced
        if not replaced:
            terminal.chmod(0o700)
            target.chmod(0o600)
            target.unlink()
            _write_frozen(target, b"x" * 64)
            terminal.chmod(0o555)
            replaced = True
        real_replay(*args, **kwargs)

    monkeypatch.setattr(closer, "_replay_at", replacing_replay)
    with pytest.raises(closer.PublicationHandoffError, match="changed during close"):
        _close(source_parent, handoff_root)

    assert not list(handoff_root.iterdir())


def test_closer_detects_destination_replacement_before_commit(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    real_replay = closer._replay_output_at
    replaced = False

    def replacing_replay(
        directory_fd: int,
        captured: closer.Captured,
        expected_identity: tuple[int, ...],
    ) -> None:
        nonlocal replaced
        if not replaced:
            os.unlink(captured.name, dir_fd=directory_fd)
            descriptor = os.open(
                captured.name,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL,
                0o600,
                dir_fd=directory_fd,
            )
            try:
                os.write(descriptor, captured.payload)
                os.fchmod(descriptor, 0o444)
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
            replaced = True
        real_replay(directory_fd, captured, expected_identity)

    monkeypatch.setattr(closer, "_replay_output_at", replacing_replay)
    with pytest.raises(closer.PublicationHandoffError, match="was replaced"):
        _close(source_parent, handoff_root)

    assert not list(handoff_root.iterdir())


def test_public_entry_point_cannot_relax_root_controller_identity(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source_parent, handoff_root, _payloads = _fixture(tmp_path)
    monkeypatch.setattr(closer.os, "geteuid", lambda: 1)
    monkeypatch.setattr(
        closer,
        "_require_authenticated_rollout_observation_authority",
        lambda: None,
    )
    monkeypatch.setattr(
        closer.rollout_observation,
        "verify_authenticated_result_files",
        lambda **_kwargs: ("a" * 64, "b" * 64),
    )

    with pytest.raises(closer.PublicationHandoffError, match="root controller"):
        closer.close_handoff(
            source_parent,
            handoff_root,
            expected_authority_uid=os.getuid(),
            expected_authority_gid=os.getgid(),
            expected_controller_uid=0,
            expected_controller_gid=os.getgid(),
            expected_qualification_receipt_id=QUALIFICATION_RECEIPT_ID,
            expected_signing_fingerprint=SIGNING_FINGERPRINT,
            expected_source_commit=SOURCE_COMMIT,
            expected_dpn_validator_release_commit=DPN_COMMIT,
            expected_cargo_lock_sha256=CARGO_LOCK_SHA256,
            expected_workspace_source_manifest_sha256=(
                WORKSPACE_MANIFEST_SHA256
            ),
            rollout_plan=tmp_path / "plan.json",
            rollout_result=tmp_path / "result.json",
            rollout_authority_envelope=tmp_path / "envelope.json",
            rollout_durable_receipt=tmp_path / "receipt.json",
        )


@pytest.mark.parametrize(
    "value",
    ("", "0", "00", "+1", " 1", "1 ", "-1", "1.0"),
)
def test_identity_parser_rejects_noncanonical_positive_values(value: str) -> None:
    with pytest.raises(closer.PublicationHandoffError):
        closer._canonical_positive(value, "identity")
