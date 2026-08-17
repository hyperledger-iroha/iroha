#!/usr/bin/env python3
"""Install one Taira testnet binary and roll four live validators in place.

This is deliberately a testnet updater, not a release or reset controller.  It
keeps the existing launchd configuration, genesis, working directories, and
storage trees byte-for-byte/inode-for-inode and changes only the supervised
binary identity.  Peers restart one at a time and the already-updated peers are
rolled back in reverse order if any peer fails to become ready.

Provision this file once as the fixed root-owned executable
``/usr/local/libexec/iroha-taira-testnet-update-v1``.  CI must invoke that
installed copy; it must never execute a checkout copy as root.
"""

from __future__ import annotations

import argparse
import contextlib
import ctypes
import dataclasses
import fcntl
import grp
import hashlib
import json
import os
import plistlib
import pwd
import re
import signal
import stat
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections.abc import Callable, Iterator, Sequence
from pathlib import Path
from typing import NoReturn, Optional

PEER_COUNT = 4
LABELS = tuple(
    f"io.soramitsu.taira.validator-{index}" for index in range(1, PEER_COUNT + 1)
)
TORII_PORTS = tuple(29_080 + index for index in range(PEER_COUNT))
LAUNCH_DAEMONS = Path("/Library/LaunchDaemons")
INSTALL_ROOT = Path("/Library/SORA/Taira")
INSTALL_BINARY_ROOT = INSTALL_ROOT / "binaries"
DEPLOYMENT_LOCK = INSTALL_ROOT / "deploy-v21.lock"
INSTALLED_COMMAND = Path("/usr/local/libexec/iroha-taira-testnet-update-v1")
MAX_BINARY_BYTES = 2 * 1024 * 1024 * 1024
MAX_PLIST_BYTES = 1024 * 1024
MAX_CONFIG_BYTES = 8 * 1024 * 1024
MAX_HTTP_BYTES = 4 * 1024 * 1024
MAX_PROCESS_ARGUMENT_BYTES = 1024 * 1024
MAX_PROCESS_ARGUMENTS = 256
DARWIN_CTL_KERN = 1
DARWIN_KERN_PROCARGS2 = 49
DEFAULT_DEADLINE_SECONDS = 10 * 60
DEFAULT_HEALTH_TIMEOUT_SECONDS = 45
CONFIG_CHECK_TIMEOUT_SECONDS = 30
SYSTEM_COMMAND_TIMEOUT_SECONDS = 5
ROLLBACK_COMMAND_ALLOWANCE_SECONDS = 15
ROLLBACK_OVERHEAD_SECONDS = 30
MIN_FORWARD_ROLLOUT_SECONDS = 60
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
BINARY_STAT_OPTIONS = (
    "--binary-device",
    "--binary-inode",
    "--binary-size",
    "--binary-mtime-ns",
    "--binary-ctime-ns",
)
PRESERVED_OPTIONS = (
    "--config",
    "--config-sha256",
    "--workdir",
    "--workdir-device",
    "--workdir-inode",
    "--storage-dir",
    "--storage-device",
    "--storage-inode",
    "--pid-file",
    "--terminal-unhealthy-file",
)


class TestnetUpdateError(RuntimeError):
    """The state-preserving Taira testnet update could not complete."""


def fail(message: str) -> NoReturn:
    """Raise one redaction-safe update error."""

    raise TestnetUpdateError(message)


def file_identity(info: os.stat_result) -> tuple[int, ...]:
    """Return the stable fields used to detect file replacement."""

    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def directory_identity(info: os.stat_result) -> tuple[int, ...]:
    """Return the identity fields that remain stable while a store is active."""

    return (
        info.st_dev,
        info.st_ino,
        stat.S_IFMT(info.st_mode),
        info.st_uid,
        info.st_gid,
    )


def remaining_seconds(deadline: float, *, maximum: float) -> float:
    """Clamp one blocking operation to the remaining absolute budget."""

    remaining = deadline - time.monotonic()
    if remaining <= 0:
        fail("Taira testnet update exceeded its absolute deadline")
    return min(maximum, remaining)


def sleep_with_deadline(deadline: float, seconds: float) -> None:
    """Sleep without crossing the absolute update deadline."""

    time.sleep(min(seconds, remaining_seconds(deadline, maximum=seconds)))


def sha256_regular(
    path: Path, maximum: int, deadline: float | None = None
) -> tuple[str, os.stat_result]:
    """Hash one stable non-symlink, single-link regular file."""

    try:
        before = path.lstat()
    except OSError as error:
        raise TestnetUpdateError(f"required file is unavailable: {path}") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum
    ):
        fail(f"required file has an unsafe identity: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        opened = os.fstat(descriptor)
        if file_identity(opened) != file_identity(before):
            fail(f"required file changed while opening: {path}")
        digest = hashlib.sha256()
        total = 0
        while chunk := os.read(descriptor, 1024 * 1024):
            if deadline is not None:
                remaining_seconds(deadline, maximum=1)
            total += len(chunk)
            if total > maximum:
                fail(f"required file exceeds its size bound: {path}")
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if total != before.st_size or file_identity(after) != file_identity(before):
        fail(f"required file changed while hashing: {path}")
    return digest.hexdigest(), after


def read_regular(path: Path, maximum: int) -> tuple[bytes, os.stat_result]:
    """Read one stable bounded regular file."""

    digest, before = sha256_regular(path, maximum)
    del digest
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        body = bytearray()
        while len(body) <= maximum:
            chunk = os.read(descriptor, min(1024 * 1024, maximum + 1 - len(body)))
            if not chunk:
                break
            body.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if len(body) > maximum or file_identity(after) != file_identity(before):
        fail(f"required file changed while reading: {path}")
    return bytes(body), after


def required_option(arguments: Sequence[str], option: str, label: str) -> str:
    """Return one exact supervisor option value."""

    indices = [index for index, value in enumerate(arguments) if value == option]
    if (
        len(indices) != 1
        or indices[0] + 1 >= len(arguments)
        or arguments[indices[0] + 1].startswith("--")
    ):
        fail(f"{label} supervisor lacks one exact {option} argument")
    return arguments[indices[0] + 1]


@dataclasses.dataclass(frozen=True)
class ConfigSeal:
    """Immutable identity of one live validator configuration file."""

    path: Path
    sha256: str
    identity: tuple[int, ...]


@dataclasses.dataclass(frozen=True)
class DirectorySeal:
    """Stable inode identity of one active directory."""

    path: Path
    identity: tuple[int, ...]


@dataclasses.dataclass(frozen=True)
class PeerSnapshot:
    """Exact pre-update launchd and live-path state for one validator."""

    label: str
    port: int
    plist_path: Path
    plist_body: bytes
    plist_mode: int
    plist_uid: int
    plist_gid: int
    payload: dict[str, object]
    arguments: tuple[str, ...]
    runtime_uid: int
    runtime_gid: int
    config: ConfigSeal
    workdir: DirectorySeal
    storage: DirectorySeal


@dataclasses.dataclass(frozen=True)
class ProcessInfo:
    """Stable parent, owner, and native argv for one managed process."""

    pid: int
    ppid: int
    uid: int
    argv: tuple[str, ...]


def parse_darwin_procargs2(raw: bytes) -> tuple[str, ...]:
    """Parse one bounded Darwin ``KERN_PROCARGS2`` payload."""

    integer_size = ctypes.sizeof(ctypes.c_int)
    if len(raw) < integer_size or len(raw) > MAX_PROCESS_ARGUMENT_BYTES:
        fail("managed process argument payload has an invalid size")
    argc = ctypes.c_int.from_buffer_copy(raw[:integer_size]).value
    if argc < 1 or argc > MAX_PROCESS_ARGUMENTS:
        fail("managed process argument count is outside its bound")
    cursor = integer_size
    executable_end = raw.find(b"\0", cursor)
    if executable_end <= cursor:
        fail("managed process executable path is incomplete")
    try:
        executable = raw[cursor:executable_end].decode("utf-8")
    except UnicodeDecodeError as error:
        raise TestnetUpdateError("managed process executable is not UTF-8") from error
    cursor = executable_end + 1
    while cursor < len(raw) and raw[cursor] == 0:
        cursor += 1
    arguments: list[str] = []
    for _index in range(argc):
        argument_end = raw.find(b"\0", cursor)
        if argument_end < cursor:
            fail("managed process argument vector is incomplete")
        try:
            argument = raw[cursor:argument_end].decode("utf-8")
        except UnicodeDecodeError as error:
            raise TestnetUpdateError("managed process argument is not UTF-8") from error
        if not argument:
            fail("managed process argument vector contains an empty value")
        arguments.append(argument)
        cursor = argument_end + 1
    result = tuple(arguments)
    if result[0] != executable:
        fail("managed process executable differs from argv[0]")
    return result


def read_darwin_process_argv(pid: int) -> tuple[str, ...]:
    """Read one exact NUL-delimited process argv through Darwin sysctl."""

    if sys.platform != "darwin" or pid <= 1:
        fail(f"native process inspection is unavailable: pid {pid}")
    libc = ctypes.CDLL(None, use_errno=True)
    sysctl = libc.sysctl
    sysctl.argtypes = (
        ctypes.POINTER(ctypes.c_int),
        ctypes.c_uint,
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_size_t),
        ctypes.c_void_p,
        ctypes.c_size_t,
    )
    sysctl.restype = ctypes.c_int
    mib = (ctypes.c_int * 3)(DARWIN_CTL_KERN, DARWIN_KERN_PROCARGS2, pid)
    size = ctypes.c_size_t()
    if sysctl(mib, 3, None, ctypes.byref(size), None, 0) != 0:
        fail(f"could not size managed process arguments: pid {pid}")
    if (
        size.value < ctypes.sizeof(ctypes.c_int)
        or size.value > MAX_PROCESS_ARGUMENT_BYTES
    ):
        fail(f"managed process argument buffer is outside its bound: pid {pid}")
    buffer = ctypes.create_string_buffer(size.value)
    actual = ctypes.c_size_t(size.value)
    if (
        sysctl(
            mib,
            3,
            ctypes.cast(buffer, ctypes.c_void_p),
            ctypes.byref(actual),
            None,
            0,
        )
        != 0
    ):
        fail(f"could not read managed process arguments: pid {pid}")
    if actual.value > size.value:
        fail(f"managed process arguments grew during capture: pid {pid}")
    return parse_darwin_procargs2(buffer.raw[: actual.value])


def launchd_pid(record: str | None, label: str) -> int:
    """Extract one positive supervisor PID from a launchd record."""

    if record is None:
        fail(f"LaunchDaemon is not loaded: {label}")
    matches = re.findall(r"(?m)^\s*pid\s*=\s*([0-9]+)\s*$", record)
    if len(matches) != 1 or int(matches[0]) <= 1:
        fail(f"LaunchDaemon has no unique supervisor PID: {label}")
    return int(matches[0])


def parse_pid_file(path: Path, uid: int, gid: int) -> int:
    """Read one owner-private managed-child PID file."""

    body, info = read_regular(path, 64)
    if info.st_uid != uid or info.st_gid != gid or stat.S_IMODE(info.st_mode) & 0o077:
        fail(f"managed PID file has an unsafe owner or mode: {path}")
    try:
        text = body.decode("ascii")
    except UnicodeDecodeError as error:
        raise TestnetUpdateError(f"managed PID file is not ASCII: {path}") from error
    if re.fullmatch(r"[1-9][0-9]*\n", text) is None or int(text) <= 1:
        fail(f"managed PID file is invalid: {path}")
    return int(text)


def require_managed_processes(
    label: str,
    arguments: tuple[str, ...],
    runtime_uid: int,
    runtime_gid: int,
    ops: SystemOps,
    deadline: float,
) -> None:
    """Authenticate one launchd supervisor and its exact single child."""

    supervisor_pid = launchd_pid(ops.launchd_record(label, deadline), label)
    supervisor = ops.inspect_process(supervisor_pid, deadline)
    if (
        supervisor.ppid != 1
        or supervisor.uid != runtime_uid
        or supervisor.argv != arguments
    ):
        fail(f"{label} live supervisor differs from its plist")
    pid_file = _absolute_option(arguments, "--pid-file", label)
    child_pid = parse_pid_file(pid_file, runtime_uid, runtime_gid)
    child = ops.inspect_process(child_pid, deadline)
    expected_child = (
        required_option(arguments, "--binary", label),
        "--sora",
        "--config",
        required_option(arguments, "--config", label),
    )
    if (
        child.ppid != supervisor_pid
        or child.uid != runtime_uid
        or child.argv != expected_child
        or ops.child_pids(supervisor_pid, deadline) != (child_pid,)
    ):
        fail(f"{label} live validator differs from its supervised plist")
    if launchd_pid(ops.launchd_record(label, deadline), label) != supervisor_pid:
        fail(f"{label} supervisor changed during capture")


def _absolute_option(arguments: Sequence[str], option: str, label: str) -> Path:
    value = Path(required_option(arguments, option, label))
    if not value.is_absolute() or ".." in value.parts:
        fail(f"{label} {option} is not one canonical absolute path")
    return value


def _directory_seal(
    arguments: Sequence[str],
    option: str,
    device_option: str,
    inode_option: str,
    label: str,
) -> DirectorySeal:
    path = _absolute_option(arguments, option, label)
    try:
        info = path.lstat()
    except OSError as error:
        raise TestnetUpdateError(f"{label} live directory is unavailable") from error
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
        fail(f"{label} live path is not a non-symlink directory: {path}")
    expected_device = required_option(arguments, device_option, label)
    expected_inode = required_option(arguments, inode_option, label)
    if expected_device != str(info.st_dev) or expected_inode != str(info.st_ino):
        fail(f"{label} live directory identity differs from its supervisor")
    return DirectorySeal(path=path, identity=directory_identity(info))


def capture_peer(
    label: str, port: int, ops: SystemOps, deadline: float
) -> PeerSnapshot:
    """Capture one root-controlled plist and its production path identities."""

    plist_path = LAUNCH_DAEMONS / f"{label}.plist"
    body, info = read_regular(plist_path, MAX_PLIST_BYTES)
    if info.st_uid != 0 or info.st_mode & 0o022:
        fail(f"LaunchDaemon plist is not root-controlled: {plist_path}")
    try:
        payload = plistlib.loads(body)
    except Exception as error:
        raise TestnetUpdateError(f"invalid LaunchDaemon plist: {plist_path}") from error
    if not isinstance(payload, dict) or payload.get("Label") != label:
        fail(f"LaunchDaemon label differs: {plist_path}")
    raw_arguments = payload.get("ProgramArguments")
    if not isinstance(raw_arguments, list) or not all(
        isinstance(value, str) for value in raw_arguments
    ):
        fail(f"LaunchDaemon is not an explicit supervised job: {label}")
    arguments = tuple(raw_arguments)
    for option in ("--binary", "--binary-sha256", "--restart-generation"):
        required_option(arguments, option, label)
    for option in PRESERVED_OPTIONS:
        required_option(arguments, option, label)
    present_stat_options = [option in arguments for option in BINARY_STAT_OPTIONS]
    if any(present_stat_options) and not all(present_stat_options):
        fail(f"{label} supervisor has a partial binary stat seal")

    runtime_user = payload.get("UserName")
    runtime_group = payload.get("GroupName")
    if not isinstance(runtime_user, str) or not isinstance(runtime_group, str):
        fail(f"{label} LaunchDaemon omits its runtime identity")
    try:
        runtime_uid = pwd.getpwnam(runtime_user).pw_uid
        runtime_gid = grp.getgrnam(runtime_group).gr_gid
    except KeyError as error:
        raise TestnetUpdateError(f"{label} runtime identity is unknown") from error
    if runtime_uid <= 0 or runtime_gid <= 0:
        fail(f"{label} validator must not run as root")

    config_path = _absolute_option(arguments, "--config", label)
    config_sha, config_info = sha256_regular(config_path, MAX_CONFIG_BYTES)
    if config_sha != required_option(arguments, "--config-sha256", label):
        fail(f"{label} live config differs from its supervisor digest")
    workdir = _directory_seal(
        arguments,
        "--workdir",
        "--workdir-device",
        "--workdir-inode",
        label,
    )
    storage = _directory_seal(
        arguments,
        "--storage-dir",
        "--storage-device",
        "--storage-inode",
        label,
    )
    if payload.get("WorkingDirectory") != str(workdir.path):
        fail(f"{label} plist working directory differs from its supervisor")
    require_managed_processes(
        label,
        arguments,
        runtime_uid,
        runtime_gid,
        ops,
        deadline,
    )
    return PeerSnapshot(
        label=label,
        port=port,
        plist_path=plist_path,
        plist_body=body,
        plist_mode=stat.S_IMODE(info.st_mode),
        plist_uid=info.st_uid,
        plist_gid=info.st_gid,
        payload=payload,
        arguments=arguments,
        runtime_uid=runtime_uid,
        runtime_gid=runtime_gid,
        config=ConfigSeal(config_path, config_sha, file_identity(config_info)),
        workdir=workdir,
        storage=storage,
    )


def require_live_paths_unchanged(snapshots: Sequence[PeerSnapshot]) -> None:
    """Recheck configs and active directory inodes without reading store data."""

    for snapshot in snapshots:
        config_sha, config_info = sha256_regular(snapshot.config.path, MAX_CONFIG_BYTES)
        if (
            config_sha != snapshot.config.sha256
            or file_identity(config_info) != snapshot.config.identity
        ):
            fail(f"live validator config changed during update: {snapshot.label}")
        for name, seal in (
            ("working", snapshot.workdir),
            ("storage", snapshot.storage),
        ):
            try:
                info = seal.path.lstat()
            except OSError as error:
                raise TestnetUpdateError(
                    f"{snapshot.label} {name} directory disappeared during update"
                ) from error
            if directory_identity(info) != seal.identity:
                fail(
                    f"{snapshot.label} {name} directory identity changed during update"
                )


def rewrite_plist(
    snapshot: PeerSnapshot,
    installed_binary: Path,
    binary_sha256: str,
    binary_info: os.stat_result,
    source_commit: str,
) -> bytes:
    """Rewrite only the binary identity fields in one supervisor plist."""

    arguments = list(snapshot.arguments)

    def replace(option: str, value: str) -> None:
        index = arguments.index(option)
        arguments[index + 1] = value

    replace("--binary", str(installed_binary))
    replace("--binary-sha256", binary_sha256)
    for option in BINARY_STAT_OPTIONS:
        while option in arguments:
            index = arguments.index(option)
            del arguments[index : index + 2]
    digest_index = arguments.index("--binary-sha256")
    stat_values = (
        str(binary_info.st_dev),
        str(binary_info.st_ino),
        str(binary_info.st_size),
        str(binary_info.st_mtime_ns),
        str(binary_info.st_ctime_ns),
    )
    sealed_arguments: list[str] = []
    for option, value in zip(BINARY_STAT_OPTIONS, stat_values):
        sealed_arguments.extend((option, value))
    arguments[digest_index + 2 : digest_index + 2] = sealed_arguments
    generation = hashlib.sha256(
        b"iroha.taira.testnet-update.v1\0"
        + source_commit.encode("ascii")
        + b"\0"
        + binary_sha256.encode("ascii")
    ).hexdigest()
    replace("--restart-generation", generation)

    for option in PRESERVED_OPTIONS:
        if required_option(arguments, option, snapshot.label) != required_option(
            snapshot.arguments, option, snapshot.label
        ):
            fail(f"{snapshot.label} update attempted to change {option}")
    payload = dict(snapshot.payload)
    payload["ProgramArguments"] = arguments
    return plistlib.dumps(payload, fmt=plistlib.FMT_XML, sort_keys=True)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def ensure_root_directory(path: Path, mode: int) -> None:
    """Create or authenticate one root-owned non-writable directory."""

    if not path.exists():
        path.mkdir(mode=mode)
        os.chown(path, 0, 0)
        path.chmod(mode)
        _fsync_directory(path.parent)
    info = path.lstat()
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or info.st_uid != 0
        or info.st_gid != 0
        or stat.S_IMODE(info.st_mode) & 0o022
    ):
        fail(f"root-controlled directory identity differs: {path}")


def install_immutable(
    source: Path, expected_sha256: str, deadline: float
) -> tuple[Path, os.stat_result]:
    """Copy candidate bytes once into the root-owned content store."""

    ensure_root_directory(INSTALL_ROOT, 0o755)
    ensure_root_directory(INSTALL_BINARY_ROOT, 0o755)
    digest_root = INSTALL_BINARY_ROOT / expected_sha256
    ensure_root_directory(digest_root, 0o755)
    destination = digest_root / "iroha3d"
    if destination.exists() or destination.is_symlink():
        actual, info = sha256_regular(destination, MAX_BINARY_BYTES, deadline)
        if (
            actual != expected_sha256
            or info.st_uid != 0
            or info.st_gid != 0
            or stat.S_IMODE(info.st_mode) != 0o555
        ):
            fail("existing content-addressed validator binary identity differs")
        return destination, info

    temporary = digest_root / f".iroha3d.{os.getpid()}.tmp"
    source_fd = os.open(
        source,
        os.O_RDONLY
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NONBLOCK", 0),
    )
    output_fd = -1
    try:
        source_before = os.fstat(source_fd)
        if (
            not stat.S_ISREG(source_before.st_mode)
            or source_before.st_nlink != 1
            or source_before.st_size <= 0
            or source_before.st_size > MAX_BINARY_BYTES
            or not source_before.st_mode & 0o111
        ):
            fail("candidate validator binary has an unsafe opened identity")
        output_fd = os.open(
            temporary,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
            0o500,
        )
        digest = hashlib.sha256()
        total = 0
        while chunk := os.read(source_fd, 1024 * 1024):
            remaining_seconds(deadline, maximum=1)
            total += len(chunk)
            if total > MAX_BINARY_BYTES or total > source_before.st_size:
                fail("candidate validator binary exceeded its size bound")
            digest.update(chunk)
            view = memoryview(chunk)
            while view:
                written = os.write(output_fd, view)
                if written <= 0:
                    fail("short write while installing validator binary")
                view = view[written:]
        if total != source_before.st_size or file_identity(
            source_before
        ) != file_identity(os.fstat(source_fd)):
            fail("candidate validator binary changed while installing")
        if digest.hexdigest() != expected_sha256:
            fail("candidate validator binary digest changed while installing")
        os.fchown(output_fd, 0, 0)
        os.fchmod(output_fd, 0o555)
        os.fsync(output_fd)
        os.close(output_fd)
        output_fd = -1
        os.replace(temporary, destination)
        _fsync_directory(digest_root)
    finally:
        os.close(source_fd)
        if output_fd >= 0:
            os.close(output_fd)
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass
    actual, info = sha256_regular(destination, MAX_BINARY_BYTES, deadline)
    if (
        actual != expected_sha256
        or info.st_uid != 0
        or info.st_gid != 0
        or stat.S_IMODE(info.st_mode) != 0o555
    ):
        fail("installed validator binary identity differs")
    return destination, info


def atomic_replace_plist(snapshot: PeerSnapshot, body: bytes) -> None:
    """Atomically replace one plist while preserving its owner and mode."""

    path = snapshot.plist_path
    if path.is_symlink():
        fail(f"refusing to replace a LaunchDaemon symlink: {path}")
    temporary = path.parent / f".{path.name}.{os.getpid()}.tmp"
    descriptor = -1
    try:
        descriptor = os.open(
            temporary,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_CLOEXEC", 0),
            snapshot.plist_mode,
        )
        try:
            view = memoryview(body)
            while view:
                written = os.write(descriptor, view)
                if written <= 0:
                    fail("short write while replacing LaunchDaemon plist")
                view = view[written:]
            os.fchown(descriptor, snapshot.plist_uid, snapshot.plist_gid)
            os.fchmod(descriptor, snapshot.plist_mode)
            os.fsync(descriptor)
        finally:
            closing_descriptor = descriptor
            descriptor = -1
            os.close(closing_descriptor)
        os.replace(temporary, path)
        _fsync_directory(path.parent)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


class SystemOps:
    """Small injectable boundary around launchd operations."""

    def run(
        self, command: Sequence[str], deadline: float
    ) -> subprocess.CompletedProcess[str]:
        """Run one command within the absolute update deadline."""

        try:
            return subprocess.run(
                command,
                check=False,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                timeout=remaining_seconds(
                    deadline, maximum=SYSTEM_COMMAND_TIMEOUT_SECONDS
                ),
                env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
            )
        except subprocess.TimeoutExpired as error:
            raise TestnetUpdateError("bounded launchd command timed out") from error

    def launchd_record(self, label: str, deadline: float) -> str | None:
        """Return one system-domain launchd record when it is loaded."""

        result = self.run(["/bin/launchctl", "print", f"system/{label}"], deadline)
        return result.stdout if result.returncode == 0 else None

    def loaded(self, label: str, deadline: float) -> bool:
        """Return whether one exact system-domain job is loaded."""

        return self.launchd_record(label, deadline) is not None

    def inspect_process(self, pid: int, deadline: float) -> ProcessInfo:
        """Read a stable parent, UID, and native argv for one process."""

        def numeric_identity() -> tuple[int, int]:
            result = self.run(
                ["/bin/ps", "-p", str(pid), "-o", "ppid=", "-o", "uid="],
                deadline,
            )
            fields = result.stdout.split() if result.returncode == 0 else []
            if len(fields) != 2:
                fail(f"managed process is not running: pid {pid}")
            try:
                ppid, uid = (int(field) for field in fields)
            except ValueError as error:
                raise TestnetUpdateError(
                    f"could not parse managed process identity: pid {pid}"
                ) from error
            if ppid < 0 or uid < 0:
                fail(f"managed process identity is invalid: pid {pid}")
            return ppid, uid

        before = numeric_identity()
        argv_before = read_darwin_process_argv(pid)
        argv_after = read_darwin_process_argv(pid)
        after = numeric_identity()
        if before != after or argv_before != argv_after:
            fail(f"managed process changed during capture: pid {pid}")
        return ProcessInfo(pid=pid, ppid=before[0], uid=before[1], argv=argv_before)

    def child_pids(self, parent_pid: int, deadline: float) -> tuple[int, ...]:
        """Return the PIDs currently parented by one supervisor."""

        result = self.run(["/bin/ps", "-axo", "pid=", "-o", "ppid="], deadline)
        if result.returncode != 0:
            fail(f"could not inspect children of supervisor: pid {parent_pid}")
        children: list[int] = []
        for line in result.stdout.splitlines():
            fields = line.split()
            if len(fields) != 2:
                fail("could not parse managed process child inventory")
            try:
                pid, ppid = (int(field) for field in fields)
            except ValueError as error:
                raise TestnetUpdateError(
                    "could not parse managed process child inventory"
                ) from error
            if pid > 1 and ppid == parent_pid:
                children.append(pid)
        return tuple(sorted(children))

    def bootout(
        self, label: str, deadline: float, *, allow_absent: bool = False
    ) -> None:
        """Unload one exact system-domain job."""

        result = self.run(["/bin/launchctl", "bootout", f"system/{label}"], deadline)
        if result.returncode != 0 and not (
            allow_absent and not self.loaded(label, deadline)
        ):
            fail(f"launchd bootout failed for {label} (status {result.returncode})")

    def bootstrap(self, plist: Path, deadline: float) -> None:
        """Load one exact LaunchDaemon plist."""

        result = self.run(
            ["/bin/launchctl", "bootstrap", "system", str(plist)], deadline
        )
        if result.returncode != 0:
            fail(
                f"launchd bootstrap failed for {plist.stem} (status {result.returncode})"
            )


def _drop_privileges(uid: int, gid: int) -> Callable[[], None]:
    """Return the child-side runtime identity transition for config checks."""

    def drop() -> None:
        os.setgroups([])
        os.setgid(gid)
        os.setuid(uid)
        os.umask(0o077)

    return drop


def validate_configs(
    binary: Path, snapshots: Sequence[PeerSnapshot], deadline: float
) -> None:
    """Validate all four live configs concurrently with the candidate binary."""

    processes: list[tuple[PeerSnapshot, subprocess.Popen[bytes]]] = []
    check_deadline = min(deadline, time.monotonic() + CONFIG_CHECK_TIMEOUT_SECONDS)
    try:
        for snapshot in snapshots:
            process = subprocess.Popen(
                [
                    str(binary),
                    "--sora",
                    "--config",
                    str(snapshot.config.path),
                    "--check-config",
                ],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
                close_fds=True,
                preexec_fn=_drop_privileges(  # noqa: PLW1509
                    snapshot.runtime_uid, snapshot.runtime_gid
                ),
            )
            processes.append((snapshot, process))
        pending = {process: snapshot for snapshot, process in processes}
        while pending:
            for process, snapshot in tuple(pending.items()):
                return_code = process.poll()
                if return_code is None:
                    continue
                del pending[process]
                if return_code != 0:
                    fail(
                        "candidate binary rejected a live validator config "
                        f"(peer={snapshot.label}, status={return_code})"
                    )
            if pending:
                if time.monotonic() >= check_deadline:
                    fail("candidate config validation exceeded 30 seconds")
                sleep_with_deadline(check_deadline, 0.1)
    finally:
        for _snapshot, process in processes:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=2)


def _http_body(url: str, deadline: float) -> bytes:
    timeout = remaining_seconds(deadline, maximum=2)
    request = urllib.request.Request(url, headers={"Accept": "application/json"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        if response.status != 200:
            fail(f"readiness endpoint returned HTTP {response.status}: {url}")
        body = response.read(MAX_HTTP_BYTES + 1)
    if len(body) > MAX_HTTP_BYTES:
        fail(f"readiness endpoint exceeded its response bound: {url}")
    return body


def probe_peer(
    snapshot: PeerSnapshot, source_commit: str | None, deadline: float
) -> None:
    """Require local health, readiness, and optionally the exact new build."""

    root = f"http://127.0.0.1:{snapshot.port}"
    _http_body(f"{root}/health", deadline)
    _http_body(f"{root}/readyz", deadline)
    status_body = _http_body(f"{root}/status", deadline)
    try:
        status_payload = json.loads(status_body)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TestnetUpdateError(f"{snapshot.label} /status is invalid JSON") from error
    if not isinstance(status_payload, dict):
        fail(f"{snapshot.label} /status is not an object")
    if source_commit is None:
        return
    build = status_payload.get("build")
    published = build.get("git_commit_sha") if isinstance(build, dict) else None
    if published != source_commit:
        fail(f"{snapshot.label} reports build {published!r}, expected {source_commit}")


def wait_for_peer(
    snapshot: PeerSnapshot,
    source_commit: str | None,
    deadline: float,
    health_timeout_seconds: int,
) -> None:
    """Wait within one peer budget for local readiness."""

    peer_deadline = min(deadline, time.monotonic() + health_timeout_seconds)
    last_error: Exception | None = None
    while time.monotonic() < peer_deadline:
        try:
            probe_peer(snapshot, source_commit, peer_deadline)
            return
        except (TestnetUpdateError, OSError, urllib.error.URLError) as error:
            last_error = error
            sleep_with_deadline(peer_deadline, 0.5)
    raise TestnetUpdateError(f"{snapshot.label} did not become ready: {last_error}")


def verify_managed_peer(
    snapshot: PeerSnapshot,
    expected_plist: bytes,
    ops: SystemOps,
    deadline: float,
) -> None:
    """Require launchd to own the supervisor and child described by a plist."""

    try:
        payload = plistlib.loads(expected_plist)
    except Exception as error:
        raise TestnetUpdateError(
            f"expected LaunchDaemon plist is invalid: {snapshot.label}"
        ) from error
    arguments = payload.get("ProgramArguments") if isinstance(payload, dict) else None
    if (
        not isinstance(payload, dict)
        or payload.get("Label") != snapshot.label
        or not isinstance(arguments, list)
        or not all(isinstance(value, str) for value in arguments)
    ):
        fail(f"expected LaunchDaemon identity is invalid: {snapshot.label}")
    expected_arguments = tuple(arguments)
    if required_option(expected_arguments, "--config", snapshot.label) != str(
        snapshot.config.path
    ):
        fail(f"{snapshot.label} managed config changed during update")
    require_managed_processes(
        snapshot.label,
        expected_arguments,
        snapshot.runtime_uid,
        snapshot.runtime_gid,
        ops,
        deadline,
    )


def wait_for_managed_peer(
    snapshot: PeerSnapshot,
    expected_plist: bytes,
    source_commit: str | None,
    deadline: float,
    health_timeout_seconds: int,
    ops: SystemOps,
) -> None:
    """Wait for exact launchd ownership, child identity, and HTTP readiness."""

    peer_deadline = min(deadline, time.monotonic() + health_timeout_seconds)
    last_error: Exception | None = None
    while time.monotonic() < peer_deadline:
        try:
            verify_managed_peer(snapshot, expected_plist, ops, peer_deadline)
            probe_peer(snapshot, source_commit, peer_deadline)
            return
        except (TestnetUpdateError, OSError, urllib.error.URLError) as error:
            last_error = error
            sleep_with_deadline(peer_deadline, 0.5)
    raise TestnetUpdateError(
        f"{snapshot.label} did not return with its managed identity: {last_error}"
    )


@contextlib.contextmanager
def exclusive_deployment_lock() -> Iterator[None]:
    """Serialize routine updates with the existing reset controller."""

    ensure_root_directory(INSTALL_ROOT, 0o755)
    descriptor = os.open(
        DEPLOYMENT_LOCK,
        os.O_RDWR
        | os.O_CREAT
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        info = os.fstat(descriptor)
        if (
            not stat.S_ISREG(info.st_mode)
            or info.st_nlink != 1
            or info.st_uid != 0
            or info.st_gid != 0
            or stat.S_IMODE(info.st_mode) != 0o600
        ):
            fail("Taira deployment lock is not root-owned mode 0600")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise TestnetUpdateError("another Taira deployment is active") from error
        yield
    finally:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
        finally:
            os.close(descriptor)


PlistWriter = Callable[[PeerSnapshot, bytes], None]
PeerWaiter = Callable[[PeerSnapshot, bytes, Optional[str], float, int, SystemOps], None]
PathVerifier = Callable[[Sequence[PeerSnapshot]], None]


def roll_peers(
    snapshots: Sequence[PeerSnapshot],
    new_plists: dict[str, bytes],
    source_commit: str,
    deadline: float,
    health_timeout_seconds: int,
    *,
    ops: SystemOps,
    rollback_deadline: float | None = None,
    writer: PlistWriter = atomic_replace_plist,
    waiter: PeerWaiter = wait_for_managed_peer,
    verifier: PathVerifier = require_live_paths_unchanged,
) -> None:
    """Restart peers sequentially and reverse every touched peer on failure."""

    touched: list[PeerSnapshot] = []
    recovery_deadline = rollback_deadline or deadline
    try:
        for snapshot in snapshots:
            verifier(snapshots)
            if not ops.loaded(snapshot.label, deadline):
                fail(f"LaunchDaemon is not loaded: {snapshot.label}")
            current, _info = read_regular(snapshot.plist_path, MAX_PLIST_BYTES)
            if current != snapshot.plist_body:
                fail(f"LaunchDaemon changed before its update: {snapshot.label}")
            touched.append(snapshot)
            writer(snapshot, new_plists[snapshot.label])
            ops.bootout(snapshot.label, deadline)
            ops.bootstrap(snapshot.plist_path, deadline)
            waiter(
                snapshot,
                new_plists[snapshot.label],
                source_commit,
                deadline,
                health_timeout_seconds,
                ops,
            )
        verifier(snapshots)
        for snapshot in snapshots:
            waiter(
                snapshot,
                new_plists[snapshot.label],
                source_commit,
                deadline,
                health_timeout_seconds,
                ops,
            )
    except BaseException as update_error:
        rollback_errors: list[str] = []
        for snapshot in reversed(touched):
            peer_errors: list[str] = []
            try:
                ops.bootout(snapshot.label, recovery_deadline, allow_absent=True)
            except BaseException as rollback_error:  # noqa: BLE001
                peer_errors.append(f"bootout-{type(rollback_error).__name__}")
            plist_restored = False
            try:
                writer(snapshot, snapshot.plist_body)
            except BaseException as rollback_error:  # noqa: BLE001
                peer_errors.append(f"plist-{type(rollback_error).__name__}")
            else:
                plist_restored = True
            if plist_restored:
                try:
                    ops.bootstrap(snapshot.plist_path, recovery_deadline)
                except BaseException as rollback_error:  # noqa: BLE001
                    peer_errors.append(f"bootstrap-{type(rollback_error).__name__}")
                else:
                    try:
                        waiter(
                            snapshot,
                            snapshot.plist_body,
                            None,
                            recovery_deadline,
                            health_timeout_seconds,
                            ops,
                        )
                    except BaseException as rollback_error:  # noqa: BLE001
                        peer_errors.append(f"readiness-{type(rollback_error).__name__}")
            if peer_errors:
                rollback_errors.append(f"{snapshot.label}:{'+'.join(peer_errors)}")
        if rollback_errors:
            combined = TestnetUpdateError(
                "Taira update failed and rollback was incomplete: "
                + ", ".join(rollback_errors)
            )
            if hasattr(combined, "add_note"):
                combined.add_note(
                    f"original update failure: {type(update_error).__name__}: {update_error}"
                )
            raise combined from update_error
        raise


def require_installed_command() -> None:
    """Refuse privileged execution from a checkout or mutable helper path."""

    invoked = Path(sys.argv[0])
    try:
        resolved = invoked.resolve(strict=True)
    except OSError as error:
        raise TestnetUpdateError("could not resolve installed updater") from error
    if resolved != INSTALLED_COMMAND:
        fail(f"run the preinstalled updater at {INSTALLED_COMMAND}")
    _digest, info = sha256_regular(INSTALLED_COMMAND, MAX_PLIST_BYTES)
    if info.st_uid != 0 or info.st_gid != 0 or stat.S_IMODE(info.st_mode) != 0o555:
        fail("installed updater must be root-owned mode 0555")


def _raise_interrupted(signum: int, _frame: object) -> NoReturn:
    """Turn process termination into the normal rollback path."""

    try:
        name = signal.Signals(signum).name
    except ValueError:
        name = str(signum)
    fail(f"Taira testnet update was interrupted by {name}")


def install_interrupt_handlers() -> None:
    """Route cancellable process signals through rollback-aware exceptions."""

    for caught_signal in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
        signal.signal(caught_signal, _raise_interrupted)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse the deliberately small testnet update interface."""

    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--expected-sha256", required=True)
    parser.add_argument("--expected-source-commit", required=True)
    parser.add_argument(
        "--deadline-seconds", type=int, default=DEFAULT_DEADLINE_SECONDS
    )
    parser.add_argument(
        "--health-timeout-seconds",
        type=int,
        default=DEFAULT_HEALTH_TIMEOUT_SECONDS,
    )
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args(argv)
    if SHA256_RE.fullmatch(args.expected_sha256) is None:
        parser.error("--expected-sha256 must be one lowercase SHA-256 digest")
    if COMMIT_RE.fullmatch(args.expected_source_commit) is None:
        parser.error("--expected-source-commit must be one lowercase 40-hex commit")
    if not 1 <= args.deadline_seconds <= 15 * 60:
        parser.error("--deadline-seconds must be between 1 and 900")
    if not 1 <= args.health_timeout_seconds <= 120:
        parser.error("--health-timeout-seconds must be between 1 and 120")
    binary = Path(args.binary)
    if not binary.is_absolute() or ".." in binary.parts:
        parser.error("--binary must be one canonical absolute path")
    args.binary = binary
    return args


def rollback_reserve_seconds(health_timeout_seconds: int) -> int:
    """Reserve a bounded reverse rollout for all four touched peers."""

    return (
        PEER_COUNT * (health_timeout_seconds + ROLLBACK_COMMAND_ALLOWANCE_SECONDS)
        + ROLLBACK_OVERHEAD_SECONDS
    )


def execute(
    args: argparse.Namespace, *, ops: SystemOps | None = None
) -> dict[str, object]:
    """Preflight and optionally perform one state-preserving rolling update."""

    if os.geteuid() != 0:
        fail("Taira testnet update requires root")
    if sys.platform != "darwin":
        fail("Taira testnet update requires macOS")
    rollback_reserve = rollback_reserve_seconds(args.health_timeout_seconds)
    if args.apply and args.deadline_seconds < (
        rollback_reserve + MIN_FORWARD_ROLLOUT_SECONDS
    ):
        fail("update deadline is too short to reserve a four-peer rollback")
    deadline = time.monotonic() + args.deadline_seconds
    rollout_deadline = deadline - rollback_reserve
    actual_sha, binary_info = sha256_regular(args.binary, MAX_BINARY_BYTES, deadline)
    if actual_sha != args.expected_sha256 or not binary_info.st_mode & 0o111:
        fail("candidate validator binary identity differs")
    selected_ops = ops or SystemOps()
    with exclusive_deployment_lock():
        snapshots = tuple(
            capture_peer(label, port, selected_ops, deadline)
            for label, port in zip(LABELS, TORII_PORTS)
        )
        if (
            len(
                {(snapshot.runtime_uid, snapshot.runtime_gid) for snapshot in snapshots}
            )
            != 1
        ):
            fail("Taira validators do not share one runtime identity")
        require_live_paths_unchanged(snapshots)
        for snapshot in snapshots:
            if not selected_ops.loaded(snapshot.label, deadline):
                fail(f"LaunchDaemon is not loaded: {snapshot.label}")
            wait_for_peer(snapshot, None, deadline, args.health_timeout_seconds)
        installed_binary: Path | None = None
        installed_info: os.stat_result | None = None
        config_check_binary = args.binary
        if args.apply:
            installed_binary, installed_info = install_immutable(
                args.binary, args.expected_sha256, deadline
            )
            config_check_binary = installed_binary
        validate_configs(config_check_binary, snapshots, deadline)
        checked_sha, checked_info = sha256_regular(
            config_check_binary, MAX_BINARY_BYTES, deadline
        )
        expected_identity = installed_info if args.apply else binary_info
        assert expected_identity is not None
        if checked_sha != args.expected_sha256 or file_identity(
            checked_info
        ) != file_identity(expected_identity):
            fail("candidate validator binary changed during config validation")
        if not args.apply:
            return {
                "applied": False,
                "binary_sha256": actual_sha,
                "peers": list(LABELS),
                "source_commit": args.expected_source_commit,
            }
        assert installed_binary is not None
        assert installed_info is not None
        new_plists = {
            snapshot.label: rewrite_plist(
                snapshot,
                installed_binary,
                args.expected_sha256,
                installed_info,
                args.expected_source_commit,
            )
            for snapshot in snapshots
        }
        # Keep a fixed reverse-rollout budget untouched. A failed readiness
        # wait must not consume the time needed to restore all touched peers.
        if rollout_deadline <= time.monotonic():
            fail("insufficient update budget remains before the first restart")
        roll_peers(
            snapshots,
            new_plists,
            args.expected_source_commit,
            rollout_deadline,
            args.health_timeout_seconds,
            ops=selected_ops,
            rollback_deadline=deadline,
        )
        return {
            "applied": True,
            "binary": str(installed_binary),
            "binary_sha256": args.expected_sha256,
            "peers": list(LABELS),
            "source_commit": args.expected_source_commit,
        }


def main(argv: list[str] | None = None) -> int:
    """Run the fixed installed testnet updater."""

    try:
        require_installed_command()
        install_interrupt_handlers()
        args = parse_args(argv)
        result = execute(args)
        sys.stdout.write(
            json.dumps(result, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
            + "\n"
        )
        return 0
    except (TestnetUpdateError, OSError, subprocess.SubprocessError) as error:
        print(f"Taira testnet update failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
