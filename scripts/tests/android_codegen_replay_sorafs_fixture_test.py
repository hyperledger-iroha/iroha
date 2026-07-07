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


def test_load_json_rejects_sensitive_duplicate_key_without_leaking(
    tmp_path: Path,
) -> None:
    source = tmp_path / "fixture.json"
    source.write_text(
        '{"private%5Fkey":"first","private%5Fkey":"shadow"}',
        encoding="utf-8",
    )

    try:
        MODULE.load_json(source, label="Android fixture")
    except ValueError as error:
        message = str(error)
        assert "evidence JSON object contains duplicate key `<sensitive-key>`" in (
            message
        )
        assert "private%5Fkey" not in message
        assert "private_key" not in message
    else:
        raise AssertionError("duplicate sensitive Android fixture key was accepted")


def test_load_json_rejects_non_standard_numeric_constants(tmp_path: Path) -> None:
    source = tmp_path / "fixture.json"
    source.write_text('{"latency_ms": Infinity}', encoding="utf-8")

    try:
        MODULE.load_json(source, label="Android fixture")
    except ValueError as error:
        assert "non-standard JSON constant `Infinity` is not allowed" in str(error)
    else:
        raise AssertionError("non-standard Android fixture numeric constant was accepted")


def test_load_json_read_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    source = tmp_path / "fixture.json"
    source.write_text('{"ready": true}', encoding="utf-8")
    raw_error = "read denied\nprivate_key"

    def open_path(path: Path, _flags: int, *args, **kwargs):
        if path == source:
            raise OSError(raw_error)
        return os.open(path, _flags, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    try:
        MODULE.load_json(source, label="Android fixture")
    except ValueError as error:
        message = str(error)
        assert message == (
            f"failed to read Android fixture `{source}`: <non-canonical-error>"
        )
        assert raw_error not in message
        assert "private_key" not in message
    else:
        raise AssertionError("Android fixture read ignored descriptor open failure")


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
    output = tmp_path / "generated" / "fixture.json"
    bad_message = "fsync\ndenied"

    def fsync(_fd: int) -> None:
        raise OSError(bad_message)

    monkeypatch.setattr(MODULE.os, "fsync", fsync)

    try:
        MODULE.write_json(output, {"ready": True}, label="Android fixture")
    except ValueError as error:
        assert str(error) == (
            f"failed to write Android fixture `{output}`: <non-canonical-error>"
        )
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
    output = tmp_path / "generated" / "fixture.json"
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


def test_ensure_codegen_directory_mkdir_error_is_sanitized(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "generated"
    raw_error = "mkdir denied\nprivate_key"
    original_mkdir = Path.mkdir

    def mkdir(path: Path, *args, **kwargs):
        if path == output:
            raise OSError(raw_error)
        return original_mkdir(path, *args, **kwargs)

    monkeypatch.setattr(Path, "mkdir", mkdir)

    try:
        MODULE.ensure_codegen_directory(output, "Android fixture output directory")
    except ValueError as error:
        message = str(error)
        assert message == (
            "failed to create Android fixture output directory "
            f"`{output}`: <non-canonical-error>"
        )
        assert raw_error not in message
        assert "private_key" not in message
    else:
        raise AssertionError("Android fixture output directory mkdir failure was ignored")


def test_validate_codegen_path_rejects_secret_looking_path_without_leaking(
    tmp_path: Path,
) -> None:
    secret_path = tmp_path / "private%26%2395%3Bkey"

    try:
        MODULE.validate_codegen_path(secret_path, "Android fixture")
    except ValueError as error:
        message = str(error)
        assert message == MODULE.CODEGEN_PATH_DIAGNOSTIC
        assert "private%26%2395%3Bkey" not in message
        assert "private&#95;key" not in message
        assert "private_key" not in message
    else:
        raise AssertionError("secret-looking Android fixture path was accepted")


def test_require_relative_fixture_path_accepts_safe_relative_path() -> None:
    assert MODULE.require_relative_fixture_path(
        "fixtures/plan.json",
        label="plan file",
    ) == Path("fixtures/plan.json")


def test_require_relative_fixture_path_rejects_unsafe_values_without_leaking() -> None:
    for value, leaked in (
        ("/tmp/private%26%2395%3Bkey", "private%26%2395%3Bkey"),
        ("../plan.json", "../plan.json"),
        ("nested/private&#95;key.json", "private&#95;key"),
        ("", ""),
        (b"plan.json", "plan.json"),
    ):
        try:
            MODULE.require_relative_fixture_path(value, label="plan file")
        except ValueError as error:
            message = str(error)
            assert message == MODULE.FIXTURE_METADATA_PATH_DIAGNOSTIC
            assert "private_key" not in message
            if leaked:
                assert leaked not in message
        else:
            raise AssertionError(f"unsafe fixture metadata path {value!r} was accepted")


def test_require_fixture_name_accepts_safe_name() -> None:
    assert MODULE.require_fixture_name("multi_peer_parity_v1") == "multi_peer_parity_v1"


def test_require_fixture_name_rejects_unsafe_values_without_leaking() -> None:
    for value, leaked in (
        ("../private%26%2395%3Bkey", "private%26%2395%3Bkey"),
        ("private&#95;key", "private&#95;key"),
        ("multi_peer_parity_v1.json", "multi_peer_parity_v1.json"),
        ("", ""),
        (b"multi_peer_parity_v1", "multi_peer_parity_v1"),
    ):
        try:
            MODULE.require_fixture_name(value)
        except ValueError as error:
            message = str(error)
            assert message == MODULE.FIXTURE_METADATA_NAME_DIAGNOSTIC
            assert "private_key" not in message
            if leaked:
                assert leaked not in message
        else:
            raise AssertionError(f"unsafe fixture name {value!r} was accepted")


def test_require_subprocess_metadata_fields_accept_canonical_values() -> None:
    assert MODULE.require_profile_handle("sorafs.sf1@1.0.0") == "sorafs.sf1@1.0.0"
    assert MODULE.require_storage_class("hot") == "hot"
    assert MODULE.require_storage_class("warm") == "warm"
    assert MODULE.require_storage_class("cold") == "cold"
    assert MODULE.require_metadata_int(0, minimum=0) == 0
    assert MODULE.require_metadata_int(3, minimum=1) == 3


def test_require_subprocess_metadata_fields_reject_unsafe_values_without_leaking() -> None:
    invalid_values = (
        lambda: MODULE.require_profile_handle("private%26%2395%3Bkey"),
        lambda: MODULE.require_profile_handle("sorafs.sf1@1.0.0\nsecret"),
        lambda: MODULE.require_profile_handle(b"sorafs.sf1@1.0.0"),
        lambda: MODULE.require_storage_class("archive"),
        lambda: MODULE.require_storage_class("private&#95;key"),
        lambda: MODULE.require_storage_class(b"hot"),
        lambda: MODULE.require_metadata_int(True, minimum=0),
        lambda: MODULE.require_metadata_int(-1, minimum=0),
        lambda: MODULE.require_metadata_int(0, minimum=1),
        lambda: MODULE.require_metadata_int(2**63, minimum=0),
        lambda: MODULE.require_metadata_int("3", minimum=1),
    )
    for invalid in invalid_values:
        try:
            invalid()
        except ValueError as error:
            message = str(error)
            assert message == MODULE.FIXTURE_METADATA_FIELD_DIAGNOSTIC
            assert "private%26%2395%3Bkey" not in message
            assert "private&#95;key" not in message
            assert "private_key" not in message
            assert "sorafs.sf1@1.0.0" not in message
        else:
            raise AssertionError("unsafe subprocess metadata field was accepted")


def test_main_rejects_absolute_payload_metadata_path_before_subprocess_without_leaking(
    tmp_path: Path,
    monkeypatch,
) -> None:
    fixture_dir = tmp_path / "fixtures"
    fixture_dir.mkdir()
    (fixture_dir / "metadata.json").write_text(
        json.dumps(
            {
                "payload_path": "/tmp/private%26%2395%3Bkey.bin",
                "plan_file": "plan.json",
                "providers_file": "providers.json",
                "telemetry_file": "telemetry.json",
                "options_file": "options.json",
                "profile_handle": "sorafs.sf1@1.0.0",
                "fixture": "multi_peer_parity_v1",
                "now_unix_secs": 1_725_000_000,
            }
        ),
        encoding="utf-8",
    )
    chunker_fixture = tmp_path / "chunker.json"
    chunker_fixture.write_text("{}", encoding="utf-8")

    def fail_manifest_stub(*_args, **_kwargs) -> None:
        raise AssertionError("manifest replay must not run for unsafe metadata paths")

    monkeypatch.setattr(MODULE, "run_manifest_stub", fail_manifest_stub)

    try:
        MODULE.main(
            [
                "--fixture-dir",
                str(fixture_dir),
                "--chunker-fixture",
                str(chunker_fixture),
                "--register-pin-example",
                str(tmp_path / "register_pin.json"),
                "--report-dir",
                str(tmp_path / "reports"),
                "--tracked-fixture-out",
                str(tmp_path / "tracked.json"),
                "--cargo-bin",
                "cargo",
            ]
        )
    except SystemExit as error:
        message = str(error)
        assert message == MODULE.FIXTURE_METADATA_PATH_DIAGNOSTIC
        assert "private%26%2395%3Bkey" not in message
        assert "private&#95;key" not in message
        assert "private_key" not in message
    else:
        raise AssertionError("unsafe payload metadata path was accepted")


def test_main_rejects_unsafe_fixture_name_before_subprocess_without_leaking(
    tmp_path: Path,
    monkeypatch,
) -> None:
    fixture_dir = tmp_path / "fixtures"
    fixture_dir.mkdir()
    (fixture_dir / "metadata.json").write_text(
        json.dumps(
            {
                "payload_path": "fuzz/sorafs_chunker/sf1_profile_v1_input.bin",
                "plan_file": "plan.json",
                "providers_file": "providers.json",
                "telemetry_file": "telemetry.json",
                "options_file": "options.json",
                "profile_handle": "sorafs.sf1@1.0.0",
                "fixture": "private%26%2395%3Bkey",
                "now_unix_secs": 1_725_000_000,
            }
        ),
        encoding="utf-8",
    )
    chunker_fixture = tmp_path / "chunker.json"
    chunker_fixture.write_text("{}", encoding="utf-8")

    def fail_manifest_stub(*_args, **_kwargs) -> None:
        raise AssertionError("manifest replay must not run for unsafe fixture names")

    monkeypatch.setattr(MODULE, "run_manifest_stub", fail_manifest_stub)

    try:
        MODULE.main(
            [
                "--fixture-dir",
                str(fixture_dir),
                "--chunker-fixture",
                str(chunker_fixture),
                "--register-pin-example",
                str(tmp_path / "register_pin.json"),
                "--report-dir",
                str(tmp_path / "reports"),
                "--tracked-fixture-out",
                str(tmp_path / "tracked.json"),
                "--cargo-bin",
                "cargo",
            ]
        )
    except SystemExit as error:
        message = str(error)
        assert message == MODULE.FIXTURE_METADATA_NAME_DIAGNOSTIC
        assert "private%26%2395%3Bkey" not in message
        assert "private&#95;key" not in message
        assert "private_key" not in message
    else:
        raise AssertionError("unsafe fixture name was accepted")


def test_main_rejects_unsafe_profile_before_subprocess_without_leaking(
    tmp_path: Path,
    monkeypatch,
) -> None:
    fixture_dir = tmp_path / "fixtures"
    fixture_dir.mkdir()
    (fixture_dir / "metadata.json").write_text(
        json.dumps(
            {
                "payload_path": "fuzz/sorafs_chunker/sf1_profile_v1_input.bin",
                "plan_file": "plan.json",
                "providers_file": "providers.json",
                "telemetry_file": "telemetry.json",
                "options_file": "options.json",
                "profile_handle": "private%26%2395%3Bkey",
                "fixture": "multi_peer_parity_v1",
                "now_unix_secs": 1_725_000_000,
            }
        ),
        encoding="utf-8",
    )
    chunker_fixture = tmp_path / "chunker.json"
    chunker_fixture.write_text("{}", encoding="utf-8")

    def fail_manifest_stub(*_args, **_kwargs) -> None:
        raise AssertionError("manifest replay must not run for unsafe profile handles")

    monkeypatch.setattr(MODULE, "run_manifest_stub", fail_manifest_stub)

    try:
        MODULE.main(
            [
                "--fixture-dir",
                str(fixture_dir),
                "--chunker-fixture",
                str(chunker_fixture),
                "--register-pin-example",
                str(tmp_path / "register_pin.json"),
                "--report-dir",
                str(tmp_path / "reports"),
                "--tracked-fixture-out",
                str(tmp_path / "tracked.json"),
                "--cargo-bin",
                "cargo",
            ]
        )
    except SystemExit as error:
        message = str(error)
        assert message == MODULE.FIXTURE_METADATA_FIELD_DIAGNOSTIC
        assert "private%26%2395%3Bkey" not in message
        assert "private&#95;key" not in message
        assert "private_key" not in message
    else:
        raise AssertionError("unsafe profile metadata was accepted")


def test_main_rejects_unsafe_display_metadata_file_before_subprocess_without_leaking(
    tmp_path: Path,
    monkeypatch,
) -> None:
    fixture_dir = tmp_path / "fixtures"
    fixture_dir.mkdir()
    (fixture_dir / "metadata.json").write_text(
        json.dumps(
            {
                "payload_path": "fuzz/sorafs_chunker/sf1_profile_v1_input.bin",
                "plan_file": "plan.json",
                "providers_file": "private%26%2395%3Bkey.json",
                "telemetry_file": "telemetry.json",
                "options_file": "options.json",
                "profile_handle": "sorafs.sf1@1.0.0",
                "fixture": "multi_peer_parity_v1",
                "now_unix_secs": 1_725_000_000,
            }
        ),
        encoding="utf-8",
    )
    chunker_fixture = tmp_path / "chunker.json"
    chunker_fixture.write_text("{}", encoding="utf-8")

    def fail_manifest_stub(*_args, **_kwargs) -> None:
        raise AssertionError("manifest replay must not run for unsafe metadata files")

    monkeypatch.setattr(MODULE, "run_manifest_stub", fail_manifest_stub)

    try:
        MODULE.main(
            [
                "--fixture-dir",
                str(fixture_dir),
                "--chunker-fixture",
                str(chunker_fixture),
                "--register-pin-example",
                str(tmp_path / "register_pin.json"),
                "--report-dir",
                str(tmp_path / "reports"),
                "--tracked-fixture-out",
                str(tmp_path / "tracked.json"),
                "--cargo-bin",
                "cargo",
            ]
        )
    except SystemExit as error:
        message = str(error)
        assert message == MODULE.FIXTURE_METADATA_PATH_DIAGNOSTIC
        assert "private%26%2395%3Bkey" not in message
        assert "private&#95;key" not in message
        assert "private_key" not in message
    else:
        raise AssertionError("unsafe display metadata file was accepted")


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
