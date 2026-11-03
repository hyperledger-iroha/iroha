"""Tests for scripts/android_codegen_replay_sorafs_fixture.py."""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
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


def test_default_tracked_fixture_uses_repository_local_specs() -> None:
    """The replay writer must not recreate the retired public-docs tree."""

    assert MODULE.DEFAULT_TRACKED_FIXTURE == (
        MODULE.REPO_ROOT
        / "specs"
        / "sdk"
        / "android"
        / "generated"
        / "fixtures"
        / "sorafs_register_pin_manifest_multi_peer_parity_v1.json"
    )


def test_cargo_policy_proxy_rejects_caller_selected_executable(tmp_path: Path) -> None:
    alternate = tmp_path / "cargo"
    alternate.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
    alternate.chmod(0o700)

    try:
        MODULE.require_policy_cargo_proxy(alternate)
    except ValueError as error:
        assert "exact source-bound policy proxy" in str(error)
    else:
        raise AssertionError("caller-selected Cargo executable bypassed the policy proxy")


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


def test_test_council_signing_seed_is_ephemeral_private_and_exact(
    tmp_path: Path,
    monkeypatch,
) -> None:
    output = tmp_path / "council-signing.seed"
    original_open = os.open
    opened: dict[str, int] = {}

    def open_path(path: Path, flags: int, mode: int = 0o777, *args, **kwargs):
        if path == output:
            opened["flags"] = flags
            opened["mode"] = mode
        return original_open(path, flags, mode, *args, **kwargs)

    monkeypatch.setattr(MODULE.os, "open", open_path)

    MODULE.write_test_council_signing_seed(output)

    assert output.read_bytes() == MODULE.ANDROID_CODEGEN_TEST_COUNCIL_SIGNING_SEED
    assert len(output.read_bytes()) == 32
    assert opened["flags"] == MODULE.signing_key_write_open_flags()
    assert opened["flags"] & os.O_EXCL
    assert opened["mode"] == 0o600
    assert output.stat().st_mode & 0o077 == 0


def test_test_council_signing_seed_rejects_symlink_and_existing_output(
    tmp_path: Path,
) -> None:
    existing = tmp_path / "existing.seed"
    existing.write_bytes(b"unchanged")
    try:
        MODULE.write_test_council_signing_seed(existing)
    except ValueError as error:
        assert "failed to write Android codegen test-only council signing seed" in str(
            error
        )
    else:
        raise AssertionError("existing test signing seed was overwritten")
    assert existing.read_bytes() == b"unchanged"

    target = tmp_path / "target.seed"
    target.write_bytes(b"target")
    linked = tmp_path / "linked.seed"
    linked.symlink_to(target)
    try:
        MODULE.write_test_council_signing_seed(linked)
    except ValueError as error:
        assert "must not be a symlink" in str(error)
    else:
        raise AssertionError("symlinked test signing seed was accepted")
    assert target.read_bytes() == b"target"


def test_manifest_builder_receives_only_ephemeral_signing_key_path(
    tmp_path: Path,
    monkeypatch,
) -> None:
    calls: list[tuple[list[str], bool, Path]] = []

    def run(command: list[str], *, check: bool, cwd: Path) -> None:
        calls.append((command, check, cwd))

    monkeypatch.setattr(MODULE.subprocess, "run", run)
    key_path = tmp_path / "council-signing.seed"
    MODULE.run_manifest_builder(
        "cargo",
        tmp_path / "payload.bin",
        tmp_path / "plan.json",
        "sorafs.sf1@1.0.0",
        3,
        "hot",
        1_725_086_400,
        key_path,
        tmp_path / "report.json",
        tmp_path / "manifest.to",
    )

    assert len(calls) == 1
    command, check, cwd = calls[0]
    assert command[:5] == ["cargo", "run", "--locked", "--offline", "--quiet"]
    assert f"--council-signing-key-file={key_path}" in command
    assert MODULE.ANDROID_CODEGEN_TEST_COUNCIL_SIGNING_SEED.hex() not in " ".join(
        command
    )
    assert check is True
    assert cwd == MODULE.REPO_ROOT


def test_generated_manifest_requires_one_canonical_council_signature() -> None:
    canonical = {
        "manifest": {
            "council_signatures": [
                {"signer_hex": "11" * 32, "signature_hex": "22" * 64}
            ]
        }
    }
    MODULE.require_generated_council_signature(canonical)

    invalid_reports = [
        {},
        {"manifest": {"council_signatures": []}},
        {
            "manifest": {
                "council_signatures": [
                    {"signer_hex": "11" * 32, "signature_hex": "22" * 64},
                    {"signer_hex": "33" * 32, "signature_hex": "44" * 64},
                ]
            }
        },
        {
            "manifest": {
                "council_signatures": [
                    {"signer_hex": "11" * 31, "signature_hex": "22" * 64}
                ]
            }
        },
        {
            "manifest": {
                "council_signatures": [
                    {"signer_hex": "AA" * 32, "signature_hex": "22" * 64}
                ]
            }
        },
        {
            "manifest": {
                "council_signatures": [
                    {
                        "signer_hex": "11" * 32,
                        "signature_hex": "22" * 64,
                        "seed": "must-not-appear",
                    }
                ]
            }
        },
    ]
    for report in invalid_reports:
        try:
            MODULE.require_generated_council_signature(report)
        except ValueError as error:
            assert "generated Android manifest" in str(error)
        else:
            raise AssertionError("malformed generated council signature was accepted")


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


def test_build_fixture_example_uses_reviewed_metadata_timestamp() -> None:
    fixture_meta = {
        "fixture": "multi_peer_parity_v1",
        "plan_file": "plan.json",
        "providers_file": "providers.json",
        "telemetry_file": "telemetry.json",
        "now_unix_secs": 1_725_000_000,
    }
    manifest_report = {
        "chunking": {
            "profile_id": "sf1",
            "namespace": "sorafs",
            "name": "sf1",
            "semver": "1.0.0",
            "handle": "sorafs.sf1@1.0.0",
            "profile_aliases": ["sorafs.sf1"],
            "multihash_code": 0x16,
        },
        "manifest": {
            "digest_hex": "b" * 64,
            "pin_policy": {"min_replicas": 4},
        },
        "chunk_digests": [{"digest_blake3": "c" * 64}],
    }

    example = MODULE.build_fixture_example(
        fixture_meta,
        manifest_report,
        "AQID",
    )

    assert example["generated_at"] == "2024-08-30T06:40:00Z"
    assert example["manifest_report_path"] == (
        "target-codex/android_codegen/sorafs_manifest/multi_peer_parity_v1.json"
    )
    assert example["instruction"]["manifest_payload_base64"] == "AQID"
    assert "submitted_epoch" not in example["instruction"]
    assert "digest_hex" not in example["instruction"]
    assert "chunk_digest_sha3_256_hex" not in example["instruction"]


def test_main_rejects_nonfuture_retention_before_subprocess(
    tmp_path: Path,
    monkeypatch,
) -> None:
    fixture_dir = tmp_path / "fixtures"
    fixture_dir.mkdir()
    now = 1_725_000_000
    (fixture_dir / "metadata.json").write_text(
        json.dumps(
            {
                "payload_path": "payload.bin",
                "plan_file": "plan.json",
                "providers_file": "providers.json",
                "telemetry_file": "telemetry.json",
                "options_file": "options.json",
                "profile_handle": "sorafs.sf1@1.0.0",
                "fixture": "multi_peer_parity_v1",
                "now_unix_secs": now,
                "retention_epoch": now,
            }
        ),
        encoding="utf-8",
    )
    chunker_fixture = tmp_path / "chunker.json"
    chunker_fixture.write_text("{}", encoding="utf-8")

    def fail_manifest_builder(*_args, **_kwargs) -> None:
        raise AssertionError("stale retention reached manifest replay")

    monkeypatch.setattr(MODULE, "run_manifest_builder", fail_manifest_builder)

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
            ]
        )
    except SystemExit as error:
        assert "retention_epoch must be later than now_unix_secs" in str(error)
    else:
        raise AssertionError("nonfuture retention was accepted")


def test_main_keeps_external_replay_stage_out_of_fixture_bytes(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    """Exercise real output I/O with only the external Cargo producer replaced."""

    chunker_fixture = json.loads(MODULE.DEFAULT_CHUNKER_FIXTURE.read_text())
    report = {
        "manifest": {
            "chunk_digest_sha3_256_hex": chunker_fixture["chunk_digest_sha3_256"],
            "council_signatures": [{"signer_hex": "a" * 64, "signature_hex": "b" * 128}],
        },
        "chunk_digests": [{"digest_blake3": "c" * 64}],
    }
    invocations = []

    def produce_report(cargo_proxy, payload_path, plan_path, profile_handle, **outputs):
        invocations.append((cargo_proxy, payload_path, plan_path, profile_handle))
        assert outputs["council_signing_key_file"].read_bytes()
        outputs["json_out"].write_text(json.dumps(report))
        outputs["manifest_out"].write_bytes(b"fixture-path-test")

    monkeypatch.setattr(MODULE, "run_manifest_builder", produce_report)
    captured = []
    for name in ("first", "second"):
        stage = (tmp_path / name).resolve()
        stage.mkdir()
        assert not stage.is_relative_to(MODULE.REPO_ROOT)
        example = stage / "codegen" / "instruction_examples" / "RegisterPinManifest.json"
        example.parent.mkdir(parents=True)
        example.write_text('{"existing":true}\n')
        report_dir = stage / "codegen" / "sorafs_manifest"
        tracked = stage / "generated" / "fixtures" / "sorafs.json"
        assert MODULE.main([
            "--register-pin-example", str(example),
            "--report-dir", str(report_dir),
            "--tracked-fixture-out", str(tracked),
        ]) == 0
        report_file = report_dir / "multi_peer_parity_v1.json"
        fixture = json.loads(tracked.read_bytes())
        assert fixture["manifest_report_path"] == (
            "target-codex/android_codegen/sorafs_manifest/multi_peer_parity_v1.json"
        )
        updated = json.loads(example.read_bytes())
        assert updated == {"existing": True, "fixture_example": fixture}
        assert json.loads(report_file.read_bytes()) == report
        captured.append((example.read_bytes(), tracked.read_bytes(), report_file.read_bytes()))
        assert all(str(stage).encode() not in data for data in captured[-1])
        diagnostics = capsys.readouterr().out
        assert str(report_file) in diagnostics
        assert str(example) in diagnostics
        assert str(tracked) in diagnostics
    assert len(invocations) == 2
    assert invocations[0] == invocations[1]
    assert captured[0] == captured[1]


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

    def fail_manifest_builder(*_args, **_kwargs) -> None:
        raise AssertionError("manifest replay must not run for unsafe metadata paths")

    monkeypatch.setattr(MODULE, "run_manifest_builder", fail_manifest_builder)

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

    def fail_manifest_builder(*_args, **_kwargs) -> None:
        raise AssertionError("manifest replay must not run for unsafe fixture names")

    monkeypatch.setattr(MODULE, "run_manifest_builder", fail_manifest_builder)

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

    def fail_manifest_builder(*_args, **_kwargs) -> None:
        raise AssertionError("manifest replay must not run for unsafe profile handles")

    monkeypatch.setattr(MODULE, "run_manifest_builder", fail_manifest_builder)

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

    def fail_manifest_builder(*_args, **_kwargs) -> None:
        raise AssertionError("manifest replay must not run for unsafe metadata files")

    monkeypatch.setattr(MODULE, "run_manifest_builder", fail_manifest_builder)

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


def test_android_replay_selects_the_exported_wire_id_example(tmp_path: Path) -> None:
    """Execute CI argument routing against the producer's canonical example identity."""
    wire_id = "iroha.instruction.v1::sorafs::RegisterPinManifest"
    exporter = (MODULE.REPO_ROOT / "tools/norito_codegen_exporter/src/main.rs").read_text()
    assert 'dir.join(format!("{}.json", spec.discriminant))' in exporter
    catalog = (MODULE.REPO_ROOT / "specs/sdk/android/generated/instructions.md").read_text()
    assert f"## `{wire_id}`" in catalog
    gate = (MODULE.REPO_ROOT / "ci/check_android_codegen.sh").read_text()
    start = gate.index('  CARGO_TARGET_DIR="${target_root}"', gate.index("run_replay() {"))
    stop = gate.index("\n\n  write_hash_tree", start)
    invocation = gate[start:stop]
    stage = (tmp_path / "external stage").resolve()
    codegen = stage / "codegen"
    examples = codegen / "instruction_examples"
    examples.mkdir(parents=True)
    canonical = examples / f"{wire_id}.json"
    canonical.write_text('{"existing":true}\n')
    captured = stage / "arguments"
    shell = r'''
set -euo pipefail
source_root="$SOURCE_ROOT"
target_root="$TARGET_ROOT"
codegen_root="$CODEGEN_ROOT"
generated_root="$GENERATED_ROOT"
python3() { printf '%s\0' "$@" > "$ARGUMENT_LOG"; }
'''
    result = subprocess.run(
        ["bash", "-c", shell + invocation],
        env={**os.environ, "SOURCE_ROOT": str(MODULE.REPO_ROOT),
             "TARGET_ROOT": str(stage / "target"), "CODEGEN_ROOT": str(codegen),
             "GENERATED_ROOT": str(stage / "generated"), "ARGUMENT_LOG": str(captured)},
        capture_output=True, text=True, check=False,
    )
    assert result.returncode == 0, result.stderr
    arguments = [os.fsdecode(value) for value in captured.read_bytes().split(b"\0")[:-1]]
    assert arguments[0] == str(MODULE_PATH.resolve())
    selected = Path(arguments[arguments.index("--register-pin-example") + 1])
    # This actual output update fails against the old Rust-type filename even
    # though the canonical exporter artifact is present in the replay stage.
    MODULE.update_register_pin_example(selected, {"route": "canonical"})
    assert selected == canonical
    assert MODULE.DEFAULT_REGISTER_PIN_EXAMPLE.name == canonical.name
    assert json.loads(canonical.read_bytes()) == {
        "existing": True, "fixture_example": {"route": "canonical"},
    }
    canonical.unlink()
    retired = examples / "iroha_data_model::isi::sorafs::RegisterPinManifest.json"
    retired.write_text('{"retired":true}\n')
    try:
        MODULE.update_register_pin_example(selected, {"route": "must refuse"})
    except ValueError as error:
        assert "must exist and be a file" in str(error)
    else:
        raise AssertionError("a retired Rust-type example was accepted as the canonical output")
    assert json.loads(retired.read_bytes()) == {"retired": True}
    assert not canonical.exists()


def test_canonical_example_name_preserves_parent_and_near_match_refusals(tmp_path: Path) -> None:
    """Only the exact public wire basename is exempt from URI-shaped name refusal."""
    name = "iroha.instruction.v1::sorafs::RegisterPinManifest.json"
    MODULE.validate_codegen_path(tmp_path / "instruction_examples" / name, "public example")
    refused = (
        tmp_path / name,
        tmp_path / "private_key" / "instruction_examples" / name,
        tmp_path / "https:remote" / "instruction_examples" / name,
        tmp_path / "%2e%2e" / "instruction_examples" / name,
        tmp_path / name / "instruction_examples" / name,
        tmp_path / "instruction_examples" / name.replace("v1::", "v2::"),
        tmp_path / "instruction_examples" / name.replace("::", "%3a%3a"),
        tmp_path / "instruction_examples" / (name + ".bak"),
        tmp_path / "instruction_examples" / (name + "\n"),
    )
    for path in refused:
        try:
            MODULE.validate_codegen_path(path, "public example")
        except ValueError as error:
            assert str(error) == MODULE.CODEGEN_PATH_DIAGNOSTIC
        else:
            raise AssertionError("noncanonical example path was admitted")


def test_canonical_example_name_still_refuses_symlink_ownership(tmp_path: Path) -> None:
    """The wire-name exception never bypasses checks of original filesystem objects."""
    actual = tmp_path / "actual.json"
    actual.write_text('{"retained":true}\n')
    examples = tmp_path / "instruction_examples"
    examples.mkdir()
    linked = examples / MODULE.REGISTER_PIN_EXAMPLE_FILENAME
    linked.symlink_to(actual)
    try:
        MODULE.update_register_pin_example(linked, {"forbidden": True})
    except ValueError as error:
        assert "must not be a symlink" in str(error)
    else:
        raise AssertionError("canonical-named symlink was accepted")
    assert json.loads(actual.read_bytes()) == {"retained": True}
    linked.unlink()
    real_parent = tmp_path / "real"
    real_parent.mkdir()
    linked_parent = tmp_path / "alternate" / "instruction_examples"
    linked_parent.parent.mkdir()
    linked_parent.symlink_to(real_parent, target_is_directory=True)
    try:
        MODULE.validate_codegen_path(linked_parent / MODULE.REGISTER_PIN_EXAMPLE_FILENAME, "example")
    except ValueError as error:
        assert "must not be a symlink" in str(error)
    else:
        raise AssertionError("canonical example under linked parent was accepted")
