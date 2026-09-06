"""Adversarial regressions for the final-V1 SCCP validator build boundary."""

from __future__ import annotations

import argparse
import ast
import copy
import hashlib
import inspect
import io
import json
import os
import re
import stat
import struct
import sys
import tarfile
import textwrap
from pathlib import Path, PurePosixPath
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, os.fspath(SCRIPTS))

import sccp_validator_builder as builder
import sccp_validator_builder_driver as driver

SOURCE_DATE_EPOCH = 1_700_000_000


def _executable_identity(path: Path) -> tuple[int, ...]:
    metadata = path.lstat()
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _container_policy(*, timeout_seconds: int = 1) -> dict[str, Any]:
    return {
        "builder": {
            "image": "registry.invalid/validator@sha256:" + "1" * 64,
            "driver_path": builder.DRIVER_MOUNT,
            "driver_sha256": hashlib.sha256(builder.DRIVER.read_bytes()).hexdigest(),
            "python_path": "/toolchain/bin/python3",
            "cargo_path": "/toolchain/bin/cargo",
            "rustc_path": "/toolchain/bin/rustc",
            "linker_path": "/toolchain/bin/cc",
            "cargo_home_path": "/toolchain/cargo-home",
        },
        "source": {
            "commit": "7" * 40,
            "source_date_epoch": 1_700_000_000,
        },
        "limits": {
            "max_inventory_files": 100,
            "max_file_bytes": 1024 * 1024,
            "max_total_bytes": 2 * 1024 * 1024,
            "max_log_bytes": 128,
            "timeout_seconds": timeout_seconds,
        },
    }


def _docker_daemon_info() -> dict[str, Any]:
    return {
        "ServerVersion": "27.5.1",
        "OperatingSystem": "Docker Engine - Community",
        "OSType": "linux",
        "Architecture": "x86_64",
        "KernelVersion": "6.8.0",
        "Driver": "overlay2",
        "CgroupDriver": "systemd",
        "CgroupVersion": "2",
        "DefaultRuntime": "runc",
        "Isolation": "",
        "ExperimentalBuild": False,
        "LiveRestoreEnabled": False,
        "SecurityOptions": [
            "name=apparmor",
            "name=seccomp,profile=builtin",
        ],
        "ServerErrors": [],
        "ContainerdCommit": {"ID": "containerd-id", "Expected": "containerd-id"},
        "RuncCommit": {"ID": "runc-id", "Expected": "runc-id"},
        "InitCommit": {"ID": "init-id", "Expected": "init-id"},
    }


def _docker_daemon_report(info: dict[str, Any]) -> dict[str, Any]:
    string_fields = (
        "ServerVersion",
        "OperatingSystem",
        "OSType",
        "Architecture",
        "KernelVersion",
        "Driver",
        "CgroupDriver",
        "CgroupVersion",
        "DefaultRuntime",
        "Isolation",
    )
    return {
        "schema": "iroha.sccp.validator-docker-daemon.final-v1",
        **{name: info[name] for name in string_fields},
        "ExperimentalBuild": info["ExperimentalBuild"],
        "LiveRestoreEnabled": info["LiveRestoreEnabled"],
        "SecurityOptions": sorted(info["SecurityOptions"]),
        "ServerErrors": [],
        "ContainerdCommit": info["ContainerdCommit"],
        "RuncCommit": info["RuncCommit"],
        "InitCommit": info["InitCommit"],
    }


def _docker_daemon_policy(info: dict[str, Any]) -> dict[str, Any]:
    report = _docker_daemon_report(info)
    digest = hashlib.sha256(
        builder.common.canonical_json_file_bytes(report)
    ).hexdigest()
    return {"builder": {"docker_daemon_report_sha256": digest}}


def _mock_docker_daemon_inspection(
    monkeypatch: pytest.MonkeyPatch,
    info: dict[str, Any],
) -> tuple[list[tuple[tuple[Any, ...], dict[str, Any]]], list[tuple[Any, ...]]]:
    run_calls: list[tuple[tuple[Any, ...], dict[str, Any]]] = []
    unchanged_calls: list[tuple[Any, ...]] = []

    def fake_run_bounded(*args: Any, **kwargs: Any) -> tuple[bytes, bytes]:
        run_calls.append((args, kwargs))
        return json.dumps(info, separators=(",", ":")).encode("utf-8"), b""

    def fake_require_unchanged(*args: Any, **kwargs: Any) -> None:
        unchanged_calls.append((*args, kwargs))

    monkeypatch.setattr(builder, "_run_bounded", fake_run_bounded)
    monkeypatch.setattr(builder, "_require_unchanged", fake_require_unchanged)
    return run_calls, unchanged_calls


class _BinaryStdout:
    def __init__(self, stream: Any) -> None:
        self.buffer = stream


def _driver_output_archive(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[Path, dict[str, bytes], int]:
    source_date_epoch = 1_700_000_000
    output = tmp_path / "driver-output"
    (output / "closure").mkdir(parents=True, mode=0o700)
    (output / "validator").mkdir(mode=0o700)
    payloads: dict[str, bytes] = {}
    for relative_name in builder._OUTPUT_ARCHIVE_FILES:
        payload = f"authenticated {relative_name}\n".encode("ascii")
        path = output / relative_name
        path.write_bytes(payload)
        path.chmod(0o700 if relative_name == f"validator/{builder.BINARY}" else 0o600)
        payloads[relative_name] = payload

    archive_path = tmp_path / "driver-output.tar"
    with archive_path.open("wb") as stream, monkeypatch.context() as patch:
        patch.setattr(driver.sys, "stdout", _BinaryStdout(stream))
        driver._emit_output_archive(
            output,
            source_date_epoch=source_date_epoch,
            maximum_file_bytes=max(map(len, payloads.values())),
            maximum_total_bytes=sum(map(len, payloads.values())),
        )
    return archive_path, payloads, source_date_epoch


def _source_archive_with_links(
    path: Path,
    links: tuple[tuple[str, str], ...],
) -> None:
    directories = {"source", "source/.cargo"}
    for name, _ in links:
        parts = PurePosixPath(name).parts
        for index in range(1, len(parts)):
            directories.add("/".join(parts[:index]))
    with tarfile.open(path, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for name in sorted(directories, key=lambda value: (value.count("/"), value)):
            member = tarfile.TarInfo(name)
            member.type = tarfile.DIRTYPE
            member.mode = 0o700
            member.mtime = SOURCE_DATE_EPOCH
            archive.addfile(member)
        files = {
            "source/Cargo.toml": b"# authenticated workspace closure\n",
            "source/Cargo.lock": b"# authenticated workspace closure\n",
            "source/.cargo/config.toml": (ROOT / ".cargo" / "config.toml").read_bytes(),
        }
        for name, payload in files.items():
            member = tarfile.TarInfo(name)
            member.type = tarfile.REGTYPE
            member.mode = 0o600
            member.size = len(payload)
            member.mtime = SOURCE_DATE_EPOCH
            archive.addfile(member, io.BytesIO(payload))
        for name, target in links:
            member = tarfile.TarInfo(name)
            member.type = tarfile.SYMTYPE
            member.mode = 0o700
            member.linkname = target
            member.mtime = SOURCE_DATE_EPOCH
            archive.addfile(member)


def _mutate_output_archive(
    canonical: Path,
    mutated: Path,
    *,
    mutation: str,
    source_date_epoch: int,
) -> None:
    if mutation == "trailing-data":
        mutated.write_bytes(canonical.read_bytes() + b"noncanonical trailing data")
        return

    records: list[tuple[tarfile.TarInfo, bytes | None]] = []
    with tarfile.open(canonical, mode="r:") as archive:
        for member in archive.getmembers():
            source = archive.extractfile(member) if member.isfile() else None
            records.append(
                (copy.copy(member), source.read() if source is not None else None)
            )

    archive_format = tarfile.PAX_FORMAT if mutation == "pax" else tarfile.USTAR_FORMAT
    with tarfile.open(mutated, mode="w", format=archive_format) as archive:
        for member, payload in records:
            if member.name == "output/builder-report.json":
                if mutation == "wrong-mode":
                    member.mode = 0o644
                elif mutation == "wrong-mtime":
                    member.mtime = source_date_epoch + 1
                elif mutation == "symlink":
                    member.type = tarfile.SYMTYPE
                    member.linkname = "../../../escaped-output"
                    member.size = 0
                    payload = None
                elif mutation == "pax":
                    member.pax_headers = {"comment": "noncanonical encoding"}
            archive.addfile(
                member, io.BytesIO(payload) if payload is not None else None
            )

        if mutation == "extra-member":
            payload = b"unexpected output\n"
            member = tarfile.TarInfo("output/unexpected")
            member.type = tarfile.REGTYPE
            member.size = len(payload)
            member.mode = 0o600
            member.mtime = source_date_epoch
            member.uid = member.gid = 0
            member.uname = member.gname = ""
            archive.addfile(member, io.BytesIO(payload))


def _assert_only_safe_partial_output(
    output: Path,
    payloads: dict[str, bytes],
) -> None:
    if not os.path.lexists(output):
        return
    root_metadata = output.lstat()
    assert stat.S_ISDIR(root_metadata.st_mode)
    assert not stat.S_ISLNK(root_metadata.st_mode)
    allowed_directories = {"closure", "validator"}
    allowed_files = set(payloads)
    for path in output.rglob("*"):
        relative_name = path.relative_to(output).as_posix()
        metadata = path.lstat()
        assert not stat.S_ISLNK(metadata.st_mode)
        if relative_name in allowed_directories:
            assert stat.S_ISDIR(metadata.st_mode)
            assert stat.S_IMODE(metadata.st_mode) == 0o700
        else:
            assert relative_name in allowed_files
            assert stat.S_ISREG(metadata.st_mode)
            expected_mode = (
                0o700 if relative_name == f"validator/{builder.BINARY}" else 0o600
            )
            assert stat.S_IMODE(metadata.st_mode) == expected_mode
            assert path.read_bytes() == payloads[relative_name]


def _fake_docker(tmp_path: Path, *, mode: str) -> tuple[Path, Path]:
    log = tmp_path / f"docker-{mode}.jsonl"
    executable = tmp_path / f"docker-{mode}"
    executable.write_text(
        textwrap.dedent(
            f"""\
            #!{sys.executable}
            import json
            import os
            import sys
            import time
            from pathlib import Path

            LOG = Path({os.fspath(log)!r})
            MODE = {mode!r}
            CID = "c" * 64
            arguments = sys.argv[1:]
            with LOG.open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(arguments, separators=(",", ":")) + "\\n")

            def option(name):
                for index, argument in enumerate(arguments):
                    if argument == name and index + 1 < len(arguments):
                        return arguments[index + 1]
                    if argument.startswith(name + "="):
                        return argument.split("=", 1)[1]
                return None

            if arguments and arguments[0] == "run":
                cidfile = option("--cidfile")
                if cidfile:
                    target = Path(cidfile)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text(CID + "\\n", encoding="ascii")
                if MODE == "timeout":
                    time.sleep(20)
                elif MODE == "overflow":
                    os.write(2, b"X" * 4096)
                raise SystemExit(0)
            if "rm" in arguments:
                raise SystemExit(0)
            if "inspect" in arguments:
                raise SystemExit(1)
            if "ps" in arguments:
                raise SystemExit(0)
            raise SystemExit(0)
            """
        ),
        encoding="utf-8",
    )
    executable.chmod(0o700)
    return executable, log


def _failed_container_commands(tmp_path: Path, *, mode: str) -> list[list[str]]:
    docker, log = _fake_docker(tmp_path, mode=mode)
    archive = tmp_path / "source.tar"
    archive.write_bytes(b"signed source archive")
    output = tmp_path / "output"
    with pytest.raises(builder.ValidatorBuilderError):
        builder._run_container_build(
            docker,
            _executable_identity(docker),
            _container_policy(),
            "2" * 64,
            archive,
            hashlib.sha256(archive.read_bytes()).hexdigest(),
            output,
        )
    return [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()]


def _run_command(commands: list[list[str]]) -> list[str]:
    matches = [command for command in commands if command and command[0] == "run"]
    assert len(matches) == 1
    return matches[0]


def test_driver_keeps_machine_stdout_separate_from_scanned_stderr(
    tmp_path: Path, capfd: pytest.CaptureFixture[str]
) -> None:
    stdout, stderr = driver._run(
        Path(sys.executable),
        (
            "-c",
            "import os; os.write(1, b'machine\\n'); os.write(2, b'diagnostic\\n')",
        ),
        cwd=tmp_path,
        environment={"LANG": "C", "LC_ALL": "C"},
        maximum_output_bytes=1024,
        label="split-stream regression",
    )

    captured = capfd.readouterr()
    assert stdout == b"machine\n"
    assert stderr == b"diagnostic\n"
    assert "diagnostic" in captured.err
    assert "machine" not in captured.err


def test_git_subprocess_environment_forbids_lazy_fetch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("GIT_NO_LAZY_FETCH", "0")
    environment = builder._closed_environment()

    assert environment["GIT_NO_LAZY_FETCH"] == "1"


def test_docker_daemon_inspection_accepts_digest_pinned_linux_amd64_report(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    info = _docker_daemon_info()
    policy = _docker_daemon_policy(info)
    run_calls, unchanged_calls = _mock_docker_daemon_inspection(monkeypatch, info)
    docker = Path("/pinned/docker")
    docker_identity = (1, 2, 3, 4, 5)
    environment = {"PATH": "/usr/bin:/bin"}

    builder._inspect_docker_daemon(
        docker,
        docker_identity,
        policy,
        environment,
    )

    assert len(run_calls) == 1
    positional, keywords = run_calls[0]
    assert positional == (docker, ("info", "--format={{json .}}"))
    assert keywords == {
        "cwd": builder.ROOT,
        "environment": environment,
        "maximum_bytes": 4 * 1024 * 1024,
        "timeout_seconds": 120,
        "label": "local validator Docker daemon inspection",
    }
    assert unchanged_calls == [
        (docker, docker_identity, {"label": "pinned Docker executable"})
    ]


def test_docker_daemon_inspection_rejects_policy_digest_mismatch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    info = _docker_daemon_info()
    policy = _docker_daemon_policy(info)
    policy["builder"]["docker_daemon_report_sha256"] = "0" * 64
    run_calls, unchanged_calls = _mock_docker_daemon_inspection(monkeypatch, info)

    with pytest.raises(
        builder.ValidatorBuilderError,
        match="differs from immutable policy",
    ):
        builder._inspect_docker_daemon(
            Path("/pinned/docker"),
            (1, 2, 3, 4, 5),
            policy,
            {"PATH": "/usr/bin:/bin"},
        )

    assert len(run_calls) == 1
    assert unchanged_calls == []


@pytest.mark.parametrize(
    ("case", "error"),
    (
        ("malformed-security-options", "malformed security state"),
        ("duplicate-security-option", "malformed security state"),
        ("runtime-commitment", "omits a runtime commitment"),
        ("daemon-error", "malformed security state"),
    ),
)
def test_docker_daemon_inspection_rejects_malformed_security_state(
    monkeypatch: pytest.MonkeyPatch,
    case: str,
    error: str,
) -> None:
    approved = _docker_daemon_info()
    policy = _docker_daemon_policy(approved)
    info = copy.deepcopy(approved)
    if case == "malformed-security-options":
        info["SecurityOptions"] = "name=apparmor"
    elif case == "duplicate-security-option":
        info["SecurityOptions"].append(info["SecurityOptions"][0])
    elif case == "runtime-commitment":
        info["RuncCommit"] = {"ID": "runc-id", "Expected": ""}
    elif case == "daemon-error":
        info["ServerErrors"] = ["daemon reported an initialization failure"]
    else:
        raise AssertionError(f"unhandled Docker daemon mutation: {case}")
    run_calls, unchanged_calls = _mock_docker_daemon_inspection(monkeypatch, info)

    with pytest.raises(builder.ValidatorBuilderError, match=error):
        builder._inspect_docker_daemon(
            Path("/pinned/docker"),
            (1, 2, 3, 4, 5),
            policy,
            {"PATH": "/usr/bin:/bin"},
        )

    assert len(run_calls) == 1
    assert unchanged_calls == []


def test_growing_file_fails_at_size_plus_one_without_draining_attacker_data(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    target = tmp_path / "growing"
    target.write_bytes(b"A")
    real_read = builder.os.read
    calls = 0

    def append_before_read(descriptor: int, maximum: int) -> bytes:
        nonlocal calls
        calls += 1
        if calls > 2:
            raise AssertionError("bounded reader continued past authenticated size + 1")
        with target.open("ab") as stream:
            stream.write(b"B")
            stream.flush()
        return real_read(descriptor, maximum)

    monkeypatch.setattr(builder.os, "read", append_before_read)
    with pytest.raises(builder.ValidatorBuilderError):
        builder._hash_direct_file(target, label="concurrently growing file", maximum=1)
    assert calls <= 2


def test_streaming_artifact_scan_detects_secret_split_across_read_boundary(
    tmp_path: Path,
) -> None:
    marker = b"-----BEGIN PRIVATE KEY-----"
    prefix = marker[:13]
    target = tmp_path / "split-secret"
    target.write_bytes(b"A" * (1024 * 1024 - len(prefix)) + prefix + marker[13:])

    with pytest.raises(
        builder.common.SccpReleaseError,
        match="forbidden credential material",
    ):
        builder._hash_direct_file(
            target,
            label="split secret artifact",
            maximum=target.stat().st_size,
            secret_scan=True,
        )


def test_signed_source_secret_exception_is_exact_path_and_git_object(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    payload = b'const VECTOR: &[u8] = b"-----BEGIN PRIVATE KEY-----";\n'
    source = tmp_path / "tests" / "rejection_vector.rs"
    source.parent.mkdir()
    source.write_bytes(payload)
    object_digest = hashlib.sha1(
        f"blob {len(payload)}\0".encode("ascii") + payload
    ).hexdigest()
    inventory = builder.common.canonical_json_file_bytes(
        {
            "schema": "iroha.sccp.git-source-tree-inventory.final-v1",
            "source_commit": "1" * 40,
            "object_format": "sha1",
            "entries": [
                {
                    "mode": "100644",
                    "object_type": "blob",
                    "object_id": object_digest,
                    "path": "tests/rejection_vector.rs",
                }
            ],
        }
    )
    policy = {
        "source": {"commit": "1" * 40, "secret_scan_exceptions": []},
        "limits": {
            "max_inventory_files": 8,
            "max_file_bytes": 1024,
            "max_total_bytes": 4096,
        },
    }
    monkeypatch.setattr(builder, "ROOT", tmp_path)
    with pytest.raises(
        builder.common.SccpReleaseError,
        match="forbidden credential material",
    ):
        builder._scan_signed_source_blobs(inventory, policy)

    policy["source"]["secret_scan_exceptions"] = [
        {"path": "tests/rejection_vector.rs", "object_id": object_digest}
    ]
    builder._scan_signed_source_blobs(inventory, policy)

    policy["source"]["secret_scan_exceptions"][0]["object_id"] = "2" * 40
    with pytest.raises(builder.ValidatorBuilderError, match="wrong Git object"):
        builder._scan_signed_source_blobs(inventory, policy)


@pytest.mark.parametrize("segment_type", (2, 3))
def test_validator_elf_rejects_dynamic_runtime_segments(
    tmp_path: Path,
    segment_type: int,
) -> None:
    identity = b"\x7fELF\x02\x01\x01\x00\x00" + b"\x00" * 7
    dynamic = struct.pack("<qQqQ", 1, 1, 0, 0)
    size = 64 + 2 * 56 + len(dynamic)
    header = struct.pack(
        "<16sHHIQQQIHHHHHH",
        identity,
        2,
        62,
        1,
        0,
        64,
        0,
        0,
        64,
        56,
        2,
        0,
        0,
        0,
    )
    load = struct.pack("<IIQQQQQQ", 1, 5, 0, 0, 0, size, size, 4096)
    forbidden = struct.pack(
        "<IIQQQQQQ",
        segment_type,
        4,
        64 + 2 * 56,
        0,
        0,
        len(dynamic),
        len(dynamic),
        8,
    )
    target = tmp_path / "dynamic-validator"
    target.write_bytes(header + load + forbidden + dynamic)
    target.chmod(0o700)

    with pytest.raises(builder.ValidatorBuilderError, match="ambient"):
        builder._hash_direct_file(
            target,
            label="dynamic validator",
            maximum=size,
            require_static_linux_amd64_elf=True,
        )


def test_growing_publication_source_never_copies_past_authenticated_size(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    source = tmp_path / "source"
    source.write_bytes(b"A")
    expected = hashlib.sha256(b"A").hexdigest()
    monkeypatch.setattr(
        builder,
        "_hash_direct_file",
        lambda *args, **kwargs: (expected, 1, False),
    )
    real_read = builder.os.read
    calls = 0

    def append_before_read(descriptor: int, maximum: int) -> bytes:
        nonlocal calls
        calls += 1
        if calls > 2:
            raise AssertionError("publication copy drained data beyond size + 1")
        with source.open("ab") as stream:
            stream.write(b"B")
            stream.flush()
        return real_read(descriptor, maximum)

    monkeypatch.setattr(builder.os, "read", append_before_read)
    destination_directory = tmp_path / "destination"
    destination_directory.mkdir(mode=0o700)
    parent = os.open(
        destination_directory,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0),
    )
    try:
        with pytest.raises(builder.ValidatorBuilderError):
            builder._copy_at(
                parent,
                "copy",
                source,
                expected_sha256=expected,
                maximum=1,
                executable=False,
            )
    finally:
        os.close(parent)
    assert calls <= 2
    copied = destination_directory / "copy"
    assert not copied.exists() or copied.stat().st_size <= 1


def test_candidate_publication_rejects_parent_name_substitution(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    build_output = tmp_path / "build-output"
    closure = build_output / "closure"
    validator = build_output / "validator"
    closure.mkdir(parents=True)
    validator.mkdir()
    closure_payloads: dict[str, bytes] = {}
    for name in builder.EXPECTED_CLOSURE_FILES:
        payload = f"authenticated {name}\n".encode("ascii")
        (closure / name).write_bytes(payload)
        closure_payloads[name] = payload
    executable = validator / builder.BINARY
    executable.write_bytes(b"ELF test executable")
    executable.chmod(0o700)
    archive = tmp_path / "source.tar"
    archive.write_bytes(b"authenticated source")
    report: dict[str, Any] = {
        "source_archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
        "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
    }
    closure_fields = {
        "build-environment.json": "build_environment_sha256",
        "build-recipe.json": "build_recipe_sha256",
        "cargo-config.toml": "cargo_config_sha256",
        "cargo-metadata-closure.json": "cargo_metadata_closure_sha256",
        "dependency-inventory.json": "dependency_inventory_sha256",
        "sbom.json": "sbom_sha256",
        "sysroot-inventory.json": "sysroot_inventory_sha256",
        "toolchain-inventory.json": "toolchain_inventory_sha256",
    }
    for name, field in closure_fields.items():
        if name in closure_payloads:
            report[field] = hashlib.sha256(closure_payloads[name]).hexdigest()

    publication_parent = tmp_path / "publication-parent"
    publication_parent.mkdir(mode=0o700)
    destination = publication_parent / "candidate"
    detached = publication_parent / "detached-authentic-candidate"
    original_write = builder._write_at
    substituted = False

    def substitute_then_write(
        parent: int, name: str, payload: bytes, *, executable: bool = False
    ) -> None:
        nonlocal substituted
        if not substituted:
            destination.rename(detached)
            destination.mkdir(mode=0o700)
            substituted = True
        original_write(parent, name, payload, executable=executable)

    monkeypatch.setattr(builder, "_write_at", substitute_then_write)
    with pytest.raises(builder.ValidatorBuilderError):
        builder._publish_candidate(
            destination,
            build_output=build_output,
            source_archive=archive,
            report=report,
            report_bytes=b"{}\n",
            unsigned={"test": "unsigned"},
            policy={"limits": {"max_file_bytes": 1024 * 1024}},
        )
    assert substituted


def test_container_uses_image_owned_driver_and_no_host_output_bind(
    tmp_path: Path,
) -> None:
    commands = _failed_container_commands(tmp_path, mode="overflow")
    command = _run_command(commands)
    rendered = "\0".join(command)

    assert os.fspath(builder.DRIVER) not in rendered
    assert f"dst={builder.DRIVER_MOUNT}" not in rendered
    assert "dst=/output" not in rendered
    assert builder.DRIVER_MOUNT in command
    assert os.fspath(tmp_path / "output") not in rendered
    assert "--log-driver=none" in command
    assert "--no-healthcheck" in command
    assert "--hostname=iroha-sccp-validator" in command
    assert "--user=65534:65534" in command
    assert any(
        item.startswith("--tmpfs=/work:")
        and "uid=65534" in item
        and "gid=65534" in item
        for item in command
    )
    assert any(item.startswith("--security-opt=seccomp=") for item in command)


def test_seccomp_profile_is_deny_by_default_and_narrows_process_creation() -> None:
    profile = builder._SECCOMP_PROFILE
    rules = profile["syscalls"]

    assert profile["defaultAction"] == "SCMP_ACT_ERRNO"
    assert profile["defaultErrnoRet"] == 1
    assert builder._SECCOMP_BASELINE == "moby/profiles:seccomp/v0.2.1"
    assert (
        builder._SECCOMP_BASELINE_SHA256
        == "536529b665dd0972c37bfb569f5d4ac8a53592e7b00752bc39ff063ca9864c74"
    )

    allowed = set(builder._SECCOMP_ALLOWED_SYSCALLS)
    assert len(allowed) == len(builder._SECCOMP_ALLOWED_SYSCALLS)
    assert tuple(sorted(allowed)) == builder._SECCOMP_ALLOWED_SYSCALLS
    assert not {
        "io_uring_setup",
        "io_uring_enter",
        "io_uring_register",
        "ipc",
        "socketcall",
        "bpf",
        "ptrace",
        "mount",
        "setns",
        "unshare",
    }.intersection(allowed)
    assert "sccp_unknown_future_syscall" not in allowed

    socket = [rule for rule in rules if rule.get("names") == ["socket"]]
    assert len(socket) == 1
    assert socket[0]["action"] == "SCMP_ACT_ALLOW"
    assert socket[0]["args"] == [
        {
            "index": 0,
            "value": 1,
            "valueTwo": 0,
            "op": "SCMP_CMP_EQ",
        }
    ]

    clone3 = [rule for rule in rules if rule.get("names") == ["clone3"]]
    assert len(clone3) == 1
    assert clone3[0]["action"] == "SCMP_ACT_ERRNO"
    assert clone3[0]["errnoRet"] == 38

    clone = [rule for rule in rules if rule.get("names") == ["clone"]]
    assert len(clone) == 1
    assert clone[0]["action"] == "SCMP_ACT_ALLOW"
    assert clone[0]["args"] == [
        {
            "index": 0,
            "value": builder._SECCOMP_CLONE_NAMESPACE_MASK,
            "valueTwo": 0,
            "op": "SCMP_CMP_MASKED_EQ",
        }
    ]
    assert builder._SECCOMP_CLONE_NAMESPACE_MASK == sum(
        builder._SECCOMP_CLONE_NAMESPACE_FLAGS
    )


def test_container_absence_probe_rejects_daemon_or_permission_failure(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        builder,
        "_run_bounded_status",
        lambda *args, **kwargs: (1, b"", b"permission denied"),
    )

    with pytest.raises(builder.ValidatorBuilderError, match="absence probe failed"):
        builder._docker_container_absent(
            Path("/pinned/docker"),
            "iroha-sccp-validator-test",
            {"PATH": "/usr/bin:/bin"},
        )


def test_container_cleanup_requires_successful_remove(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        builder,
        "_run_bounded_status",
        lambda *args, **kwargs: (1, b"", b"daemon unavailable"),
    )

    with pytest.raises(builder.ValidatorBuilderError, match="cleanup"):
        builder._remove_container_and_verify(
            Path("/pinned/docker"),
            "iroha-sccp-validator-test",
            {"PATH": "/usr/bin:/bin"},
        )


@pytest.mark.parametrize("mode", ("timeout", "overflow"))
def test_tracked_container_is_force_removed_and_absence_verified(
    tmp_path: Path, mode: str
) -> None:
    commands = _failed_container_commands(tmp_path, mode=mode)
    run_index = commands.index(_run_command(commands))
    run_command = commands[run_index]

    assert any(
        argument == "--name" or argument.startswith("--name=")
        for argument in run_command
    )
    assert any(
        argument == "--cidfile" or argument.startswith("--cidfile=")
        for argument in run_command
    )
    removals = [
        (index, command)
        for index, command in enumerate(commands)
        if "rm" in command and ("--force" in command or "-f" in command)
    ]
    absence_checks = [
        (index, command) for index, command in enumerate(commands) if "ls" in command
    ]
    assert len(removals) == 1
    assert absence_checks
    assert run_index < removals[0][0] < absence_checks[-1][0]


def test_host_extracts_exact_driver_produced_canonical_ustar(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    archive, payloads, source_date_epoch = _driver_output_archive(tmp_path, monkeypatch)
    assert archive.read_bytes()[257:263] == b"ustar\x00"
    output = tmp_path / "extracted-output"
    policy = _container_policy()
    policy["source"]["source_date_epoch"] = source_date_epoch

    builder._extract_output_archive(archive, output, policy=policy)

    _assert_only_safe_partial_output(output, payloads)
    assert {path.relative_to(output).as_posix() for path in output.rglob("*")} == {
        "closure",
        "validator",
        *payloads,
    }


def test_driver_accepts_internal_dangling_source_symlink(
    tmp_path: Path,
) -> None:
    archive = tmp_path / "source.tar"
    _source_archive_with_links(
        archive,
        (
            (
                "source/IrohaSwift/NoritoBridge.xcframework",
                "../dist/NoritoBridge.xcframework",
            ),
        ),
    )
    work = tmp_path / "work"
    work.mkdir()

    source = driver._extract_source_archive(
        archive,
        work,
        source_date_epoch=SOURCE_DATE_EPOCH,
        maximum_files=16,
        maximum_file_bytes=1024,
        maximum_total_bytes=4096,
    )

    link = source / "IrohaSwift" / "NoritoBridge.xcframework"
    assert link.is_symlink()
    assert os.readlink(link) == "../dist/NoritoBridge.xcframework"
    assert (
        driver._validate_source_cargo_configuration(source)
        == driver.APPROVED_SOURCE_CARGO_CONFIG_SHA256
    )


@pytest.mark.parametrize(
    "links",
    (
        (
            ("source/d/e/l", "../.."),
            ("source/a", "d/e/l/../../outside"),
        ),
        (
            ("source/a", "b"),
            ("source/b", "a"),
        ),
    ),
)
def test_driver_rejects_escaping_or_looping_source_symlink_graph(
    tmp_path: Path,
    links: tuple[tuple[str, str], ...],
) -> None:
    archive = tmp_path / "source.tar"
    _source_archive_with_links(archive, links)
    work = tmp_path / "work"
    work.mkdir()

    with pytest.raises(driver.DriverError, match="escapes or loops"):
        driver._extract_source_archive(
            archive,
            work,
            source_date_epoch=SOURCE_DATE_EPOCH,
            maximum_files=16,
            maximum_file_bytes=1024,
            maximum_total_bytes=4096,
        )


def test_driver_authenticates_repository_cargo_configuration(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source"
    cargo = source / ".cargo"
    cargo.mkdir(parents=True)
    approved = ROOT / ".cargo" / "config.toml"
    (cargo / "config.toml").write_bytes(approved.read_bytes())

    assert (
        driver._validate_source_cargo_configuration(source)
        == driver.APPROVED_SOURCE_CARGO_CONFIG_SHA256
        == builder.APPROVED_SOURCE_CARGO_CONFIG_SHA256
    )

    with (cargo / "config.toml").open("ab") as stream:
        stream.write(b"\n[net]\noffline = false\n")
    with pytest.raises(driver.DriverError, match="reviewed alias-only"):
        driver._validate_source_cargo_configuration(source)


def test_driver_extracts_only_retained_authenticated_source_snapshot(
    tmp_path: Path,
) -> None:
    mounted = tmp_path / "mounted-source.tar"
    _source_archive_with_links(mounted, ())
    authenticated = mounted.read_bytes()
    work = tmp_path / "work"
    work.mkdir()

    descriptor, size, identity = driver._snapshot_source_archive(
        mounted,
        work / "authenticated-source.tar",
        expected_sha256=hashlib.sha256(authenticated).hexdigest(),
        maximum=len(authenticated),
    )
    try:
        mounted.write_bytes(b"attacker-controlled replacement")
        source = driver._extract_source_archive(
            descriptor,
            work,
            source_date_epoch=SOURCE_DATE_EPOCH,
            maximum_files=16,
            maximum_file_bytes=1024,
            maximum_total_bytes=4096,
        )
        driver._verify_open_source_archive(
            descriptor,
            expected_sha256=hashlib.sha256(authenticated).hexdigest(),
            expected_size=size,
            identity=identity,
        )
    finally:
        os.close(descriptor)

    assert (
        source / "Cargo.toml"
    ).read_bytes() == b"# authenticated workspace closure\n"


@pytest.mark.parametrize(
    "record_type",
    (tarfile.XHDTYPE, tarfile.GNUTYPE_LONGNAME, tarfile.GNUTYPE_SPARSE),
)
def test_raw_ustar_prevalidation_rejects_truncated_extension_before_tarfile(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    record_type: bytes,
) -> None:
    archive = tmp_path / "hostile-output.tar"
    member = tarfile.TarInfo("hostile-extension")
    member.type = record_type
    member.mode = 0o600
    member.size = 1024**3
    archive.write_bytes(member.tobuf(format=tarfile.USTAR_FORMAT))
    output = tmp_path / "output"

    def forbidden_tarfile_open(*args: Any, **kwargs: Any) -> Any:
        del args, kwargs
        raise AssertionError("tarfile parsed an extension payload before rejection")

    monkeypatch.setattr(builder.tarfile, "open", forbidden_tarfile_open)
    with pytest.raises(
        builder.ValidatorBuilderError,
        match="forbidden tar record type",
    ):
        builder._extract_output_archive(
            archive,
            output,
            policy=_container_policy(),
        )
    assert not output.exists()


@pytest.mark.parametrize(
    "mutation",
    (
        "trailing-data",
        "wrong-mode",
        "wrong-mtime",
        "extra-member",
        "symlink",
        "pax",
    ),
)
def test_host_rejects_noncanonical_output_archives_without_unsafe_output(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation: str,
) -> None:
    canonical, payloads, source_date_epoch = _driver_output_archive(
        tmp_path, monkeypatch
    )
    mutated = tmp_path / "mutated-output.tar"
    _mutate_output_archive(
        canonical,
        mutated,
        mutation=mutation,
        source_date_epoch=source_date_epoch,
    )
    assert mutated.read_bytes() != canonical.read_bytes()
    output = tmp_path / "rejected-output"
    policy = _container_policy()
    policy["source"]["source_date_epoch"] = source_date_epoch

    with pytest.raises(builder.ValidatorBuilderError):
        builder._extract_output_archive(mutated, output, policy=policy)

    assert not (tmp_path.parent / "escaped-output").exists()
    assert not (tmp_path / "escaped-output").exists()
    _assert_only_safe_partial_output(output, payloads)


def test_exact_generated_cargo_config_is_a_closure_artifact(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    assert "cargo-config.toml" in builder.EXPECTED_CLOSURE_FILES
    real_path = Path
    work = tmp_path / "work"
    output = tmp_path / "output"
    source_archive = tmp_path / "source.tar"
    source = work / "source"
    sysroot = tmp_path / "sysroot"
    cargo_home = tmp_path / "bootstrap-cargo-home"
    tools = {
        "/toolchain/bin/python3": tmp_path / "python3",
        "/toolchain/bin/cargo": tmp_path / "cargo",
        "/toolchain/bin/rustc": tmp_path / "rustc",
        "/toolchain/bin/cc": tmp_path / "cc",
        "/toolchain/cargo-home": cargo_home,
    }
    logical_output = (
        "/work/output" if "/work/output" in inspect.getsource(driver.run) else "/output"
    )
    path_map = {
        "/input/source.tar": source_archive,
        "/work": work,
        logical_output: output,
        **tools,
    }

    def mapped_path(*parts: object) -> Path:
        if len(parts) == 1:
            key = os.fspath(parts[0])  # type: ignore[arg-type]
            if key in path_map:
                return path_map[key]
        return real_path(*parts)  # type: ignore[arg-type]

    monkeypatch.setattr(driver, "Path", mapped_path)
    monkeypatch.setattr(driver, "SAFE_PATH_RE", re.compile(r"^/[^\x00]*$"))
    work.mkdir()
    source.mkdir()
    (source / ".cargo").mkdir()
    (source / ".cargo" / "config.toml").write_bytes(
        (ROOT / ".cargo" / "config.toml").read_bytes()
    )
    sysroot.mkdir()
    cargo_home.mkdir()
    source_archive.write_bytes(b"source archive")
    for path in tools.values():
        if path == cargo_home:
            continue
        path.write_bytes(f"tool {path.name}".encode("ascii"))
        path.chmod(0o700)
    image_driver = tmp_path / "image-owned-driver.py"
    image_driver.write_bytes(b"#!/usr/bin/env python3\n")
    image_driver.chmod(0o700)
    monkeypatch.setattr(driver, "__file__", os.fspath(image_driver))
    monkeypatch.setattr(driver, "DRIVER_IMAGE_PATH", image_driver)

    monkeypatch.setattr(
        driver, "_extract_source_archive", lambda *args, **kwargs: source
    )
    monkeypatch.setattr(
        driver, "_validate_source_tree_inventory", lambda *args, **kwargs: None
    )

    def copy_cargo_home(source_path: Path, destination: Path, **kwargs: object) -> None:
        del source_path, kwargs
        destination.mkdir()

    monkeypatch.setattr(driver, "_copy_regular_tree", copy_cargo_home)
    monkeypatch.setattr(driver, "_version", lambda *args, **kwargs: "approved version")
    monkeypatch.setattr(
        driver,
        "_scan_tree",
        lambda root, *, prefix, **kwargs: [
            {
                "path": f"{prefix}/entry",
                "sha256": "3" * 64,
                "size_bytes": 1,
                "executable": False,
            }
        ],
    )
    monkeypatch.setattr(driver, "_build_sbom", lambda metadata: {"metadata": metadata})
    monkeypatch.setattr(driver, "_emit_output_archive", lambda *args, **kwargs: None)
    vendor_config = b'[source.crates-io]\nreplace-with = "vendored-sources"\n'

    def fake_run(
        executable: Path,
        arguments: tuple[str, ...],
        **kwargs: object,
    ) -> tuple[bytes, bytes]:
        del executable
        if arguments[:2] == ("--print", "sysroot"):
            return os.fsencode(sysroot) + b"\n", b""
        if arguments and arguments[0] == "vendor":
            (work / "vendor").mkdir()
            return vendor_config, b"vendor diagnostic\n"
        if arguments and arguments[0] == "metadata":
            return (
                (
                    b'{"packages":[],"resolve":{"nodes":[]},'
                    b'"target_directory":"${TARGET}",'
                    b'"workspace_root":"${SOURCE}"}'
                ),
                b"metadata diagnostic\n",
            )
        if arguments and arguments[0] == "build":
            built = (
                Path(kwargs["environment"]["CARGO_TARGET_DIR"])
                / driver.TARGET
                / driver.BUILD_PROFILE
                / driver.BINARY
            )
            built.parent.mkdir(parents=True, exist_ok=True)
            built.write_bytes(b"validator executable")
            built.chmod(0o700)
            return b"", b"build diagnostic\n"
        raise AssertionError(f"unexpected mocked tool invocation: {arguments!r}")

    monkeypatch.setattr(driver, "_run", fake_run)
    arguments = argparse.Namespace(
        source_archive="/input/source.tar",
        output_directory=logical_output,
        source_commit="4" * 40,
        source_archive_sha256=hashlib.sha256(source_archive.read_bytes()).hexdigest(),
        source_date_epoch=1_700_000_000,
        builder_image="registry.invalid/validator@sha256:" + "5" * 64,
        policy_sha256="6" * 64,
        driver_sha256=hashlib.sha256(image_driver.read_bytes()).hexdigest(),
        python_path="/toolchain/bin/python3",
        cargo_path="/toolchain/bin/cargo",
        rustc_path="/toolchain/bin/rustc",
        linker_path="/toolchain/bin/cc",
        cargo_home="/toolchain/cargo-home",
        max_inventory_files=100,
        max_file_bytes=4 * 1024 * 1024,
        max_total_bytes=8 * 1024 * 1024,
        max_log_bytes=1024 * 1024,
    )

    driver.run(arguments)

    assert (output / "closure" / "cargo-config.toml").read_bytes() == vendor_config


def test_all_driver_subprocesses_quiesce_and_only_build_installs_landlock() -> None:
    assert hasattr(driver, "_install_compile_sandbox")
    run_helper_source = inspect.getsource(driver._run)
    sandbox_source = inspect.getsource(driver._install_compile_sandbox)
    assert "_install_compile_sandbox" in run_helper_source
    assert "preexec_fn" in run_helper_source
    assert "_LANDLOCK_RESTRICT_SELF" in sandbox_source
    assert "_LANDLOCK_WRITE_RIGHTS" in sandbox_source

    tree = ast.parse(Path(driver.__file__).read_text(encoding="utf-8"))
    subprocess_calls = [
        node
        for node in ast.walk(tree)
        if isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == "subprocess"
    ]
    assert len(subprocess_calls) == 1
    assert subprocess_calls[0].func.attr == "Popen"
    run_function = next(
        node
        for node in tree.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
        and node.name == "_run"
    )
    assert run_function.end_lineno is not None
    assert run_function.lineno <= subprocess_calls[0].lineno <= run_function.end_lineno

    calls = [
        node
        for node in ast.walk(tree)
        if isinstance(node, ast.Call)
        and isinstance(node.func, ast.Name)
        and node.func.id == "_run"
    ]
    assert calls
    for call in calls:
        quiescence = [
            keyword
            for keyword in call.keywords
            if keyword.arg == "quiesce_process_namespace"
        ]
        assert len(quiescence) == 1
        assert isinstance(quiescence[0].value, ast.Constant)
        assert quiescence[0].value.value is True

    guarded = [
        (call, keyword)
        for call in calls
        for keyword in call.keywords
        if keyword.arg == "compile_sandbox_writable_roots"
    ]
    assert len(guarded) == 1
    call, keyword = guarded[0]
    assert len(call.args) >= 2
    assert isinstance(call.args[1], ast.Name)
    assert call.args[1].id == "build_arguments"
    assert isinstance(keyword.value, (ast.Tuple, ast.List))
    assert len(keyword.value.elts) == 1
    assert isinstance(keyword.value.elts[0], ast.Name)
    assert keyword.value.elts[0].id == "target"


@pytest.mark.parametrize("helper_name", ("_run_bounded", "_run_bounded_status"))
def test_host_probe_kills_residual_process_group(
    tmp_path: Path, helper_name: str
) -> None:
    helper = getattr(builder, helper_name)
    arguments = (
        "-c",
        (
            "import os, time; "
            "pid = os.fork(); "
            "time.sleep(30) if pid == 0 else print('parent-complete', flush=True)"
        ),
    )
    result = helper(
        Path(sys.executable),
        arguments,
        cwd=tmp_path,
        environment={"PATH": os.defpath},
        maximum_bytes=1024,
        timeout_seconds=5,
        label="residual process regression",
    )
    stdout = result[1] if helper_name == "_run_bounded_status" else result[0]
    assert stdout == b"parent-complete\n"


def test_every_host_subprocess_starts_in_a_private_process_group() -> None:
    tree = ast.parse(Path(builder.__file__).read_text(encoding="utf-8"))
    popen_calls = [
        node
        for node in ast.walk(tree)
        if isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == "subprocess"
        and node.func.attr == "Popen"
    ]
    assert popen_calls
    for call in popen_calls:
        session_keywords = [
            keyword for keyword in call.keywords if keyword.arg == "start_new_session"
        ]
        assert len(session_keywords) == 1
        assert isinstance(session_keywords[0].value, ast.Constant)
        assert session_keywords[0].value.value is True


def test_prepare_uses_a_fixed_shell_inert_temporary_root() -> None:
    source = inspect.getsource(builder.prepare_rebuild)
    assert builder.HOST_TEMP_ROOT == Path("/tmp")
    assert "dir=HOST_TEMP_ROOT" in source
    assert builder.SHELL_INERT_HOST_PATH_RE.fullmatch(
        "/tmp/iroha-sccp-validator-release-security-abc123/commit-verifier-tool/verifier"
    )
    assert not builder.SHELL_INERT_HOST_PATH_RE.fullmatch(
        "/tmp/iroha-sccp-$(touch injected)/verifier"
    )


def _metadata_with_closed_input_paths() -> dict[str, Any]:
    local_id = "path+file://${SOURCE}/crates/iroha_sccp#2.0.0"
    vendor_id = "registry+https://example.invalid/index#dependency@1.2.3"
    return {
        "workspace_root": "${SOURCE}",
        "target_directory": "${TARGET}",
        "packages": [
            {
                "id": local_id,
                "name": builder.CRATE,
                "version": "2.0.0",
                "source": None,
                "license": "Apache-2.0",
                "license_file": "${SOURCE}/LICENSE",
                "readme": "${SOURCE}/crates/iroha_sccp/README.md",
                "manifest_path": "${SOURCE}/crates/iroha_sccp/Cargo.toml",
                "targets": [
                    {
                        "src_path": (
                            "${SOURCE}/crates/iroha_sccp/src/bin/"
                            "sccp_release_evidence.rs"
                        )
                    }
                ],
                "dependencies": [{"path": "${SOURCE}/crates/iroha_data_model"}],
            },
            {
                "id": vendor_id,
                "name": "dependency",
                "version": "1.2.3",
                "source": "registry+https://example.invalid/index",
                "license": "MIT",
                "license_file": "${VENDOR}/dependency-1.2.3/LICENSE",
                "readme": None,
                "manifest_path": "${VENDOR}/dependency-1.2.3/Cargo.toml",
                "targets": [{"src_path": "${VENDOR}/dependency-1.2.3/src/lib.rs"}],
                "dependencies": [],
            },
        ],
        "resolve": {
            "nodes": [
                {"id": local_id, "dependencies": [vendor_id]},
                {"id": vendor_id, "dependencies": []},
            ]
        },
    }


@pytest.mark.parametrize(
    "mutation",
    (
        "workspace-root",
        "target-root",
        "local-manifest",
        "vendored-manifest",
        "target-source",
        "dependency-path",
        "readme",
    ),
)
def test_cargo_metadata_paths_are_closed_over_inventoried_inputs(
    mutation: str,
) -> None:
    valid = _metadata_with_closed_input_paths()
    builder._validate_metadata_input_boundaries(valid)
    driver._validate_metadata_input_boundaries(valid)
    assert builder._derive_sbom(valid)["root_package_id"].startswith("path+file://")

    candidate = json.loads(json.dumps(valid))
    if mutation == "workspace-root":
        candidate["workspace_root"] = "/untracked/source"
    elif mutation == "target-root":
        candidate["target_directory"] = "/untracked/target"
    elif mutation == "local-manifest":
        candidate["packages"][0]["manifest_path"] = "/untracked/Cargo.toml"
    elif mutation == "vendored-manifest":
        candidate["packages"][1]["manifest_path"] = "${SOURCE}/forged-vendor/Cargo.toml"
    elif mutation == "target-source":
        candidate["packages"][0]["targets"][0]["src_path"] = "/etc/passwd"
    elif mutation == "dependency-path":
        candidate["packages"][0]["dependencies"][0]["path"] = "${SOURCE}/../escape"
    elif mutation == "readme":
        candidate["packages"][0]["readme"] = "${VENDOR}/substituted.md"
    else:
        raise AssertionError(f"unhandled metadata mutation: {mutation}")

    with pytest.raises(builder.ValidatorBuilderError):
        builder._validate_metadata_input_boundaries(candidate)
    with pytest.raises(driver.DriverError):
        driver._validate_metadata_input_boundaries(candidate)
