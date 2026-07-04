"""Tests for scripts/android_codegen_replay_sorafs_fixture.py."""

from __future__ import annotations

import importlib.util
import json
import os
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "android_codegen_replay_sorafs_fixture.py"
)
SPEC = importlib.util.spec_from_file_location(
    "android_codegen_replay_sorafs_fixture",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_load_json_uses_no_follow_descriptor_open(
    tmp_path: Path,
    monkeypatch,
) -> None:
    source = tmp_path / "fixture.json"
    source.write_text('{"ready": true}', encoding="utf-8")
    original_open = os.open
    opened: dict[str, int] = {}

    def open_path(path: Path, flags: int, *args, **kwargs):
        if path == source:
            opened["flags"] = flags
        return original_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    assert MODULE.load_json(source) == {"ready": True}
    assert opened["flags"] == MODULE.read_open_flags()
    if hasattr(os, "O_NOFOLLOW"):
        assert opened["flags"] & os.O_NOFOLLOW


def test_load_json_rejects_symlink_before_open(
    tmp_path: Path,
    monkeypatch,
) -> None:
    target = tmp_path / "target.json"
    target.write_text("{}", encoding="utf-8")
    source = tmp_path / "fixture.json"
    source.symlink_to(target)

    def open_path(path: Path, _flags: int, *args, **kwargs):
        if path == source:
            raise AssertionError("symlinked fixture must not be opened")
        return os.open(path, _flags, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    try:
        MODULE.load_json(source, label="Android fixture")
    except ValueError as error:
        assert "Android fixture" in str(error)
        assert "must not be a symlink" in str(error)
    else:
        raise AssertionError("symlinked Android fixture was accepted")


def test_write_json_uses_no_follow_descriptor_open(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "generated" / "fixture.json"
    original_open = os.open
    opened: dict[str, int] = {}

    def open_path(path: Path, flags: int, mode: int = 0o777, *args, **kwargs):
        if path == output:
            opened["flags"] = flags
            opened["mode"] = mode
        return original_open(path, flags, mode, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    payload = {"ready": True}
    MODULE.write_json(output, payload, label="Android fixture")

    assert output.read_text(encoding="utf-8") == (
        json.dumps(payload, indent=2, allow_nan=False) + "\n"
    )
    assert json.loads(output.read_text(encoding="utf-8")) == payload
    assert opened["flags"] & os.O_WRONLY
    assert opened["flags"] & os.O_CREAT
    assert opened["flags"] & os.O_TRUNC
    if hasattr(os, "O_NOFOLLOW"):
        assert opened["flags"] & os.O_NOFOLLOW
    assert opened["mode"] == 0o666
    assert opened["flags"] == MODULE.write_open_flags()


def test_write_json_completes_partial_descriptor_writes(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "generated" / "fixture.json"
    payload = {"chunks": [1, 2, 3], "ready": True}
    original_write = os.write
    writes: list[int] = []

    def partial_write(fd: int, data) -> int:
        chunk = bytes(data)
        limit = max(1, min(3, len(chunk)))
        writes.append(limit)
        return original_write(fd, chunk[:limit])

    monkeypatch.setattr(MODULE.os, "write", partial_write)

    MODULE.write_json(output, payload, label="Android fixture")

    assert output.read_text(encoding="utf-8") == (
        json.dumps(payload, indent=2, allow_nan=False) + "\n"
    )
    assert len(writes) > 1


def test_write_json_fsyncs_descriptor_before_close(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "generated" / "fixture.json"
    payload = {"ready": True}
    original_fsync = os.fsync
    fsynced: list[int] = []

    def fsync(fd: int) -> None:
        fsynced.append(fd)
        original_fsync(fd)

    monkeypatch.setattr(MODULE.os, "fsync", fsync)

    MODULE.write_json(output, payload, label="Android fixture")

    assert json.loads(output.read_text(encoding="utf-8")) == payload
    assert len(fsynced) == 2


def test_write_json_propagates_fsync_failure_without_leaking_path(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "bad\nfixture.json"
    bad_message = "fsync denied"

    def fsync(_fd: int) -> None:
        raise OSError(bad_message)

    monkeypatch.setattr(MODULE.os, "fsync", fsync)

    try:
        MODULE.write_json(output, {"ready": True}, label="Android fixture")
    except ValueError as error:
        assert str(error) == (
            "failed to write Android fixture `<non-canonical-path>`: "
            "<non-canonical-error>"
        )
        assert str(output) not in str(error)
        assert bad_message not in str(error)
    else:
        raise AssertionError("Android fixture write ignored fsync failure")


def test_write_json_fsyncs_output_parent_after_descriptor_close(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "generated" / "fixture.json"
    calls: list[tuple[Path, str]] = []

    def record_parent_sync(path: Path, *, label: str) -> list[str]:
        calls.append((path, label))
        return []

    monkeypatch.setattr(MODULE, "fsync_checker_output_parent", record_parent_sync)

    MODULE.write_json(output, {"ready": True}, label="Android fixture")

    assert calls == [(output, "Android fixture")]


def test_write_json_parent_fsync_failure_does_not_leak_path(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "bad\nfixture.json"
    raw_message = f"parent fsync denied for {output}\nsecret"

    def fail_parent_sync(_path: Path, *, label: str) -> list[str]:
        assert label == "Android fixture"
        return [
            "failed to fsync Android fixture parent `<non-canonical-path>`: "
            "<non-canonical-error>"
        ]

    monkeypatch.setattr(MODULE, "fsync_checker_output_parent", fail_parent_sync)

    try:
        MODULE.write_json(output, {"ready": True}, label="Android fixture")
    except ValueError as error:
        assert str(error) == (
            "failed to fsync Android fixture parent `<non-canonical-path>`: "
            "<non-canonical-error>"
        )
        assert str(output) not in str(error)
        assert raw_message not in str(error)
    else:
        raise AssertionError("Android fixture write ignored parent fsync failure")


def test_write_json_rejects_symlinked_parent_before_create(
    tmp_path: Path,
    monkeypatch,
) -> None:
    target_dir = tmp_path / "target"
    target_dir.mkdir()
    linked_parent = tmp_path / "linked-parent"
    linked_parent.symlink_to(target_dir, target_is_directory=True)
    output = linked_parent / "fixture.json"

    def open_path(path: Path, _flags: int, *args, **kwargs):
        if path == output:
            raise AssertionError("symlinked output parent must not be opened")
        return os.open(path, _flags, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    try:
        MODULE.write_json(output, {"ready": True}, label="Android fixture")
    except ValueError as error:
        assert "Android fixture" in str(error)
        assert "must not be a symlink" in str(error)
    else:
        raise AssertionError("symlinked Android fixture output parent was accepted")


def test_require_codegen_file_rejects_symlink_before_subprocess(
    tmp_path: Path,
) -> None:
    target = tmp_path / "payload.bin"
    target.write_bytes(b"payload")
    payload = tmp_path / "payload-link.bin"
    payload.symlink_to(target)

    try:
        MODULE.require_codegen_file(payload, "payload path")
    except ValueError as error:
        assert "payload path" in str(error)
        assert "must not be a symlink" in str(error)
    else:
        raise AssertionError("symlinked payload path was accepted")
