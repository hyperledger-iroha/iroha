#!/usr/bin/env python3
"""Network-isolated build driver for the final-V1 SCCP Rust validator.

The digest-pinned Linux/amd64 image owns this reviewed file at one fixed path;
the host mounts only a deterministic Git archive. The driver emits one exact
canonical USTAR result on stdout, vendors the complete Cargo dependency closure
offline, records the compiler/sysroot/linker closure, and performs the sole
permitted validator build. It never signs release material or accesses the
network.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import threading
import time
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

REPORT_SCHEMA = "iroha.sccp.validator-build-report.final-v1"
SBOM_SCHEMA = "iroha.sccp.rust-validator-sbom.final-v1"
TARGET = "x86_64-unknown-linux-gnu"
CRATE = "iroha_sccp"
BINARY = "sccp_release_evidence"
FEATURES = ("dev-tools",)
BUILD_PROFILE = "release"
MAX_JSON_BYTES = 128 * 1024 * 1024
CHUNK_BYTES = 1024 * 1024
SOURCE_TREE_INVENTORY = ".sccp-source-tree-inventory.json"
APPROVED_SOURCE_CARGO_CONFIG_SHA256 = (
    "99ccdf420c7fd6f7c4abcb1908a653b832791cb169d020448fb23cda85b40014"
)
HEX32_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
SAFE_PATH_RE = re.compile(r"^/[A-Za-z0-9][A-Za-z0-9._+/-]{0,1023}$")
DRIVER_IMAGE_PATH = Path("/opt/iroha/sccp_validator_builder_driver.py")

# Linux x86_64 Landlock ABI. The release container is fixed to this platform;
# absence of ABI v3 (which added truncate mediation) is a hard failure.
_LANDLOCK_CREATE_RULESET = 444
_LANDLOCK_ADD_RULE = 445
_LANDLOCK_RESTRICT_SELF = 446
_LANDLOCK_CREATE_RULESET_VERSION = 1
_LANDLOCK_RULE_PATH_BENEATH = 1
_PR_SET_NO_NEW_PRIVS = 38
_PR_SET_DUMPABLE = 4
_PR_GET_DUMPABLE = 3
_LANDLOCK_ACCESS_FS_WRITE_FILE = 1 << 1
_LANDLOCK_ACCESS_FS_REMOVE_DIR = 1 << 4
_LANDLOCK_ACCESS_FS_REMOVE_FILE = 1 << 5
_LANDLOCK_ACCESS_FS_MAKE_CHAR = 1 << 6
_LANDLOCK_ACCESS_FS_MAKE_DIR = 1 << 7
_LANDLOCK_ACCESS_FS_MAKE_REG = 1 << 8
_LANDLOCK_ACCESS_FS_MAKE_SOCK = 1 << 9
_LANDLOCK_ACCESS_FS_MAKE_FIFO = 1 << 10
_LANDLOCK_ACCESS_FS_MAKE_BLOCK = 1 << 11
_LANDLOCK_ACCESS_FS_MAKE_SYM = 1 << 12
_LANDLOCK_ACCESS_FS_REFER = 1 << 13
_LANDLOCK_ACCESS_FS_TRUNCATE = 1 << 14
_LANDLOCK_WRITE_RIGHTS = (
    _LANDLOCK_ACCESS_FS_WRITE_FILE
    | _LANDLOCK_ACCESS_FS_REMOVE_DIR
    | _LANDLOCK_ACCESS_FS_REMOVE_FILE
    | _LANDLOCK_ACCESS_FS_MAKE_CHAR
    | _LANDLOCK_ACCESS_FS_MAKE_DIR
    | _LANDLOCK_ACCESS_FS_MAKE_REG
    | _LANDLOCK_ACCESS_FS_MAKE_SOCK
    | _LANDLOCK_ACCESS_FS_MAKE_FIFO
    | _LANDLOCK_ACCESS_FS_MAKE_BLOCK
    | _LANDLOCK_ACCESS_FS_MAKE_SYM
    | _LANDLOCK_ACCESS_FS_REFER
    | _LANDLOCK_ACCESS_FS_TRUNCATE
)


class DriverError(RuntimeError):
    """Bounded public-safe build-driver failure."""


class _Parser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        del message
        raise DriverError("driver command line has an invalid final-V1 shape")


def _fail(message: str) -> None:
    raise DriverError(message)


class _LandlockRulesetAttr(ctypes.Structure):
    _fields_ = (("handled_access_fs", ctypes.c_uint64),)


class _LandlockPathBeneathAttr(ctypes.Structure):
    _fields_ = (
        ("allowed_access", ctypes.c_uint64),
        ("parent_fd", ctypes.c_int32),
    )


def _seal_driver_process() -> None:
    """Make PID 1 inaccessible to same-UID compiler and build-script children."""

    production_path = Path("/opt/iroha/sccp_validator_builder_driver.py")
    if DRIVER_IMAGE_PATH != production_path:
        return
    if sys.platform != "linux" or os.getpid() != 1:
        raise OSError("builder driver sealing requires Linux PID 1")
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(_PR_SET_DUMPABLE, 0, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "could not make builder driver nondumpable")
    if libc.prctl(_PR_GET_DUMPABLE, 0, 0, 0, 0) != 0:
        raise OSError("builder driver remained dumpable")


def _install_compile_sandbox(writable_roots: Sequence[Path]) -> None:
    """Deny Cargo and descendants writes outside exact scratch paths."""

    if sys.platform != "linux" or os.uname().machine not in ("x86_64", "amd64"):
        raise OSError("compile sandbox requires exact Linux/amd64")
    libc = ctypes.CDLL(None, use_errno=True)
    syscall = libc.syscall
    syscall.restype = ctypes.c_long
    abi = syscall(
        _LANDLOCK_CREATE_RULESET,
        ctypes.c_void_p(),
        ctypes.c_size_t(0),
        ctypes.c_uint(_LANDLOCK_CREATE_RULESET_VERSION),
    )
    if abi < 3:
        raise OSError("compile sandbox requires Landlock ABI v3")
    ruleset_attr = _LandlockRulesetAttr(_LANDLOCK_WRITE_RIGHTS)
    ruleset = syscall(
        _LANDLOCK_CREATE_RULESET,
        ctypes.byref(ruleset_attr),
        ctypes.sizeof(ruleset_attr),
        ctypes.c_uint(0),
    )
    if ruleset < 0:
        raise OSError(ctypes.get_errno(), "could not create compile sandbox")
    try:
        if not writable_roots:
            raise OSError("compile sandbox has no writable scratch path")
        seen: set[tuple[int, int]] = set()
        o_path = getattr(os, "O_PATH", None)
        if o_path is None:
            raise OSError("compile sandbox requires Linux O_PATH")
        for root in writable_roots:
            metadata = root.lstat()
            identity = (metadata.st_dev, metadata.st_ino)
            if root.is_symlink() or identity in seen:
                raise OSError("compile sandbox scratch path is unsafe")
            if stat.S_ISDIR(metadata.st_mode):
                allowed_access = _LANDLOCK_WRITE_RIGHTS
            elif stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1:
                allowed_access = (
                    _LANDLOCK_ACCESS_FS_WRITE_FILE | _LANDLOCK_ACCESS_FS_TRUNCATE
                )
            else:
                raise OSError("compile sandbox scratch path is unsafe")
            seen.add(identity)
            path_fd = os.open(
                root,
                o_path | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
            )
            try:
                rule = _LandlockPathBeneathAttr(allowed_access, path_fd)
                if (
                    syscall(
                        _LANDLOCK_ADD_RULE,
                        ruleset,
                        _LANDLOCK_RULE_PATH_BENEATH,
                        ctypes.byref(rule),
                        ctypes.c_uint(0),
                    )
                    != 0
                ):
                    raise OSError(
                        ctypes.get_errno(), "could not add compile sandbox rule"
                    )
            finally:
                os.close(path_fd)
        if libc.prctl(_PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0:
            raise OSError(ctypes.get_errno(), "could not seal compile privileges")
        if syscall(_LANDLOCK_RESTRICT_SELF, ruleset, ctypes.c_uint(0)) != 0:
            raise OSError(ctypes.get_errno(), "could not enter compile sandbox")
    finally:
        os.close(ruleset)


def _quiesce_container_processes() -> None:
    """Kill and reap every build descendant before closure bytes are inspected."""

    production_path = Path("/opt/iroha/sccp_validator_builder_driver.py")
    if DRIVER_IMAGE_PATH != production_path:
        # Unit tests execute the driver in the host test process and replace the
        # fixed image path. They cannot safely emulate a PID namespace.
        return
    if os.getpid() != 1 or not Path("/proc/self/status").is_file():
        raise OSError("builder driver must be PID 1 in a private Linux PID namespace")

    for _ in range(100):
        remaining: list[int] = []
        try:
            entries = tuple(Path("/proc").iterdir())
        except OSError as error:
            raise OSError("could not enumerate builder process namespace") from error
        for entry in entries:
            if not entry.name.isascii() or not entry.name.isdigit():
                continue
            pid = int(entry.name)
            if pid <= 1:
                continue
            remaining.append(pid)
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except OSError as error:
                raise OSError("could not terminate a residual build process") from error
        while True:
            try:
                reaped, _ = os.waitpid(-1, os.WNOHANG)
            except ChildProcessError:
                break
            if reaped <= 0:
                break
        if not remaining:
            return
        time.sleep(0.01)
    raise OSError("residual build processes did not quiesce")


def _canonical_json(value: Any) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=True,
                allow_nan=False,
                sort_keys=True,
                separators=(",", ":"),
            ).encode("ascii")
            + b"\n"
        )
    except (TypeError, ValueError, RecursionError):
        _fail("driver could not encode one canonical closure document")


def _hash_file(
    path: Path, *, maximum: int, allow_empty: bool = False
) -> tuple[str, int, bool]:
    try:
        before = path.lstat()
    except OSError:
        _fail("a required build-closure file is unavailable")
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or (before.st_size == 0 and not allow_empty)
        or before.st_size > maximum
        or before.st_mode & (stat.S_ISUID | stat.S_ISGID | stat.S_ISVTX)
    ):
        _fail("a required build-closure file is unsafe or oversized")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    digest = hashlib.sha256()
    observed = 0
    try:
        descriptor = os.open(path, flags)
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns) != (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_ctime_ns,
        ):
            _fail("a build-closure file changed while opening")
        remaining = opened.st_size
        while remaining:
            chunk = os.read(descriptor, min(CHUNK_BYTES, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail("a build-closure file changed while hashing")
            observed += len(chunk)
            digest.update(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail("a build-closure file changed while hashing")
        after = os.fstat(descriptor)
    except OSError:
        _fail("a build-closure file could not be read safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    if observed != before.st_size or (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_ctime_ns,
    ) != (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns):
        _fail("a build-closure file changed while hashing")
    return digest.hexdigest(), observed, bool(before.st_mode & stat.S_IXUSR)


def _safe_member_path(name: str) -> tuple[str, ...]:
    if len(name.encode("utf-8", "surrogatepass")) > 8192 or any(
        ord(character) < 0x20 for character in name
    ):
        _fail("source archive contains an oversized or control-bearing member path")
    path = PurePosixPath(name)
    if (
        path.is_absolute()
        or str(path) != name
        or not path.parts
        or path.parts[0] != "source"
    ):
        _fail("source archive contains a non-canonical member path")
    if any(part in ("", ".", "..") for part in path.parts):
        _fail("source archive contains an unsafe member path")
    return path.parts


def _safe_link_target(parts: Sequence[str], target: str) -> str:
    if len(target.encode("utf-8", "surrogatepass")) > 8192 or any(
        ord(character) < 0x20 for character in target
    ):
        _fail("source archive contains an oversized or control-bearing symbolic link")
    link = PurePosixPath(target)
    if link.is_absolute() or str(link) != target or not link.parts:
        _fail("source archive contains an unsafe symbolic link")
    resolved = list(parts[:-1])
    for component in link.parts:
        if component in ("", "."):
            continue
        if component == "..":
            if len(resolved) <= 1:
                _fail("source archive symbolic link escapes the source tree")
            resolved.pop()
        else:
            resolved.append(component)
    if not resolved or resolved[0] != "source":
        _fail("source archive symbolic link escapes the source tree")
    return target


def _validate_restored_symlink_graph(source: Path, symlinks: Sequence[Path]) -> None:
    """Reject link chains whose fully resolved target leaves the source tree."""

    try:
        source_root = source.resolve(strict=True)
    except (OSError, RuntimeError):
        _fail("source archive root could not be resolved safely")
    for link in symlinks:
        try:
            metadata = link.lstat()
            resolved = link.resolve(strict=False)
            resolved.relative_to(source_root)
        except (OSError, RuntimeError, ValueError):
            _fail("source archive symbolic-link graph escapes or loops")
        if not stat.S_ISLNK(metadata.st_mode):
            _fail("source archive symbolic link changed during graph validation")


def _mkdir_chain(root: Path, components: Sequence[str]) -> Path:
    current = root
    for component in components:
        current = current / component
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            current.mkdir(mode=0o700)
            metadata = current.lstat()
        except OSError:
            _fail("source archive directory could not be created safely")
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            _fail("source archive path collides with a non-directory")
    return current


def _extract_source_archive(
    archive: Path | int,
    work: Path,
    *,
    source_date_epoch: int,
    maximum_files: int,
    maximum_file_bytes: int,
    maximum_total_bytes: int,
) -> Path:
    archive_file = None
    try:
        if isinstance(archive, int):
            os.lseek(archive, 0, os.SEEK_SET)
            archive_file = os.fdopen(os.dup(archive), "rb", closefd=True)
            stream = tarfile.open(  # noqa: SIM115 - translated errors
                fileobj=archive_file,
                mode="r:",
            )
        else:
            stream = tarfile.open(  # noqa: SIM115 - translated errors
                archive,
                mode="r:",
            )
    except (OSError, tarfile.TarError):
        _fail("source archive is not one canonical uncompressed tar stream")
    seen: set[str] = set()
    files = 0
    members = 0
    total = 0
    symlinks: list[tuple[Path, str]] = []
    directories: list[Path] = []
    try:
        for member in stream:
            members += 1
            if members > min(1_000_000, maximum_files * 8 + 16):
                _fail("source archive exceeds its bounded member-count limit")
            parts = _safe_member_path(member.name.rstrip("/") or member.name)
            if member.mtime != source_date_epoch:
                _fail("source archive member has a noncanonical modification time")
            canonical = "/".join(parts)
            if canonical in seen:
                _fail("source archive repeats a member path")
            seen.add(canonical)
            destination = work.joinpath(*parts)
            if member.isdir():
                _mkdir_chain(work, parts)
                os.chmod(destination, 0o700)
                directories.append(destination)
                continue
            _mkdir_chain(work, parts[:-1])
            if member.issym():
                target = _safe_link_target(parts, member.linkname)
                target_size = len(target.encode("utf-8", "surrogatepass"))
                if target_size > maximum_file_bytes:
                    _fail("source archive contains an oversized symbolic link")
                files += 1
                total += target_size
                if files > maximum_files or total > maximum_total_bytes:
                    _fail("source archive exceeds its signed extraction limits")
                symlinks.append((destination, target))
                continue
            if not member.isfile() or member.islnk():
                _fail(
                    "source archive contains a forbidden special or hard-linked member"
                )
            if member.size < 0 or member.size > maximum_file_bytes:
                _fail("source archive contains an oversized tracked file")
            files += 1
            total += member.size
            if files > maximum_files or total > maximum_total_bytes:
                _fail("source archive exceeds its signed extraction limits")
            extracted = stream.extractfile(member)
            if extracted is None:
                _fail("source archive regular file has no payload")
            flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
            try:
                descriptor = os.open(
                    destination, flags, 0o700 if member.mode & 0o100 else 0o600
                )
                remaining = member.size
                while remaining:
                    chunk = extracted.read(min(CHUNK_BYTES, remaining))
                    if not chunk:
                        _fail("source archive file ended before its declared size")
                    view = memoryview(chunk)
                    while view:
                        written = os.write(descriptor, view)
                        if written <= 0:
                            _fail("source archive extraction made no progress")
                        view = view[written:]
                    remaining -= len(chunk)
                if extracted.read(1):
                    _fail("source archive file exceeds its declared size")
                os.fsync(descriptor)
                os.fchmod(descriptor, 0o700 if member.mode & 0o100 else 0o600)
                os.utime(
                    descriptor,
                    ns=(source_date_epoch * 1_000_000_000,) * 2,
                )
            except OSError:
                _fail("source archive file could not be extracted safely")
            finally:
                if "descriptor" in locals():
                    os.close(descriptor)
                    del descriptor
                extracted.close()
        for destination, target in symlinks:
            try:
                os.symlink(target, destination)
                os.utime(
                    destination,
                    ns=(source_date_epoch * 1_000_000_000,) * 2,
                    follow_symlinks=False,
                )
            except OSError:
                _fail("source archive symbolic link could not be restored safely")
        for directory in reversed(directories):
            try:
                os.utime(
                    directory,
                    ns=(source_date_epoch * 1_000_000_000,) * 2,
                    follow_symlinks=False,
                )
            except OSError:
                _fail("source archive directory time could not be normalized")
    finally:
        stream.close()
        if archive_file is not None:
            archive_file.close()
    source = work / "source"
    _validate_restored_symlink_graph(
        source,
        tuple(destination for destination, _ in symlinks),
    )
    if (
        not source.is_dir()
        or not (source / "Cargo.toml").is_file()
        or not (source / "Cargo.lock").is_file()
    ):
        _fail("source archive omits the Cargo workspace closure")
    return source


def _snapshot_source_archive(
    source: Path,
    destination: Path,
    *,
    expected_sha256: str,
    maximum: int,
) -> tuple[int, int, tuple[int, ...]]:
    """Copy one authenticated mount inode into tmpfs and retain its descriptor."""

    source_descriptor = None
    destination_descriptor = None
    try:
        before = source.lstat()
        if (
            stat.S_ISLNK(before.st_mode)
            or not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or not 0 < before.st_size <= maximum
        ):
            _fail("source archive mount is not one bounded direct file")
        source_descriptor = os.open(
            source,
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
        source_opened = os.fstat(source_descriptor)
        if (
            source_opened.st_dev,
            source_opened.st_ino,
            source_opened.st_size,
            source_opened.st_ctime_ns,
        ) != (before.st_dev, before.st_ino, before.st_size, before.st_ctime_ns):
            _fail("source archive mount changed while opening")
        destination_descriptor = os.open(
            destination,
            os.O_RDWR
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600,
        )
        digest = hashlib.sha256()
        observed = 0
        remaining = source_opened.st_size
        while remaining:
            chunk = os.read(source_descriptor, min(CHUNK_BYTES, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail("source archive mount changed while snapshotting")
            digest.update(chunk)
            observed += len(chunk)
            view = memoryview(chunk)
            while view:
                written = os.write(destination_descriptor, view)
                if written <= 0:
                    _fail("source archive snapshot made no progress")
                view = view[written:]
            remaining -= len(chunk)
        if os.read(source_descriptor, 1):
            _fail("source archive mount grew while snapshotting")
        os.fsync(destination_descriptor)
        os.fchmod(destination_descriptor, 0o400)
        source_after = os.fstat(source_descriptor)
        snapshot = os.fstat(destination_descriptor)
    except OSError:
        _fail("source archive could not be snapshotted safely")
    finally:
        if source_descriptor is not None:
            os.close(source_descriptor)
    if (
        observed != source_opened.st_size
        or digest.hexdigest() != expected_sha256
        or (
            source_after.st_dev,
            source_after.st_ino,
            source_after.st_size,
            source_after.st_ctime_ns,
        )
        != (
            source_opened.st_dev,
            source_opened.st_ino,
            source_opened.st_size,
            source_opened.st_ctime_ns,
        )
        or not stat.S_ISREG(snapshot.st_mode)
        or snapshot.st_nlink != 1
        or snapshot.st_size != observed
    ):
        if destination_descriptor is not None:
            os.close(destination_descriptor)
        _fail("source archive snapshot failed authenticated inode readback")
    assert destination_descriptor is not None
    os.lseek(destination_descriptor, 0, os.SEEK_SET)
    return (
        destination_descriptor,
        observed,
        (
            snapshot.st_dev,
            snapshot.st_ino,
            snapshot.st_size,
            snapshot.st_mtime_ns,
            snapshot.st_ctime_ns,
        ),
    )


def _verify_open_source_archive(
    descriptor: int,
    *,
    expected_sha256: str,
    expected_size: int,
    identity: tuple[int, ...],
) -> None:
    """Rehash the exact descriptor extracted by the driver before running source."""

    digest = hashlib.sha256()
    observed = 0
    try:
        os.lseek(descriptor, 0, os.SEEK_SET)
        remaining = expected_size
        while remaining:
            chunk = os.read(descriptor, min(CHUNK_BYTES, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail("authenticated source archive changed during extraction")
            digest.update(chunk)
            observed += len(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail("authenticated source archive grew during extraction")
        after = os.fstat(descriptor)
    except OSError:
        _fail("authenticated source archive could not be reverified")
    if (
        observed != expected_size
        or digest.hexdigest() != expected_sha256
        or (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        )
        != identity
    ):
        _fail("authenticated source archive changed during extraction")


def _validate_source_cargo_configuration(source: Path) -> str:
    """Authenticate the repository's reviewed, alias-only Cargo configuration."""

    source_cargo = source / ".cargo"
    if os.path.lexists(source_cargo / "config"):
        _fail("tracked source contains an unapproved Cargo configuration name")
    payload = _read_stable_file(source_cargo / "config.toml", maximum=16 * 1024)
    digest = hashlib.sha256(payload).hexdigest()
    if digest != APPROVED_SOURCE_CARGO_CONFIG_SHA256:
        _fail("tracked source Cargo configuration is not the reviewed alias-only file")
    return digest


def _read_stable_file(path: Path, *, maximum: int) -> bytes:
    try:
        before = path.lstat()
    except OSError:
        _fail("source tree inventory is unavailable")
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or not 0 < before.st_size <= maximum
    ):
        _fail("source tree inventory is not one bounded direct file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    payload = bytearray()
    try:
        descriptor = os.open(path, flags)
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns) != (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_ctime_ns,
        ):
            _fail("source tree inventory changed while opening")
        remaining = opened.st_size
        while remaining:
            chunk = os.read(descriptor, min(CHUNK_BYTES, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail("source tree inventory changed while reading")
            payload.extend(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail("source tree inventory changed while reading")
        after = os.fstat(descriptor)
    except OSError:
        _fail("source tree inventory could not be read safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    if len(payload) != before.st_size or (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_ctime_ns,
    ) != (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns):
        _fail("source tree inventory changed while reading")
    return bytes(payload)


def _git_blob_id(path: Path, *, algorithm: str, maximum: int) -> str:
    try:
        metadata = path.lstat()
    except OSError:
        _fail("tracked source blob is missing from the archive")
    if stat.S_ISLNK(metadata.st_mode):
        try:
            payload = os.fsencode(os.readlink(path))
        except OSError:
            _fail("tracked source symbolic link could not be read")
        if len(payload) > maximum:
            _fail("tracked source symbolic link exceeds its declared bound")
        digest = hashlib.new(algorithm)
        digest.update(f"blob {len(payload)}\0".encode("ascii"))
        digest.update(payload)
        return digest.hexdigest()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_size > maximum
    ):
        _fail("tracked source blob is not one bounded direct file")
    digest = hashlib.new(algorithm)
    digest.update(f"blob {metadata.st_size}\0".encode("ascii"))
    observed = 0
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
            _fail("tracked source blob changed while opening")
        remaining = opened.st_size
        while remaining:
            chunk = os.read(descriptor, min(CHUNK_BYTES, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail("tracked source blob changed while hashing")
            observed += len(chunk)
            digest.update(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail("tracked source blob changed while hashing")
        after = os.fstat(descriptor)
    except OSError:
        _fail("tracked source blob could not be hashed safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    if observed != metadata.st_size or (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_ctime_ns,
    ) != (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns):
        _fail("tracked source blob changed while hashing")
    return digest.hexdigest()


def _validate_source_tree_inventory(
    source: Path,
    *,
    source_commit: str,
    maximum_files: int,
    maximum_file_bytes: int,
) -> None:
    payload = _read_stable_file(source / SOURCE_TREE_INVENTORY, maximum=MAX_JSON_BYTES)

    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                _fail("source tree inventory repeats a JSON field")
            result[key] = value
        return result

    try:
        value = json.loads(
            payload.decode("utf-8", "strict"), object_pairs_hook=reject_duplicates
        )
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError):
        _fail("source tree inventory is not strict canonical JSON")
    if (
        payload != _canonical_json(value)
        or type(value) is not dict
        or set(value)
        != {
            "schema",
            "source_commit",
            "object_format",
            "entries",
        }
    ):
        _fail("source tree inventory has the wrong final-V1 shape")
    algorithm = "sha1" if len(source_commit) == 40 else "sha256"
    if (
        value["schema"] != "iroha.sccp.git-source-tree-inventory.final-v1"
        or value["source_commit"] != source_commit
        or value["object_format"] != algorithm
        or type(value["entries"]) is not list
        or not value["entries"]
        or len(value["entries"]) > maximum_files
    ):
        _fail("source tree inventory does not bind the requested signed commit")
    blob_paths: set[str] = set()
    gitlink_paths: set[str] = set()
    previous = ""
    for entry in value["entries"]:
        if type(entry) is not dict or set(entry) != {
            "mode",
            "object_type",
            "object_id",
            "path",
        }:
            _fail("source tree inventory entry has the wrong final-V1 shape")
        mode = entry["mode"]
        object_type = entry["object_type"]
        object_id = entry["object_id"]
        relative = entry["path"]
        if (
            type(relative) is not str
            or not relative
            or relative <= previous
            or relative == SOURCE_TREE_INVENTORY
            or PurePosixPath(relative).is_absolute()
            or str(PurePosixPath(relative)) != relative
            or any(part in ("", ".", "..") for part in PurePosixPath(relative).parts)
            or type(object_id) is not str
            or not re.fullmatch(
                r"[0-9a-f]{40}" if algorithm == "sha1" else r"[0-9a-f]{64}", object_id
            )
        ):
            _fail("source tree inventory entry is not canonical")
        previous = relative
        path = source.joinpath(*PurePosixPath(relative).parts)
        if (mode, object_type) == ("160000", "commit"):
            gitlink_paths.add(relative)
            if path.exists() and (
                path.is_symlink() or not path.is_dir() or any(path.iterdir())
            ):
                _fail("source archive expands a gitlink into unauthenticated content")
            continue
        if (mode, object_type) not in {
            ("100644", "blob"),
            ("100755", "blob"),
            ("120000", "blob"),
        }:
            _fail("source tree inventory contains an unsupported object kind")
        blob_paths.add(relative)
        try:
            metadata = path.lstat()
        except OSError:
            _fail("source archive omits a tracked blob")
        if mode == "120000":
            if not stat.S_ISLNK(metadata.st_mode):
                _fail("source archive changed a tracked symbolic-link mode")
        else:
            expected_executable = mode == "100755"
            if (
                not stat.S_ISREG(metadata.st_mode)
                or bool(metadata.st_mode & stat.S_IXUSR) != expected_executable
            ):
                _fail("source archive changed a tracked regular-file mode")
        if (
            _git_blob_id(path, algorithm=algorithm, maximum=maximum_file_bytes)
            != object_id
        ):
            _fail("source archive blob differs from the signed Git tree object")
    actual_paths: set[str] = set()
    actual_directories: set[str] = set()
    for directory, directory_names, file_names in os.walk(source, followlinks=False):
        directory_names.sort()
        file_names.sort()
        retained_directories: list[str] = []
        for name in directory_names:
            child = Path(directory) / name
            if child.is_symlink():
                actual_paths.add(child.relative_to(source).as_posix())
            else:
                actual_directories.add(child.relative_to(source).as_posix())
                retained_directories.append(name)
        directory_names[:] = retained_directories
        for name in file_names:
            actual_paths.add((Path(directory) / name).relative_to(source).as_posix())
    expected_paths = blob_paths | {SOURCE_TREE_INVENTORY}
    expected_directories: set[str] = set(gitlink_paths)
    for relative in blob_paths | gitlink_paths:
        parent = PurePosixPath(relative).parent
        while parent != PurePosixPath("."):
            expected_directories.add(parent.as_posix())
            parent = parent.parent
    if (
        actual_paths != expected_paths
        or actual_directories != expected_directories
        or blob_paths & gitlink_paths
    ):
        _fail("source archive file inventory differs from the complete signed Git tree")


def _run(
    executable: Path,
    arguments: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    maximum_output_bytes: int,
    label: str,
    publish_for_host_scan: bool = False,
    compile_sandbox_writable_roots: Sequence[Path] | None = None,
    quiesce_process_namespace: bool = False,
) -> tuple[bytes, bytes]:
    def child_setup() -> None:
        os.setsid()
        if compile_sandbox_writable_roots is not None:
            _install_compile_sandbox(compile_sandbox_writable_roots)

    if threading.active_count() != 1:
        _fail("driver refuses unsafe fork setup while another thread is active")
    try:
        process = subprocess.Popen(
            [os.fspath(executable), *arguments],
            cwd=cwd,
            env=dict(environment),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            preexec_fn=child_setup,  # noqa: PLW1509 - single-threaded, required for Landlock
        )
    except (OSError, subprocess.SubprocessError):
        _fail(f"{label} could not start")
    assert process.stdout is not None and process.stderr is not None
    buffers = (bytearray(), bytearray())
    total = [0]
    overflow = [False]
    errors: list[Exception] = []
    lock = threading.Lock()
    stop = threading.Event()

    def terminate_group() -> None:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            return
        except OSError as error:
            errors.append(error)

    def read_pipe(pipe: Any, index: int) -> None:
        try:
            while True:
                chunk = pipe.read(64 * 1024)
                if not chunk:
                    return
                with lock:
                    total[0] += len(chunk)
                    if total[0] > maximum_output_bytes:
                        overflow[0] = True
                        stop.set()
                    elif not overflow[0]:
                        buffers[index].extend(chunk)
        except Exception as error:  # noqa: BLE001 - fail closed on any reader fault
            errors.append(error)
            stop.set()

    readers = (
        threading.Thread(target=read_pipe, args=(process.stdout, 0), daemon=True),
        threading.Thread(target=read_pipe, args=(process.stderr, 1), daemon=True),
    )
    for reader in readers:
        reader.start()
    return_code: int | None = None
    try:
        while return_code is None:
            if stop.wait(0.05):
                terminate_group()
            try:
                return_code = process.wait(timeout=0.05)
            except subprocess.TimeoutExpired:
                continue
        if quiesce_process_namespace:
            _quiesce_container_processes()
    except OSError:
        terminate_group()
        process.wait()
        if quiesce_process_namespace:
            _quiesce_container_processes()
        _fail(f"{label} output could not be read safely")
    finally:
        for reader in readers:
            reader.join(timeout=1)
        if any(reader.is_alive() for reader in readers):
            terminate_group()
            for reader in readers:
                reader.join(timeout=1)
    if any(reader.is_alive() for reader in readers) or errors:
        _fail(f"{label} output could not be drained safely")
    if overflow[0]:
        _fail(f"{label} exceeded its output limit")
    stdout, stderr = bytes(buffers[0]), bytes(buffers[1])
    # Child diagnostics are always forwarded to the host-side bounded secret
    # scanner. Machine-readable stdout is forwarded only for the build command.
    published = stderr + (stdout if publish_for_host_scan else b"")
    if published:
        try:
            sys.stderr.buffer.write(published)
            sys.stderr.buffer.flush()
        except OSError:
            _fail(f"{label} could not publish its bounded log for secret scanning")
    if return_code != 0:
        _fail(f"{label} failed")
    return stdout, stderr


def _version(
    executable: Path, arguments: Sequence[str], environment: Mapping[str, str]
) -> str:
    stdout, stderr = _run(
        executable,
        arguments,
        cwd=Path("/work"),
        environment=environment,
        maximum_output_bytes=64 * 1024,
        label="pinned tool version query",
        quiesce_process_namespace=True,
    )
    streams = [stream.strip() for stream in (stdout, stderr) if stream.strip()]
    if len(streams) != 1:
        _fail("pinned tool version did not use one unambiguous output stream")
    output = streams[0]
    try:
        value = output.decode("utf-8", "strict").strip()
    except UnicodeDecodeError:
        _fail("pinned tool version output is not UTF-8")
    if not value or "\x00" in value or len(value) > 8192:
        _fail("pinned tool version output is not bounded canonical text")
    return value


def _scan_tree(
    root: Path,
    *,
    prefix: str,
    maximum_files: int,
    maximum_file_bytes: int,
    maximum_total_bytes: int,
) -> list[dict[str, Any]]:
    if not root.is_dir() or root.is_symlink():
        _fail("one required build-closure tree is unavailable")
    entries: list[dict[str, Any]] = []
    total = 0
    for directory, directory_names, file_names in os.walk(root, followlinks=False):
        directory_names.sort()
        file_names.sort()
        for name in directory_names:
            path = Path(directory) / name
            if path.is_symlink():
                _fail("a build-closure tree contains a symbolic-link directory")
        for name in file_names:
            path = Path(directory) / name
            digest, size, executable = _hash_file(
                path, maximum=maximum_file_bytes, allow_empty=True
            )
            total += size
            if total > maximum_total_bytes:
                _fail("a build-closure tree exceeds its aggregate size limit")
            relative = path.relative_to(root).as_posix()
            entries.append(
                {
                    "path": f"{prefix}/{relative}",
                    "sha256": digest,
                    "size_bytes": size,
                    "executable": executable,
                }
            )
            if len(entries) > maximum_files:
                _fail("a build-closure tree exceeds its file-count limit")
    if not entries:
        _fail("a required build-closure tree is empty")
    entries.sort(key=lambda entry: entry["path"])
    return entries


def _copy_regular_tree(
    source: Path,
    destination: Path,
    *,
    maximum_files: int,
    maximum_file_bytes: int,
    maximum_total_bytes: int,
) -> None:
    """Copy one image-owned cache into tmpfs without following links."""

    if not source.is_dir() or source.is_symlink() or destination.exists():
        _fail("pinned Cargo cache is not one direct source directory")
    destination.mkdir(mode=0o700)
    files = 0
    total = 0
    for directory, directory_names, file_names in os.walk(source, followlinks=False):
        directory_names.sort()
        file_names.sort()
        relative_directory = Path(directory).relative_to(source)
        target_directory = destination / relative_directory
        for name in directory_names:
            child = Path(directory) / name
            if child.is_symlink() or not child.is_dir():
                _fail("pinned Cargo cache contains an unsafe directory entry")
            (target_directory / name).mkdir(mode=0o700)
        for name in file_names:
            child = Path(directory) / name
            relative_file = child.relative_to(source)
            lowered_parts = tuple(part.casefold() for part in relative_file.parts)
            if any(
                part in {"credentials", "credentials.toml", ".netrc"}
                for part in lowered_parts
            ) or relative_file.as_posix() in {"config", "config.toml"}:
                _fail(
                    "pinned Cargo cache contains forbidden configuration or credentials"
                )
            digest, size, executable = _hash_file(
                child, maximum=maximum_file_bytes, allow_empty=True
            )
            files += 1
            total += size
            if files > maximum_files or total > maximum_total_bytes:
                _fail("pinned Cargo cache exceeds its signed copy limits")
            destination_file = target_directory / name
            flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
            try:
                source_descriptor = os.open(child, flags)
                source_opened = os.fstat(source_descriptor)
                if (
                    not stat.S_ISREG(source_opened.st_mode)
                    or source_opened.st_nlink != 1
                    or source_opened.st_size != size
                ):
                    _fail("pinned Cargo cache changed while opening for copy")
                destination_descriptor = os.open(
                    destination_file,
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
                    0o700 if executable else 0o600,
                )
                copied = hashlib.sha256()
                observed = 0
                remaining = size
                while remaining:
                    chunk = os.read(source_descriptor, min(CHUNK_BYTES, remaining + 1))
                    if not chunk or len(chunk) > remaining:
                        _fail("pinned Cargo cache grew during its tmpfs copy")
                    observed += len(chunk)
                    copied.update(chunk)
                    view = memoryview(chunk)
                    while view:
                        written = os.write(destination_descriptor, view)
                        if written <= 0:
                            _fail("pinned Cargo cache copy made no progress")
                        view = view[written:]
                    remaining -= len(chunk)
                if os.read(source_descriptor, 1):
                    _fail("pinned Cargo cache grew during its tmpfs copy")
                os.fsync(destination_descriptor)
            except OSError:
                _fail("pinned Cargo cache could not be copied safely")
            finally:
                if "source_descriptor" in locals():
                    os.close(source_descriptor)
                    del source_descriptor
                if "destination_descriptor" in locals():
                    os.close(destination_descriptor)
                    del destination_descriptor
            if observed != size or copied.hexdigest() != digest:
                _fail("pinned Cargo cache changed during its tmpfs copy")


def _normalize_metadata(value: Any, replacements: Sequence[tuple[str, str]]) -> Any:
    if value is None or type(value) in (bool, int):
        return value
    if type(value) is str:
        result = value
        for source, replacement in replacements:
            result = result.replace(source, replacement)
        return result
    if type(value) is list:
        return [_normalize_metadata(item, replacements) for item in value]
    if type(value) is dict:
        return {
            key: _normalize_metadata(item, replacements)
            for key, item in sorted(value.items())
        }
    _fail("Cargo metadata contains an unsupported JSON value")


def _require_metadata_path(value: Any, *, roots: tuple[str, ...]) -> None:
    """Require one normalized Cargo path to live in an inventoried tree."""

    if type(value) is not str:
        _fail("Cargo metadata contains a malformed build-input path")
    for root in roots:
        prefix = root + "/"
        if not value.startswith(prefix):
            continue
        relative = value[len(prefix) :]
        pure = PurePosixPath(relative)
        if (
            relative
            and not pure.is_absolute()
            and str(pure) == relative
            and all(part not in ("", ".", "..") for part in pure.parts)
        ):
            return
    _fail("Cargo metadata references a build input outside inventoried closure trees")


def _validate_metadata_input_boundaries(metadata: Any) -> None:
    """Close every Cargo source path over the tracked source or vendor inventory."""

    if (
        type(metadata) is not dict
        or metadata.get("workspace_root") != "${SOURCE}"
        or metadata.get("target_directory") != "${TARGET}"
        or type(metadata.get("packages")) is not list
    ):
        _fail("Cargo metadata does not bind the isolated workspace and target roots")
    for package in metadata["packages"]:
        if type(package) is not dict:
            _fail("Cargo metadata contains a malformed package")
        source = package.get("source")
        if source is not None and type(source) is not str:
            _fail("Cargo metadata contains a malformed package source")
        package_roots = ("${SOURCE}",) if source is None else ("${VENDOR}",)
        _require_metadata_path(package.get("manifest_path"), roots=package_roots)
        for optional in ("license_file", "readme"):
            path = package.get(optional)
            if path is not None:
                _require_metadata_path(path, roots=package_roots)
        targets = package.get("targets")
        dependencies = package.get("dependencies")
        if type(targets) is not list or not targets or type(dependencies) is not list:
            _fail("Cargo metadata omits package target or dependency paths")
        for target in targets:
            if type(target) is not dict:
                _fail("Cargo metadata contains a malformed package target")
            _require_metadata_path(target.get("src_path"), roots=package_roots)
        for dependency in dependencies:
            if type(dependency) is not dict:
                _fail("Cargo metadata contains a malformed package dependency")
            path = dependency.get("path")
            if path is not None:
                _require_metadata_path(path, roots=("${SOURCE}", "${VENDOR}"))


def _build_sbom(metadata: Any) -> dict[str, Any]:
    """Derive one deterministic package/dependency SBOM from Cargo's closure."""

    if type(metadata) is not dict:
        _fail("Cargo metadata cannot produce the final-V1 SBOM")
    packages = metadata.get("packages")
    resolve = metadata.get("resolve")
    if type(packages) is not list or type(resolve) is not dict:
        _fail("Cargo metadata omits the resolved package graph")
    nodes = resolve.get("nodes")
    if type(nodes) is not list:
        _fail("Cargo metadata omits resolved dependency nodes")
    dependency_map: dict[str, list[str]] = {}
    for node in nodes:
        if type(node) is not dict or type(node.get("id")) is not str:
            _fail("Cargo metadata contains a malformed dependency node")
        dependencies = node.get("dependencies")
        if type(dependencies) is not list or any(
            type(item) is not str for item in dependencies
        ):
            _fail("Cargo metadata dependency node has a malformed edge list")
        if node["id"] in dependency_map:
            _fail("Cargo metadata repeats a resolved package id")
        dependency_map[node["id"]] = sorted(dependencies)
    entries: list[dict[str, Any]] = []
    roots: list[str] = []
    for package in packages:
        if type(package) is not dict or package.get("id") not in dependency_map:
            continue
        required = ("id", "name", "version", "manifest_path")
        if any(
            type(package.get(field)) is not str or not package[field]
            for field in required
        ):
            _fail("Cargo metadata package has an incomplete SBOM identity")
        for optional in ("source", "license", "license_file"):
            if (
                package.get(optional) is not None
                and type(package.get(optional)) is not str
            ):
                _fail("Cargo metadata package has a malformed SBOM field")
        entry = {
            "id": package["id"],
            "name": package["name"],
            "version": package["version"],
            "source": package.get("source"),
            "license": package.get("license"),
            "license_file": package.get("license_file"),
            "manifest_path": package["manifest_path"],
            "dependency_ids": dependency_map[package["id"]],
        }
        entries.append(entry)
        if package["name"] == CRATE and package.get("source") is None:
            roots.append(package["id"])
    entries.sort(key=lambda entry: entry["id"])
    if not entries or len(roots) != 1:
        _fail("Cargo metadata does not identify one exact SCCP validator root package")
    return {
        "schema": SBOM_SCHEMA,
        "target_triple": TARGET,
        "root_package_id": roots[0],
        "binary": BINARY,
        "enabled_features": list(FEATURES),
        "packages": entries,
    }


def _write_new(path: Path, payload: bytes, *, executable: bool = False) -> None:
    if not payload:
        _fail("driver refuses to publish an empty closure artifact")
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    if path.parent.is_symlink():
        _fail("driver output parent is a symbolic link")
    flags = os.O_RDWR | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    mode = 0o700 if executable else 0o600
    try:
        descriptor = os.open(path, flags, mode)
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail("driver output write made no progress")
            view = view[written:]
        os.fsync(descriptor)
        os.fchmod(descriptor, mode)
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_nlink != 1
            or metadata.st_size != len(payload)
        ):
            _fail("driver output inode changed while publishing")
    except OSError:
        _fail("driver output could not be published safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)


def _copy_new(
    source: Path,
    destination: Path,
    *,
    expected_sha256: str,
    expected_size: int,
    maximum: int,
    executable: bool,
) -> None:
    """Publish one authenticated large file without buffering it in memory."""

    digest, size, source_executable = _hash_file(source, maximum=maximum)
    if (
        digest != expected_sha256
        or size != expected_size
        or (executable and not source_executable)
    ):
        _fail("driver publication source changed before streaming copy")
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    if destination.parent.is_symlink():
        _fail("driver output parent is a symbolic link")
    source_flags = (
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    )
    destination_flags = (
        os.O_RDWR
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    mode = 0o700 if executable else 0o600
    try:
        source_descriptor = os.open(source, source_flags)
        source_opened = os.fstat(source_descriptor)
        destination_descriptor = os.open(destination, destination_flags, mode)
        copied = hashlib.sha256()
        observed = 0
        remaining = expected_size
        while remaining:
            chunk = os.read(source_descriptor, min(CHUNK_BYTES, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail("driver publication source grew during streaming copy")
            observed += len(chunk)
            copied.update(chunk)
            view = memoryview(chunk)
            while view:
                written = os.write(destination_descriptor, view)
                if written <= 0:
                    _fail("driver publication copy made no progress")
                view = view[written:]
            remaining -= len(chunk)
        if os.read(source_descriptor, 1):
            _fail("driver publication source grew during streaming copy")
        os.fsync(destination_descriptor)
        os.fchmod(destination_descriptor, mode)
        source_after = os.fstat(source_descriptor)
        published = os.fstat(destination_descriptor)
    except OSError:
        _fail("driver could not stream one authenticated output")
    finally:
        if "source_descriptor" in locals():
            os.close(source_descriptor)
        if "destination_descriptor" in locals():
            os.close(destination_descriptor)
    if (
        observed != expected_size
        or copied.hexdigest() != expected_sha256
        or (
            source_opened.st_dev,
            source_opened.st_ino,
            source_opened.st_size,
            source_opened.st_ctime_ns,
        )
        != (
            source_after.st_dev,
            source_after.st_ino,
            source_after.st_size,
            source_after.st_ctime_ns,
        )
        or not stat.S_ISREG(published.st_mode)
        or published.st_nlink != 1
        or published.st_size != expected_size
    ):
        _fail("driver authenticated output copy failed inode readback")


def _document(output: Path, name: str, value: Any) -> tuple[str, int]:
    payload = _canonical_json(value)
    _write_new(output / "closure" / name, payload)
    return hashlib.sha256(payload).hexdigest(), len(payload)


def _emit_output_archive(
    output: Path,
    *,
    source_date_epoch: int,
    maximum_file_bytes: int,
    maximum_total_bytes: int,
) -> None:
    """Stream the exact authenticated build output out of quota-backed tmpfs."""

    files = (
        "builder-report.json",
        "closure/build-environment.json",
        "closure/build-recipe.json",
        "closure/cargo-config.toml",
        "closure/cargo-metadata-closure.json",
        "closure/dependency-inventory.json",
        "closure/sbom.json",
        "closure/sysroot-inventory.json",
        "closure/toolchain-inventory.json",
        f"validator/{BINARY}",
    )
    if (
        sorted(entry.name for entry in os.scandir(output))
        != ["builder-report.json", "closure", "validator"]
        or sorted(entry.name for entry in os.scandir(output / "closure"))
        != sorted(
            PurePosixPath(path).name for path in files if path.startswith("closure/")
        )
        or sorted(entry.name for entry in os.scandir(output / "validator")) != [BINARY]
    ):
        _fail("driver output archive has an inexact final-V1 inventory")

    try:
        with tarfile.open(
            fileobj=sys.stdout.buffer,
            mode="w|",
            format=tarfile.USTAR_FORMAT,
        ) as archive:
            for name in ("output", "output/closure", "output/validator"):
                info = tarfile.TarInfo(name)
                info.type = tarfile.DIRTYPE
                info.mode = 0o700
                info.mtime = source_date_epoch
                info.uid = 0
                info.gid = 0
                info.uname = ""
                info.gname = ""
                archive.addfile(info)
            total = 0
            for relative in files:
                path = output / relative
                descriptor = os.open(
                    path,
                    os.O_RDONLY
                    | getattr(os, "O_CLOEXEC", 0)
                    | getattr(os, "O_NOFOLLOW", 0),
                )
                opened = os.fstat(descriptor)
                if (
                    not stat.S_ISREG(opened.st_mode)
                    or opened.st_nlink != 1
                    or not 0 < opened.st_size <= maximum_file_bytes
                ):
                    os.close(descriptor)
                    _fail("driver output archive contains an unsafe file")
                size = opened.st_size
                total += size
                if total > maximum_total_bytes:
                    os.close(descriptor)
                    _fail("driver output archive exceeds its signed aggregate limit")
                executable = bool(opened.st_mode & stat.S_IXUSR)
                info = tarfile.TarInfo(f"output/{relative}")
                info.type = tarfile.REGTYPE
                info.size = size
                info.mode = 0o700 if executable else 0o600
                info.mtime = source_date_epoch
                info.uid = 0
                info.gid = 0
                info.uname = ""
                info.gname = ""
                with os.fdopen(descriptor, "rb", closefd=True) as source:
                    archive.addfile(info, source)
                    if source.read(1):
                        _fail("driver output archive source grew while streaming")
                    after = os.fstat(source.fileno())
                    if (
                        opened.st_dev,
                        opened.st_ino,
                        opened.st_size,
                        opened.st_ctime_ns,
                    ) != (
                        after.st_dev,
                        after.st_ino,
                        after.st_size,
                        after.st_ctime_ns,
                    ):
                        _fail("driver output archive source changed while streaming")
        sys.stdout.buffer.flush()
    except (OSError, tarfile.TarError):
        _fail("driver could not stream its authenticated output archive")


def _parse() -> argparse.Namespace:
    parser = _Parser(add_help=False, allow_abbrev=False)
    parser.add_argument("--source-archive", required=True)
    parser.add_argument("--output-directory", required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--source-archive-sha256", required=True)
    parser.add_argument("--source-date-epoch", required=True, type=int)
    parser.add_argument("--builder-image", required=True)
    parser.add_argument("--policy-sha256", required=True)
    parser.add_argument("--driver-sha256", required=True)
    parser.add_argument("--python-path", required=True)
    parser.add_argument("--cargo-path", required=True)
    parser.add_argument("--rustc-path", required=True)
    parser.add_argument("--linker-path", required=True)
    parser.add_argument("--cargo-home", required=True)
    parser.add_argument("--max-inventory-files", required=True, type=int)
    parser.add_argument("--max-file-bytes", required=True, type=int)
    parser.add_argument("--max-total-bytes", required=True, type=int)
    parser.add_argument("--max-log-bytes", required=True, type=int)
    try:
        return parser.parse_args()
    except SystemExit:
        raise DriverError("driver command line has an invalid final-V1 shape") from None


def run(arguments: argparse.Namespace) -> None:
    _seal_driver_process()
    if not COMMIT_RE.fullmatch(arguments.source_commit):
        _fail("source commit is not one full lowercase Git object id")
    for value in (
        arguments.source_archive_sha256,
        arguments.policy_sha256,
        arguments.driver_sha256,
    ):
        if not HEX32_RE.fullmatch(value) or value == "00" * 32:
            _fail("driver received an invalid closure digest")
    if not 1 <= arguments.source_date_epoch <= 4_102_444_800:
        _fail("source date epoch is outside the final-V1 range")
    for value in (
        arguments.max_inventory_files,
        arguments.max_file_bytes,
        arguments.max_total_bytes,
        arguments.max_log_bytes,
    ):
        if type(value) is not int or value <= 0:
            _fail("driver received an invalid resource bound")
    if (
        arguments.max_inventory_files > 250_000
        or arguments.max_file_bytes > 2 * 1024**3
        or arguments.max_total_bytes > 64 * 1024**3
        or arguments.max_log_bytes > 64 * 1024**2
        or arguments.max_total_bytes < arguments.max_file_bytes
    ):
        _fail("driver resource bounds exceed the final-V1 hard ceiling")
    if Path(__file__) != DRIVER_IMAGE_PATH:
        _fail("driver is not executing from its image-owned final-V1 path")
    driver_hash, _, driver_executable = _hash_file(
        DRIVER_IMAGE_PATH,
        maximum=2 * 1024 * 1024,
    )
    if not driver_executable or driver_hash != arguments.driver_sha256:
        _fail("image-owned driver does not match the approved SHA-256 identity")
    source_archive = Path(arguments.source_archive)
    output = Path(arguments.output_directory)
    if source_archive != Path("/input/source.tar") or output != Path("/work/output"):
        _fail("driver paths do not match the isolated final-V1 mount contract")
    if os.path.lexists(output):
        _fail("driver output path must not pre-exist in isolated tmpfs")
    work = Path("/work")
    if not work.is_dir() or work.is_symlink():
        _fail("isolated work tmpfs is unavailable")
    authenticated_archive = work / "authenticated-source.tar"
    archive_descriptor, archive_size, archive_identity = _snapshot_source_archive(
        source_archive,
        authenticated_archive,
        expected_sha256=arguments.source_archive_sha256,
        maximum=min(arguments.max_total_bytes, 4 * 1024**3),
    )
    archive_hash = arguments.source_archive_sha256

    python = Path(arguments.python_path)
    cargo = Path(arguments.cargo_path)
    rustc = Path(arguments.rustc_path)
    linker = Path(arguments.linker_path)
    bootstrap_cargo_home = Path(arguments.cargo_home)
    image_paths = (python, cargo, rustc, linker, bootstrap_cargo_home)
    for path in image_paths:
        if not path.is_absolute() or not SAFE_PATH_RE.fullmatch(path.as_posix()):
            _fail("one pinned container path is not canonical")
        if (
            path == work
            or work in path.parents
            or path == Path("/input")
            or Path("/input") in path.parents
        ):
            _fail("one pinned container path overlaps mutable or mounted state")
    if any(
        left in right.parents or right in left.parents
        for index, left in enumerate(image_paths)
        for right in image_paths[index + 1 :]
    ):
        _fail("pinned container tool and cache paths overlap")
    if not bootstrap_cargo_home.is_dir() or bootstrap_cargo_home.is_symlink():
        _fail("pinned Cargo home is not one direct directory")

    try:
        source = _extract_source_archive(
            archive_descriptor,
            work,
            source_date_epoch=arguments.source_date_epoch,
            maximum_files=arguments.max_inventory_files,
            maximum_file_bytes=arguments.max_file_bytes,
            maximum_total_bytes=arguments.max_total_bytes,
        )
        _verify_open_source_archive(
            archive_descriptor,
            expected_sha256=archive_hash,
            expected_size=archive_size,
            identity=archive_identity,
        )
    finally:
        os.close(archive_descriptor)
    _validate_source_tree_inventory(
        source,
        source_commit=arguments.source_commit,
        maximum_files=arguments.max_inventory_files,
        maximum_file_bytes=arguments.max_file_bytes,
    )
    source_cargo_config_sha256 = _validate_source_cargo_configuration(source)
    for ancestor in (work, Path("/")):
        for forbidden in (
            ancestor / ".cargo" / "config",
            ancestor / ".cargo" / "config.toml",
        ):
            if os.path.lexists(forbidden):
                _fail(
                    "builder image injects an unapproved ancestor Cargo configuration"
                )
    vendor = work / "vendor"
    preflight = work / "preflight"
    preflight.mkdir(mode=0o700)
    preflight_target = preflight / "target"
    preflight_home = preflight / "home"
    cargo_home = preflight / "cargo-home"
    preflight_home.mkdir(mode=0o700)
    preflight_target.mkdir(mode=0o700)
    preflight_temporary = preflight_target / "tmp"
    preflight_temporary.mkdir(mode=0o700)
    _copy_regular_tree(
        bootstrap_cargo_home,
        cargo_home,
        maximum_files=arguments.max_inventory_files,
        maximum_file_bytes=arguments.max_file_bytes,
        maximum_total_bytes=arguments.max_total_bytes,
    )
    child_environment = {
        "HOME": preflight_home.as_posix(),
        "CARGO_HOME": cargo_home.as_posix(),
        "CARGO_TARGET_DIR": preflight_target.as_posix(),
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "CARGO_TERM_COLOR": "never",
        "CARGO_BUILD_JOBS": "1",
        "TMPDIR": preflight_temporary.as_posix(),
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "SOURCE_DATE_EPOCH": str(arguments.source_date_epoch),
        "RUST_BACKTRACE": "0",
        "RUSTC": rustc.as_posix(),
        "RUSTFLAGS": (
            "--remap-path-prefix=/work/source=. "
            "--remap-path-prefix=/work/vendor=vendor "
            "--remap-path-prefix=/work/preflight/target=target "
            "-C target-feature=+crt-static "
            "-C strip=symbols"
        ),
        f"CARGO_TARGET_{TARGET.upper().replace('-', '_')}_LINKER": linker.as_posix(),
        "PATH": "/usr/local/bin:/usr/bin:/bin",
    }

    python_version = _version(python, ("--version",), child_environment)
    cargo_version = _version(cargo, ("-Vv",), child_environment)
    rustc_version = _version(rustc, ("-Vv",), child_environment)
    linker_version = _version(linker, ("--version",), child_environment)
    sysroot_output, _ = _run(
        rustc,
        ("--print", "sysroot"),
        cwd=source,
        environment=child_environment,
        maximum_output_bytes=64 * 1024,
        label="pinned Rust sysroot query",
        quiesce_process_namespace=True,
    )
    try:
        sysroot_text = sysroot_output.decode("utf-8", "strict").strip()
    except UnicodeDecodeError:
        _fail("pinned Rust sysroot output is not UTF-8")
    sysroot = Path(sysroot_text)
    if (
        not sysroot.is_absolute()
        or not sysroot.is_dir()
        or sysroot.is_symlink()
        or sysroot == work
        or work in sysroot.parents
        or sysroot == Path("/input")
        or Path("/input") in sysroot.parents
        or any(
            sysroot in path.parents or path in sysroot.parents for path in image_paths
        )
    ):
        _fail("pinned Rust sysroot is not one direct absolute directory")

    vendor_output, _ = _run(
        cargo,
        (
            "vendor",
            "--locked",
            "--offline",
            "--versioned-dirs",
            vendor.as_posix(),
        ),
        cwd=source,
        environment=child_environment,
        maximum_output_bytes=arguments.max_log_bytes,
        label="offline Cargo vendor closure",
        quiesce_process_namespace=True,
    )
    try:
        vendor_config = vendor_output.decode("utf-8", "strict")
    except UnicodeDecodeError:
        _fail("Cargo vendor configuration is not UTF-8")
    if (
        not vendor_config.strip()
        or len(vendor_config.encode()) > arguments.max_log_bytes
    ):
        _fail("Cargo vendor did not emit one bounded source replacement")
    config_path = cargo_home / "config.toml"
    _write_new(config_path, vendor_output)

    metadata_arguments = (
        "metadata",
        "--locked",
        "--offline",
        "--format-version=1",
        "--filter-platform",
        TARGET,
        "--no-default-features",
        "--features",
        ",".join(f"{CRATE}/{feature}" for feature in FEATURES),
    )
    metadata_raw, _ = _run(
        cargo,
        metadata_arguments,
        cwd=source,
        environment=child_environment,
        maximum_output_bytes=MAX_JSON_BYTES,
        label="offline Cargo metadata closure",
        quiesce_process_namespace=True,
    )
    try:
        metadata_value = json.loads(metadata_raw.decode("utf-8", "strict"))
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError):
        _fail("Cargo metadata closure is not strict JSON")
    normalized_metadata = _normalize_metadata(
        metadata_value,
        (
            (source.as_posix(), "${SOURCE}"),
            (vendor.as_posix(), "${VENDOR}"),
            (preflight_target.as_posix(), "${TARGET}"),
            (cargo_home.as_posix(), "${CARGO_HOME}"),
            (sysroot.as_posix(), "${SYSROOT}"),
        ),
    )
    _validate_metadata_input_boundaries(normalized_metadata)
    sbom = _build_sbom(normalized_metadata)

    dependency_inventory = _scan_tree(
        vendor,
        prefix="vendor",
        maximum_files=arguments.max_inventory_files,
        maximum_file_bytes=arguments.max_file_bytes,
        maximum_total_bytes=arguments.max_total_bytes,
    )
    sysroot_inventory = _scan_tree(
        sysroot,
        prefix="sysroot",
        maximum_files=arguments.max_inventory_files,
        maximum_file_bytes=arguments.max_file_bytes,
        maximum_total_bytes=arguments.max_total_bytes,
    )
    driver_path = Path(__file__)
    toolchain_inventory: list[dict[str, Any]] = []
    for role, path in (
        ("builder-driver", driver_path),
        ("container-python", python),
        ("cargo", cargo),
        ("rustc", rustc),
        ("linker", linker),
    ):
        digest, size, executable = _hash_file(path, maximum=arguments.max_file_bytes)
        toolchain_inventory.append(
            {
                "role": role,
                "path": path.as_posix(),
                "sha256": digest,
                "size_bytes": size,
                "executable": executable,
            }
        )
    toolchain_inventory.sort(key=lambda entry: entry["role"])
    linker_identity = next(
        entry for entry in toolchain_inventory if entry["role"] == "linker"
    )
    toolchain_document = {
        "python_version": python_version,
        "cargo_version": cargo_version,
        "rustc_version": rustc_version,
        "linker_version": linker_version,
        "sysroot": "${SYSROOT}",
        "tools": toolchain_inventory,
    }
    build_arguments = (
        "build",
        "--release",
        "--locked",
        "--frozen",
        "--offline",
        "--no-default-features",
        "--features",
        ",".join(FEATURES),
        "-p",
        CRATE,
        "--bin",
        BINARY,
        "--jobs",
        "1",
        "--target",
        TARGET,
    )
    try:
        shutil.rmtree(preflight)
    except OSError:
        _fail("preflight validator build state could not be destroyed safely")
    if os.path.lexists(preflight):
        _fail("preflight validator build state remained visible")

    build_root = work / "build"
    target = build_root / "target"
    home = build_root / "home"
    build_cargo_home = build_root / "cargo-home"
    build_root.mkdir(mode=0o700)
    target.mkdir(mode=0o700)
    home.mkdir(mode=0o700)
    (target / "tmp").mkdir(mode=0o700)
    build_cargo_home.mkdir(mode=0o700)
    build_config_path = build_cargo_home / "config.toml"
    _write_new(build_config_path, vendor_output)
    build_environment = dict(child_environment)
    build_environment["HOME"] = home.as_posix()
    build_environment["CARGO_HOME"] = build_cargo_home.as_posix()
    build_environment["CARGO_TARGET_DIR"] = target.as_posix()
    build_environment["TMPDIR"] = (target / "tmp").as_posix()
    build_environment["RUSTFLAGS"] = (
        "--remap-path-prefix=/work/source=. "
        "--remap-path-prefix=/work/vendor=vendor "
        "--remap-path-prefix=/work/build/target=target "
        "-C target-feature=+crt-static "
        "-C strip=symbols"
    )
    build_recipe = {
        "program": cargo.as_posix(),
        "arguments": list(build_arguments),
        "working_directory": "${SOURCE}",
        "cargo_vendor_arguments": [
            "vendor",
            "--locked",
            "--offline",
            "--versioned-dirs",
            "${VENDOR}",
        ],
        "cargo_metadata_arguments": list(metadata_arguments),
        "cargo_config_sha256": hashlib.sha256(vendor_output).hexdigest(),
        "source_cargo_config_sha256": source_cargo_config_sha256,
        "driver_sha256": next(
            entry["sha256"]
            for entry in toolchain_inventory
            if entry["role"] == "builder-driver"
        ),
    }

    _run(
        cargo,
        build_arguments,
        cwd=source,
        environment=build_environment,
        maximum_output_bytes=arguments.max_log_bytes,
        label="network-isolated SCCP validator build",
        publish_for_host_scan=True,
        compile_sandbox_writable_roots=(target,),
        quiesce_process_namespace=True,
    )
    _validate_source_tree_inventory(
        source,
        source_commit=arguments.source_commit,
        maximum_files=arguments.max_inventory_files,
        maximum_file_bytes=arguments.max_file_bytes,
    )
    if (
        _scan_tree(
            vendor,
            prefix="vendor",
            maximum_files=arguments.max_inventory_files,
            maximum_file_bytes=arguments.max_file_bytes,
            maximum_total_bytes=arguments.max_total_bytes,
        )
        != dependency_inventory
    ):
        _fail("vendored dependency closure changed during the sandboxed build")
    if (
        _scan_tree(
            sysroot,
            prefix="sysroot",
            maximum_files=arguments.max_inventory_files,
            maximum_file_bytes=arguments.max_file_bytes,
            maximum_total_bytes=arguments.max_total_bytes,
        )
        != sysroot_inventory
    ):
        _fail("Rust sysroot closure changed during the sandboxed build")
    for entry in toolchain_inventory:
        path = Path(entry["path"])
        digest, size, executable = _hash_file(
            path,
            maximum=arguments.max_file_bytes,
        )
        if (
            digest != entry["sha256"]
            or size != entry["size_bytes"]
            or executable != entry["executable"]
        ):
            _fail("validator toolchain identity changed during the sandboxed build")
    config_hash, config_size, config_executable = _hash_file(
        build_config_path,
        maximum=arguments.max_log_bytes,
    )
    if (
        config_executable
        or config_size != len(vendor_output)
        or config_hash != hashlib.sha256(vendor_output).hexdigest()
    ):
        _fail("effective Cargo configuration changed during the sandboxed build")

    output.mkdir(mode=0o700)
    dependency_hash, dependency_size = _document(
        output, "dependency-inventory.json", dependency_inventory
    )
    metadata_hash, metadata_size = _document(
        output, "cargo-metadata-closure.json", normalized_metadata
    )
    sbom_hash, sbom_size = _document(output, "sbom.json", sbom)
    toolchain_hash, toolchain_size = _document(
        output, "toolchain-inventory.json", toolchain_document
    )
    sysroot_hash, sysroot_size = _document(
        output, "sysroot-inventory.json", sysroot_inventory
    )
    recipe_hash, recipe_size = _document(output, "build-recipe.json", build_recipe)
    environment_hash, environment_size = _document(
        output, "build-environment.json", build_environment
    )
    _write_new(output / "closure" / "cargo-config.toml", vendor_output)
    built = target / TARGET / BUILD_PROFILE / BINARY
    executable_hash, executable_size, executable = _hash_file(
        built, maximum=arguments.max_file_bytes
    )
    if not executable:
        _fail("SCCP validator output is not executable")
    validator_path = output / "validator" / BINARY
    _copy_new(
        built,
        validator_path,
        expected_sha256=executable_hash,
        expected_size=executable_size,
        maximum=arguments.max_file_bytes,
        executable=True,
    )

    report = {
        "schema": REPORT_SCHEMA,
        "policy_sha256": arguments.policy_sha256,
        "source_commit": arguments.source_commit,
        "source_archive_sha256": archive_hash,
        "source_archive_size_bytes": archive_size,
        "builder_image": arguments.builder_image,
        "platform": "linux/amd64",
        "target_triple": TARGET,
        "crate": CRATE,
        "binary": BINARY,
        "build_profile": BUILD_PROFILE,
        "enabled_features": list(FEATURES),
        "build_jobs": 1,
        "default_features": False,
        "cargo_locked": True,
        "cargo_frozen": True,
        "cargo_offline": True,
        "network_disabled": True,
        "dependency_inventory_sha256": dependency_hash,
        "dependency_inventory_size_bytes": dependency_size,
        "cargo_metadata_closure_sha256": metadata_hash,
        "cargo_metadata_closure_size_bytes": metadata_size,
        "sbom_sha256": sbom_hash,
        "sbom_size_bytes": sbom_size,
        "toolchain_inventory_sha256": toolchain_hash,
        "toolchain_inventory_size_bytes": toolchain_size,
        "sysroot_inventory_sha256": sysroot_hash,
        "sysroot_inventory_size_bytes": sysroot_size,
        "linker_sha256": linker_identity["sha256"],
        "build_recipe_sha256": recipe_hash,
        "build_recipe_size_bytes": recipe_size,
        "build_environment_sha256": environment_hash,
        "build_environment_size_bytes": environment_size,
        "executable_path": f"validator/{BINARY}",
        "executable_sha256": executable_hash,
        "executable_size_bytes": executable_size,
    }
    report_bytes = _canonical_json(report)
    if len(report_bytes) > MAX_JSON_BYTES:
        _fail("validator build report exceeds its final-V1 bound")
    _write_new(output / "builder-report.json", report_bytes)
    for directory in (output / "closure", output / "validator", output):
        try:
            descriptor = os.open(directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
            os.fsync(descriptor)
        finally:
            if "descriptor" in locals():
                os.close(descriptor)
                del descriptor
    _emit_output_archive(
        output,
        source_date_epoch=arguments.source_date_epoch,
        maximum_file_bytes=arguments.max_file_bytes,
        maximum_total_bytes=arguments.max_total_bytes,
    )


def main() -> int:
    try:
        run(_parse())
        return 0
    except DriverError as error:
        print(f"SCCP validator builder driver failed: {error}", file=sys.stderr)
        return 125
    except (OSError, ValueError, TypeError, UnicodeError):
        print("SCCP validator builder driver failed safely", file=sys.stderr)
        return 125


if __name__ == "__main__":
    raise SystemExit(main())
