#!/usr/bin/env python3
"""Private pinned verifier staging and bounded POSIX process-group execution.

Only the owned verifier session is signalled. This preserves the existing process-group
boundary; it does not authorize arbitrary executables or prove closure of escaped sessions.
Callers independently authenticate executable and input sources before invoking a verifier.
"""
from __future__ import annotations

import hashlib
import os
import re
import selectors
import signal
import stat
import subprocess
import time
from pathlib import Path
from typing import Any

from sorafs_evidence_json import (
    _anchored_evidence_identity_matches,
    _open_anchored_evidence_parent,
    evidence_read_open_flags,
    validate_evidence_file_for_read,
)

VERIFIER_TIMEOUT_SECS = 30
VERIFIER_CLEANUP_TIMEOUT_SECS = 1
MAX_VERIFIER_STDOUT_BYTES = 16 * 1024
MAX_VERIFIER_STDERR_BYTES = 16 * 1024
MAX_VERIFIER_EXECUTABLE_BYTES = 512 * 1024 * 1024
COPY_CHUNK_BYTES = 1024 * 1024

def _verifier_process_group_exists(process: subprocess.Popen[bytes]) -> bool:
    """Return whether the verifier's isolated POSIX process group is live."""

    if os.name != "posix":
        return process.poll() is None
    try:
        os.killpg(process.pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    except OSError:
        return True
    return True


def _kill_and_reap_verifier(process: subprocess.Popen[bytes]) -> bool:
    """Kill the isolated verifier group and reap its direct child, boundedly."""

    deadline = time.monotonic() + VERIFIER_CLEANUP_TIMEOUT_SECS

    def kill_group() -> bool:
        try:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGKILL)
            elif process.poll() is None:  # pragma: no cover - POSIX release host
                process.kill()
        except ProcessLookupError:
            return True
        except OSError:
            return False
        return True

    try:
        signal_ok = kill_group()
        while process.poll() is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return False
            try:
                process.wait(timeout=min(remaining, 0.05))
            except subprocess.TimeoutExpired:
                signal_ok = kill_group() and signal_ok

        if os.name != "posix":  # pragma: no cover - POSIX release host
            return signal_ok
        while _verifier_process_group_exists(process):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return False
            signal_ok = kill_group() and signal_ok
            time.sleep(min(0.01, remaining))
        return signal_ok
    except (OSError, ValueError, subprocess.SubprocessError):
        return False


def _close_verifier_pipe(
    selector: selectors.BaseSelector,
    pipe: Any,
) -> bool:
    """Unregister and close one verifier diagnostic pipe."""

    closed = True
    try:
        selector.unregister(pipe)
    except KeyError:
        pass
    except (OSError, ValueError):
        closed = False
    try:
        pipe.close()
    except OSError:
        closed = False
    return closed


def _write_all(descriptor: int, payload: bytes | memoryview) -> None:
    view = memoryview(payload)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            raise OSError("verifier private input write failed")
        view = view[written:]


def _stable_file(before: os.stat_result, after: os.stat_result) -> bool:
    return all(
        getattr(before, field) == getattr(after, field)
        for field in (
            "st_dev", "st_ino", "st_size", "st_mode", "st_uid", "st_nlink",
            "st_mtime_ns", "st_ctime_ns",
        )
    )


def _private_destination(path: Path) -> tuple[int, os.stat_result, list[int]]:
    parent, leaf, lineage = _open_anchored_evidence_parent(path)
    try:
        metadata = os.fstat(parent)
        if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) != 0o700:
            raise OSError("verifier input directory is not private")
        descriptor = os.open(
            leaf,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
            0o600,
            dir_fd=parent,
        )
        return descriptor, metadata, lineage
    except BaseException:
        for item in reversed(lineage):
            os.close(item)
        raise


def _publish_input(
    path: Path, descriptor: int, parent_fd: int, parent: os.stat_result, mode: int, size: int,
) -> None:
    current_parent = os.fstat(parent_fd)
    before = os.fstat(descriptor)
    if (
        (current_parent.st_dev, current_parent.st_ino) != (parent.st_dev, parent.st_ino)
        or current_parent.st_uid != os.geteuid()
        or stat.S_IMODE(current_parent.st_mode) != 0o700
        or not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
        or before.st_uid != os.geteuid() or before.st_size != size
        or stat.S_IMODE(before.st_mode) & 0o077
        or not _anchored_evidence_identity_matches(
            path, expected_parent=parent, expected_leaf=before,
        )
    ):
        raise OSError("verifier private input identity changed")
    os.fsync(descriptor)
    os.fchmod(descriptor, mode)
    os.fsync(descriptor)
    after = os.fstat(descriptor)
    current_parent = os.fstat(parent_fd)
    if (
        current_parent.st_uid != os.geteuid()
        or stat.S_IMODE(current_parent.st_mode) != 0o700
        or after.st_nlink != 1 or after.st_size != size
        or stat.S_IMODE(after.st_mode) != mode
        or not _anchored_evidence_identity_matches(
            path, expected_parent=parent, expected_leaf=after,
        )
    ):
        raise OSError("verifier private input identity changed")


def write_private_input(path: Path, payload: bytes, mode: int = 0o400) -> None:
    """Exclusively stage bytes in an existing owner-private directory, then make them read-only.

    Failed partial writes remain private tombstones; no substituted path is removed.
    All failures are fixed diagnostics and contain no candidate path or content.
    """
    if os.name != "posix":
        raise OSError("verifier private staging is unavailable")
    if (
        not isinstance(path, Path) or not isinstance(payload, bytes)
        or mode not in (0o400, 0o500) or isinstance(mode, bool)
        or len(payload) > MAX_VERIFIER_EXECUTABLE_BYTES
    ):
        raise ValueError("invalid verifier private input")
    descriptor = -1
    lineage: list[int] = []
    try:
        descriptor, parent, lineage = _private_destination(path)
        _write_all(descriptor, payload)
        _publish_input(path, descriptor, lineage[-1], parent, mode, len(payload))
        os.fsync(lineage[-1])
    except (OSError, RuntimeError, ValueError):
        raise OSError("verifier private input could not be staged") from None
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        for item in reversed(lineage):
            os.close(item)


def snapshot_executable(source: Path, dest: Path, expected_sha256: str) -> None:
    """Stream one independently pinned executable into a private immutable mode-0500 copy.

    The source must be owned, executable, regular, single-link and not group/world writable.
    Every ancestor is opened without following links. Source descriptor metadata and both path
    identities are checked after copying; a failed digest never publishes executable permission.
    """
    if os.name != "posix":
        raise OSError("verifier executable snapshot unavailable")
    if (
        not isinstance(source, Path) or not isinstance(dest, Path)
        or not isinstance(expected_sha256, str)
        or re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is None
    ):
        raise ValueError("invalid verifier executable pin")
    source_fd = dest_fd = -1
    source_lineage: list[int] = []
    dest_lineage: list[int] = []
    try:
        validate_evidence_file_for_read(source)
        parent_fd, leaf, source_lineage = _open_anchored_evidence_parent(source)
        parent = os.fstat(parent_fd)
        path_before = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        source_fd = os.open(leaf, evidence_read_open_flags(), dir_fd=parent_fd)
        before = os.fstat(source_fd)
        if (
            not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
            or before.st_uid != os.geteuid() or not before.st_mode & stat.S_IXUSR
            or stat.S_IMODE(before.st_mode) & 0o7022
            or not 0 < before.st_size <= MAX_VERIFIER_EXECUTABLE_BYTES
            or not _stable_file(path_before, before)
        ):
            raise ValueError("unsafe verifier executable")
        dest_fd, dest_parent, dest_lineage = _private_destination(dest)
        digest = hashlib.sha256()
        size = 0
        while True:
            chunk = os.read(source_fd, min(COPY_CHUNK_BYTES, MAX_VERIFIER_EXECUTABLE_BYTES + 1 - size))
            if not chunk:
                break
            size += len(chunk)
            if size > MAX_VERIFIER_EXECUTABLE_BYTES:
                raise ValueError("oversized verifier executable")
            digest.update(chunk)
            _write_all(dest_fd, chunk)
        after = os.fstat(source_fd)
        if (
            size != before.st_size or not _stable_file(before, after)
            or not _anchored_evidence_identity_matches(
                source, expected_parent=parent, expected_leaf=after,
            )
            or digest.hexdigest() != expected_sha256
        ):
            raise ValueError("verifier executable differs from its pin")
        _publish_input(dest, dest_fd, dest_lineage[-1], dest_parent, 0o500, size)
        os.fsync(dest_lineage[-1])
    except ValueError:
        raise ValueError("verifier executable snapshot rejected") from None
    except (OSError, RuntimeError):
        raise OSError("verifier executable snapshot unavailable") from None
    finally:
        for descriptor in (dest_fd, source_fd):
            if descriptor >= 0:
                os.close(descriptor)
        for item in reversed(dest_lineage + source_lineage):
            os.close(item)


def run_verifier(
    command: list[str], root: Path, *, max_stdout_bytes: int, expected_stderr: bytes,
) -> bytes:
    """Run an authenticated verifier with bounded stdout, exact stderr and owned-group cleanup.

    Zero stdout allowance preserves the silent software-verifier contract. A nonzero allowance
    is only transport admission: the caller must validate the returned exact bytes. The required
    stderr frame is compared incrementally without retaining or reporting process diagnostics.
    Output or exit rejection raises payload-free ValueError; execution/cleanup failure raises
    OSError.
    """
    if (
        not isinstance(max_stdout_bytes, int) or isinstance(max_stdout_bytes, bool)
        or not 0 <= max_stdout_bytes <= MAX_VERIFIER_STDOUT_BYTES
        or not isinstance(expected_stderr, bytes)
        or len(expected_stderr) > MAX_VERIFIER_STDERR_BYTES
        or not isinstance(root, Path) or not isinstance(command, list) or not command
        or any(not isinstance(part, str) or not part or "\0" in part for part in command)
    ):
        raise ValueError("invalid verifier invocation")
    if os.name != "posix":  # Fail closed without enforceable process-group ownership.
        raise OSError("verifier process groups are unavailable")
    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    failed = False
    unavailable = False
    cleanup_required = True
    stdout = bytearray()
    stderr_received = 0
    try:
        process = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=root,
            env={"LANG": "C", "LC_ALL": "C", "PATH": os.defpath},
            bufsize=0,
            start_new_session=True,
            umask=0o077,
        )
        assert process.stdout is not None and process.stderr is not None
        for pipe in (process.stdout, process.stderr):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ)
        deadline = time.monotonic() + VERIFIER_TIMEOUT_SECS
        while selector.get_map() and not failed and not unavailable:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                unavailable = True
                break
            for key, _events in selector.select(min(remaining, 0.05)):
                is_stdout = key.fileobj is process.stdout
                allowance = (
                    max_stdout_bytes - len(stdout)
                    if is_stdout else len(expected_stderr) - stderr_received
                )
                try:
                    chunk = os.read(key.fd, min(4096, allowance + 1))
                except BlockingIOError:
                    continue
                if not chunk:
                    unavailable = not _close_verifier_pipe(selector, key.fileobj)
                    if unavailable:
                        break
                    continue
                if len(chunk) > allowance:
                    failed = True
                    break
                if is_stdout:
                    stdout.extend(chunk)
                else:
                    end = stderr_received + len(chunk)
                    if chunk != expected_stderr[stderr_received:end]:
                        failed = True
                        break
                    stderr_received = end
        if not failed and not unavailable:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                unavailable = True
            else:
                try:
                    returncode = process.wait(timeout=remaining)
                except subprocess.TimeoutExpired:
                    unavailable = True
                else:
                    if returncode != 0:
                        failed = True
                    elif _verifier_process_group_exists(process):
                        unavailable = True
                    else:
                        cleanup_required = False
    except (OSError, ValueError, subprocess.SubprocessError):
        unavailable = True
    finally:
        cleanup_ok = True
        if process is not None and cleanup_required:
            cleanup_ok = _kill_and_reap_verifier(process)
        for pipe in (
            None if process is None else process.stdout,
            None if process is None else process.stderr,
        ):
            if pipe is not None and not pipe.closed:
                cleanup_ok = _close_verifier_pipe(selector, pipe) and cleanup_ok
        try:
            selector.close()
        except OSError:
            cleanup_ok = False
        unavailable = unavailable or not cleanup_ok
    if unavailable:
        raise OSError("verifier process unavailable")
    if failed or stderr_received != len(expected_stderr):
        raise ValueError("verifier output rejected")
    return bytes(stdout)


__all__ = ["write_private_input", "snapshot_executable", "run_verifier"]
