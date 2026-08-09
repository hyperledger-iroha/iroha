#!/usr/bin/env python3
"""Run the Sumeragi TLAPS body under cooperative resource observation."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import ctypes
from dataclasses import dataclass
from datetime import datetime, timezone
import errno
import fcntl
import json
import os
from pathlib import Path
import secrets
import select
import selectors
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
# Generic TLAPS runs retain a slower physical-footprint cadence. The Kagemusha
# release runner explicitly selects the RSS cadence because Darwin libproc
# exposes physical footprint without suspending the inspected process.
PHYSICAL_FOOTPRINT_INTERVAL_SECONDS = 5.0
MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT = "max_rss_physical_footprint"
MEMORY_ENFORCEMENT_PROCESS_TREE_RSS = "process_tree_rss"
MEMORY_ENFORCEMENT_MODES = frozenset(
    {
        MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT,
        MEMORY_ENFORCEMENT_PROCESS_TREE_RSS,
    }
)
CONTROL_RECORD_TIMEOUT_SECONDS = 0.2
# macOS does not provide a short completion guarantee for ``ps`` under
# proof-generation memory and APFS write pressure. Keep full-host admission and
# final probes bounded. Runtime supervision uses the same observation-only
# snapshot path so it never needs process-group creation or control.
PROCESS_INSPECTION_TIMEOUT_SECONDS = 10.0
PROCESS_INSPECTION_OUTPUT_LIMIT_BYTES = 4 * 1024 * 1024
SESSION_READY_TIMEOUT_SECONDS = 2.0
SESSION_OBSERVATION_INTERVAL_SECONDS = 0.05
MEMORY_LIMIT_EXIT_CODE = 75
LOCK_UNAVAILABLE_EXIT_CODE = 73
FOREIGN_JOB_EXIT_CODE = 74
CANCELLATION_EXIT_CODE = 125
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
    None,
)
PROCESS_INSPECTION_ENVIRONMENT = {
    "HOME": "/var/empty",
    "LANG": "C",
    "LC_ALL": "C",
    "PATH": "/usr/bin:/bin",
}
DARWIN_LIBPROC = "/usr/lib/libproc.dylib"
DARWIN_RUSAGE_INFO_V4 = 4
DARWIN_PROC_PIDTBSDINFO = 3
# The Kagemusha body is one Rayon process and TLAPS has only a small backend
# tree. Treat reaching this fixed buffer as an accounting failure rather than
# silently omitting a newly forked process.
DARWIN_PROCESS_GROUP_PID_CAPACITY = 256
DARWIN_STABLE_SNAPSHOT_ATTEMPTS = 3
LOCK_PATH = Path("/tmp") / f"iroha-sumeragi-v2-tlapm-{os.getuid()}.lock"
HEAVY_JOB_LOCK_PATH = Path("/tmp") / f"iroha-memory-heavy-{os.getuid()}.lock"


class GuardError(RuntimeError):
    """Raised when the guard cannot safely supervise the requested command."""


class GuardViolation(GuardError):
    """A bounded-observation violation whose verdict is latched by the guard."""

    def __init__(self, reason: str, message: str) -> None:
        super().__init__(message)
        self.reason = reason


class DarwinProcessUnavailable(GuardError):
    """A scoped Darwin accounting read raced with natural process exit."""


class LockUnavailable(GuardError):
    """Raised when another guarded TLAPS run owns the per-user lock."""


@dataclass(frozen=True)
class ProcessRow:
    """One process snapshot returned by host or scoped kernel accounting."""

    pid: int
    parent_pid: int
    process_group_id: int
    uid: int
    rss_bytes: int
    command: str
    physical_footprint_bytes: int | None = None


@dataclass(frozen=True)
class DarwinProcessIdentity:
    """Stable Darwin identity used to reject PID and process-group reuse."""

    pid: int
    parent_pid: int
    process_group_id: int
    effective_uid: int
    real_uid: int
    start_time_seconds: int
    start_time_microseconds: int
    command: str

    def stable_key(self) -> tuple[int, int, int, int, int]:
        """Return fields which cannot change during one process lifetime."""

        return (
            self.pid,
            self.process_group_id,
            self.real_uid,
            self.start_time_seconds,
            self.start_time_microseconds,
        )


@dataclass(frozen=True)
class DarwinProcessMemory:
    """One Darwin task's kernel-accounted resident and footprint evidence."""

    rss_bytes: int
    physical_footprint_bytes: int


@dataclass(frozen=True)
class MemorySample:
    """Aggregate memory attributed to the supervised process group."""

    memory_bytes: int
    rss_bytes: int
    physical_footprint_bytes: int
    process_count: int
    accounting_method: str


class _DarwinRusageInfoV4(ctypes.Structure):
    """Darwin ``rusage_info_v4`` layout from ``sys/resource.h``."""

    _fields_ = [
        ("ri_uuid", ctypes.c_uint8 * 16),
        *[
            (name, ctypes.c_uint64)
            for name in (
                "ri_user_time",
                "ri_system_time",
                "ri_pkg_idle_wkups",
                "ri_interrupt_wkups",
                "ri_pageins",
                "ri_wired_size",
                "ri_resident_size",
                "ri_phys_footprint",
                "ri_proc_start_abstime",
                "ri_proc_exit_abstime",
                "ri_child_user_time",
                "ri_child_system_time",
                "ri_child_pkg_idle_wkups",
                "ri_child_interrupt_wkups",
                "ri_child_pageins",
                "ri_child_elapsed_abstime",
                "ri_diskio_bytesread",
                "ri_diskio_byteswritten",
                "ri_cpu_time_qos_default",
                "ri_cpu_time_qos_maintenance",
                "ri_cpu_time_qos_background",
                "ri_cpu_time_qos_utility",
                "ri_cpu_time_qos_legacy",
                "ri_cpu_time_qos_user_initiated",
                "ri_cpu_time_qos_user_interactive",
                "ri_billed_system_time",
                "ri_serviced_system_time",
                "ri_logical_writes",
                "ri_lifetime_max_phys_footprint",
                "ri_instructions",
                "ri_cycles",
                "ri_billed_energy",
                "ri_serviced_energy",
                "ri_interval_max_phys_footprint",
                "ri_runnable_time",
            )
        ],
    ]


class _DarwinProcBsdInfo(ctypes.Structure):
    """Darwin ``proc_bsdinfo`` layout from ``sys/proc_info.h``."""

    _fields_ = [
        ("pbi_flags", ctypes.c_uint32),
        ("pbi_status", ctypes.c_uint32),
        ("pbi_xstatus", ctypes.c_uint32),
        ("pbi_pid", ctypes.c_uint32),
        ("pbi_ppid", ctypes.c_uint32),
        ("pbi_uid", ctypes.c_uint32),
        ("pbi_gid", ctypes.c_uint32),
        ("pbi_ruid", ctypes.c_uint32),
        ("pbi_rgid", ctypes.c_uint32),
        ("pbi_svuid", ctypes.c_uint32),
        ("pbi_svgid", ctypes.c_uint32),
        ("rfu_1", ctypes.c_uint32),
        ("pbi_comm", ctypes.c_char * 16),
        ("pbi_name", ctypes.c_char * 32),
        ("pbi_nfiles", ctypes.c_uint32),
        ("pbi_pgid", ctypes.c_uint32),
        ("pbi_pjobc", ctypes.c_uint32),
        ("e_tdev", ctypes.c_uint32),
        ("e_tpgid", ctypes.c_uint32),
        ("pbi_nice", ctypes.c_int32),
        ("pbi_start_tvsec", ctypes.c_uint64),
        ("pbi_start_tvusec", ctypes.c_uint64),
    ]


_darwin_libproc: ctypes.CDLL | None = None


class SessionControl:
    """Bounded control channel from the cooperative session wrapper."""

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
                    raise GuardViolation(
                        "control_output_failure", f"{description} is not ASCII"
                    ) from error
            if len(self._buffer) >= 256:
                raise GuardViolation(
                    "control_output_limit", f"{description} exceeds 255 bytes"
                )
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise GuardViolation(
                    "control_timeout", f"timed out waiting for {description}"
                )
            readable, _, _ = select.select([self._descriptor], [], [], remaining)
            if not readable:
                raise GuardViolation(
                    "control_timeout", f"timed out waiting for {description}"
                )
            chunk = os.read(self._descriptor, 256 - len(self._buffer))
            if not chunk:
                raise GuardViolation(
                    "control_output_failure",
                    f"session wrapper closed before {description}",
                )
            self._buffer.extend(chunk)

    def drain_to_eof(self) -> bytes:
        """Drain the wrapper channel after natural completion, retaining one cap."""

        retained = bytearray(self._buffer)
        self._buffer.clear()
        output_limit_exceeded = len(retained) > 256
        while True:
            chunk = os.read(self._descriptor, 4096)
            if not chunk:
                break
            capacity = max(256 - len(retained), 0)
            retained.extend(chunk[:capacity])
            if len(chunk) > capacity:
                output_limit_exceeded = True
        if output_limit_exceeded:
            raise GuardViolation(
                "control_output_limit", "session wrapper output exceeds 256 bytes"
            )
        return bytes(retained)

    def close(self) -> None:
        """Close the control descriptor."""

        if self._descriptor >= 0:
            os.close(self._descriptor)
            self._descriptor = -1


@dataclass
class GuardedSession:
    """The cooperative wrapper process plus its naturally completing body."""

    wrapper: subprocess.Popen[bytes]
    process_group_id: int
    lifeline_writer: int
    control: SessionControl
    body_identity: DarwinProcessIdentity | None = None

    def close(self) -> None:
        """Release control pipes only after the wrapper finishes naturally."""

        if self.lifeline_writer >= 0:
            os.close(self.lifeline_writer)
            self.lifeline_writer = -1
        self.wrapper.wait()
        try:
            self.control.drain_to_eof()
        finally:
            self.control.close()


def _read_guarded_exit(
    session: GuardedSession,
) -> tuple[int, bool, bool, bool, int]:
    """Read the wrapper's natural child status and latched observations."""

    wrapper_exit = session.control.read_line(
        timeout=CONTROL_RECORD_TIMEOUT_SECONDS,
        description="lifeline wrapper exit status",
    )
    fields = wrapper_exit.split()
    if (
        len(fields) != 6
        or fields[0] != "EXIT"
        or fields[2] not in {"0", "1"}
        or fields[3] not in {"0", "1"}
        or fields[4] not in {"0", "1"}
        or not fields[5].isdigit()
    ):
        raise GuardViolation(
            "control_output_failure", "session wrapper emitted invalid exit status"
        )
    try:
        child_exit_code = int(fields[1])
        kernel_peak_rss_bytes = int(fields[5])
    except ValueError as error:
        raise GuardViolation(
            "control_output_failure",
            "session wrapper emitted a non-integer child status",
        ) from error
    trailing = session.control.drain_to_eof()
    if trailing:
        raise GuardViolation(
            "control_output_failure",
            "session wrapper emitted unexpected trailing output",
        )
    return (
        child_exit_code,
        fields[2] == "1",
        fields[3] == "1",
        fields[4] == "1",
        kernel_peak_rss_bytes,
    )


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


def _run_bounded_observation(
    argv: Sequence[str],
    *,
    timeout_seconds: float,
    maximum_output_bytes: int,
    environment: Mapping[str, str],
) -> subprocess.CompletedProcess[bytes]:
    """Drain a bounded observation command naturally and latch its verdict."""

    try:
        process = subprocess.Popen(
            list(argv),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            env=dict(environment),
        )
    except OSError as error:
        raise GuardError("could not start process inspection") from error
    assert process.stdout is not None and process.stderr is not None
    os.set_blocking(process.stdout.fileno(), False)
    os.set_blocking(process.stderr.fileno(), False)
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    retained_bytes = 0
    deadline = time.monotonic() + timeout_seconds
    latched_reason: str | None = None
    cancellation_latched = False
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0 and latched_reason is None:
                latched_reason = "process_inspection_timeout"
            try:
                events = selector.select(
                    1.0 if remaining <= 0 else min(remaining, 1.0)
                )
            except KeyboardInterrupt:
                cancellation_latched = True
                continue
            for key, _ in events:
                try:
                    chunk = os.read(key.fileobj.fileno(), 64 * 1024)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                capacity = max(maximum_output_bytes - retained_bytes, 0)
                retained = chunk[:capacity]
                buffers[key.data].extend(retained)
                retained_bytes += len(retained)
                if len(retained) != len(chunk) and latched_reason is None:
                    latched_reason = "process_inspection_output_limit"
        returncode = process.wait()
        if time.monotonic() > deadline and latched_reason is None:
            latched_reason = "process_inspection_timeout"
    finally:
        selector.close()
        process.stdout.close()
        process.stderr.close()
    if cancellation_latched:
        raise GuardViolation(
            "cancellation", "process inspection was cooperatively cancelled"
        )
    if latched_reason == "process_inspection_timeout":
        raise GuardViolation(
            latched_reason,
            "process inspection exceeded "
            f"{timeout_seconds:g} s before natural completion",
        )
    if latched_reason == "process_inspection_output_limit":
        raise GuardViolation(
            latched_reason,
            "process inspection exceeded its retained-output limit before "
            "natural completion",
        )
    return subprocess.CompletedProcess(
        list(argv),
        returncode,
        stdout=bytes(buffers["stdout"]),
        stderr=bytes(buffers["stderr"]),
    )


def _process_rows() -> list[ProcessRow]:
    """Snapshot process identity, ownership, grouping, and RSS."""

    if PS is None:
        raise GuardError("absolute process-inspection utility is unavailable")
    completed = _run_bounded_observation(
        [PS, "-axo", "pid=,ppid=,pgid=,uid=,rss=,comm="],
        timeout_seconds=PROCESS_INSPECTION_TIMEOUT_SECONDS,
        maximum_output_bytes=PROCESS_INSPECTION_OUTPUT_LIMIT_BYTES,
        environment=PROCESS_INSPECTION_ENVIRONMENT,
    )
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", errors="replace").strip()
        detail = detail or f"exit status {completed.returncode}"
        raise GuardError(f"could not inspect processes with ps: {detail}")
    rows: list[ProcessRow] = []
    for raw_line in completed.stdout.splitlines():
        line = raw_line.decode("utf-8", errors="replace")
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


def _darwin_libproc_handle() -> ctypes.CDLL:
    """Load and type Darwin's scoped process-accounting entry points."""

    global _darwin_libproc
    try:
        if _darwin_libproc is None:
            _darwin_libproc = ctypes.CDLL(DARWIN_LIBPROC, use_errno=True)
        _darwin_libproc.proc_listpgrppids.argtypes = (
            ctypes.c_int,
            ctypes.c_void_p,
            ctypes.c_int,
        )
        _darwin_libproc.proc_listpgrppids.restype = ctypes.c_int
        _darwin_libproc.proc_pidinfo.argtypes = (
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_uint64,
            ctypes.c_void_p,
            ctypes.c_int,
        )
        _darwin_libproc.proc_pidinfo.restype = ctypes.c_int
        _darwin_libproc.proc_pid_rusage.argtypes = (
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_void_p,
        )
        _darwin_libproc.proc_pid_rusage.restype = ctypes.c_int
    except (AttributeError, OSError) as error:
        raise GuardError("could not load Darwin process accounting") from error
    return _darwin_libproc


def _darwin_list_process_group_pids(process_group_id: int) -> tuple[int, ...]:
    """Enumerate one PGID into a fixed buffer, rejecting any truncation."""

    if process_group_id <= 1:
        raise GuardError("refusing to inspect an invalid owned process group")
    pid_buffer = (ctypes.c_int * DARWIN_PROCESS_GROUP_PID_CAPACITY)()
    libproc = _darwin_libproc_handle()
    ctypes.set_errno(0)
    count = libproc.proc_listpgrppids(
        process_group_id,
        pid_buffer,
        ctypes.sizeof(pid_buffer),
    )
    if count < 0:
        error_number = ctypes.get_errno()
        detail = os.strerror(error_number) if error_number else "unknown error"
        raise GuardError(
            "could not enumerate owned Darwin process group "
            f"{process_group_id}: {detail}"
        )
    if count >= DARWIN_PROCESS_GROUP_PID_CAPACITY:
        raise GuardError(
            "owned Darwin process-group PID buffer saturated; refusing "
            "incomplete accounting"
        )
    process_ids = tuple(sorted(int(pid_buffer[index]) for index in range(count)))
    if any(process_id <= 1 for process_id in process_ids):
        raise GuardError("owned Darwin process-group enumeration returned an invalid PID")
    if len(set(process_ids)) != len(process_ids):
        raise GuardError("owned Darwin process-group enumeration returned duplicate PIDs")
    return process_ids


def _darwin_process_identity(process_id: int) -> DarwinProcessIdentity:
    """Read one process's real ownership, grouping, and start identity."""

    if process_id <= 1:
        raise GuardError("refusing to inspect an invalid process identifier")
    info = _DarwinProcBsdInfo()
    libproc = _darwin_libproc_handle()
    ctypes.set_errno(0)
    result = libproc.proc_pidinfo(
        process_id,
        DARWIN_PROC_PIDTBSDINFO,
        0,
        ctypes.byref(info),
        ctypes.sizeof(info),
    )
    if result != ctypes.sizeof(info):
        error_number = ctypes.get_errno()
        detail = os.strerror(error_number) if error_number else "short result"
        raise GuardError(
            f"could not inspect Darwin identity for pid {process_id}: {detail}"
        )
    command_bytes = bytes(info.pbi_name) or bytes(info.pbi_comm)
    command = command_bytes.split(b"\0", 1)[0].decode("utf-8", errors="replace")
    return DarwinProcessIdentity(
        pid=int(info.pbi_pid),
        parent_pid=int(info.pbi_ppid),
        process_group_id=int(info.pbi_pgid),
        effective_uid=int(info.pbi_uid),
        real_uid=int(info.pbi_ruid),
        start_time_seconds=int(info.pbi_start_tvsec),
        start_time_microseconds=int(info.pbi_start_tvusec),
        command=command,
    )


def _validate_darwin_process_identity(
    identity: DarwinProcessIdentity,
    *,
    requested_process_id: int,
    process_group_id: int,
) -> None:
    """Reject foreign, reused, or malformed entries from a scoped PID list."""

    if identity.pid != requested_process_id or identity.pid <= 1:
        raise GuardError("Darwin process identity did not match the enumerated PID")
    if identity.parent_pid < 0 or identity.start_time_seconds <= 0:
        raise GuardError("Darwin process identity was malformed")
    if identity.process_group_id != process_group_id:
        raise GuardError(
            "Darwin process-group enumeration contained a foreign process group"
        )
    if identity.real_uid != os.getuid():
        raise GuardError("Darwin process-group enumeration contained a foreign real UID")
    if identity.effective_uid != os.geteuid():
        raise GuardError(
            "Darwin process-group enumeration contained a foreign effective UID"
        )


def _capture_darwin_body_identity(
    process_group_id: int, wrapper_process_id: int
) -> DarwinProcessIdentity:
    """Pin the body leader identity before runtime accounting begins."""

    identity = _darwin_process_identity(process_group_id)
    _validate_darwin_process_identity(
        identity,
        requested_process_id=process_group_id,
        process_group_id=process_group_id,
    )
    if identity.parent_pid != wrapper_process_id:
        raise GuardError("Darwin guarded body was not owned by its lifeline wrapper")
    return identity


def _darwin_process_memory(process_id: int) -> DarwinProcessMemory:
    """Read one task's resident bytes and physical-footprint high water."""

    if process_id <= 1:
        raise GuardError("refusing to inspect an invalid process identifier")
    usage = _DarwinRusageInfoV4()
    libproc = _darwin_libproc_handle()
    ctypes.set_errno(0)
    result = libproc.proc_pid_rusage(
        process_id,
        DARWIN_RUSAGE_INFO_V4,
        ctypes.byref(usage),
    )
    if result != 0:
        error_number = ctypes.get_errno()
        detail = os.strerror(error_number) if error_number else "unknown error"
        error_type = (
            DarwinProcessUnavailable
            if error_number == errno.ESRCH
            else GuardError
        )
        raise error_type(
            f"could not inspect Darwin resource usage for pid {process_id}: {detail}"
        )
    return DarwinProcessMemory(
        rss_bytes=max(0, int(usage.ri_resident_size)),
        physical_footprint_bytes=max(
            0,
            int(usage.ri_phys_footprint),
            int(usage.ri_lifetime_max_phys_footprint),
            int(usage.ri_interval_max_phys_footprint),
        ),
    )


def _darwin_process_group_rows(
    process_group_id: int,
    *,
    expected_body_identity: DarwinProcessIdentity | None = None,
) -> list[ProcessRow]:
    """Return a stable, real-UID-checked libproc snapshot of one owned PGID."""

    if process_group_id <= 1:
        raise GuardError("refusing to inspect an invalid owned process group")
    if (
        expected_body_identity is not None
        and expected_body_identity.pid != process_group_id
    ):
        raise GuardError("expected Darwin body identity does not lead the owned group")

    last_race_error: GuardError | None = None
    for attempt in range(DARWIN_STABLE_SNAPSHOT_ATTEMPTS):
        process_ids = _darwin_list_process_group_pids(process_group_id)
        if not process_ids:
            if _process_group_exists(process_group_id):
                raise GuardError(
                    "Darwin scoped accounting omitted an existing process group"
                )
            return []
        if process_group_id not in process_ids:
            raise GuardError("Darwin scoped accounting omitted the guarded body leader")

        identities: dict[int, DarwinProcessIdentity] = {}
        memory_by_pid: dict[int, DarwinProcessMemory] = {}
        try:
            for process_id in process_ids:
                identity = _darwin_process_identity(process_id)
                _validate_darwin_process_identity(
                    identity,
                    requested_process_id=process_id,
                    process_group_id=process_group_id,
                )
                identities[process_id] = identity
                memory_by_pid[process_id] = _darwin_process_memory(process_id)
        except GuardError as error:
            # A normal fork/exit race changes the scoped membership. Retry only
            # for a small fixed count. Darwin can retain a just-exited PID in
            # proc_listpgrppids for one turn after proc_pidinfo stops exposing
            # it; persistent errors still fail closed.
            last_race_error = error
            refreshed_process_ids = _darwin_list_process_group_pids(
                process_group_id
            )
            if not refreshed_process_ids and not _process_group_exists(
                process_group_id
            ):
                return []
            if attempt + 1 < DARWIN_STABLE_SNAPSHOT_ATTEMPTS:
                continue
            raise

        final_process_ids = _darwin_list_process_group_pids(process_group_id)
        if final_process_ids != process_ids:
            continue
        final_identities: dict[int, DarwinProcessIdentity] = {}
        try:
            for process_id in process_ids:
                final_identities[process_id] = _darwin_process_identity(process_id)
        except GuardError as error:
            last_race_error = error
            refreshed_process_ids = _darwin_list_process_group_pids(
                process_group_id
            )
            if not refreshed_process_ids and not _process_group_exists(
                process_group_id
            ):
                return []
            if attempt + 1 < DARWIN_STABLE_SNAPSHOT_ATTEMPTS:
                continue
            raise
        for process_id, final_identity in final_identities.items():
            _validate_darwin_process_identity(
                final_identity,
                requested_process_id=process_id,
                process_group_id=process_group_id,
            )
            if final_identity.stable_key() != identities[process_id].stable_key():
                raise GuardError(
                    f"Darwin PID {process_id} changed identity during accounting"
                )

        body_identity = identities[process_group_id]
        if expected_body_identity is not None:
            if body_identity.stable_key() != expected_body_identity.stable_key():
                raise GuardError("Darwin guarded body identity changed during supervision")
            if body_identity.parent_pid != expected_body_identity.parent_pid:
                raise GuardError("Darwin guarded body changed lifeline parent")

        return [
            ProcessRow(
                pid=process_id,
                parent_pid=identities[process_id].parent_pid,
                process_group_id=process_group_id,
                uid=identities[process_id].real_uid,
                rss_bytes=memory_by_pid[process_id].rss_bytes,
                command=identities[process_id].command,
                physical_footprint_bytes=(
                    memory_by_pid[process_id].physical_footprint_bytes
                ),
            )
            for process_id in process_ids
        ]

    raise GuardError("Darwin process-group membership did not stabilize") from last_race_error


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
    owned_process_ids: frozenset[int] | set[int] | None = None,
) -> list[ProcessRow]:
    """Find same-user heavy jobs outside the observed owned process tree."""

    uid = os.getuid()
    excluded_process_ids = frozenset(owned_process_ids or ())
    return sorted(
        (
            row
            for row in rows
            if row.uid == uid
            and _is_formal_heavy_process(row)
            and row.pid not in excluded_process_ids
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
    """Persist one foreign-heavy-job conflict without controlling that job."""

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
    *,
    expected_body_identity: DarwinProcessIdentity | None = None,
    known_owned_process_ids: set[int] | None = None,
) -> list[ProcessRow]:
    """Return the current same-UID descendants of one guarded root PID.

    ``process_group_id`` remains the evidence-field name for schema stability;
    cooperative supervision treats its value as the guarded root PID and never
    creates or controls an operating-system process group.
    """

    del expected_body_identity
    snapshot = _process_rows() if rows is None else rows
    owned = known_owned_process_ids if known_owned_process_ids is not None else set()
    owned.add(process_group_id)
    uid = os.getuid()
    changed = True
    while changed:
        changed = False
        for row in snapshot:
            if row.uid == uid and row.pid not in owned and row.parent_pid in owned:
                owned.add(row.pid)
                changed = True
    return [row for row in snapshot if row.uid == uid and row.pid in owned]


def _darwin_process_physical_footprint_bytes(process_id: int) -> int:
    """Return one process's kernel-accounted footprint high water."""

    return _darwin_process_memory(process_id).physical_footprint_bytes


def _physical_footprint_bytes(process_ids: Sequence[int]) -> int:
    """Return Darwin footprint high water, tolerating a confirmed exit race."""

    if sys.platform != "darwin" or not process_ids:
        return 0
    total = 0
    for process_id in sorted(set(process_ids)):
        try:
            total += _darwin_process_physical_footprint_bytes(process_id)
        except DarwinProcessUnavailable:
            # A process can exit after the authenticated tree snapshot and
            # before proc_pid_rusage. Confirm exact PID absence in a fresh
            # observation; a still-present or reused same-UID PID remains a
            # fail-closed accounting error.
            if any(
                row.pid == process_id and row.uid == os.getuid()
                for row in _process_rows()
            ):
                raise
    return total


def _sample_group(
    process_group_id: int,
    rows: Sequence[ProcessRow] | None = None,
    *,
    include_physical_footprint: bool = True,
    memory_enforcement_mode: str = MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT,
    expected_body_identity: DarwinProcessIdentity | None = None,
    known_owned_process_ids: set[int] | None = None,
) -> MemorySample:
    """Measure RSS and footprint, selecting the configured enforcement value."""

    if memory_enforcement_mode not in MEMORY_ENFORCEMENT_MODES:
        raise GuardError("unknown memory enforcement mode")

    if expected_body_identity is None:
        group_rows = _group_rows(
            process_group_id,
            rows,
            known_owned_process_ids=known_owned_process_ids,
        )
    else:
        group_rows = _group_rows(
            process_group_id,
            rows,
            expected_body_identity=expected_body_identity,
            known_owned_process_ids=known_owned_process_ids,
        )
    rss_bytes = sum(row.rss_bytes for row in group_rows)
    scoped_footprints_available = all(
        row.physical_footprint_bytes is not None for row in group_rows
    ) and bool(group_rows)
    if scoped_footprints_available:
        # proc_pid_rusage supplies resident size and footprint together. Darwin
        # therefore enforces the footprint on every cheap scoped sample rather
        # than deferring data already obtained from the kernel.
        footprint_bytes = sum(
            int(row.physical_footprint_bytes or 0) for row in group_rows
        )
    elif include_physical_footprint:
        footprint_bytes = _physical_footprint_bytes(
            [row.pid for row in group_rows]
        )
    else:
        footprint_bytes = 0
    if memory_enforcement_mode == MEMORY_ENFORCEMENT_PROCESS_TREE_RSS:
        memory_bytes = rss_bytes
        method = MEMORY_ENFORCEMENT_PROCESS_TREE_RSS
    elif footprint_bytes > 0:
        memory_bytes = max(rss_bytes, footprint_bytes)
        method = MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT
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
    """Observe whether an exact process group is present in a fresh snapshot."""

    return any(
        row.process_group_id == process_group_id and row.uid == os.getuid()
        for row in _process_rows()
    )


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
    """Observe the supervisor lifeline and let the complete body tree finish."""

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

    child: subprocess.Popen[bytes] | None = None
    try:
        if _lifeline_closed(args.lifeline_fd, 0):
            return 1
        child = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            pass_fds=(args.auth_fd, *args.child_directory_fd),
            env=os.environ.copy(),
        )
        process_group_id = child.pid
        _close_descriptor(args.auth_fd)
        args.auth_fd = -1
        if process_group_id <= 1:
            raise GuardError("guarded body reported an invalid root process")
        lifeline_lost = False
        observation_failed = False
        cancellation_latched = False
        try:
            _write_wrapper_control(args.control_fd, f"READY {process_group_id}")
        except BaseException as error:
            observation_failed = True
            print(
                f"resource session wrapper control output failed: {error}",
                file=sys.stderr,
            )
        known_owned_process_ids = {process_group_id}
        completed: tuple[int, int] | None = None
        while True:
            try:
                if _lifeline_closed(args.lifeline_fd, 0):
                    lifeline_lost = True
                live_owned_rows: list[ProcessRow] | None = None
                try:
                    rows = _process_rows()
                    live_owned_rows = _group_rows(
                        process_group_id,
                        rows,
                        known_owned_process_ids=known_owned_process_ids,
                    )
                except GuardViolation as error:
                    if error.reason == "cancellation":
                        cancellation_latched = True
                    else:
                        observation_failed = True
                except GuardError:
                    observation_failed = True
                if completed is None:
                    try:
                        completed = _wait4_nonblocking(child)
                    except GuardError:
                        observation_failed = True
                        fallback_returncode = child.poll()
                        if fallback_returncode is not None:
                            completed = (fallback_returncode, 0)
                if completed is not None and live_owned_rows == []:
                    break
                time.sleep(SESSION_OBSERVATION_INTERVAL_SECONDS)
            except KeyboardInterrupt:
                cancellation_latched = True

        returncode, kernel_peak_rss_bytes = completed
        try:
            _write_wrapper_control(
                args.control_fd,
                "EXIT "
                f"{returncode} {1 if lifeline_lost else 0} "
                f"{1 if observation_failed else 0} "
                f"{1 if cancellation_latched else 0} {kernel_peak_rss_bytes}",
            )
        except BaseException as error:
            observation_failed = True
            print(
                f"resource session wrapper control output failed: {error}",
                file=sys.stderr,
            )
        if lifeline_lost or observation_failed or cancellation_latched:
            return 1
        return _exit_status(returncode)
    except BaseException as error:
        if child is not None:
            try:
                child.wait()
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
            raise GuardViolation(
                "control_output_failure",
                "session wrapper emitted invalid readiness",
            )
        process_group_id = int(fields[1])
        if process_group_id <= 1 or process_group_id == wrapper.pid:
            raise GuardViolation(
                "control_output_failure",
                "session wrapper reported an invalid body root process",
            )
        session = GuardedSession(
            wrapper,
            process_group_id,
            lifeline_writer,
            control,
            None,
        )
        lifeline_writer = -1
        control = None
        return session
    except BaseException:
        _close_descriptor(lifeline_writer)
        lifeline_writer = -1
        if wrapper is not None:
            wrapper.wait()
        if control is not None:
            try:
                control.drain_to_eof()
            finally:
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
    memory_enforcement_mode: str = MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT,
    held_lock_descriptors: Sequence[int] = (),
    child_directory_descriptors: Sequence[int] = (),
    post_run_cleanup: Callable[[], int | None] | None = None,
    post_run_validation: Callable[[], None] | None = None,
    post_success_finalize: Callable[[], int | None] | None = None,
    report_context: Mapping[str, object] | None = None,
    child_environment: Mapping[str, str] | None = None,
    cancellation_requested: Callable[[], bool] | None = None,
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
    if memory_enforcement_mode not in MEMORY_ENFORCEMENT_MODES:
        raise GuardError("unknown memory enforcement mode")
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
    exit_reason = "guard_error"
    child_exit_code: int | None = None
    final_status = 1
    cleanup_removed: int | None = None
    cleanup_status: str | None = None
    validation_status: str | None = None
    finalize_result: int | None = None
    finalize_status: str | None = None
    kernel_peak_rss_bytes = 0
    verdict_latched = False
    known_owned_process_ids: set[int] = set()

    def latch_verdict(reason: str, status: int) -> bool:
        """Latch the first supervision violation without controlling a process."""

        nonlocal exit_reason
        nonlocal final_status
        nonlocal verdict_latched
        if verdict_latched:
            return False
        exit_reason = reason
        final_status = status
        verdict_latched = True
        return True

    def record_wrapper_completion(completed_session: GuardedSession) -> None:
        """Consume authenticated body status after the wrapper has exited."""

        nonlocal child_exit_code
        nonlocal exit_reason
        nonlocal final_status
        nonlocal kernel_peak_rss_bytes
        (
            child_exit_code,
            lifeline_lost,
            observation_failed,
            cancellation_latched,
            kernel_peak_rss_bytes,
        ) = _read_guarded_exit(completed_session)
        if lifeline_lost:
            latch_verdict("lifeline_lost", 1)
        if observation_failed:
            latch_verdict("process_observation_failure", 1)
        if cancellation_latched:
            latch_verdict("cancellation", CANCELLATION_EXIT_CODE)
        if kernel_peak_rss_bytes > memory_limit_bytes:
            latch_verdict("kernel_memory_limit", MEMORY_LIMIT_EXIT_CODE)
        if not verdict_latched:
            exit_reason = "completed" if child_exit_code == 0 else "child_exit"
            final_status = _exit_status(child_exit_code)

    try:
        report.write(
            {
                "event": "start",
                "memory_limit_bytes": memory_limit_bytes,
                "memory_enforcement_mode": memory_enforcement_mode,
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
            latch_verdict("foreign_heavy_job", FOREIGN_JOB_EXIT_CODE)
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
        known_owned_process_ids.add(session.process_group_id)
        next_sample = time.monotonic()
        next_physical_footprint = (
            next_sample + physical_footprint_interval_seconds
        )
        recorded_violations: set[str] = set()
        while True:
            try:
                if cancellation_requested is not None and cancellation_requested():
                    if latch_verdict("cancellation", CANCELLATION_EXIT_CODE):
                        report.write(
                            {
                                "event": "cancellation_observed",
                                "process_group_id": session.process_group_id,
                                "schema_version": 1,
                                "timestamp_utc": _utc_now(),
                            }
                        )

                if session.wrapper.poll() is not None:
                    record_wrapper_completion(session)
                    break

                now = time.monotonic()
                if now >= next_sample:
                    try:
                        process_rows = _process_rows()
                        owned_rows = _group_rows(
                            session.process_group_id,
                            process_rows,
                            known_owned_process_ids=known_owned_process_ids,
                        )
                        foreign = _foreign_heavy_jobs(
                            process_rows,
                            owned_process_ids=known_owned_process_ids,
                        )
                        if foreign and "foreign_heavy_job" not in recorded_violations:
                            first = foreign[0]
                            latch_verdict(
                                "foreign_heavy_job", FOREIGN_JOB_EXIT_CODE
                            )
                            _record_foreign_heavy_job(
                                report,
                                first,
                                phase="runtime",
                                owned_process_group_id=session.process_group_id,
                            )
                            recorded_violations.add("foreign_heavy_job")
                        if session.wrapper.poll() is not None:
                            record_wrapper_completion(session)
                            break
                        include_physical_footprint = (
                            now >= next_physical_footprint
                        )
                        sample = _sample_group(
                            session.process_group_id,
                            process_rows,
                            include_physical_footprint=include_physical_footprint,
                            memory_enforcement_mode=memory_enforcement_mode,
                            expected_body_identity=session.body_identity,
                            known_owned_process_ids=known_owned_process_ids,
                        )
                    except GuardViolation as error:
                        violation_status = (
                            CANCELLATION_EXIT_CODE
                            if error.reason == "cancellation"
                            else 1
                        )
                        latch_verdict(error.reason, violation_status)
                        if error.reason not in recorded_violations:
                            report.write(
                                {
                                    "detail": str(error),
                                    "event": "observation_violation",
                                    "process_group_id": session.process_group_id,
                                    "reason": error.reason,
                                    "schema_version": 1,
                                    "timestamp_utc": _utc_now(),
                                }
                            )
                            recorded_violations.add(error.reason)
                        next_sample = time.monotonic() + sample_interval_seconds
                        continue
                    except GuardError as error:
                        latch_verdict("process_observation_failure", 1)
                        if "process_observation_failure" not in recorded_violations:
                            report.write(
                                {
                                    "detail": str(error),
                                    "event": "observation_violation",
                                    "process_group_id": session.process_group_id,
                                    "reason": "process_observation_failure",
                                    "schema_version": 1,
                                    "timestamp_utc": _utc_now(),
                                }
                            )
                            recorded_violations.add("process_observation_failure")
                        next_sample = time.monotonic() + sample_interval_seconds
                        continue
                    if include_physical_footprint:
                        next_physical_footprint = (
                            time.monotonic() + physical_footprint_interval_seconds
                        )
                    if sample.process_count > 0:
                        sample_count += 1
                        peak_memory_bytes = max(
                            peak_memory_bytes, sample.memory_bytes
                        )
                        peak_rss_bytes = max(peak_rss_bytes, sample.rss_bytes)
                        peak_footprint_bytes = max(
                            peak_footprint_bytes,
                            sample.physical_footprint_bytes,
                        )
                        report.write(
                            {
                                "accounting_method": sample.accounting_method,
                                "elapsed_seconds": round(
                                    now - started_monotonic, 6
                                ),
                                "event": "sample",
                                "memory_bytes": sample.memory_bytes,
                                "memory_limit_bytes": memory_limit_bytes,
                                "physical_footprint_bytes": (
                                    sample.physical_footprint_bytes
                                ),
                                "process_count": sample.process_count,
                                "process_group_id": session.process_group_id,
                                "rss_bytes": sample.rss_bytes,
                                "schema_version": 1,
                                "timestamp_utc": _utc_now(),
                            }
                        )
                        if (
                            sample.memory_bytes > memory_limit_bytes
                            and "memory_limit" not in recorded_violations
                        ):
                            latch_verdict(
                                "memory_limit", MEMORY_LIMIT_EXIT_CODE
                            )
                            report.write(
                                {
                                    "accounting_method": sample.accounting_method,
                                    "elapsed_seconds": round(
                                        time.monotonic() - started_monotonic, 6
                                    ),
                                    "event": "memory_limit_observed",
                                    "memory_bytes": sample.memory_bytes,
                                    "memory_limit_bytes": memory_limit_bytes,
                                    "physical_footprint_bytes": (
                                        sample.physical_footprint_bytes
                                    ),
                                    "process_count": sample.process_count,
                                    "process_group_id": session.process_group_id,
                                    "rss_bytes": sample.rss_bytes,
                                    "schema_version": 1,
                                    "timestamp_utc": _utc_now(),
                                }
                            )
                            recorded_violations.add("memory_limit")
                    # Schedule from probe completion. Reusing the timestamp
                    # from before inspection can create a catch-up storm.
                    next_sample = time.monotonic() + sample_interval_seconds

                if session.wrapper.poll() is not None:
                    record_wrapper_completion(session)
                    break
                try:
                    time.sleep(
                        min(0.05, max(0.0, next_sample - time.monotonic()))
                    )
                except KeyboardInterrupt:
                    if latch_verdict("cancellation", CANCELLATION_EXIT_CODE):
                        report.write(
                            {
                                "event": "cancellation_observed",
                                "process_group_id": session.process_group_id,
                                "schema_version": 1,
                                "timestamp_utc": _utc_now(),
                            }
                        )
            except KeyboardInterrupt:
                if latch_verdict("cancellation", CANCELLATION_EXIT_CODE):
                    report.write(
                        {
                            "event": "cancellation_observed",
                            "process_group_id": session.process_group_id,
                            "schema_version": 1,
                            "timestamp_utc": _utc_now(),
                        }
                    )
    except BaseException as error:
        if isinstance(error, GuardViolation):
            latch_verdict(
                error.reason,
                (
                    CANCELLATION_EXIT_CODE
                    if error.reason == "cancellation"
                    else 1
                ),
            )
        elif isinstance(error, KeyboardInterrupt):
            latch_verdict("cancellation", CANCELLATION_EXIT_CODE)
        elif isinstance(error, GuardError) and verdict_latched:
            pass
        else:
            latch_verdict("guard_error", 1)
        print(f"TLAPS guard failed: {error}", file=sys.stderr)
    finally:
        if session is not None:
            try:
                session.close()
            except GuardViolation as error:
                latch_verdict(error.reason, 1)
                print(f"TLAPS guard control failed: {error}", file=sys.stderr)
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
                if final_status == 0:
                    latch_verdict("post_run_cleanup_error", 1)
                print(f"resource guard post-run cleanup failed: {error}", file=sys.stderr)
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
            "memory_enforcement_mode": memory_enforcement_mode,
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
