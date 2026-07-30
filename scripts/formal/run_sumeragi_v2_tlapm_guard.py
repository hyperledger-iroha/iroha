#!/usr/bin/env python3
"""Run the Sumeragi TLAPS body inside a bounded, globally serialized process group."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import datetime, timezone
import errno
import fcntl
import json
import os
from pathlib import Path
import secrets
import select
import signal
import stat
import subprocess
import sys
import time
from typing import Callable, Iterator, Mapping, Sequence


# This is a last-resort ceiling, not the expected working set.  The physical
# proof shards keep normal TLAPM runs well below it while preserving enough
# headroom for the pinned frontend and one backend worker.
MAX_MEMORY_BYTES = 2 * 1024 * 1024 * 1024
MAX_CONFIGURABLE_MEMORY_BYTES = 4 * 1024 * 1024 * 1024
SAMPLE_INTERVAL_SECONDS = 0.25
# `footprint` suspends each inspected task while collecting its stronger macOS
# accounting. Probing a just-execed task in dyld at the RSS cadence can prevent
# it from completing loader startup. Keep cheap `ps` RSS sampling frequent and
# run the intrusive footprint measurement on the same reviewed cadence used by
# the Kagemusha staged-resource guard.
PHYSICAL_FOOTPRINT_INTERVAL_SECONDS = 5.0
CONTROL_RECORD_TIMEOUT_SECONDS = 0.2
# macOS does not provide a sub-200 ms completion guarantee for ``ps`` or
# ``footprint``, even when the host is otherwise idle. Keep the probes bounded
# and fail closed, but allow enough time to avoid scheduler-jitter false
# positives during long-running release ceremonies.
PROCESS_INSPECTION_TIMEOUT_SECONDS = 2.0
TERM_GRACE_SECONDS = 2.0
SESSION_READY_TIMEOUT_SECONDS = 2.0
MEMORY_LIMIT_EXIT_CODE = 75
LOCK_UNAVAILABLE_EXIT_CODE = 73
FOREIGN_JOB_EXIT_CODE = 74
RESOURCE_GUARD_AUTH_FD_ENV = "IROHA_RESOURCE_GUARD_AUTH_FD"
RESOURCE_GUARD_AUTH_TOKEN_ENV = "IROHA_RESOURCE_GUARD_AUTH_TOKEN"
RESOURCE_GUARD_AUTH_MAGIC = "IROHA_RESOURCE_GUARD_AUTH_V1"
SESSION_WRAPPER_FLAG = "--resource-session-wrapper"
PS = next(
    (
        str(candidate)
        for candidate in (Path("/bin/ps"), Path("/usr/bin/ps"))
        if candidate.is_file()
    ),
    "ps",
)
FOOTPRINT = next(
    (
        str(candidate)
        for candidate in (Path("/usr/bin/footprint"), Path("/bin/footprint"))
        if candidate.is_file()
    ),
    None,
)
LOCK_PATH = Path("/tmp") / f"iroha-sumeragi-v2-tlapm-{os.getuid()}.lock"
HEAVY_JOB_LOCK_PATH = Path("/tmp") / f"iroha-memory-heavy-{os.getuid()}.lock"


class GuardError(RuntimeError):
    """Raised when the guard cannot safely supervise the requested command."""


class LockUnavailable(GuardError):
    """Raised when another guarded TLAPS run owns the per-user lock."""


@dataclass(frozen=True)
class ProcessRow:
    """One process snapshot returned by ``ps``."""

    pid: int
    parent_pid: int
    process_group_id: int
    uid: int
    rss_bytes: int
    command: str


@dataclass(frozen=True)
class MemorySample:
    """Aggregate memory attributed to the supervised process group."""

    memory_bytes: int
    rss_bytes: int
    physical_footprint_bytes: int
    process_count: int
    accounting_method: str


class SessionControl:
    """Bounded control channel from the lifeline wrapper."""

    def __init__(self, descriptor: int) -> None:
        self._descriptor = descriptor
        self._buffer = bytearray()

    def read_line(self, *, timeout: float, description: str) -> str:
        """Read one bounded newline-terminated ASCII control record."""

        deadline = time.monotonic() + timeout
        while True:
            newline = self._buffer.find(b"\n")
            if newline >= 0:
                raw = bytes(self._buffer[:newline])
                del self._buffer[: newline + 1]
                try:
                    return raw.decode("ascii")
                except UnicodeDecodeError as error:
                    raise GuardError(f"{description} is not ASCII") from error
            if len(self._buffer) >= 256:
                raise GuardError(f"{description} exceeds 255 bytes")
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise GuardError(f"timed out waiting for {description}")
            readable, _, _ = select.select([self._descriptor], [], [], remaining)
            if not readable:
                raise GuardError(f"timed out waiting for {description}")
            chunk = os.read(self._descriptor, 256 - len(self._buffer))
            if not chunk:
                raise GuardError(f"lifeline wrapper closed before {description}")
            self._buffer.extend(chunk)

    def close(self) -> None:
        """Close the control descriptor."""

        if self._descriptor >= 0:
            os.close(self._descriptor)
            self._descriptor = -1


@dataclass
class GuardedSession:
    """The wrapper process plus the separately sessioned heavy body."""

    wrapper: subprocess.Popen[bytes]
    process_group_id: int
    lifeline_writer: int
    control: SessionControl

    def close(self) -> None:
        """Release the supervisor ends of the session control pipes."""

        if self.lifeline_writer >= 0:
            os.close(self.lifeline_writer)
            self.lifeline_writer = -1
        try:
            self.wrapper.wait(timeout=TERM_GRACE_SECONDS * 2 + 1)
        except subprocess.TimeoutExpired:
            _signal_process_group(self.wrapper.pid, signal.SIGKILL)
            self.wrapper.wait(timeout=TERM_GRACE_SECONDS)
        self.control.close()


def _read_guarded_exit(session: GuardedSession) -> tuple[int, bool, int]:
    """Read and validate the wrapper's child status and kernel RSS evidence."""

    wrapper_exit = session.control.read_line(
        timeout=CONTROL_RECORD_TIMEOUT_SECONDS,
        description="lifeline wrapper exit status",
    )
    fields = wrapper_exit.split()
    if (
        len(fields) != 4
        or fields[0] != "EXIT"
        or fields[2] not in {"0", "1"}
        or not fields[3].isdigit()
    ):
        raise GuardError("lifeline wrapper emitted invalid exit status")
    try:
        child_exit_code = int(fields[1])
        kernel_peak_rss_bytes = int(fields[3])
    except ValueError as error:
        raise GuardError(
            "lifeline wrapper emitted a non-integer child status"
        ) from error
    return child_exit_code, fields[2] == "1", kernel_peak_rss_bytes


def _utc_now() -> str:
    """Return a stable UTC timestamp suitable for JSON evidence."""

    return datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace(
        "+00:00", "Z"
    )


def _canonical_json(value: object) -> bytes:
    """Encode one JSON object in the canonical report representation."""

    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("utf-8")


def _write_all(descriptor: int, data: bytes) -> None:
    """Write every byte to a file descriptor."""

    offset = 0
    while offset < len(data):
        written = os.write(descriptor, data[offset:])
        if written <= 0:
            raise GuardError("resource report write made no progress")
        offset += written


class JsonlReport:
    """Owner-only append stream for resource events."""

    def __init__(self, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        flags = os.O_WRONLY | os.O_CREAT | getattr(os, "O_CLOEXEC", 0)
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(path, flags, 0o600)
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.getuid()
            or metadata.st_nlink != 1
        ):
            os.close(descriptor)
            raise GuardError(f"resource report has unsafe metadata: {path}")
        os.fchmod(descriptor, 0o600)
        os.ftruncate(descriptor, 0)
        self._descriptor = descriptor

    def write(self, event: dict[str, object]) -> None:
        """Append and durably flush one report event."""

        _write_all(self._descriptor, _canonical_json(event))
        os.fsync(self._descriptor)

    def close(self) -> None:
        """Close the report descriptor."""

        if self._descriptor >= 0:
            os.close(self._descriptor)
            self._descriptor = -1


def _write_summary(path: Path, summary: dict[str, object]) -> None:
    """Atomically publish an owner-only JSON summary."""

    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.partial")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(temporary, flags, 0o600)
    try:
        _write_all(descriptor, _canonical_json(summary))
        os.fsync(descriptor)
    except BaseException:
        os.close(descriptor)
        temporary.unlink(missing_ok=True)
        raise
    else:
        os.close(descriptor)
    os.replace(temporary, path)


@contextmanager
def _host_lock(path: Path = LOCK_PATH, *, description: str = "TLAPS") -> Iterator[int]:
    """Acquire one secure per-UID host-global execution lock."""

    flags = os.O_RDWR | os.O_CREAT | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as error:
        raise GuardError(f"could not open {description} host lock {path}: {error}") from error
    try:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.getuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise GuardError(f"{description} host lock has unsafe metadata: {path}")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except (BlockingIOError, OSError) as error:
            if isinstance(error, BlockingIOError) or error.errno in {
                errno.EACCES,
                errno.EAGAIN,
            }:
                raise LockUnavailable(
                    f"another guarded {description} run owns the host lock"
                ) from error
            raise
        os.ftruncate(descriptor, 0)
        _write_all(
            descriptor,
            f"pid={os.getpid()}\nstarted_utc={_utc_now()}\n".encode("ascii"),
        )
        os.fsync(descriptor)
        yield descriptor
    finally:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
        finally:
            os.close(descriptor)


def _process_rows() -> list[ProcessRow]:
    """Snapshot process identity, ownership, grouping, and RSS."""

    try:
        completed = subprocess.run(
            [PS, "-axo", "pid=,ppid=,pgid=,uid=,rss=,comm="],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=PROCESS_INSPECTION_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired as error:
        raise GuardError(
            "process inspection exceeded "
            f"{PROCESS_INSPECTION_TIMEOUT_SECONDS:g} s"
        ) from error
    except OSError as error:
        raise GuardError("could not start process inspection") from error
    if completed.returncode != 0:
        detail = completed.stderr.strip() or f"exit status {completed.returncode}"
        raise GuardError(f"could not inspect processes with ps: {detail}")
    rows: list[ProcessRow] = []
    for line in completed.stdout.splitlines():
        fields = line.split(None, 5)
        if len(fields) != 6:
            continue
        try:
            pid, parent_pid, process_group_id, uid, rss_kib = map(int, fields[:5])
        except ValueError:
            continue
        rows.append(
            ProcessRow(
                pid=pid,
                parent_pid=parent_pid,
                process_group_id=process_group_id,
                uid=uid,
                rss_bytes=max(0, rss_kib) * 1024,
                command=fields[5],
            )
        )
    return rows


def _is_formal_heavy_process(row: ProcessRow) -> bool:
    """Return whether a row is a known formal or Kagemusha heavy process."""

    name = Path(row.command).name.lower()
    return (
        name == "tlapm"
        or name == "isabelle"
        or name.startswith("isabelle-")
        or name in {"poly", "polyml", "polyml.exe"}
        or name == "kagemusha_recursive_spend_v4_bundle"
        or name.startswith("kagemusha_recu")
    )


def _foreign_heavy_jobs(
    rows: Sequence[ProcessRow],
    *,
    owned_process_group_id: int | None = None,
) -> list[ProcessRow]:
    """Find same-user heavy jobs outside the optionally owned process group."""

    uid = os.getuid()
    return sorted(
        (
            row
            for row in rows
            if row.uid == uid
            and _is_formal_heavy_process(row)
            and (
                owned_process_group_id is None
                or row.process_group_id != owned_process_group_id
            )
        ),
        key=lambda row: row.pid,
    )


def _record_foreign_heavy_job(
    report: JsonlReport,
    row: ProcessRow,
    *,
    phase: str,
    owned_process_group_id: int | None,
) -> None:
    """Persist one foreign-heavy-job conflict without signalling that job."""

    report.write(
        {
            "event": "foreign_heavy_job",
            "foreign_command": Path(row.command).name,
            "foreign_pid": row.pid,
            "foreign_process_group_id": row.process_group_id,
            "owned_process_group_id": owned_process_group_id,
            "phase": phase,
            "schema_version": 1,
            "timestamp_utc": _utc_now(),
        }
    )


def _group_rows(
    process_group_id: int,
    rows: Sequence[ProcessRow] | None = None,
) -> list[ProcessRow]:
    """Return all current members of the owned process group."""

    snapshot = _process_rows() if rows is None else rows
    return [
        row
        for row in snapshot
        if row.process_group_id == process_group_id and row.uid == os.getuid()
    ]


def _physical_footprint_bytes(process_ids: Sequence[int]) -> int:
    """Return summed macOS physical footprint, or zero when unavailable."""

    if sys.platform != "darwin" or FOOTPRINT is None or not process_ids:
        return 0
    try:
        completed = subprocess.run(
            [
                FOOTPRINT,
                "--format",
                "bytes",
                "--noCategories",
                *[str(pid) for pid in sorted(set(process_ids))],
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=PROCESS_INSPECTION_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired as error:
        raise GuardError(
            "macOS physical-footprint inspection exceeded "
            f"{PROCESS_INSPECTION_TIMEOUT_SECONDS:g} s"
        ) from error
    except OSError as error:
        raise GuardError("could not start macOS physical-footprint inspection") from error
    if completed.returncode != 0:
        return 0
    total = 0
    for line in completed.stdout.splitlines():
        fields = line.strip().split()
        if len(fields) == 3 and fields[0] == "phys_footprint:" and fields[2] == "B":
            try:
                total += max(0, int(fields[1]))
            except ValueError:
                continue
    return total


def _sample_group(
    process_group_id: int,
    rows: Sequence[ProcessRow] | None = None,
    *,
    include_physical_footprint: bool = True,
) -> MemorySample:
    """Measure the larger aggregate RSS or macOS physical footprint."""

    group_rows = _group_rows(process_group_id, rows)
    rss_bytes = sum(row.rss_bytes for row in group_rows)
    footprint_bytes = (
        _physical_footprint_bytes([row.pid for row in group_rows])
        if include_physical_footprint
        else 0
    )
    if footprint_bytes > 0:
        memory_bytes = max(rss_bytes, footprint_bytes)
        method = "max_rss_physical_footprint"
    else:
        memory_bytes = rss_bytes
        method = "rss"
    return MemorySample(
        memory_bytes=memory_bytes,
        rss_bytes=rss_bytes,
        physical_footprint_bytes=footprint_bytes,
        process_count=len(group_rows),
        accounting_method=method,
    )


def _process_group_exists(process_group_id: int) -> bool:
    """Return whether the exact known process group still exists."""

    try:
        os.killpg(process_group_id, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        # Darwin reports EPERM for a group containing only an unreaped zombie.
        # The known same-UID group was already signalled; no live member remains
        # available to receive another signal in that state.
        return False
    return True


def _signal_process_group(process_group_id: int, signum: int) -> None:
    """Signal the exact known process group, accepting only its disappearance."""

    try:
        os.killpg(process_group_id, signum)
    except ProcessLookupError:
        return
    except OSError as error:
        raise GuardError(
            f"could not signal owned process group {process_group_id}"
        ) from error


def _terminate_owned_group(
    process: subprocess.Popen[bytes], process_group_id: int
) -> None:
    """Unconditionally TERM, then KILL and reap one exact known process group."""

    if process_group_id <= 1:
        raise GuardError("refusing to signal an invalid owned process group")
    _signal_process_group(process_group_id, signal.SIGTERM)
    deadline = time.monotonic() + TERM_GRACE_SECONDS
    while time.monotonic() < deadline:
        if not _process_group_exists(process_group_id):
            break
        time.sleep(0.05)
    if _process_group_exists(process_group_id):
        _signal_process_group(process_group_id, signal.SIGKILL)
    try:
        process.wait(timeout=TERM_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        _signal_process_group(process_group_id, signal.SIGKILL)
        process.wait(timeout=TERM_GRACE_SECONDS)
    final_deadline = time.monotonic() + TERM_GRACE_SECONDS
    while time.monotonic() < final_deadline:
        if not _process_group_exists(process_group_id):
            return
        _signal_process_group(process_group_id, signal.SIGKILL)
        time.sleep(0.05)
    raise GuardError(f"owned TLAPS process group {process_group_id} is still present")


def _kill_owned_group_immediately(
    process: subprocess.Popen[bytes], process_group_id: int
) -> None:
    """KILL and reap an owned group without a runaway-allocation grace period."""

    if process_group_id <= 1:
        raise GuardError("refusing to signal an invalid owned process group")
    _signal_process_group(process_group_id, signal.SIGKILL)
    try:
        process.wait(timeout=TERM_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        _signal_process_group(process_group_id, signal.SIGKILL)
        process.wait(timeout=TERM_GRACE_SECONDS)
    final_deadline = time.monotonic() + TERM_GRACE_SECONDS
    while time.monotonic() < final_deadline:
        if not _process_group_exists(process_group_id):
            return
        _signal_process_group(process_group_id, signal.SIGKILL)
        time.sleep(0.05)
    raise GuardError(f"owned TLAPS process group {process_group_id} is still present")


def _exit_status(returncode: int) -> int:
    """Translate a subprocess return code into a shell exit status."""

    if returncode < 0:
        return min(255, 128 - returncode)
    return min(255, returncode)


def _pipe() -> tuple[int, int]:
    """Create one close-on-exec pipe whose ends require explicit inheritance."""

    reader, writer = os.pipe()
    os.set_inheritable(reader, False)
    os.set_inheritable(writer, False)
    return reader, writer


def _close_descriptor(descriptor: int) -> None:
    """Best-effort close for one owned descriptor."""

    try:
        os.close(descriptor)
    except OSError:
        pass


def _require_pipe_descriptor(descriptor: int, description: str) -> None:
    """Reject a forged wrapper control descriptor before spawning the body."""

    if descriptor < 3:
        raise GuardError(f"{description} descriptor is invalid")
    try:
        metadata = os.fstat(descriptor)
    except OSError as error:
        raise GuardError(f"{description} descriptor is unavailable") from error
    if not stat.S_ISFIFO(metadata.st_mode):
        raise GuardError(f"{description} descriptor is not a pipe")


def _lifeline_closed(descriptor: int, timeout: float) -> bool:
    """Return whether the supervisor lifeline reached EOF or was violated."""

    readable, _, _ = select.select([descriptor], [], [], timeout)
    if not readable:
        return False
    try:
        os.read(descriptor, 1)
        return True
    except OSError:
        return True


def _write_wrapper_control(descriptor: int, record: str) -> None:
    """Write one bounded ASCII wrapper-control record."""

    encoded = f"{record}\n".encode("ascii")
    if len(encoded) > 256:
        raise GuardError("wrapper control record is too large")
    _write_all(descriptor, encoded)


def _normalized_wait4_max_rss_bytes(max_rss: int | float) -> int:
    """Normalize ``wait4``'s platform-specific maximum-RSS unit to bytes."""

    value = max(0, int(max_rss))
    # Darwin reports bytes. Linux and the BSDs exposed by Python report KiB.
    return value if sys.platform == "darwin" else value * 1024


def _wait4_nonblocking(
    process: subprocess.Popen[bytes],
) -> tuple[int, int] | None:
    """Reap one completed direct child and retain its kernel RSS high-water mark."""

    if not hasattr(os, "wait4"):
        returncode = process.poll()
        return None if returncode is None else (returncode, 0)
    try:
        waited_pid, status, usage = os.wait4(process.pid, os.WNOHANG)
    except InterruptedError:
        return None
    if waited_pid == 0:
        return None
    if waited_pid != process.pid:
        raise GuardError("wait4 reaped an unexpected guarded process")
    returncode = os.waitstatus_to_exitcode(status)
    # Mark the Popen as reaped so its destructor and later wait calls do not
    # issue a second waitpid for a PID which may already have been reused.
    process.returncode = returncode
    return returncode, _normalized_wait4_max_rss_bytes(usage.ru_maxrss)


def _run_session_wrapper(argv: Sequence[str]) -> int:
    """Watch the supervisor lifeline while owning the separately sessioned body."""

    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--lifeline-fd", required=True, type=int)
    parser.add_argument("--control-fd", required=True, type=int)
    parser.add_argument("--auth-fd", required=True, type=int)
    parser.add_argument("--held-lock-fd", action="append", default=[], type=int)
    parser.add_argument("--child-directory-fd", action="append", default=[], type=int)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    command = list(args.command)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        raise GuardError("session wrapper command is empty")
    descriptors = (
        args.lifeline_fd,
        args.control_fd,
        args.auth_fd,
        *args.held_lock_fd,
        *args.child_directory_fd,
    )
    if len(set(descriptors)) != len(descriptors):
        raise GuardError("session wrapper control descriptors overlap")
    _require_pipe_descriptor(args.lifeline_fd, "lifeline")
    _require_pipe_descriptor(args.control_fd, "control")
    _require_pipe_descriptor(args.auth_fd, "authorization")
    for descriptor in args.held_lock_fd:
        try:
            metadata = os.fstat(descriptor)
        except OSError as error:
            raise GuardError("held lock descriptor is unavailable") from error
        if (
            descriptor < 3
            or not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.getuid()
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise GuardError("held lock descriptor is invalid")
    for descriptor in args.child_directory_fd:
        try:
            metadata = os.fstat(descriptor)
        except OSError as error:
            raise GuardError("child directory descriptor is unavailable") from error
        if descriptor < 3 or not stat.S_ISDIR(metadata.st_mode):
            raise GuardError("child directory descriptor is invalid")
    expected_auth_fd = os.environ.get(RESOURCE_GUARD_AUTH_FD_ENV)
    if expected_auth_fd != str(args.auth_fd):
        raise GuardError("authorization descriptor environment is inconsistent")

    received_signal = 0

    def receive_signal(signum: int, _frame: object) -> None:
        nonlocal received_signal
        if received_signal == 0:
            received_signal = signum

    watched_signals = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
    for signum in watched_signals:
        signal.signal(signum, receive_signal)

    child: subprocess.Popen[bytes] | None = None
    try:
        if _lifeline_closed(args.lifeline_fd, 0):
            return 1
        child = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            pass_fds=(args.auth_fd, *args.child_directory_fd),
            start_new_session=True,
            env=os.environ.copy(),
        )
        process_group_id = child.pid
        _close_descriptor(args.auth_fd)
        args.auth_fd = -1
        if process_group_id <= 1 or process_group_id == os.getpgrp():
            raise GuardError("guarded body did not enter its own process group")
        _write_wrapper_control(args.control_fd, f"READY {process_group_id}")

        lifeline_lost = False
        completed: tuple[int, int] | None = None
        while completed is None:
            completed = _wait4_nonblocking(child)
            if completed is not None:
                break
            if received_signal or _lifeline_closed(args.lifeline_fd, 0.05):
                lifeline_lost = True
                break
        if lifeline_lost:
            _terminate_owned_group(child, process_group_id)
            return 1

        if completed is None:
            raise GuardError("session wrapper lost the body return code")
        returncode, kernel_peak_rss_bytes = completed
        lingering = _process_group_exists(process_group_id)
        if lingering:
            _terminate_owned_group(child, process_group_id)
        _write_wrapper_control(
            args.control_fd,
            f"EXIT {returncode} {1 if lingering else 0} {kernel_peak_rss_bytes}",
        )
        return 1 if lingering else _exit_status(returncode)
    except BaseException as error:
        if child is not None:
            try:
                _terminate_owned_group(child, child.pid)
            except BaseException:
                pass
        try:
            _write_wrapper_control(args.control_fd, "ERROR")
        except BaseException:
            pass
        print(f"resource session wrapper failed: {error}", file=sys.stderr)
        return 1
    finally:
        for descriptor in descriptors:
            _close_descriptor(descriptor)


def _spawn_guarded_session(
    command: Sequence[str],
    environment: dict[str, str],
    held_lock_descriptors: Sequence[int],
    child_directory_descriptors: Sequence[int],
) -> GuardedSession:
    """Spawn the lifeline wrapper and authenticate exactly one guarded body."""

    auth_reader, auth_writer = _pipe()
    lifeline_reader, lifeline_writer = _pipe()
    control_reader, control_writer = _pipe()
    token = secrets.token_hex(32)
    child_environment = environment.copy()
    child_environment.pop("SUMERAGI_TLAPS_SUPERVISOR_PID", None)
    child_environment[RESOURCE_GUARD_AUTH_FD_ENV] = str(auth_reader)
    child_environment[RESOURCE_GUARD_AUTH_TOKEN_ENV] = token
    wrapper_command = [
        sys.executable,
        str(Path(__file__).resolve()),
        SESSION_WRAPPER_FLAG,
        "--lifeline-fd",
        str(lifeline_reader),
        "--control-fd",
        str(control_writer),
        "--auth-fd",
        str(auth_reader),
    ]
    for descriptor in held_lock_descriptors:
        wrapper_command.extend(("--held-lock-fd", str(descriptor)))
    for descriptor in child_directory_descriptors:
        wrapper_command.extend(("--child-directory-fd", str(descriptor)))
    wrapper_command.extend(("--", *command))
    wrapper: subprocess.Popen[bytes] | None = None
    control: SessionControl | None = None
    try:
        _write_all(
            auth_writer,
            f"{RESOURCE_GUARD_AUTH_MAGIC}:{token}\n".encode("ascii"),
        )
        _close_descriptor(auth_writer)
        auth_writer = -1
        wrapper = subprocess.Popen(
            wrapper_command,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            pass_fds=(
                auth_reader,
                lifeline_reader,
                control_writer,
                *held_lock_descriptors,
                *child_directory_descriptors,
            ),
            start_new_session=True,
            env=child_environment,
        )
        for descriptor in (auth_reader, lifeline_reader, control_writer):
            _close_descriptor(descriptor)
        auth_reader = -1
        lifeline_reader = -1
        control_writer = -1
        control = SessionControl(control_reader)
        control_reader = -1
        ready = control.read_line(
            timeout=SESSION_READY_TIMEOUT_SECONDS,
            description="lifeline wrapper readiness",
        )
        fields = ready.split()
        if len(fields) != 2 or fields[0] != "READY" or not fields[1].isdigit():
            raise GuardError("lifeline wrapper emitted invalid readiness")
        process_group_id = int(fields[1])
        if process_group_id <= 1 or process_group_id == wrapper.pid:
            raise GuardError("lifeline wrapper reported an invalid body process group")
        session = GuardedSession(wrapper, process_group_id, lifeline_writer, control)
        lifeline_writer = -1
        control = None
        return session
    except BaseException:
        _close_descriptor(lifeline_writer)
        lifeline_writer = -1
        if wrapper is not None:
            try:
                wrapper.wait(timeout=TERM_GRACE_SECONDS * 2 + 1)
            except subprocess.TimeoutExpired:
                try:
                    _terminate_owned_group(wrapper, wrapper.pid)
                except BaseException:
                    pass
        if control is not None:
            control.close()
        raise
    finally:
        for descriptor in (
            auth_reader,
            auth_writer,
            lifeline_reader,
            lifeline_writer,
            control_reader,
            control_writer,
        ):
            if descriptor >= 0:
                _close_descriptor(descriptor)


def _run_guarded(
    command: Sequence[str],
    *,
    report_path: Path,
    summary_path: Path,
    memory_limit_bytes: int = MAX_MEMORY_BYTES,
    maximum_memory_bytes: int = MAX_MEMORY_BYTES,
    absolute_memory_ceiling_bytes: int = MAX_CONFIGURABLE_MEMORY_BYTES,
    sample_interval_seconds: float = SAMPLE_INTERVAL_SECONDS,
    physical_footprint_interval_seconds: float = (
        PHYSICAL_FOOTPRINT_INTERVAL_SECONDS
    ),
    held_lock_descriptors: Sequence[int] = (),
    child_directory_descriptors: Sequence[int] = (),
    post_run_cleanup: Callable[[], int | None] | None = None,
    post_run_validation: Callable[[], None] | None = None,
    post_success_finalize: Callable[[], int | None] | None = None,
    report_context: Mapping[str, object] | None = None,
    child_environment: Mapping[str, str] | None = None,
) -> int:
    """Run one command under the formal resource and lifecycle policy."""

    if not command:
        raise GuardError("guarded command is empty")
    if not 0 < maximum_memory_bytes <= absolute_memory_ceiling_bytes:
        raise GuardError(
            "guard ceiling must be positive and no greater than the selected absolute maximum"
        )
    if not 0 < memory_limit_bytes <= maximum_memory_bytes:
        raise GuardError(
            "memory limit must be positive and no greater than the selected guard ceiling"
        )
    if sample_interval_seconds <= 0:
        raise GuardError("sample interval must be positive")
    if physical_footprint_interval_seconds <= 0:
        raise GuardError("physical-footprint interval must be positive")
    frozen_context: dict[str, object] | None = None
    if report_context is not None:
        try:
            encoded_context = _canonical_json(dict(report_context))
            decoded_context = json.loads(encoded_context)
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise GuardError("report context must be canonical JSON data") from error
        if not isinstance(decoded_context, dict):
            raise GuardError("report context must be a JSON object")
        frozen_context = decoded_context
    environment = os.environ.copy()
    if child_environment is not None:
        environment = dict(child_environment)
        if not all(
            isinstance(key, str) and isinstance(value, str)
            for key, value in environment.items()
        ):
            raise GuardError("child environment must contain only text keys and values")

    report = JsonlReport(report_path)
    session: GuardedSession | None = None
    started_monotonic = time.monotonic()
    started_utc = _utc_now()
    peak_memory_bytes = 0
    peak_rss_bytes = 0
    peak_footprint_bytes = 0
    sample_count = 0
    received_signal = 0
    exit_reason = "guard_error"
    child_exit_code: int | None = None
    final_status = 1
    cleanup_removed: int | None = None
    cleanup_status: str | None = None
    validation_status: str | None = None
    finalize_result: int | None = None
    finalize_status: str | None = None
    kernel_peak_rss_bytes = 0

    def receive_signal(signum: int, _frame: object) -> None:
        nonlocal received_signal
        if received_signal == 0:
            received_signal = signum

    watched_signals = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
    previous_handlers = {
        signum: signal.getsignal(signum) for signum in watched_signals
    }
    for signum in watched_signals:
        signal.signal(signum, receive_signal)

    try:
        report.write(
            {
                "event": "start",
                "memory_limit_bytes": memory_limit_bytes,
                "physical_footprint_interval_seconds": (
                    physical_footprint_interval_seconds
                ),
                "report_context": frozen_context,
                "sample_interval_seconds": sample_interval_seconds,
                "schema_version": 1,
                "started_utc": started_utc,
                "supervisor_pid": os.getpid(),
            }
        )
        foreign = _foreign_heavy_jobs(_process_rows())
        if foreign:
            first = foreign[0]
            exit_reason = "foreign_heavy_job"
            final_status = FOREIGN_JOB_EXIT_CODE
            _record_foreign_heavy_job(
                report,
                first,
                phase="pre_spawn",
                owned_process_group_id=None,
            )
            raise GuardError(
                "pre-existing TLAPM/Isabelle/Poly/Kagemusha job is outside this guard "
                f"(pid={first.pid}, pgid={first.process_group_id}, "
                f"command={Path(first.command).name})"
            )

        session = _spawn_guarded_session(
            list(command),
            environment,
            held_lock_descriptors,
            child_directory_descriptors,
        )
        report.write(
            {
                "event": "spawn",
                "process_group_id": session.process_group_id,
                "schema_version": 1,
                "timestamp_utc": _utc_now(),
                "wrapper_pid": session.wrapper.pid,
            }
        )
        next_sample = time.monotonic()
        next_physical_footprint = (
            next_sample + physical_footprint_interval_seconds
        )
        while True:
            if received_signal:
                exit_reason = "signal"
                final_status = min(255, 128 + received_signal)
                _terminate_owned_group(session.wrapper, session.process_group_id)
                (
                    child_exit_code,
                    _lingering,
                    kernel_peak_rss_bytes,
                ) = _read_guarded_exit(session)
                break

            now = time.monotonic()
            if now >= next_sample:
                process_rows = _process_rows()
                foreign = _foreign_heavy_jobs(
                    process_rows,
                    owned_process_group_id=session.process_group_id,
                )
                if foreign:
                    first = foreign[0]
                    exit_reason = "foreign_heavy_job"
                    final_status = FOREIGN_JOB_EXIT_CODE
                    _kill_owned_group_immediately(
                        session.wrapper, session.process_group_id
                    )
                    (
                        child_exit_code,
                        _lingering,
                        kernel_peak_rss_bytes,
                    ) = _read_guarded_exit(session)
                    _record_foreign_heavy_job(
                        report,
                        first,
                        phase="runtime",
                        owned_process_group_id=session.process_group_id,
                    )
                    break
                include_physical_footprint = now >= next_physical_footprint
                sample = _sample_group(
                    session.process_group_id,
                    process_rows,
                    include_physical_footprint=include_physical_footprint,
                )
                if include_physical_footprint:
                    next_physical_footprint = (
                        time.monotonic() + physical_footprint_interval_seconds
                    )
                sample_count += 1
                peak_memory_bytes = max(peak_memory_bytes, sample.memory_bytes)
                peak_rss_bytes = max(peak_rss_bytes, sample.rss_bytes)
                peak_footprint_bytes = max(
                    peak_footprint_bytes, sample.physical_footprint_bytes
                )
                report.write(
                    {
                        "accounting_method": sample.accounting_method,
                        "elapsed_seconds": round(now - started_monotonic, 6),
                        "event": "sample",
                        "memory_bytes": sample.memory_bytes,
                        "memory_limit_bytes": memory_limit_bytes,
                        "physical_footprint_bytes": sample.physical_footprint_bytes,
                        "process_count": sample.process_count,
                        "process_group_id": session.process_group_id,
                        "rss_bytes": sample.rss_bytes,
                        "schema_version": 1,
                        "timestamp_utc": _utc_now(),
                    }
                )
                if sample.memory_bytes > memory_limit_bytes:
                    exit_reason = "memory_limit"
                    final_status = MEMORY_LIMIT_EXIT_CODE
                    _kill_owned_group_immediately(
                        session.wrapper, session.process_group_id
                    )
                    (
                        child_exit_code,
                        _lingering,
                        kernel_peak_rss_bytes,
                    ) = _read_guarded_exit(session)
                    break
                # Schedule from probe completion. Reusing the timestamp from
                # before ``ps``/``footprint`` can create an immediate catch-up
                # storm whenever an inspection exceeds the target cadence.
                next_sample = time.monotonic() + sample_interval_seconds

            wrapper_returncode = session.wrapper.poll()
            if wrapper_returncode is not None:
                child_exit_code, lingering, kernel_peak_rss_bytes = (
                    _read_guarded_exit(session)
                )
                if lingering:
                    exit_reason = "lingering_process_group"
                    final_status = 1
                else:
                    exit_reason = (
                        "completed" if child_exit_code == 0 else "child_exit"
                    )
                    final_status = _exit_status(child_exit_code)
                if kernel_peak_rss_bytes > memory_limit_bytes:
                    exit_reason = "kernel_memory_limit"
                    final_status = MEMORY_LIMIT_EXIT_CODE
                break
            time.sleep(min(0.05, max(0.0, next_sample - time.monotonic())))
    except BaseException as error:
        if session is not None:
            try:
                _terminate_owned_group(session.wrapper, session.process_group_id)
            except BaseException as cleanup_error:
                print(f"TLAPS guard cleanup failed: {cleanup_error}", file=sys.stderr)
            child_exit_code = session.wrapper.returncode
        if isinstance(error, GuardError) and exit_reason == "foreign_heavy_job":
            pass
        else:
            exit_reason = "guard_error"
            final_status = 1
        print(f"TLAPS guard failed: {error}", file=sys.stderr)
    finally:
        if session is not None:
            session.close()
        if post_run_validation is not None:
            try:
                post_run_validation()
                validation_status = "completed"
            except BaseException as error:
                validation_status = "failed"
                if final_status == 0:
                    exit_reason = "post_run_validation_error"
                    final_status = 1
                print(f"resource guard post-run validation failed: {error}", file=sys.stderr)
        if final_status == 0:
            try:
                foreign = _foreign_heavy_jobs(_process_rows())
                if foreign:
                    first = foreign[0]
                    exit_reason = "foreign_heavy_job"
                    final_status = FOREIGN_JOB_EXIT_CODE
                    _record_foreign_heavy_job(
                        report,
                        first,
                        phase="final_success_gate",
                        owned_process_group_id=None,
                    )
            except BaseException as error:
                exit_reason = "guard_error"
                final_status = 1
                print(
                    f"resource guard final foreign-job inspection failed: {error}",
                    file=sys.stderr,
                )
        if post_success_finalize is not None:
            if final_status == 0:
                try:
                    finalize_result = post_success_finalize()
                    finalize_status = "completed"
                except BaseException as error:
                    finalize_status = "failed"
                    exit_reason = "post_success_finalize_error"
                    final_status = 1
                    print(
                        f"resource guard post-success finalize failed: {error}",
                        file=sys.stderr,
                    )
            else:
                finalize_status = "skipped"
        if post_run_cleanup is not None:
            try:
                cleanup_removed = post_run_cleanup()
                cleanup_status = "completed"
            except BaseException as error:
                cleanup_status = "failed"
                exit_reason = "post_run_cleanup_error"
                final_status = 1
                print(f"resource guard post-run cleanup failed: {error}", file=sys.stderr)
        for signum, handler in previous_handlers.items():
            signal.signal(signum, handler)
        ended_utc = _utc_now()
        summary: dict[str, object] = {
            "child_exit_code": child_exit_code,
            "ended_utc": ended_utc,
            "event": "summary",
            "exit_reason": exit_reason,
            "exit_status": final_status,
            "evidence_peak_rss_bytes": max(peak_rss_bytes, kernel_peak_rss_bytes),
            "kernel_peak_rss_bytes": kernel_peak_rss_bytes,
            "kernel_peak_rss_method": (
                "wait4_ru_maxrss" if kernel_peak_rss_bytes > 0 else "unavailable"
            ),
            "kernel_peak_rss_scope": "direct_guarded_body",
            "memory_limit_bytes": memory_limit_bytes,
            "physical_footprint_interval_seconds": (
                physical_footprint_interval_seconds
            ),
            "peak_memory_bytes": peak_memory_bytes,
            "peak_physical_footprint_bytes": peak_footprint_bytes,
            "peak_rss_bytes": peak_rss_bytes,
            "report_context": frozen_context,
            "sample_count": sample_count,
            "sample_interval_seconds": sample_interval_seconds,
            "schema_version": 1,
            "started_utc": started_utc,
            "supervisor_pid": os.getpid(),
        }
        if cleanup_status is not None:
            summary["post_run_cleanup"] = cleanup_status
            summary["post_run_cleanup_removed"] = cleanup_removed
        if validation_status is not None:
            summary["post_run_validation"] = validation_status
        if finalize_status is not None:
            summary["post_success_finalize"] = finalize_status
            summary["post_success_finalize_result"] = finalize_result
        try:
            report.write(summary)
            _write_summary(summary_path, summary)
        finally:
            report.close()
    return final_status


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jsonl", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    return parser


def main() -> int:
    """Acquire the host lock and supervise the requested TLAPS body."""

    if len(sys.argv) > 1 and sys.argv[1] == SESSION_WRAPPER_FLAG:
        return _run_session_wrapper(sys.argv[2:])
    args = _parser().parse_args()
    command = list(args.command)
    if command and command[0] == "--":
        command.pop(0)
    try:
        with _host_lock(
            HEAVY_JOB_LOCK_PATH, description="memory-heavy job"
        ) as heavy_lock:
            with _host_lock() as tlaps_lock:
                return _run_guarded(
                    command,
                    report_path=args.jsonl,
                    summary_path=args.summary,
                    held_lock_descriptors=(heavy_lock, tlaps_lock),
                )
    except LockUnavailable as error:
        print(f"TLAPS guard refused to start: {error}", file=sys.stderr)
        return LOCK_UNAVAILABLE_EXIT_CODE
    except (GuardError, OSError) as error:
        print(f"TLAPS guard failed closed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
