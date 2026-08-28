#!/usr/bin/env python3
"""Reproducible, fail-closed builder for the first-release TON SCCP contracts.

Production builds are performed only by an externally approved, digest-addressed
Linux/amd64 image.  This orchestrator never downloads a toolchain and never owns
release signing keys.  It verifies two isolated builds and an independently
signed output lock before publishing release artifacts.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import threading
from pathlib import Path, PurePosixPath
from typing import Any, Mapping, Sequence

import sccp_release_common as common


ROOT = Path(__file__).resolve().parents[1]
PROJECT = ROOT / "contracts" / "ton" / "sccp"
POLICY_SCHEMA = "iroha.sccp.ton-builder-policy.final-v1"
REPORT_SCHEMA = "iroha.sccp.ton-builder-report.final-v1"
LOCK_SCHEMA = "iroha.sccp.ton-output-lock.final-v1"
RECEIPT_SCHEMA = "iroha.sccp.ton-build-receipt.final-v1"
LOCK_SIGNING_DOMAIN = b"iroha:sccp:ton-output-lock:final-v1\x00"
TREE_HASH_DOMAIN = b"iroha:sccp:ton-output-tree:final-v1\x00"
PLATFORM = "linux/amd64"
ACTON_VERSION = "acton 1.1.0 (9cf4d1f 2026-05-22)"
TOLK_VERSION = "1.4.1"
ACTON_ARCHIVE_SHA256 = (
    "c2e640eacbb5b6ece1c343cab2ab6d2db74643d0706777aad181ed7e6e1bfc16"
)
APPROVER_ROLES = ("release-engineering", "release-security")
HEX_32_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
IMAGE_RE = re.compile(r"^[a-z0-9][a-z0-9._:/-]{0,200}@sha256:[0-9a-f]{64}$")
ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9._:+-]{0,127})$")
SEGMENT_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
FINGERPRINT_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9:+/=_-]{15,199}$")
MAX_POLICY_BYTES = 2 * 1024 * 1024
MAX_LOCK_BYTES = 8 * 1024 * 1024
MAX_REPORT_BYTES = 8 * 1024 * 1024
MAX_HOST_EXECUTABLE_BYTES = 256 * 1024 * 1024
MAX_SOURCE_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024
REQUIRED_ARTIFACT_PATHS = frozenset(
    {
        "build/TairaXorJettonMaster.json",
        "build/TairaXorJettonWallet.json",
        "build/TairaXorSccpBridge.json",
        "build/abi/TairaXorJettonMaster.json",
        "build/abi/TairaXorJettonWallet.json",
        "build/abi/TairaXorSccpBridge.json",
        "gen/TairaXorJettonMaster.code.tolk",
        "gen/TairaXorJettonWallet.code.tolk",
        "wrappers/TairaXorJettonMaster.gen.tolk",
        "wrappers/TairaXorJettonWallet.gen.tolk",
        "wrappers/TairaXorSccpBridge.gen.tolk",
    }
)


class TonBuilderError(RuntimeError):
    """A bounded error safe to show in release logs."""


class _SafeArgumentParser(argparse.ArgumentParser):
    """Argument parser whose diagnostics never reflect caller-controlled text."""

    def error(self, message: str) -> None:
        del message
        _fail("command line has an invalid final-V1 shape")


def _fail(message: str) -> None:
    raise TonBuilderError(message)


def _object(value: Any, *, label: str, keys: Sequence[str]) -> dict[str, Any]:
    if type(value) is not dict or set(value) != set(keys) or len(value) != len(keys):
        _fail(f"{label} must contain the exact final-V1 fields")
    return value


def _list(value: Any, *, label: str, minimum: int, maximum: int) -> list[Any]:
    if type(value) is not list or not minimum <= len(value) <= maximum:
        _fail(f"{label} has an invalid final-V1 cardinality")
    return value


def _string(value: Any, *, label: str, maximum: int = 256) -> str:
    if type(value) is not str or not value or len(value.encode("utf-8")) > maximum:
        _fail(f"{label} must be a bounded nonempty string")
    if value != value.strip() or any(ord(character) < 0x20 for character in value):
        _fail(f"{label} must use canonical printable text")
    return value


def _hex32(value: Any, *, label: str, nonzero: bool = True) -> str:
    value = _string(value, label=label, maximum=64)
    if not HEX_32_RE.fullmatch(value) or (nonzero and value == "00" * 32):
        _fail(f"{label} must be one nonzero lowercase SHA-256 value")
    return value


def _positive_int(value: Any, *, label: str, maximum: int) -> int:
    if type(value) is not int or not 1 <= value <= maximum:
        _fail(f"{label} is outside its final-V1 bound")
    return value


def _valid_ed25519_public_key(encoded: bytes) -> bool:
    point = common._ed_decode(encoded)
    if point is None or point == common._ED_IDENTITY:
        return False
    return common._ed_extended_equal(
        common._ed_scalar_multiply_extended(common._ed_extended(point), common._ED_L),
        common._ED_EXTENDED_IDENTITY,
    )


def _relative_path(value: Any, *, label: str) -> str:
    value = _string(value, label=label, maximum=512)
    path = PurePosixPath(value)
    if path.is_absolute() or str(path) != value or not path.parts:
        _fail(f"{label} must be a canonical relative POSIX path")
    if any(part in ("", ".", "..") or not SEGMENT_RE.fullmatch(part) for part in path.parts):
        _fail(f"{label} contains an unsafe path component")
    common.reject_secret_material(value.encode("utf-8"), label=label)
    return value


def _absolute_container_path(value: Any, *, label: str) -> str:
    value = _string(value, label=label, maximum=256)
    path = PurePosixPath(value)
    if not path.is_absolute() or str(path) != value or len(path.parts) < 2:
        _fail(f"{label} must be one canonical absolute container path")
    if any(part in ("", ".", "..") or not SEGMENT_RE.fullmatch(part) for part in path.parts[1:]):
        _fail(f"{label} contains an unsafe path component")
    return value


def _inventory_entry(value: Any, *, label: str, with_role: bool) -> dict[str, Any]:
    keys = ("path", "role", "sha256", "size_bytes", "executable") if with_role else (
        "path",
        "sha256",
        "size_bytes",
        "executable",
    )
    entry = _object(value, label=label, keys=keys)
    path = _relative_path(entry["path"], label=f"{label}.path")
    role: str | None = None
    if with_role:
        role = _string(entry["role"], label=f"{label}.role", maximum=128)
        if not ID_RE.fullmatch(role):
            _fail(f"{label}.role is not a canonical identifier")
    digest = _hex32(entry["sha256"], label=f"{label}.sha256")
    size = _positive_int(entry["size_bytes"], label=f"{label}.size_bytes", maximum=2**31)
    executable = entry["executable"]
    if type(executable) is not bool:
        _fail(f"{label}.executable must be boolean")
    result: dict[str, Any] = {
        "path": path,
        "sha256": digest,
        "size_bytes": size,
        "executable": executable,
    }
    if role is not None:
        result = {
            "path": path,
            "role": role,
            "sha256": digest,
            "size_bytes": size,
            "executable": executable,
        }
    return result


def _ordered_inventory(
    value: Any,
    *,
    label: str,
    maximum: int,
    with_role: bool,
) -> list[dict[str, Any]]:
    entries = [
        _inventory_entry(entry, label=f"{label}[{index}]", with_role=with_role)
        for index, entry in enumerate(_list(value, label=label, minimum=1, maximum=maximum))
    ]
    paths = [entry["path"] for entry in entries]
    if paths != sorted(paths) or len(paths) != len(set(paths)):
        _fail(f"{label} paths must be unique and strictly sorted")
    return entries


def _load_canonical_json(path: Path, *, label: str, maximum: int) -> tuple[dict[str, Any], bytes]:
    data = common.read_direct_file(path, label=label, maximum=maximum)
    common.reject_secret_material(data, label=label)
    value = common.parse_json_bytes(data, label=label, maximum=maximum)
    common.require_canonical_json_file(data, value, label=label)
    if type(value) is not dict:
        _fail(f"{label} must be a JSON object")
    return value, data


def validate_policy(value: Any) -> dict[str, Any]:
    """Validate and normalize one externally approved builder policy."""

    policy = _object(
        value,
        label="TON builder policy",
        keys=("schema", "source", "builder", "limits", "approvers"),
    )
    if policy["schema"] != POLICY_SCHEMA:
        _fail("TON builder policy has the wrong final-V1 schema")
    source = _object(
        policy["source"],
        label="TON builder source policy",
        keys=("commit", "commit_signer_fingerprint", "source_date_epoch"),
    )
    commit = _string(source["commit"], label="source.commit", maximum=64)
    if not COMMIT_RE.fullmatch(commit):
        _fail("source.commit must be one full lowercase Git object id")
    fingerprint = _string(
        source["commit_signer_fingerprint"],
        label="source.commit_signer_fingerprint",
        maximum=200,
    )
    if not FINGERPRINT_RE.fullmatch(fingerprint):
        _fail("source.commit_signer_fingerprint is not canonical")
    source_date_epoch = _positive_int(
        source["source_date_epoch"],
        label="source.source_date_epoch",
        maximum=4_102_444_800,
    )

    builder = _object(
        policy["builder"],
        label="TON builder policy.builder",
        keys=(
            "image",
            "platform",
            "driver_path",
            "acton_archive_sha256",
            "acton_reported_version",
            "tolk_reported_version",
            "host_python_sha256",
            "host_git_sha256",
            "host_docker_sha256",
            "toolchain_inventory",
        ),
    )
    image = _string(builder["image"], label="builder.image", maximum=256)
    if not IMAGE_RE.fullmatch(image) or image.endswith("@sha256:" + "0" * 64):
        _fail("builder.image must be one nonzero digest-pinned OCI reference")
    if builder["platform"] != PLATFORM:
        _fail("builder.platform must be exactly linux/amd64")
    driver_path = _absolute_container_path(builder["driver_path"], label="builder.driver_path")
    if builder["acton_archive_sha256"] != ACTON_ARCHIVE_SHA256:
        _fail("builder.acton_archive_sha256 is not the reviewed Acton 1.1.0 archive")
    if builder["acton_reported_version"] != ACTON_VERSION:
        _fail("builder.acton_reported_version is not exact Acton 1.1.0")
    if builder["tolk_reported_version"] != TOLK_VERSION:
        _fail("builder.tolk_reported_version is not exact Tolk 1.4.1")
    python_hash = _hex32(
        builder["host_python_sha256"], label="builder.host_python_sha256"
    )
    git_hash = _hex32(builder["host_git_sha256"], label="builder.host_git_sha256")
    docker_hash = _hex32(builder["host_docker_sha256"], label="builder.host_docker_sha256")
    toolchain = _ordered_inventory(
        builder["toolchain_inventory"],
        label="builder.toolchain_inventory",
        maximum=512,
        with_role=True,
    )
    if any(not entry["path"].startswith("toolchain/") for entry in toolchain):
        _fail("builder.toolchain_inventory must be contained under toolchain/")
    roles = {entry["role"] for entry in toolchain}
    if "acton-executable" not in roles or "tolk-stdlib" not in roles or "builder-driver" not in roles:
        _fail("builder.toolchain_inventory omits a required Acton/Tolk builder role")

    limits = _object(
        policy["limits"],
        label="TON builder limits",
        keys=(
            "max_artifacts",
            "max_artifact_bytes",
            "max_total_bytes",
            "max_log_bytes",
            "timeout_seconds",
        ),
    )
    normalized_limits = {
        "max_artifacts": _positive_int(
            limits["max_artifacts"], label="limits.max_artifacts", maximum=512
        ),
        "max_artifact_bytes": _positive_int(
            limits["max_artifact_bytes"],
            label="limits.max_artifact_bytes",
            maximum=512 * 1024 * 1024,
        ),
        "max_total_bytes": _positive_int(
            limits["max_total_bytes"],
            label="limits.max_total_bytes",
            maximum=2 * 1024 * 1024 * 1024,
        ),
        "max_log_bytes": _positive_int(
            limits["max_log_bytes"], label="limits.max_log_bytes", maximum=8 * 1024 * 1024
        ),
        "timeout_seconds": _positive_int(
            limits["timeout_seconds"], label="limits.timeout_seconds", maximum=3_600
        ),
    }
    if normalized_limits["max_total_bytes"] < normalized_limits["max_artifact_bytes"]:
        _fail("limits.max_total_bytes must cover one maximum-size artifact")

    approvers = _list(policy["approvers"], label="TON builder approvers", minimum=2, maximum=2)
    normalized_approvers: list[dict[str, str]] = []
    keys: set[str] = set()
    for index, expected_role in enumerate(APPROVER_ROLES):
        approver = _object(
            approvers[index],
            label=f"approvers[{index}]",
            keys=("role", "signer_id", "public_key_hex"),
        )
        if approver["role"] != expected_role:
            _fail("TON builder approvers must use the exact ordered independent roles")
        signer_id = _string(approver["signer_id"], label=f"approvers[{index}].signer_id")
        if not ID_RE.fullmatch(signer_id):
            _fail(f"approvers[{index}].signer_id is not canonical")
        public_key = _hex32(
            approver["public_key_hex"],
            label=f"approvers[{index}].public_key_hex",
        )
        if not _valid_ed25519_public_key(bytes.fromhex(public_key)):
            _fail(f"approvers[{index}].public_key_hex is not a prime-subgroup Ed25519 key")
        if public_key in keys:
            _fail("TON builder approvers must use independent public keys")
        keys.add(public_key)
        normalized_approvers.append(
            {"role": expected_role, "signer_id": signer_id, "public_key_hex": public_key}
        )

    return {
        "schema": POLICY_SCHEMA,
        "source": {
            "commit": commit,
            "commit_signer_fingerprint": fingerprint,
            "source_date_epoch": source_date_epoch,
        },
        "builder": {
            "image": image,
            "platform": PLATFORM,
            "driver_path": driver_path,
            "acton_archive_sha256": ACTON_ARCHIVE_SHA256,
            "acton_reported_version": ACTON_VERSION,
            "tolk_reported_version": TOLK_VERSION,
            "host_python_sha256": python_hash,
            "host_git_sha256": git_hash,
            "host_docker_sha256": docker_hash,
            "toolchain_inventory": toolchain,
        },
        "limits": normalized_limits,
        "approvers": normalized_approvers,
    }


def _open_stable_executable(path_text: str, *, label: str) -> tuple[Path, tuple[int, ...], str]:
    path = Path(path_text)
    if not path.is_absolute():
        _fail(f"{label} must be an absolute path")
    try:
        before = path.lstat()
    except OSError:
        _fail(f"{label} is unavailable")
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > MAX_HOST_EXECUTABLE_BYTES
        or before.st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH) == 0
    ):
        _fail(f"{label} must be one direct bounded executable")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    digest = hashlib.sha256()
    try:
        descriptor = os.open(path, flags)
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns) != (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_ctime_ns,
        ):
            _fail(f"{label} changed while opening")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    except OSError:
        _fail(f"{label} could not be read safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    identity = (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    )
    return path, identity, digest.hexdigest()


def _require_unchanged_executable(path: Path, identity: tuple[int, ...], *, label: str) -> None:
    try:
        after = path.lstat()
    except OSError:
        _fail(f"{label} disappeared")
    if (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ) != identity:
        _fail(f"{label} changed during the build")


def _run_bounded(
    executable: Path,
    arguments: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    maximum_bytes: int,
    timeout_seconds: int,
    label: str,
) -> tuple[bytes, bytes]:
    try:
        process = subprocess.Popen(
            [os.fspath(executable), *arguments],
            cwd=cwd,
            env=dict(environment),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
        )
    except OSError:
        _fail(f"{label} could not start")
    assert process.stdout is not None and process.stderr is not None
    buffers = [bytearray(), bytearray()]
    overflow = [False, False]

    def read_pipe(pipe: Any, index: int) -> None:
        while True:
            chunk = pipe.read(64 * 1024)
            if not chunk:
                break
            if len(buffers[index]) + len(chunk) > maximum_bytes:
                overflow[index] = True
            elif not overflow[index]:
                buffers[index].extend(chunk)

    threads = [
        threading.Thread(target=read_pipe, args=(process.stdout, 0), daemon=True),
        threading.Thread(target=read_pipe, args=(process.stderr, 1), daemon=True),
    ]
    for thread in threads:
        thread.start()
    try:
        return_code = process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
        _fail(f"{label} exceeded its signed time limit")
    finally:
        for thread in threads:
            thread.join(timeout=5)
    if any(thread.is_alive() for thread in threads) or any(overflow):
        _fail(f"{label} exceeded its signed output limit")
    stdout = bytes(buffers[0])
    stderr = bytes(buffers[1])
    common.reject_secret_material(stdout, label=f"{label} stdout")
    common.reject_secret_material(stderr, label=f"{label} stderr")
    if return_code != 0:
        _fail(f"{label} failed")
    return stdout, stderr


def _closed_environment(*, source_date_epoch: int | None = None) -> dict[str, str]:
    environment = {"LANG": "C", "LC_ALL": "C", "TZ": "UTC", "PATH": os.defpath}
    if source_date_epoch is not None:
        environment["SOURCE_DATE_EPOCH"] = str(source_date_epoch)
    for name in ("SYSTEMROOT", "WINDIR"):
        if name in os.environ:
            environment[name] = os.environ[name]
    return environment


def _git_command(
    git: Path,
    root: Path,
    arguments: Sequence[str],
    *,
    maximum: int = 1024 * 1024,
) -> bytes:
    stdout, _ = _run_bounded(
        git,
        ("-C", os.fspath(root), *arguments),
        cwd=root,
        environment=_closed_environment(),
        maximum_bytes=maximum,
        timeout_seconds=120,
        label="pinned Git operation",
    )
    return stdout


def _verify_source_and_archive(
    git: Path,
    policy: Mapping[str, Any],
    archive_path: Path,
) -> str:
    source = policy["source"]
    commit = source["commit"]
    top = _git_command(git, ROOT, ("rev-parse", "--show-toplevel")).decode("utf-8", "strict").strip()
    if Path(top).resolve() != ROOT.resolve():
        _fail("source root is not the exact Git top-level")
    head = _git_command(git, ROOT, ("rev-parse", "--verify", "HEAD")).decode().strip()
    if head != commit:
        _fail("source HEAD does not match the approved full commit")
    status = _git_command(
        git,
        ROOT,
        ("status", "--porcelain=v1", "--untracked-files=all"),
        maximum=16 * 1024 * 1024,
    )
    if status:
        _fail("production TON builds require a completely clean tracked and untracked tree")
    _git_command(git, ROOT, ("verify-commit", "--raw", commit), maximum=256 * 1024)
    signature = _git_command(
        git,
        ROOT,
        ("show", "--no-patch", "--format=%G?%x00%GF%x00%GP%x00", commit),
        maximum=8 * 1024,
    ).rstrip(b"\n")
    fields = signature.split(b"\x00")
    if len(fields) != 4 or fields[0] != b"G" or fields[3] != b"":
        _fail("source commit signature is not fully valid")
    try:
        fingerprints = {fields[1].decode("ascii"), fields[2].decode("ascii")}
    except UnicodeDecodeError:
        _fail("source commit signature fingerprint is malformed")
    if source["commit_signer_fingerprint"] not in fingerprints:
        _fail("source commit signer does not match the approved fingerprint")
    timestamp = _git_command(git, ROOT, ("show", "-s", "--format=%ct", commit)).decode().strip()
    if timestamp != str(source["source_date_epoch"]):
        _fail("source commit time does not match the approved SOURCE_DATE_EPOCH")

    try:
        descriptor = os.open(
            archive_path,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
    except OSError:
        _fail("source archive could not be reserved safely")
    digest = hashlib.sha256()
    total = [0]
    archive_overflow = [False]
    diagnostic_overflow = [False]
    diagnostics = bytearray()
    stream_errors: list[BaseException] = []
    try:
        process = subprocess.Popen(
            [
                os.fspath(git),
                "-C",
                os.fspath(ROOT),
                "archive",
                "--format=tar",
                "--prefix=source/",
                commit,
            ],
            cwd=ROOT,
            env=_closed_environment(source_date_epoch=source["source_date_epoch"]),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
        )
        assert process.stdout is not None and process.stderr is not None

        def stream_archive() -> None:
            try:
                while True:
                    chunk = process.stdout.read(1024 * 1024)
                    if not chunk:
                        break
                    total[0] += len(chunk)
                    if total[0] > MAX_SOURCE_ARCHIVE_BYTES:
                        archive_overflow[0] = True
                        continue
                    digest.update(chunk)
                    view = memoryview(chunk)
                    while view:
                        written = os.write(descriptor, view)
                        if written <= 0:
                            raise OSError("archive write made no progress")
                        view = view[written:]
            except BaseException as error:  # Propagated on the coordinating thread.
                stream_errors.append(error)

        def stream_diagnostics() -> None:
            try:
                while True:
                    chunk = process.stderr.read(64 * 1024)
                    if not chunk:
                        break
                    if len(diagnostics) + len(chunk) > 256 * 1024:
                        diagnostic_overflow[0] = True
                    elif not diagnostic_overflow[0]:
                        diagnostics.extend(chunk)
            except BaseException as error:  # Propagated on the coordinating thread.
                stream_errors.append(error)

        threads = [
            threading.Thread(target=stream_archive, daemon=True),
            threading.Thread(target=stream_diagnostics, daemon=True),
        ]
        for thread in threads:
            thread.start()
        try:
            return_code = process.wait(timeout=120)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            _fail("Git source archive operation exceeded its time limit")
        finally:
            for thread in threads:
                thread.join(timeout=5)
        if any(thread.is_alive() for thread in threads) or stream_errors:
            _fail("Git source archive streaming failed safely")
        common.reject_secret_material(bytes(diagnostics), label="Git archive diagnostics")
        if return_code != 0 or archive_overflow[0] or diagnostic_overflow[0] or total[0] == 0:
            _fail("Git could not produce the complete tracked source archive")
        os.fsync(descriptor)
        if os.fstat(descriptor).st_size != total[0]:
            _fail("tracked source archive changed during publication")
    except (OSError, subprocess.TimeoutExpired):
        _fail("Git source archive operation failed safely")
    finally:
        os.close(descriptor)
    if _git_command(git, ROOT, ("rev-parse", "--verify", "HEAD")).decode().strip() != commit:
        _fail("source HEAD changed during archival")
    if _git_command(git, ROOT, ("status", "--porcelain=v1", "--untracked-files=all")):
        _fail("source tree changed during archival")
    return digest.hexdigest()


def _scan_tree(
    root: Path,
    *,
    label: str,
    maximum_files: int,
    maximum_file_bytes: int,
    maximum_total_bytes: int,
    scan_text: bool,
) -> list[dict[str, Any]]:
    try:
        root_metadata = root.lstat()
    except OSError:
        _fail(f"{label} is missing")
    if stat.S_ISLNK(root_metadata.st_mode) or not stat.S_ISDIR(root_metadata.st_mode):
        _fail(f"{label} must be one direct directory")
    entries: list[dict[str, Any]] = []
    total = 0
    stack = [root]
    while stack:
        directory = stack.pop()
        try:
            children = sorted(os.scandir(directory), key=lambda item: item.name)
        except OSError:
            _fail(f"{label} could not be enumerated safely")
        for child in children:
            if not SEGMENT_RE.fullmatch(child.name):
                _fail(f"{label} contains a noncanonical path")
            try:
                metadata = child.stat(follow_symlinks=False)
            except OSError:
                _fail(f"{label} changed during enumeration")
            path = Path(child.path)
            if stat.S_ISLNK(metadata.st_mode):
                _fail(f"{label} must not contain symlinks")
            if stat.S_ISDIR(metadata.st_mode):
                if metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
                    _fail(f"{label} contains a writable shared directory")
                stack.append(path)
                continue
            if (
                not stat.S_ISREG(metadata.st_mode)
                or metadata.st_nlink != 1
                or metadata.st_size <= 0
                or metadata.st_size > maximum_file_bytes
                or metadata.st_mode & (stat.S_ISUID | stat.S_ISGID | stat.S_ISVTX)
            ):
                _fail(f"{label} contains an unsafe or oversized file")
            relative = path.relative_to(root).as_posix()
            _relative_path(relative, label=f"{label} path")
            digest = hashlib.sha256()
            captured = bytearray()
            flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
            try:
                descriptor = os.open(path, flags)
                opened = os.fstat(descriptor)
                if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns) != (
                    metadata.st_dev,
                    metadata.st_ino,
                    metadata.st_size,
                    metadata.st_ctime_ns,
                ):
                    _fail(f"{label} changed while opening")
                while True:
                    chunk = os.read(descriptor, 1024 * 1024)
                    if not chunk:
                        break
                    digest.update(chunk)
                    if scan_text and metadata.st_size <= common.MAX_ARTIFACT_BYTES:
                        captured.extend(chunk)
                after = os.fstat(descriptor)
            except OSError:
                _fail(f"{label} file could not be read safely")
            finally:
                if "descriptor" in locals():
                    os.close(descriptor)
                    del descriptor
            if (after.st_dev, after.st_ino, after.st_size, after.st_ctime_ns) != (
                opened.st_dev,
                opened.st_ino,
                opened.st_size,
                opened.st_ctime_ns,
            ):
                _fail(f"{label} changed while hashing")
            if captured:
                common.reject_secret_material(bytes(captured), label=label)
            total += metadata.st_size
            if total > maximum_total_bytes:
                _fail(f"{label} exceeds its signed aggregate size")
            entries.append(
                {
                    "path": relative,
                    "sha256": digest.hexdigest(),
                    "size_bytes": metadata.st_size,
                    "executable": bool(metadata.st_mode & stat.S_IXUSR),
                }
            )
            if len(entries) > maximum_files:
                _fail(f"{label} exceeds its signed file count")
    entries.sort(key=lambda entry: entry["path"])
    return entries


def _validate_report(
    output: Path,
    *,
    policy: Mapping[str, Any],
    source_closure_sha256: str,
) -> dict[str, Any]:
    try:
        names = sorted(entry.name for entry in os.scandir(output))
    except OSError:
        _fail("TON builder output could not be enumerated")
    if names != ["artifacts", "builder-report.json", "toolchain"]:
        _fail("TON builder output has an unexpected top-level shape")
    report, _ = _load_canonical_json(
        output / "builder-report.json", label="TON builder report", maximum=MAX_REPORT_BYTES
    )
    report = _object(
        report,
        label="TON builder report",
        keys=(
            "schema",
            "source_commit",
            "source_closure_sha256",
            "platform",
            "builder_image",
            "acton_reported_version",
            "tolk_reported_version",
            "toolchain_inventory",
            "artifacts",
        ),
    )
    builder = policy["builder"]
    if (
        report["schema"] != REPORT_SCHEMA
        or report["source_commit"] != policy["source"]["commit"]
        or report["source_closure_sha256"] != source_closure_sha256
        or report["platform"] != PLATFORM
        or report["builder_image"] != builder["image"]
        or report["acton_reported_version"] != ACTON_VERSION
        or report["tolk_reported_version"] != TOLK_VERSION
    ):
        _fail("TON builder report does not bind the approved source and toolchain")
    limits = policy["limits"]
    actual_artifacts = _scan_tree(
        output / "artifacts",
        label="TON contract artifacts",
        maximum_files=limits["max_artifacts"],
        maximum_file_bytes=limits["max_artifact_bytes"],
        maximum_total_bytes=limits["max_total_bytes"],
        scan_text=True,
    )
    if not REQUIRED_ARTIFACT_PATHS.issubset({entry["path"] for entry in actual_artifacts}):
        _fail("TON builder omitted a mandatory contract artifact")
    actual_toolchain = _scan_tree(
        output / "toolchain",
        label="TON toolchain inventory",
        maximum_files=512,
        maximum_file_bytes=MAX_HOST_EXECUTABLE_BYTES,
        maximum_total_bytes=limits["max_total_bytes"],
        scan_text=False,
    )
    actual_toolchain = [{**entry, "path": f"toolchain/{entry['path']}"} for entry in actual_toolchain]
    policy_by_path = {entry["path"]: entry for entry in builder["toolchain_inventory"]}
    with_roles: list[dict[str, Any]] = []
    for entry in actual_toolchain:
        expected = policy_by_path.get(entry["path"])
        if expected is None:
            _fail("TON builder emitted an unapproved toolchain file")
        with_roles.append(
            {
                "path": entry["path"],
                "role": expected["role"],
                "sha256": entry["sha256"],
                "size_bytes": entry["size_bytes"],
                "executable": entry["executable"],
            }
        )
    if with_roles != builder["toolchain_inventory"]:
        _fail("TON builder toolchain bytes differ from the approved exact inventory")
    reported_toolchain = _ordered_inventory(
        report["toolchain_inventory"],
        label="builder report toolchain_inventory",
        maximum=512,
        with_role=True,
    )
    reported_artifacts = _ordered_inventory(
        report["artifacts"],
        label="builder report artifacts",
        maximum=limits["max_artifacts"],
        with_role=False,
    )
    if reported_toolchain != with_roles or reported_artifacts != actual_artifacts:
        _fail("TON builder report inventory does not match its output inodes")
    return {
        "schema": REPORT_SCHEMA,
        "source_commit": policy["source"]["commit"],
        "source_closure_sha256": source_closure_sha256,
        "platform": PLATFORM,
        "builder_image": builder["image"],
        "acton_reported_version": ACTON_VERSION,
        "tolk_reported_version": TOLK_VERSION,
        "toolchain_inventory": with_roles,
        "artifacts": actual_artifacts,
    }


def _inspect_image(
    docker: Path,
    policy: Mapping[str, Any],
    *,
    identity: tuple[int, ...],
) -> None:
    stdout, _ = _run_bounded(
        docker,
        ("image", "inspect", policy["builder"]["image"]),
        cwd=ROOT,
        environment=_closed_environment(),
        maximum_bytes=2 * 1024 * 1024,
        timeout_seconds=120,
        label="pinned Docker image inspection",
    )
    try:
        value = json.loads(stdout.decode("utf-8", "strict"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        _fail("pinned Docker image inspection returned malformed JSON")
    if type(value) is not list or len(value) != 1 or type(value[0]) is not dict:
        _fail("pinned Docker image inspection returned the wrong shape")
    image = value[0]
    if image.get("Os") != "linux" or image.get("Architecture") != "amd64":
        _fail("pinned TON builder image is not linux/amd64")
    if type(image.get("Id")) is not str or not re.fullmatch(r"sha256:[0-9a-f]{64}", image["Id"]):
        _fail("pinned TON builder image lacks a content-addressed local identity")
    expected_digest = policy["builder"]["image"].rsplit("@", 1)[1]
    repo_digests = image.get("RepoDigests")
    if (
        type(repo_digests) is not list
        or not repo_digests
        or any(type(digest) is not str for digest in repo_digests)
        or not any(digest.endswith("@" + expected_digest) for digest in repo_digests)
    ):
        _fail("local TON builder image does not expose the approved manifest digest")
    _require_unchanged_executable(docker, identity, label="pinned Docker executable")


def _run_container_build(
    docker: Path,
    docker_identity: tuple[int, ...],
    policy: Mapping[str, Any],
    source_archive: Path,
    source_closure_sha256: str,
    output: Path,
) -> dict[str, Any]:
    output.mkdir(mode=0o700)
    os.chmod(output, 0o700)
    builder = policy["builder"]
    limits = policy["limits"]
    def mount_source_path(path: Path, *, label: str) -> str:
        value = os.fspath(path.absolute())
        if "," in value or any(ord(character) < 0x20 for character in value):
            _fail(f"{label} cannot be represented as one Docker bind source")
        return value

    mount_source = (
        "type=bind,src="
        + mount_source_path(source_archive, label="source archive path")
        + ",dst=/input/source.tar,readonly"
    )
    mount_output = (
        "type=bind,src="
        + mount_source_path(output, label="builder output path")
        + ",dst=/output"
    )
    arguments = (
        "run",
        "--rm",
        "--pull=never",
        "--platform=linux/amd64",
        "--network=none",
        "--read-only",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
        "--pids-limit=256",
        f"--user={os.geteuid()}:{os.getegid()}",
        "--tmpfs=/work:rw,nosuid,nodev,size=2147483648",
        "--mount",
        mount_source,
        "--mount",
        mount_output,
        "--env=LANG=C",
        "--env=LC_ALL=C",
        "--env=TZ=UTC",
        f"--env=SOURCE_DATE_EPOCH={policy['source']['source_date_epoch']}",
        "--entrypoint",
        builder["driver_path"],
        builder["image"],
        "--source-archive=/input/source.tar",
        "--output-directory=/output",
        f"--source-commit={policy['source']['commit']}",
        f"--source-closure-sha256={source_closure_sha256}",
        f"--builder-image={builder['image']}",
    )
    _run_bounded(
        docker,
        arguments,
        cwd=ROOT,
        environment=_closed_environment(source_date_epoch=policy["source"]["source_date_epoch"]),
        maximum_bytes=limits["max_log_bytes"],
        timeout_seconds=limits["timeout_seconds"],
        label="network-disabled TON contract build",
    )
    _require_unchanged_executable(docker, docker_identity, label="pinned Docker executable")
    return _validate_report(
        output,
        policy=policy,
        source_closure_sha256=source_closure_sha256,
    )


def _tree_hash(report: Mapping[str, Any]) -> str:
    payload = {
        "artifacts": report["artifacts"],
        "toolchain_inventory": report["toolchain_inventory"],
    }
    return hashlib.sha256(TREE_HASH_DOMAIN + common.canonical_json_bytes(payload)).hexdigest()


def _unsigned_lock(
    policy_sha256: str,
    source_closure_sha256: str,
    policy: Mapping[str, Any],
    report: Mapping[str, Any],
) -> dict[str, Any]:
    return {
        "schema": LOCK_SCHEMA,
        "builder_policy_sha256": policy_sha256,
        "source_closure_sha256": source_closure_sha256,
        "source_commit": policy["source"]["commit"],
        "artifact_tree_sha256": _tree_hash(report),
        "artifacts": report["artifacts"],
        "toolchain_inventory": report["toolchain_inventory"],
    }


def output_lock_signing_payload(unsigned_lock: Mapping[str, Any]) -> bytes:
    """Return the exact payload the two offline release roles sign."""

    return LOCK_SIGNING_DOMAIN + common.canonical_json_bytes(unsigned_lock)


def validate_signed_lock(
    value: Any,
    *,
    expected_unsigned: Mapping[str, Any],
    policy: Mapping[str, Any],
) -> dict[str, Any]:
    """Authenticate the exact two-role lock for the reproduced output."""

    lock = _object(
        value,
        label="TON signed output lock",
        keys=(*expected_unsigned.keys(), "provenance"),
    )
    unsigned = {key: lock[key] for key in expected_unsigned}
    if unsigned != expected_unsigned:
        _fail("TON signed output lock does not match the reproduced output")
    payload = output_lock_signing_payload(unsigned)
    provenance = _list(lock["provenance"], label="TON output-lock provenance", minimum=2, maximum=2)
    signatures: set[bytes] = set()
    for index, expected_role in enumerate(APPROVER_ROLES):
        entry = _object(
            provenance[index],
            label=f"provenance[{index}]",
            keys=("role", "signer_id", "algorithm", "public_key_hex", "signature_b64"),
        )
        approver = policy["approvers"][index]
        if (
            entry["role"] != expected_role
            or entry["signer_id"] != approver["signer_id"]
            or entry["algorithm"] != "ed25519"
            or entry["public_key_hex"] != approver["public_key_hex"]
        ):
            _fail("TON output-lock provenance does not match the approved independent roles")
        try:
            signature = base64.b64decode(entry["signature_b64"], validate=True)
        except (binascii.Error, ValueError, TypeError):
            _fail("TON output-lock signature is not canonical base64")
        if len(signature) != 64 or base64.b64encode(signature).decode("ascii") != entry["signature_b64"]:
            _fail("TON output-lock signature has the wrong canonical encoding")
        if signature in signatures:
            _fail("TON output-lock signatures must be independent")
        signatures.add(signature)
        if not common.verify_ed25519(bytes.fromhex(entry["public_key_hex"]), signature, payload):
            _fail("TON output-lock signature is invalid")
    return {**unsigned, "provenance": provenance}


def _ensure_outside_repository(path: Path, *, label: str) -> None:
    try:
        absolute = path.parent.resolve(strict=True) / path.name
    except OSError:
        _fail(f"{label} parent is unavailable")
    try:
        absolute.relative_to(ROOT.resolve())
    except ValueError:
        return
    _fail(f"{label} must be outside the repository source tree")


def _new_private_output(path: Path, *, label: str) -> int:
    _ensure_outside_repository(path, label=label)
    if not SEGMENT_RE.fullmatch(path.name):
        _fail(f"{label} name is not canonical")
    try:
        absolute_parent = path.parent.absolute()
        components = absolute_parent.parts[1:]
        parent_descriptor = os.open(
            os.sep,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        for component in components:
            child = os.open(
                component,
                os.O_RDONLY
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0),
                dir_fd=parent_descriptor,
            )
            os.close(parent_descriptor)
            parent_descriptor = child
        parent_metadata = os.fstat(parent_descriptor)
    except OSError:
        if "parent_descriptor" in locals():
            os.close(parent_descriptor)
        _fail(f"{label} parent must be a direct directory tree")
    if (
        not stat.S_ISDIR(parent_metadata.st_mode)
        or parent_metadata.st_uid != os.geteuid()
        or parent_metadata.st_mode & 0o022
    ):
        os.close(parent_descriptor)
        _fail(f"{label} parent must be owner-controlled")
    try:
        os.mkdir(path.name, mode=0o700, dir_fd=parent_descriptor)
        descriptor = os.open(
            path.name,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent_descriptor,
        )
        os.fchmod(descriptor, 0o700)
        os.fsync(parent_descriptor)
    except FileExistsError:
        os.close(parent_descriptor)
        _fail(f"{label} already exists; publication never overwrites")
    except OSError:
        os.close(parent_descriptor)
        _fail(f"{label} could not be created safely")
    os.close(parent_descriptor)
    metadata = os.fstat(descriptor)
    if not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != os.geteuid() or metadata.st_mode & 0o077:
        os.close(descriptor)
        _fail(f"{label} is not an owner-only direct directory")
    return descriptor


def _write_file_at(directory: int, name: str, payload: bytes, *, executable: bool = False) -> None:
    if not SEGMENT_RE.fullmatch(name) or not payload:
        _fail("TON publication contains an invalid file")
    flags = (
        os.O_RDWR
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    mode = 0o700 if executable else 0o600
    try:
        descriptor = os.open(name, flags, mode, dir_fd=directory)
    except OSError:
        _fail("TON publication could not create an output file safely")
    try:
        os.fchmod(descriptor, mode)
        opened = os.fstat(descriptor)
        identity = (opened.st_dev, opened.st_ino)
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail("TON publication write made no progress")
            view = view[written:]
        os.fsync(descriptor)
        metadata = os.fstat(descriptor)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or metadata.st_size != len(payload):
            _fail("TON publication output inode changed")
        named = os.stat(name, dir_fd=directory, follow_symlinks=False)
        if (named.st_dev, named.st_ino) != identity:
            _fail("TON publication output name changed")
        os.lseek(descriptor, 0, os.SEEK_SET)
        digest = hashlib.sha256()
        observed = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            observed += len(chunk)
            digest.update(chunk)
        if observed != len(payload) or digest.digest() != hashlib.sha256(payload).digest():
            _fail("TON publication output failed inode readback")
        final_named = os.stat(name, dir_fd=directory, follow_symlinks=False)
        final_opened = os.fstat(descriptor)
        if (
            (final_named.st_dev, final_named.st_ino) != identity
            or (final_opened.st_dev, final_opened.st_ino) != identity
            or final_opened.st_size != len(payload)
        ):
            _fail("TON publication output changed during readback")
    finally:
        os.close(descriptor)


def _mkdir_at(parent: int, name: str) -> int:
    if not SEGMENT_RE.fullmatch(name):
        _fail("TON publication contains an invalid directory")
    try:
        os.mkdir(name, mode=0o700, dir_fd=parent)
        descriptor = os.open(
            name,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent,
        )
        os.fchmod(descriptor, 0o700)
        os.fsync(parent)
    except OSError:
        _fail("TON publication could not create an output directory safely")
    metadata = os.fstat(descriptor)
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077
    ):
        os.close(descriptor)
        _fail("TON publication output directory is not owner-only")
    return descriptor


def _publish_tree(source_root: Path, entries: Sequence[Mapping[str, Any]], parent: int) -> None:
    directories: dict[tuple[str, ...], int] = {(): parent}
    try:
        for entry in entries:
            parts = PurePosixPath(entry["path"]).parts
            current: tuple[str, ...] = ()
            for component in parts[:-1]:
                next_path = (*current, component)
                if next_path not in directories:
                    directories[next_path] = _mkdir_at(directories[current], component)
                current = next_path
            payload = common.read_direct_file(
                source_root.joinpath(*parts),
                label="reproduced TON artifact",
                maximum=entry["size_bytes"],
            )
            if len(payload) != entry["size_bytes"] or hashlib.sha256(payload).hexdigest() != entry["sha256"]:
                _fail("reproduced TON artifact changed before publication")
            _write_file_at(directories[current], parts[-1], payload, executable=entry["executable"])
        for descriptor in directories.values():
            os.fsync(descriptor)
    finally:
        for path, descriptor in sorted(directories.items(), reverse=True):
            if path:
                os.close(descriptor)


def _publish_candidate(
    output: Path,
    *,
    build_output: Path,
    unsigned_lock: Mapping[str, Any],
) -> None:
    descriptor = _new_private_output(output, label="TON candidate output")
    try:
        artifacts = _mkdir_at(descriptor, "artifacts")
        try:
            _publish_tree(build_output / "artifacts", unsigned_lock["artifacts"], artifacts)
        finally:
            os.close(artifacts)
        unsigned_bytes = common.canonical_json_file_bytes(unsigned_lock)
        payload = output_lock_signing_payload(unsigned_lock)
        _write_file_at(descriptor, "unsigned-output-lock.json", unsigned_bytes)
        _write_file_at(descriptor, "output-lock-signing-payload.bin", payload)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _publish_release(
    output: Path,
    *,
    build_output: Path,
    policy_bytes: bytes,
    lock_bytes: bytes,
    lock: Mapping[str, Any],
    receipt: Mapping[str, Any],
) -> None:
    descriptor = _new_private_output(output, label="TON release output")
    try:
        artifacts = _mkdir_at(descriptor, "artifacts")
        try:
            _publish_tree(build_output / "artifacts", lock["artifacts"], artifacts)
        finally:
            os.close(artifacts)
        _write_file_at(descriptor, "ton-builder-policy.json", policy_bytes)
        _write_file_at(descriptor, "ton-output-lock.json", lock_bytes)
        # Receipt is the publication manifest and is deliberately written last.
        receipt_bytes = common.canonical_json_file_bytes(receipt)
        common.reject_secret_material(receipt_bytes, label="TON build receipt")
        _write_file_at(descriptor, "ton-build-receipt.json", receipt_bytes)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _production_build(
    *,
    policy_path: Path,
    trusted_policy_sha256: str,
    git_path: str,
    docker_path: str,
) -> tuple[
    dict[str, Any],
    bytes,
    str,
    str,
    tempfile.TemporaryDirectory[str],
    Path,
    dict[str, Any],
]:
    _ensure_outside_repository(policy_path, label="TON builder policy")
    policy_value, policy_bytes = _load_canonical_json(
        policy_path, label="TON builder policy", maximum=MAX_POLICY_BYTES
    )
    policy = validate_policy(policy_value)
    if common.canonical_json_file_bytes(policy) != policy_bytes:
        _fail("TON builder policy is not in normalized final-V1 form")
    policy_sha256 = hashlib.sha256(policy_bytes).hexdigest()
    if _hex32(trusted_policy_sha256, label="trusted policy SHA-256") != policy_sha256:
        _fail("TON builder policy does not match the externally trusted digest")
    python, python_identity, python_hash = _open_stable_executable(
        os.fspath(Path(sys.executable).resolve()),
        label="pinned Python executable",
    )
    if python_hash != policy["builder"]["host_python_sha256"]:
        _fail("pinned Python executable does not match builder policy")
    git, git_identity, git_hash = _open_stable_executable(git_path, label="pinned Git executable")
    docker, docker_identity, docker_hash = _open_stable_executable(
        docker_path, label="pinned Docker executable"
    )
    if git_hash != policy["builder"]["host_git_sha256"]:
        _fail("pinned Git executable does not match builder policy")
    if docker_hash != policy["builder"]["host_docker_sha256"]:
        _fail("pinned Docker executable does not match builder policy")
    _inspect_image(docker, policy, identity=docker_identity)
    temporary = tempfile.TemporaryDirectory(prefix="iroha-sccp-ton-builder-")
    temporary_root = Path(temporary.name)
    os.chmod(temporary_root, 0o700)
    archive = temporary_root / "source.tar"
    source_closure_sha256 = _verify_source_and_archive(git, policy, archive)
    _require_unchanged_executable(git, git_identity, label="pinned Git executable")
    _require_unchanged_executable(python, python_identity, label="pinned Python executable")
    build_one = temporary_root / "build-one"
    build_two = temporary_root / "build-two"
    report_one = _run_container_build(
        docker,
        docker_identity,
        policy,
        archive,
        source_closure_sha256,
        build_one,
    )
    report_two = _run_container_build(
        docker,
        docker_identity,
        policy,
        archive,
        source_closure_sha256,
        build_two,
    )
    if report_one != report_two or _tree_hash(report_one) != _tree_hash(report_two):
        temporary.cleanup()
        _fail("independent TON contract builds are not byte-identical")
    _require_unchanged_executable(git, git_identity, label="pinned Git executable")
    _require_unchanged_executable(python, python_identity, label="pinned Python executable")
    _require_unchanged_executable(docker, docker_identity, label="pinned Docker executable")
    return (
        policy,
        policy_bytes,
        policy_sha256,
        source_closure_sha256,
        temporary,
        build_one,
        report_one,
    )


def prepare_release(arguments: argparse.Namespace) -> None:
    """Reproduce a candidate and emit the exact bytes for offline signatures."""

    policy, _, policy_sha256, source_sha256, temporary, build_one, report = _production_build(
        policy_path=arguments.policy,
        trusted_policy_sha256=arguments.trusted_policy_sha256,
        git_path=arguments.git,
        docker_path=arguments.docker,
    )
    try:
        unsigned = _unsigned_lock(policy_sha256, source_sha256, policy, report)
        _publish_candidate(arguments.output_dir, build_output=build_one, unsigned_lock=unsigned)
    finally:
        temporary.cleanup()


def release(arguments: argparse.Namespace) -> None:
    """Reproduce, authenticate, and publish a production TON contract build."""

    (
        policy,
        policy_bytes,
        policy_sha256,
        source_sha256,
        temporary,
        build_one,
        report,
    ) = _production_build(
        policy_path=arguments.policy,
        trusted_policy_sha256=arguments.trusted_policy_sha256,
        git_path=arguments.git,
        docker_path=arguments.docker,
    )
    try:
        _ensure_outside_repository(
            arguments.signed_output_lock,
            label="TON signed output lock",
        )
        expected_unsigned = _unsigned_lock(policy_sha256, source_sha256, policy, report)
        lock_value, lock_bytes = _load_canonical_json(
            arguments.signed_output_lock,
            label="TON signed output lock",
            maximum=MAX_LOCK_BYTES,
        )
        lock = validate_signed_lock(lock_value, expected_unsigned=expected_unsigned, policy=policy)
        if common.canonical_json_file_bytes(lock) != lock_bytes:
            _fail("TON signed output lock is not in normalized final-V1 form")
        lock_sha256 = hashlib.sha256(lock_bytes).hexdigest()
        if len({policy_sha256, source_sha256, lock_sha256}) != 3:
            _fail("TON release identities must be role-distinct")
        receipt = {
            "schema": RECEIPT_SCHEMA,
            "ton_builder_policy_sha256": policy_sha256,
            "ton_source_closure_sha256": source_sha256,
            "ton_output_lock_sha256": lock_sha256,
            "source_commit": policy["source"]["commit"],
            "artifact_tree_sha256": lock["artifact_tree_sha256"],
        }
        _publish_release(
            arguments.output_dir,
            build_output=build_one,
            policy_bytes=policy_bytes,
            lock_bytes=lock_bytes,
            lock=lock,
            receipt=receipt,
        )
        sys.stdout.buffer.write(common.canonical_json_file_bytes(receipt))
    finally:
        temporary.cleanup()


def development_local(arguments: argparse.Namespace) -> None:
    """Run an explicitly non-release local Acton smoke build."""

    acton, identity, _ = _open_stable_executable(arguments.acton, label="development Acton executable")
    environment = _closed_environment()
    stdout, _ = _run_bounded(
        acton,
        ("--version",),
        cwd=PROJECT,
        environment=environment,
        maximum_bytes=16 * 1024,
        timeout_seconds=30,
        label="development Acton version probe",
    )
    if stdout.decode("utf-8", "strict").strip() != ACTON_VERSION:
        _fail("development Acton executable is not exact Acton 1.1.0")
    doctor, _ = _run_bounded(
        acton,
        ("doctor", "--project-root", os.fspath(PROJECT)),
        cwd=PROJECT,
        environment=environment,
        maximum_bytes=256 * 1024,
        timeout_seconds=60,
        label="development Acton doctor",
    )
    if len(re.findall(rb"(?m)^tolk\.version:[ \t]+1\.4\.1[ \t]*$", doctor)) != 1:
        _fail("development Acton does not expose exact embedded Tolk 1.4.1")
    for command in (("fmt", "--check"), ("check",), ("build",), ("test",)):
        _run_bounded(
            acton,
            command,
            cwd=PROJECT,
            environment=environment,
            maximum_bytes=8 * 1024 * 1024,
            timeout_seconds=1_800,
            label="development-only TON contract smoke build",
        )
        _require_unchanged_executable(acton, identity, label="development Acton executable")
    print("TON SCCP local Acton validation passed (development-only; no release receipt emitted).")


def _parser() -> argparse.ArgumentParser:
    parser = _SafeArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="mode", required=True)
    development = subparsers.add_parser(
        "development-local", help="run a non-release local Acton smoke build"
    )
    development.add_argument("--acton", required=True, help="absolute Acton 1.1.0 executable")
    development.set_defaults(handler=development_local)
    for name, handler in (("production-prepare", prepare_release), ("production-release", release)):
        production = subparsers.add_parser(name)
        production.add_argument("--policy", required=True, type=Path)
        production.add_argument("--trusted-policy-sha256", required=True)
        production.add_argument("--git", required=True, help="absolute pinned Git executable")
        production.add_argument("--docker", required=True, help="absolute pinned Docker executable")
        production.add_argument("--output-dir", required=True, type=Path)
        if name == "production-release":
            production.add_argument("--signed-output-lock", required=True, type=Path)
        production.set_defaults(handler=handler)
    return parser


def main(arguments: Sequence[str] | None = None) -> int:
    """CLI entry point with bounded, secret-free failure output."""

    try:
        parsed = _parser().parse_args(arguments)
        parsed.handler(parsed)
        return 0
    except (TonBuilderError, common.SccpReleaseError) as error:
        print(f"TON SCCP builder failed: {error}", file=sys.stderr)
        return 2
    except (OSError, UnicodeError, ValueError, TypeError):
        print("TON SCCP builder failed safely", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
