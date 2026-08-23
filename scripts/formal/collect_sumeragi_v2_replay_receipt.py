#!/usr/bin/env python3
"""Run the sealed Sumeragi V2 replay corridor and collect one V1 receipt.

Prerequisites are canonical non-symlink files: pinned TLA2Tools 1.7.4, a
read-only two-file TLAPM projection, Java, and Python 3.9 or newer. Outputs are
create-only. Every child receives a closed environment, separate stdout and
stderr regular files, a new process group, a timeout, and process-group cleanup.

The collector emits the exact V1 detached-SSH release-signing payload. It never
reads a private key and cannot promote its own output. Release acceptance
requires an external OpenSSH SSHSIG over the canonical ``receipt.json`` bytes.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import signal
import stat
import subprocess
import sys
import time
from typing import Any, Iterable, Union

from sumeragi_v2_replay_signing import SIGNING_CONTRACT


SCHEMA_NAME = "iroha-sumeragi-v2-replay-receipt-v1"
TLA2TOOLS_VERSION = "1.7.4"
TLA2TOOLS_SHA256 = "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
TLAPM_COMMIT = "3ab43c7ff31db4ced850619d4746fa4c841a7681"
TLAPM_MODULES = {
    "Folds.tla": "aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da",
    "Functions.tla": "b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063",
}
SEED = 19349663
ARIL = 0
EXPECTED_STATES = 101
EXPECTED_ACTIONS = 100
TLC_MAX_SET_SIZE = 1000000
TIMEOUT_GRACE_SECONDS = 2.0
# The graph is literal source data. The collector builds from it and the
# independent checker extracts it with ``ast.literal_eval``; no obsolete graph
# is duplicated in a registry.
EVENT_TEMPLATES = {
    "formal-only": (
        ("standalone_sany", ()),
        ("raw_tlc", ("standalone_sany",)),
        ("normalizer", ("raw_tlc",)),
    ),
}
class CollectionError(RuntimeError):
    """The replay run or one of its sealed inputs is invalid."""


@dataclass(frozen=True)
class FileSnapshot:
    """Stable identity of one regular file."""

    path: Path
    logical_path: str
    sha256: str
    size_bytes: int
    mode: int
    device: int
    inode: int
    nlink: int
    mtime_ns: int
    ctime_ns: int

    def receipt_record(self) -> dict[str, Any]:
        return {
            "path": self.logical_path,
            "sha256": self.sha256,
            "size_bytes": self.size_bytes,
            "mode": self.mode,
            "nlink": self.nlink,
        }


@dataclass
class OwnedDirectory:
    """One private directory held open by the collector."""

    path: Path
    descriptor: int
    device: int
    inode: int


def canonical_json(value: Any) -> bytes:
    """Return the only accepted JSON representation."""

    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _absolute(path: Path) -> Path:
    return Path(os.path.abspath(path))


def _read_snapshot(
    path: Path,
    logical_path: str,
    *,
    executable: bool = False,
    require_single_link: bool = False,
    require_sealed: bool = False,
) -> FileSnapshot:
    """Read one canonical file without following a final symlink."""

    path = _absolute(path)
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise CollectionError(f"{logical_path} is unavailable") from error
    if resolved != path or stat.S_ISLNK(before.st_mode):
        raise CollectionError(f"{logical_path} must have a canonical non-symlink path")
    if not stat.S_ISREG(before.st_mode):
        raise CollectionError(f"{logical_path} must be a regular file")
    if executable and before.st_mode & 0o111 == 0:
        raise CollectionError(f"{logical_path} is not executable")
    if require_single_link and before.st_nlink != 1:
        raise CollectionError(f"{logical_path} must not be hard linked")
    if require_sealed and stat.S_IMODE(before.st_mode) & 0o222:
        raise CollectionError(f"{logical_path} must be read-only")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise CollectionError(f"{logical_path} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or (opened.st_dev, opened.st_ino) != (
            before.st_dev,
            before.st_ino,
        ):
            raise CollectionError(f"{logical_path} changed while opening")
        digest = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
            total += len(chunk)
        after = os.fstat(descriptor)
        stable_before = (
            opened.st_dev,
            opened.st_ino,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
            stat.S_IMODE(opened.st_mode),
            opened.st_nlink,
        )
        stable_after = (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
            stat.S_IMODE(after.st_mode),
            after.st_nlink,
        )
        if stable_after != stable_before or total != opened.st_size:
            raise CollectionError(f"{logical_path} changed while reading")
        try:
            linked = path.lstat()
        except OSError as error:
            raise CollectionError(f"{logical_path} changed while reading") from error
        if (
            stat.S_ISLNK(linked.st_mode)
            or (linked.st_dev, linked.st_ino) != (opened.st_dev, opened.st_ino)
        ):
            raise CollectionError(f"{logical_path} pathname changed while reading")
        return FileSnapshot(
            path,
            logical_path,
            digest.hexdigest(),
            total,
            stat.S_IMODE(opened.st_mode),
            opened.st_dev,
            opened.st_ino,
            opened.st_nlink,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _require_unchanged(snapshot: FileSnapshot, *, executable: bool = False) -> None:
    current = _read_snapshot(snapshot.path, snapshot.logical_path, executable=executable)
    if current != snapshot:
        raise CollectionError(f"{snapshot.logical_path} drifted during replay collection")


def _read_file_at(
    directory: OwnedDirectory,
    name: str,
    logical_path: str,
    *,
    require_single_link: bool = False,
) -> tuple[FileSnapshot, bytes]:
    """Read one file relative to a held private directory."""

    if not name or name in {".", ".."} or "/" in name or os.sep in name:
        raise CollectionError("owned file name is unsafe")
    try:
        before = os.stat(name, dir_fd=directory.descriptor, follow_symlinks=False)
    except OSError as error:
        raise CollectionError(f"{logical_path} is unavailable") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise CollectionError(f"{logical_path} must be one regular file")
    if require_single_link and before.st_nlink != 1:
        raise CollectionError(f"{logical_path} must not be hard linked")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, dir_fd=directory.descriptor)
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or not _same_inode(opened, before.st_dev, before.st_ino)
        ):
            raise CollectionError(f"{logical_path} changed while opening")
        chunks: list[bytes] = []
        digest = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
            digest.update(chunk)
            total += len(chunk)
        after = os.fstat(descriptor)
        stable_before = (
            opened.st_dev,
            opened.st_ino,
            opened.st_size,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
            stat.S_IMODE(opened.st_mode),
            opened.st_nlink,
        )
        stable_after = (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
            stat.S_IMODE(after.st_mode),
            after.st_nlink,
        )
        if stable_after != stable_before or total != opened.st_size:
            raise CollectionError(f"{logical_path} changed while reading")
        linked = os.stat(name, dir_fd=directory.descriptor, follow_symlinks=False)
        if stat.S_ISLNK(linked.st_mode) or not _same_inode(
            linked, opened.st_dev, opened.st_ino
        ):
            raise CollectionError(f"{logical_path} pathname changed while reading")
        snapshot = FileSnapshot(
            directory.path / name,
            logical_path,
            digest.hexdigest(),
            total,
            stat.S_IMODE(opened.st_mode),
            opened.st_dev,
            opened.st_ino,
            opened.st_nlink,
            opened.st_mtime_ns,
            opened.st_ctime_ns,
        )
        return snapshot, b"".join(chunks)
    finally:
        os.close(descriptor)


def _write_exclusive_at(
    directory: OwnedDirectory,
    name: str,
    logical_path: str,
    data: bytes,
    mode: int = 0o600,
) -> FileSnapshot:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, mode, dir_fd=directory.descriptor)
    except OSError as error:
        raise CollectionError(f"create-only output already exists: {logical_path}") from error
    try:
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    snapshot, _ = _read_file_at(
        directory, name, logical_path, require_single_link=True
    )
    return snapshot


def _directory_open_flags() -> int:
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _open_owned_child(parent: OwnedDirectory, name: str, path: Path) -> OwnedDirectory:
    if not name or name in {".", ".."} or "/" in name or os.sep in name:
        raise CollectionError("owned directory name is unsafe")
    try:
        before = os.stat(name, dir_fd=parent.descriptor, follow_symlinks=False)
        descriptor = os.open(name, _directory_open_flags(), dir_fd=parent.descriptor)
    except OSError as error:
        raise CollectionError(f"owned directory is unavailable: {path}") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISDIR(before.st_mode)
            or stat.S_ISLNK(before.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
            or opened.st_uid != os.geteuid()
            or stat.S_IMODE(opened.st_mode) != 0o700
        ):
            raise CollectionError(f"owned directory identity differs: {path}")
        return OwnedDirectory(path, descriptor, opened.st_dev, opened.st_ino)
    except BaseException:
        os.close(descriptor)
        raise


def _safe_output_root(path: Path) -> OwnedDirectory:
    path = _absolute(path)
    parent = path.parent
    if path == parent or not path.name:
        raise CollectionError("output root is unsafe")
    try:
        parent_metadata = parent.lstat()
    except OSError as error:
        raise CollectionError("output parent is unavailable") from error
    if (
        parent.resolve(strict=True) != parent
        or stat.S_ISLNK(parent_metadata.st_mode)
        or not stat.S_ISDIR(parent_metadata.st_mode)
    ):
        raise CollectionError("output parent must be one canonical directory")
    parent_descriptor = os.open(parent, _directory_open_flags())
    try:
        parent_opened = os.fstat(parent_descriptor)
        if (
            not stat.S_ISDIR(parent_opened.st_mode)
            or (parent_opened.st_dev, parent_opened.st_ino)
            != (parent_metadata.st_dev, parent_metadata.st_ino)
        ):
            raise CollectionError("output parent changed while opening")
        try:
            os.mkdir(path.name, 0o700, dir_fd=parent_descriptor)
        except FileExistsError as error:
            raise CollectionError("receipt output root is create-only") from error
        temporary_parent = OwnedDirectory(
            parent, parent_descriptor, parent_opened.st_dev, parent_opened.st_ino
        )
        return _open_owned_child(temporary_parent, path.name, path)
    finally:
        os.close(parent_descriptor)


def _create_owned_child(parent: OwnedDirectory, prefix: str) -> OwnedDirectory:
    for _ in range(128):
        name = f"{prefix}{secrets.token_hex(16)}"
        try:
            os.mkdir(name, 0o700, dir_fd=parent.descriptor)
        except FileExistsError:
            continue
        return _open_owned_child(parent, name, parent.path / name)
    raise CollectionError("could not reserve an owned runtime directory")


def _create_owned_named_child(parent: OwnedDirectory, name: str) -> OwnedDirectory:
    try:
        os.mkdir(name, 0o700, dir_fd=parent.descriptor)
    except OSError as error:
        raise CollectionError(f"create-only directory already exists: {name}") from error
    return _open_owned_child(parent, name, parent.path / name)


def _same_inode(metadata: os.stat_result, device: int, inode: int) -> bool:
    return (metadata.st_dev, metadata.st_ino) == (device, inode)


def _require_owned_entry(parent: OwnedDirectory, child: OwnedDirectory) -> None:
    try:
        current = os.stat(
            child.path.name, dir_fd=parent.descriptor, follow_symlinks=False
        )
    except OSError as error:
        raise CollectionError("owned cleanup entry is unavailable") from error
    opened = os.fstat(child.descriptor)
    if (
        not stat.S_ISDIR(current.st_mode)
        or stat.S_ISLNK(current.st_mode)
        or not _same_inode(current, child.device, child.inode)
        or not _same_inode(opened, child.device, child.inode)
        or current.st_uid != os.geteuid()
        or stat.S_IMODE(current.st_mode) != 0o700
    ):
        raise CollectionError("owned cleanup entry was renamed or replaced")


def _require_owned_path(directory: OwnedDirectory) -> None:
    try:
        linked = directory.path.lstat()
    except OSError as error:
        raise CollectionError("owned directory pathname is unavailable") from error
    opened = os.fstat(directory.descriptor)
    if (
        stat.S_ISLNK(linked.st_mode)
        or not stat.S_ISDIR(linked.st_mode)
        or not _same_inode(linked, directory.device, directory.inode)
        or not _same_inode(opened, directory.device, directory.inode)
        or linked.st_uid != os.geteuid()
        or stat.S_IMODE(linked.st_mode) != 0o700
    ):
        raise CollectionError("owned directory pathname was renamed or replaced")


def _remove_open_directory_contents(descriptor: int) -> None:
    for name in os.listdir(descriptor):
        try:
            before = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
        except OSError as error:
            raise CollectionError("owned cleanup entry disappeared") from error
        if stat.S_ISDIR(before.st_mode) and not stat.S_ISLNK(before.st_mode):
            try:
                child_descriptor = os.open(name, _directory_open_flags(), dir_fd=descriptor)
            except OSError as error:
                raise CollectionError("owned cleanup directory could not be opened") from error
            try:
                opened = os.fstat(child_descriptor)
                if not _same_inode(opened, before.st_dev, before.st_ino):
                    raise CollectionError("owned cleanup directory was replaced")
                _remove_open_directory_contents(child_descriptor)
            finally:
                os.close(child_descriptor)
            current = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
            if not _same_inode(current, before.st_dev, before.st_ino):
                raise CollectionError("owned cleanup directory changed")
            os.rmdir(name, dir_fd=descriptor)
        else:
            current = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
            if not _same_inode(current, before.st_dev, before.st_ino):
                raise CollectionError("owned cleanup file changed")
            os.unlink(name, dir_fd=descriptor)


def _remove_owned_child(parent: OwnedDirectory, child: OwnedDirectory) -> None:
    """Delete only a still-linked child whose held descriptor proves ownership."""

    _require_owned_entry(parent, child)
    _remove_open_directory_contents(child.descriptor)
    _require_owned_entry(parent, child)
    os.close(child.descriptor)
    child.descriptor = -1
    os.rmdir(child.path.name, dir_fd=parent.descriptor)


def remove_owned_directory_path(path: Path, device: int, inode: int) -> None:
    """Safely remove a private directory captured earlier by device and inode."""

    path = _absolute(path)
    parent_path = path.parent
    if path == parent_path or not path.name or parent_path.resolve(strict=True) != parent_path:
        raise CollectionError("refusing unsafe owned cleanup target")
    parent_descriptor = os.open(parent_path, _directory_open_flags())
    parent_metadata = os.fstat(parent_descriptor)
    parent = OwnedDirectory(
        parent_path, parent_descriptor, parent_metadata.st_dev, parent_metadata.st_ino
    )
    child: Union[OwnedDirectory, None] = None
    try:
        child = _open_owned_child(parent, path.name, path)
        if (child.device, child.inode) != (device, inode):
            raise CollectionError("owned cleanup target was renamed or replaced")
        _remove_owned_child(parent, child)
    finally:
        if child is not None and child.descriptor >= 0:
            os.close(child.descriptor)
        os.close(parent.descriptor)


def _tla_dependencies(path: Path) -> list[str]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise CollectionError(f"could not read TLA module {path.name}") from error
    collected: list[str] = []
    active = False
    for line in lines[:80]:
        if line.startswith("EXTENDS "):
            active = True
            collected.append(line[len("EXTENDS ") :])
        elif active and (line.startswith(" ") or line.startswith("\t")):
            collected.append(line)
        elif active:
            break
    return re.findall(r"[A-Za-z][A-Za-z0-9_]*", " ".join(collected), re.ASCII)


def tla_source_closure(formal_dir: Path) -> list[Path]:
    """Derive the local witness closure from the final TLA sources."""

    pending = ["SumeragiV2TraceWitness"]
    visited: set[str] = set()
    paths: list[Path] = []
    while pending:
        module = pending.pop()
        if module in visited:
            continue
        visited.add(module)
        path = formal_dir / f"{module}.tla"
        if not path.is_file():
            continue
        paths.append(path)
        for dependency in reversed(_tla_dependencies(path)):
            if (formal_dir / f"{dependency}.tla").is_file():
                pending.append(dependency)
    if not paths or paths[0].name != "SumeragiV2TraceWitness.tla":
        raise CollectionError("could not derive the replay witness source closure")
    return sorted(paths)


def _source_paths(root: Path) -> list[Path]:
    formal_dir = root / "formal/sumeragi_v2"
    relative = [
        "Cargo.lock",
        "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.cfg",
        "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv",
        "scripts/normalize_sumeragi_v2_tlc_trace.py",
        "scripts/formal/check_sumeragi_v2_replay_receipt.py",
        "scripts/formal/check_sumeragi_v2_replay_trace.sh",
        "scripts/formal/collect_sumeragi_v2_replay_receipt.py",
        "scripts/formal/finalize_sumeragi_v2_replay_receipt.py",
        "scripts/formal/resolve_java.sh",
        "scripts/formal/sumeragi_v2_replay_receipt_v1.schema.json",
        "scripts/formal/sumeragi_v2_replay_signing.py",
        "scripts/formal/sumeragi_v2_tlc_result_contract.sh",
        "scripts/formal/verify_sumeragi_v2_replay_release.py",
    ]
    return sorted({root / item for item in relative} | set(tla_source_closure(formal_dir)))


def _manifest(records: Iterable[FileSnapshot]) -> str:
    return sha256_bytes(
        canonical_json([record.receipt_record() for record in sorted(records, key=lambda item: item.logical_path)])
    )


def build_event_graph(mode: str) -> dict[str, Any]:
    """Build the event graph from the literal runner templates above."""

    templates = list(EVENT_TEMPLATES[mode])
    return {
        "nodes": [name for name, _ in templates],
        "edges": [
            {"from": dependency, "to": name}
            for name, dependencies in templates
            for dependency in dependencies
        ],
    }


def _process_group_exists(process_group: int) -> bool:
    try:
        os.killpg(process_group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def _terminate_group(
    process_group: int, leader: Union[subprocess.Popen[bytes], None] = None
) -> tuple[bool, bool]:
    term_sent = False
    kill_sent = False
    if _process_group_exists(process_group):
        try:
            os.killpg(process_group, signal.SIGTERM)
            term_sent = True
        except ProcessLookupError:
            return False, False
        if leader is not None:
            try:
                leader.wait(timeout=TIMEOUT_GRACE_SECONDS)
            except subprocess.TimeoutExpired:
                pass
        deadline = time.monotonic() + TIMEOUT_GRACE_SECONDS
        while _process_group_exists(process_group) and time.monotonic() < deadline:
            time.sleep(0.02)
        if _process_group_exists(process_group):
            try:
                os.killpg(process_group, signal.SIGKILL)
                kill_sent = True
            except (ProcessLookupError, PermissionError):
                # A dead but not-yet-reaped group can report EPERM on Darwin.
                # The final quiescence check remains authoritative.
                pass
            deadline = time.monotonic() + TIMEOUT_GRACE_SECONDS
            while _process_group_exists(process_group) and time.monotonic() < deadline:
                time.sleep(0.02)
    return term_sent, kill_sent


def _open_event_output(directory: OwnedDirectory, name: str) -> int:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return os.open(name, flags, 0o600, dir_fd=directory.descriptor)


def run_process_event(
    *,
    name: str,
    argv: list[str],
    cwd: Path,
    environment: dict[str, str],
    expected_status: int,
    timeout_seconds: float,
    events_directory: OwnedDirectory,
    sequence: int,
) -> dict[str, Any]:
    """Run one bounded process and return its complete receipt event."""

    stdout_relative = f"events/{sequence:02d}-{name}.stdout"
    stderr_relative = f"events/{sequence:02d}-{name}.stderr"
    stdout_name = f"{sequence:02d}-{name}.stdout"
    stderr_name = f"{sequence:02d}-{name}.stderr"
    stdout_fd = _open_event_output(events_directory, stdout_name)
    try:
        stderr_fd = _open_event_output(events_directory, stderr_name)
    except BaseException:
        os.close(stdout_fd)
        raise
    start_ns = time.monotonic_ns()
    timed_out = False
    term_sent = False
    kill_sent = False
    post_exit_group_members = False
    process: Union[subprocess.Popen[bytes], None] = None
    try:
        process = subprocess.Popen(
            argv,
            cwd=str(cwd),
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout_fd,
            stderr=stderr_fd,
            close_fds=True,
            start_new_session=True,
        )
        try:
            status = process.wait(timeout=timeout_seconds)
        except subprocess.TimeoutExpired:
            timed_out = True
            term_sent, kill_sent = _terminate_group(process.pid, process)
            try:
                status = process.wait(timeout=TIMEOUT_GRACE_SECONDS)
            except subprocess.TimeoutExpired:
                if _process_group_exists(process.pid):
                    os.killpg(process.pid, signal.SIGKILL)
                    kill_sent = True
                status = process.wait()
        if _process_group_exists(process.pid):
            post_exit_group_members = True
            extra_term, extra_kill = _terminate_group(process.pid)
            term_sent = term_sent or extra_term
            kill_sent = kill_sent or extra_kill
        quiescent = not _process_group_exists(process.pid)
    finally:
        os.close(stdout_fd)
        os.close(stderr_fd)
    end_ns = time.monotonic_ns()
    stdout, _ = _read_file_at(
        events_directory, stdout_name, stdout_relative, require_single_link=True
    )
    stderr, _ = _read_file_at(
        events_directory, stderr_name, stderr_relative, require_single_link=True
    )
    return {
        "name": name,
        "argv": argv,
        "cwd": str(cwd),
        "environment": environment,
        "descriptors": {
            "stdin": {"fd": 0, "kind": "null", "path": "/dev/null"},
            "stdout": {"fd": 1, "kind": "create-only-regular-file", "artifact": stdout_relative},
            "stderr": {"fd": 2, "kind": "create-only-regular-file", "artifact": stderr_relative},
            "close_fds": True,
            "new_session": True,
        },
        "status": {
            "actual": status,
            "expected": expected_status,
            "matched": status == expected_status,
        },
        "timeout": {
            "seconds": timeout_seconds,
            "occurred": timed_out,
            "grace_seconds": TIMEOUT_GRACE_SECONDS,
            "sigterm_sent": term_sent,
            "sigkill_sent": kill_sent,
        },
        "cleanup": {
            "process_group": process.pid if process is not None else None,
            "scope": "new-session-process-group",
            "post_exit_group_members_observed": post_exit_group_members,
            "process_group_quiescent": quiescent if process is not None else False,
        },
        "duration_monotonic_ns": end_ns - start_ns,
        "outputs": {"stdout": stdout.receipt_record(), "stderr": stderr.receipt_record()},
    }


def _event_succeeded(event: dict[str, Any]) -> bool:
    return bool(
        event["status"]["matched"]
        and not event["timeout"]["occurred"]
        and event["cleanup"]["process_group_quiescent"]
        and not event["cleanup"]["post_exit_group_members_observed"]
    )


def _read_event_output(
    events_directory: OwnedDirectory, event: dict[str, Any], stream: str
) -> bytes:
    logical_path = event["outputs"][stream]["path"]
    _, data = _read_file_at(
        events_directory,
        Path(logical_path).name,
        logical_path,
        require_single_link=True,
    )
    return data


def _validate_projection(path: Path) -> tuple[Path, list[FileSnapshot]]:
    path = _absolute(path)
    try:
        if path.resolve(strict=True) != path or path.is_symlink():
            raise CollectionError("TLAPM projection must be canonical and non-symlinked")
        metadata = path.stat()
    except OSError as error:
        raise CollectionError("TLAPM projection is unavailable") from error
    if not stat.S_ISDIR(metadata.st_mode) or stat.S_IMODE(metadata.st_mode) & 0o222:
        raise CollectionError("TLAPM projection must be a sealed read-only directory")
    entries = sorted(item.name for item in path.iterdir())
    if entries != sorted(TLAPM_MODULES):
        raise CollectionError("TLAPM projection must contain exactly Functions.tla and Folds.tla")
    snapshots = []
    for name, expected_hash in sorted(TLAPM_MODULES.items()):
        snapshot = _read_snapshot(
            path / name,
            f"tlapm-projection/{name}",
            require_single_link=True,
            require_sealed=True,
        )
        if snapshot.sha256 != expected_hash:
            raise CollectionError(f"pinned TLAPM {name} checksum differs")
        snapshots.append(snapshot)
    return path, snapshots


def _artifact_inventory(
    events_directory: OwnedDirectory, relative_paths: Iterable[str]
) -> list[dict[str, Any]]:
    records = []
    for relative in sorted(set(relative_paths)):
        path = Path(relative)
        if path.parent != Path("events"):
            raise CollectionError("artifact escaped the events directory")
        snapshot, _ = _read_file_at(
            events_directory, path.name, relative, require_single_link=True
        )
        records.append(snapshot.receipt_record())
    return records


def collect(args: argparse.Namespace) -> Path:
    root = _absolute(args.root)
    if root.resolve(strict=True) != root or not (root / ".git").exists():
        raise CollectionError("root must be the canonical repository worktree")
    output_directory = _safe_output_root(args.output_root)
    output_root = output_directory.path
    events_directory: Union[OwnedDirectory, None] = None
    runtime_directory: Union[OwnedDirectory, None] = None
    try:
        _require_owned_path(output_directory)
        events_directory = _create_owned_named_child(output_directory, "events")
        formal_dir = root / "formal/sumeragi_v2"
        fixture_dir = root / "crates/iroha_sumeragi_core/tests/fixtures"
        expected_trace = fixture_dir / "tlc_replay_witness.tsv"
        config = fixture_dir / "tlc_replay_witness.cfg"
        normalizer = root / "scripts/normalize_sumeragi_v2_tlc_trace.py"

        java = _read_snapshot(args.java_bin, "tool/java", executable=True)
        python = _read_snapshot(args.python_bin, "tool/python", executable=True)
        jar = _read_snapshot(
            args.tla2tools_jar,
            "tool/tla2tools.jar",
            require_single_link=True,
        )
        if jar.sha256 != TLA2TOOLS_SHA256:
            raise CollectionError(f"TLA2Tools {TLA2TOOLS_VERSION} checksum differs")
        projection, projection_files = _validate_projection(args.tlapm_projection)
        source_snapshots = [
            _read_snapshot(path, str(path.relative_to(root)), require_single_link=True)
            for path in _source_paths(root)
        ]
        tool_snapshots = [java, python, jar, *projection_files]

        runtime_directory = _create_owned_child(
            output_directory, ".sumeragi-v2-replay-runtime."
        )
        runtime_root = runtime_directory.path
        runtime_tmp = runtime_root / "tmp"
        os.mkdir("tmp", 0o700, dir_fd=runtime_directory.descriptor)
        environment = {
            "LANG": "C",
            "LC_ALL": "C",
            "TMPDIR": str(runtime_tmp),
            "TZ": "UTC",
        }
        common_java = [
            str(java.path),
            f"-DTLA-Library={projection}",
            "-cp",
            str(jar.path),
        ]
        plans: list[tuple[str, list[str], Path, int]] = [
            (
                "standalone_sany",
                [*common_java, "tla2sany.SANY", str(formal_dir / "SumeragiV2TraceWitness.tla")],
                formal_dir,
                0,
            ),
            (
                "raw_tlc",
                [
                    str(java.path),
                    "-XX:+UseParallelGC",
                    f"-DTLA-Library={projection}",
                    "-cp",
                    str(jar.path),
                    "tlc2.TLC",
                    "-maxSetSize",
                    str(TLC_MAX_SET_SIZE),
                    "-metadir",
                    str(runtime_root / "states"),
                    "-workers",
                    "1",
                    "-depth",
                    "500",
                    "-seed",
                    str(SEED),
                    "-aril",
                    str(ARIL),
                    "-simulate",
                    "num=200",
                    "-config",
                    str(config),
                    "-tool",
                    "SumeragiV2TraceWitness",
                ],
                formal_dir,
                12,
            ),
        ]
        raw_tlc_path = output_root / "events/02-raw_tlc.stdout"
        normalized_path = output_root / "events/03-normalizer.stdout"
        plans.append(
            (
                "normalizer",
                [
                    str(python.path), "-B", "-I", "-S", str(normalizer),
                    str(raw_tlc_path), "--seed", str(SEED), "--aril", str(ARIL),
                ],
                root,
                0,
            )
        )
        events: list[dict[str, Any]] = []
        for sequence, (name, argv, cwd, status) in enumerate(plans, 1):
            _require_owned_path(output_directory)
            _require_owned_entry(output_directory, events_directory)
            _require_owned_entry(output_directory, runtime_directory)
            event = run_process_event(
                name=name,
                argv=argv,
                cwd=cwd,
                environment=environment,
                expected_status=status,
                timeout_seconds=args.timeout_seconds,
                events_directory=events_directory,
                sequence=sequence,
            )
            _require_owned_path(output_directory)
            _require_owned_entry(output_directory, events_directory)
            _require_owned_entry(output_directory, runtime_directory)
            events.append(event)
            if not _event_succeeded(event):
                raise CollectionError(f"{name} did not satisfy its status/timeout/cleanup contract")
            if _read_event_output(events_directory, event, "stderr"):
                raise CollectionError(f"{name} emitted separate stderr")
            if name == "normalizer" and _read_event_output(
                events_directory, event, "stdout"
            ) != expected_trace.read_bytes():
                raise CollectionError("normalized replay TSV differs byte-for-byte from the fixture")
        normalized_event = next(event for event in events if event["name"] == "normalizer")
        raw_event = next(event for event in events if event["name"] == "raw_tlc")
        normalized_bytes = _read_event_output(
            events_directory, normalized_event, "stdout"
        )
        action_count = sum(
            1 for line in normalized_bytes.decode("utf-8").splitlines() if re.match(r"^[0-9]+\t", line)
        )
        raw_bytes = _read_event_output(events_directory, raw_event, "stdout")
        state_count = len(re.findall(rb"^@!@!@STARTMSG 2217:4 @!@!@$", raw_bytes, re.MULTILINE))
        if state_count != EXPECTED_STATES or action_count != EXPECTED_ACTIONS:
            raise CollectionError(
                f"replay counts differ: states={state_count}, actions={action_count}"
            )

        signing = dict(SIGNING_CONTRACT)

        for snapshot in source_snapshots:
            _require_unchanged(snapshot)
        for snapshot in tool_snapshots:
            _require_unchanged(snapshot, executable=snapshot.logical_path in {"tool/java", "tool/python"})

        artifact_paths = []
        for event in events:
            artifact_paths.extend(
                (event["outputs"]["stdout"]["path"], event["outputs"]["stderr"]["path"])
            )
        inventory = _artifact_inventory(events_directory, artifact_paths)
        receipt = {
            "schema": SCHEMA_NAME,
            "schema_version": 1,
            "evidence_class": "release-receipt",
            "mode": args.mode,
            "runner": {
                "path": "scripts/formal/collect_sumeragi_v2_replay_receipt.py",
                "sha256": next(
                    item.sha256
                    for item in source_snapshots
                    if item.logical_path == "scripts/formal/collect_sumeragi_v2_replay_receipt.py"
                ),
                "event_graph": build_event_graph(args.mode),
            },
            "invocation": {
                "argv": [str(item) for item in sys.argv],
                "cwd": str(Path.cwd()),
                "environment": environment,
                "timeout_seconds": args.timeout_seconds,
                "runtime_root": str(runtime_root),
            },
            "source_identity": {
                "root": str(root),
                "files": [
                    item.receipt_record()
                    for item in sorted(source_snapshots, key=lambda value: value.logical_path)
                ],
                "manifest_sha256": _manifest(source_snapshots),
            },
            "tool_identity": {
                "tla2tools_version": TLA2TOOLS_VERSION,
                "tlapm_commit": TLAPM_COMMIT,
                "files": [
                    item.receipt_record()
                    for item in sorted(tool_snapshots, key=lambda value: value.logical_path)
                ],
                "manifest_sha256": _manifest(tool_snapshots),
            },
            "events": events,
            "result": {
                "execution_validated": True,
                "sany_status": 0,
                "tlc_status": 12,
                "normalizer_status": 0,
                "tool_states": state_count,
                "actions": action_count,
                "separate_stderr_empty": True,
                "normalized_fixture": "crates/iroha_sumeragi_core/tests/fixtures/tlc_replay_witness.tsv",
                "normalized_sha256": sha256_bytes(normalized_bytes),
                "normalized_matches_fixture": True,
            },
            "signing": signing,
            "artifact_inventory": inventory,
            "publication": {
                "create_only": True,
                "unexpected_files_allowed": False,
                "symlinks_allowed": False,
                "hard_links_allowed": False,
                "partial": False,
            },
        }
        receipt_path = output_root / "receipt.json"
        _require_owned_path(output_directory)
        _require_owned_entry(output_directory, events_directory)
        _write_exclusive_at(
            output_directory,
            "receipt.json",
            "receipt.json",
            canonical_json(receipt),
            0o600,
        )
        _require_owned_path(output_directory)
        _require_owned_entry(output_directory, events_directory)
        return receipt_path
    finally:
        cleanup_error: Union[BaseException, None] = None
        if runtime_directory is not None:
            try:
                _remove_owned_child(output_directory, runtime_directory)
            except BaseException as error:
                cleanup_error = error
            finally:
                if runtime_directory.descriptor >= 0:
                    os.close(runtime_directory.descriptor)
                    runtime_directory.descriptor = -1
        if events_directory is not None and events_directory.descriptor >= 0:
            os.close(events_directory.descriptor)
            events_directory.descriptor = -1
        try:
            os.close(output_directory.descriptor)
        except OSError as error:
            if cleanup_error is None:
                cleanup_error = error
        # A failed caller-selected output root is deliberately left reserved.
        # Recursively deleting it after a rename/replacement race would cross
        # the collector's ownership boundary.
        if cleanup_error is not None:
            raise cleanup_error


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--java-bin", type=Path, required=True)
    parser.add_argument("--python-bin", type=Path, required=True)
    parser.add_argument("--tla2tools-jar", type=Path, required=True)
    parser.add_argument("--tlapm-projection", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--mode", choices=sorted(EVENT_TEMPLATES), default="formal-only")
    parser.add_argument("--timeout-seconds", type=float, default=1800.0)
    return parser


def main() -> int:
    args = _parser().parse_args()
    if not 1.0 <= args.timeout_seconds <= 86400.0:
        print("error: timeout must be between 1 and 86400 seconds", file=sys.stderr)
        return 2
    try:
        receipt = collect(args)
    except (CollectionError, OSError, subprocess.SubprocessError, ValueError) as error:
        print(f"Sumeragi V2 replay collection failed: {error}", file=sys.stderr)
        return 2
    print(receipt)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
