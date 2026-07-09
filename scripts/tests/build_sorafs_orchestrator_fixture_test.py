"""Tests for scripts/build_sorafs_orchestrator_fixture.py."""

from __future__ import annotations

import importlib.util
import json
import os
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "build_sorafs_orchestrator_fixture.py"
SPEC = importlib.util.spec_from_file_location("build_sorafs_orchestrator_fixture", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def test_load_chunker_fixture_rejects_symlink_before_open(
    tmp_path: Path,
    monkeypatch,
) -> None:
    target = tmp_path / "target.json"
    target.write_text("{}", encoding="utf-8")
    fixture = tmp_path / "fixtures" / "sorafs_chunker" / "sf1_profile_v1.json"
    fixture.parent.mkdir(parents=True)
    fixture.symlink_to(target)
    original_open = os.open

    def open_path(path: Path, _flags: int, *args, **kwargs):
        if path == fixture:
            raise AssertionError("symlinked chunker fixture must not be opened")
        return original_open(path, _flags, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    try:
        MODULE.load_chunker_fixture(tmp_path)
    except ValueError as error:
        assert "chunker fixture" in str(error)
        assert "must not be a symlink" in str(error)
    else:
        raise AssertionError("symlinked chunker fixture was accepted")


def test_load_chunker_fixture_rejects_sensitive_duplicate_key_without_leaking(
    tmp_path: Path,
) -> None:
    fixture = tmp_path / "fixtures" / "sorafs_chunker" / "sf1_profile_v1.json"
    fixture.parent.mkdir(parents=True)
    fixture.write_text(
        '{"private&#95;key":"first","private&#95;key":"shadow"}',
        encoding="utf-8",
    )

    try:
        MODULE.load_chunker_fixture(tmp_path)
    except ValueError as error:
        message = str(error)
        assert "evidence JSON object contains duplicate key `<sensitive-key>`" in (
            message
        )
        assert "private&#95;key" not in message
        assert "private_key" not in message
    else:
        raise AssertionError("duplicate sensitive chunker fixture key was accepted")


def test_load_chunker_fixture_rejects_non_standard_numeric_constants(
    tmp_path: Path,
) -> None:
    fixture = tmp_path / "fixtures" / "sorafs_chunker" / "sf1_profile_v1.json"
    fixture.parent.mkdir(parents=True)
    fixture.write_text('{"chunk_lengths":[NaN]}', encoding="utf-8")

    try:
        MODULE.load_chunker_fixture(tmp_path)
    except ValueError as error:
        assert "non-standard JSON constant `NaN` is not allowed" in str(error)
    else:
        raise AssertionError("non-standard chunker fixture numeric constant was accepted")


def test_load_chunker_fixture_read_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    fixture = tmp_path / "fixtures" / "sorafs_chunker" / "sf1_profile_v1.json"
    fixture.parent.mkdir(parents=True)
    fixture.write_text('{"chunk_lengths":[1]}', encoding="utf-8")
    raw_error = "read denied\nprivate_key"

    def open_path(path: Path, _flags: int, *args, **kwargs):
        if path == fixture:
            raise OSError(raw_error)
        return os.open(path, _flags, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    try:
        MODULE.load_chunker_fixture(tmp_path)
    except ValueError as error:
        message = str(error)
        assert message == (
            f"failed to read chunker fixture `{fixture}`: <non-canonical-error>"
        )
        assert raw_error not in message
        assert "private_key" not in message
    else:
        raise AssertionError("chunker fixture read ignored descriptor open failure")


def test_write_json_uses_no_follow_descriptor_open(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "fixture.json"
    original_open = os.open
    opened: dict[str, int] = {}

    def open_path(path: Path, flags: int, mode: int = 0o777, *args, **kwargs):
        if path == output:
            opened["flags"] = flags
            opened["mode"] = mode
        return original_open(path, flags, mode, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    payload = {"ready": True}
    MODULE.write_json(output, payload)

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
    output = tmp_path / "fixture.json"
    payload = {"chunks": [1, 2, 3], "ready": True}
    original_write = os.write
    writes: list[int] = []

    def partial_write(fd: int, data) -> int:
        chunk = bytes(data)
        limit = max(1, min(3, len(chunk)))
        writes.append(limit)
        return original_write(fd, chunk[:limit])

    monkeypatch.setattr(MODULE.os, "write", partial_write)

    MODULE.write_json(output, payload)

    assert output.read_text(encoding="utf-8") == (
        json.dumps(payload, indent=2, allow_nan=False) + "\n"
    )
    assert len(writes) > 1


def test_write_json_fsyncs_descriptor_before_close(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "fixture.json"
    payload = {"ready": True}
    original_fsync = os.fsync
    fsynced: list[int] = []

    def fsync(fd: int) -> None:
        fsynced.append(fd)
        original_fsync(fd)

    monkeypatch.setattr(MODULE.os, "fsync", fsync)

    MODULE.write_json(output, payload)

    assert json.loads(output.read_text(encoding="utf-8")) == payload
    assert len(fsynced) == 2


def test_write_json_propagates_fsync_failure_without_leaking_error(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "fixture.json"
    raw_error = "fsync denied\nprivate_key"

    def fsync(_fd: int) -> None:
        raise OSError(raw_error)

    monkeypatch.setattr(MODULE.os, "fsync", fsync)

    try:
        MODULE.write_json(output, {"ready": True})
    except ValueError as error:
        message = str(error)
        assert message == (
            "failed to write orchestrator fixture output "
            f"`{output}`: <non-canonical-error>"
        )
        assert raw_error not in message
        assert "private_key" not in message
    else:
        raise AssertionError("fixture write ignored fsync failure")


def test_write_json_fsyncs_output_parent_after_descriptor_close(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "fixture.json"
    calls: list[tuple[Path, str]] = []

    def record_parent_sync(path: Path, *, label: str) -> list[str]:
        calls.append((path, label))
        return []

    monkeypatch.setattr(MODULE, "fsync_checker_output_parent", record_parent_sync)

    MODULE.write_json(output, {"ready": True})

    assert calls == [(output, "orchestrator fixture output")]


def test_write_json_propagates_parent_fsync_failure(tmp_path: Path, monkeypatch) -> None:
    output = tmp_path / "fixture.json"

    def fail_parent_sync(_path: Path, *, label: str) -> list[str]:
        return [f"{label} parent cannot be fsynced"]

    monkeypatch.setattr(MODULE, "fsync_checker_output_parent", fail_parent_sync)

    try:
        MODULE.write_json(output, {"ready": True})
    except ValueError as error:
        assert str(error) == "orchestrator fixture output parent cannot be fsynced"
    else:
        raise AssertionError("fixture write ignored parent fsync failure")


def test_ensure_fixture_directory_rejects_symlink_before_create(
    tmp_path: Path,
) -> None:
    target = tmp_path / "target-output"
    target.mkdir()
    output = tmp_path / "output"
    output.symlink_to(target, target_is_directory=True)

    try:
        MODULE.ensure_fixture_directory(output, "orchestrator fixture output directory")
    except ValueError as error:
        assert "orchestrator fixture output directory" in str(error)
        assert "must not be a symlink" in str(error)
    else:
        raise AssertionError("symlinked output directory was accepted")


def test_ensure_fixture_directory_mkdir_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "output"
    raw_error = "mkdir denied\nprivate_key"
    original_mkdir = Path.mkdir

    def mkdir(path: Path, *args, **kwargs):
        if path == output:
            raise OSError(raw_error)
        return original_mkdir(path, *args, **kwargs)

    monkeypatch.setattr(Path, "mkdir", mkdir)

    try:
        MODULE.ensure_fixture_directory(output, "orchestrator fixture output directory")
    except ValueError as error:
        message = str(error)
        assert message == (
            "failed to create orchestrator fixture output directory "
            f"`{output}`: <non-canonical-error>"
        )
        assert raw_error not in message
        assert "private_key" not in message
    else:
        raise AssertionError("orchestrator fixture output directory mkdir failure was ignored")


def test_validate_fixture_path_rejects_secret_looking_path_without_leaking(
    tmp_path: Path,
) -> None:
    secret_path = tmp_path / "private%26%2395%3Bkey"

    try:
        MODULE.validate_fixture_path(secret_path, "orchestrator fixture output")
    except ValueError as error:
        message = str(error)
        assert message == MODULE.FIXTURE_PATH_DIAGNOSTIC
        assert "private%26%2395%3Bkey" not in message
        assert "private&#95;key" not in message
        assert "private_key" not in message
    else:
        raise AssertionError("secret-looking orchestrator fixture path was accepted")


def test_fixture_file_size_uses_no_follow_descriptor_fstat(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = tmp_path / "payload.bin"
    payload.write_bytes(b"fixture")
    original_open = os.open
    opened: dict[str, int] = {}

    def open_path(path: Path, flags: int, *args, **kwargs):
        if path == payload:
            opened["flags"] = flags
        return original_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    assert MODULE.fixture_file_size(payload, "chunker payload") == len(b"fixture")
    assert opened["flags"] == MODULE.read_open_flags()
    if hasattr(os, "O_NOFOLLOW"):
        assert opened["flags"] & os.O_NOFOLLOW


def test_fixture_file_size_fstat_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    payload = tmp_path / "payload.bin"
    payload.write_bytes(b"fixture")
    raw_error = "fstat denied\nprivate_key"

    def fstat(_fd: int):
        raise OSError(raw_error)

    monkeypatch.setattr(MODULE.os, "fstat", fstat)

    try:
        MODULE.fixture_file_size(payload, "chunker payload")
    except ValueError as error:
        message = str(error)
        assert message == (
            f"failed to inspect chunker payload `{payload}`: <non-canonical-error>"
        )
        assert raw_error not in message
        assert "private_key" not in message
    else:
        raise AssertionError("fixture file size ignored fstat failure")


def test_build_telemetry_includes_reputation_score() -> None:
    telemetry = MODULE.build_telemetry([{"provider_id": "fixture-provider-0"}])

    assert telemetry == [
        {
            "provider_id": "fixture-provider-0",
            "qos_score": 95.0,
            "latency_p95_ms": 120.0,
            "failure_rate_ewma": 0.0,
            "token_health": 0.9,
            "staking_weight": 1.0,
            "reputation_score_bps": 10_000,
            "penalty": False,
            "last_updated_unix": MODULE.FIXTURE_NOW_UNIX_SECS - 120,
        }
    ]
