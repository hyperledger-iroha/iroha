"""Actual process/descriptor protection, never Native installation or issuer authority.

The installed Native owner must still authenticate the launched Python/archive,
native signer images, private role originals and current governed policy. These
local kernel checks cannot construct that owner or admit a public caller.
"""
from __future__ import annotations

import ctypes as c
import errno
import fcntl
import os
import resource
import signal
import stat
import struct
import sys
import time

from .attestation import require

_PROBE_SECONDS = 2.0
_PACKET = struct.Struct("<4sIIIII")
_MACH_APIS = ("task_for_pid", "task_read_for_pid", "task_inspect_for_pid")
_WORKER_REQUIRED_FDS = frozenset((9, 10, 12, 14, 15, 17, 18))


def disable_core_dumps() -> None:
    """Set and read back the irreversible zero core-file limit in this image."""
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    require(resource.getrlimit(resource.RLIMIT_CORE) == (0, 0),
            "private process core-file protection unavailable")


def _descriptor_inventory() -> frozenset[int]:
    require(sys.platform in ("darwin", "linux"), "private descriptor inventory unavailable")
    directory = "/dev/fd" if sys.platform == "darwin" else "/proc/self/fd"
    opened = set()
    for entry in os.listdir(directory):
        require(entry.isascii() and entry.isdigit(), "private descriptor inventory differs")
        descriptor = int(entry)
        try:
            os.fstat(descriptor)
        except OSError as error:
            # listdir's own transient directory descriptor has already closed.
            require(error.errno == errno.EBADF, "private descriptor observation failed")
        else:
            opened.add(descriptor)
    return frozenset(opened)


def _close_except(keep: frozenset[int]) -> None:
    for descriptor in _descriptor_inventory() - keep:
        os.close(descriptor)
    require(_descriptor_inventory() == keep,
            "private process inherited descriptor closure unavailable")


class _ProcTaskInfo(c.Structure):
    # Exact proc_taskinfo from the installed macOS SDK's sys/proc_info.h.
    _fields_ = [(name, c.c_uint64) for name in (
        "virtual_size", "resident_size", "total_user", "total_system",
        "threads_user", "threads_system")] + [(name, c.c_int32) for name in (
        "policy", "faults", "pageins", "cow_faults", "messages_sent",
        "messages_received", "syscalls_mach", "syscalls_unix", "context_switches",
        "thread_count", "running_count", "priority")]


def _darwin_library():
    library = c.CDLL(None, use_errno=True)
    library.ptrace.argtypes = [c.c_int, c.c_int, c.c_void_p, c.c_int]
    library.ptrace.restype = c.c_int
    library.mach_task_self.argtypes = []
    library.mach_task_self.restype = c.c_uint
    library.mach_port_deallocate.argtypes = [c.c_uint, c.c_uint]
    library.mach_port_deallocate.restype = c.c_int
    library.proc_pidinfo.argtypes = [c.c_int, c.c_int, c.c_uint64, c.c_void_p, c.c_int]
    library.proc_pidinfo.restype = c.c_int
    for name in _MACH_APIS:
        api = getattr(library, name)
        api.argtypes = [c.c_uint, c.c_int, c.POINTER(c.c_uint)]
        api.restype = c.c_int
    return library


def _single_darwin_thread(library) -> None:
    value = _ProcTaskInfo()
    require(c.sizeof(value) == 96
            and library.proc_pidinfo(os.getpid(), 4, 0, c.byref(value), c.sizeof(value)) == c.sizeof(value)
            and value.thread_count == 1,
            "private process rejection probe requires one actual native thread")


def _darwin_rejection_probe(library) -> None:
    """Probe only this process, from one owned disposable same-UID child.

    The child closes every inherited descriptor except its public result pipe
    before any observation. Each Mach API first succeeds against the child's
    own task, then must reject control/read/inspection of its actual live parent.
    PT_ATTACHEXC must terminate the child with SIGSEGV after PT_DENY_ATTACH;
    a generic permission error, missing API or timeout is insufficient evidence.
    """
    target, uid = os.getpid(), os.getuid()
    read_fd, write_fd = os.pipe()
    child = -1
    reaped = False
    try:
        _single_darwin_thread(library)
        child = os.fork()
        if child == 0:
            try:
                _close_except(frozenset((write_fd,)))
                if os.getppid() != target or os.getuid() != uid or os.geteuid() != uid:
                    os._exit(80)
                if resource.getrlimit(resource.RLIMIT_CORE) != (0, 0):
                    os._exit(81)
                control_mask = denial_mask = 0
                task = library.mach_task_self()
                for index, name in enumerate(_MACH_APIS):
                    api = getattr(library, name)
                    own = c.c_uint()
                    if api(task, os.getpid(), c.byref(own)) == 0 and own.value:
                        control_mask |= 1 << index
                    if own.value:
                        library.mach_port_deallocate(task, own.value)
                    offered = c.c_uint()
                    result = api(task, target, c.byref(offered))
                    if result != 0 and offered.value == 0:
                        denial_mask |= 1 << index
                    if offered.value:
                        library.mach_port_deallocate(task, offered.value)
                if _descriptor_inventory() != frozenset((write_fd,)):
                    os._exit(82)
                packet = _PACKET.pack(b"DPV1", target, uid, os.getpid(), control_mask, denial_mask)
                if os.write(write_fd, packet) != len(packet):
                    os._exit(83)
                os.close(write_fd)
                if control_mask != 7 or denial_mask != 7:
                    os._exit(84)
                # Constants are the current SDK PT_ATTACHEXC=14/PT_DETACH=11.
                # If an unprotected disposable target accidentally allows attach,
                # detach immediately; a successful attach can never pass this probe.
                if library.ptrace(14, target, None, 0) == 0:
                    library.ptrace(11, target, c.c_void_p(1), 0)
                os._exit(85)
            except BaseException:
                os._exit(86)
        os.close(write_fd)
        write_fd = -1
        os.set_blocking(read_fd, False)
        deadline = time.monotonic() + _PROBE_SECONDS
        status = None
        original = bytearray()
        while time.monotonic() < deadline:
            try:
                chunk = os.read(read_fd, _PACKET.size + 1 - len(original))
            except BlockingIOError:
                chunk = b""
            original.extend(chunk)
            require(len(original) <= _PACKET.size, "private process probe packet changed")
            observed, value = os.waitpid(child, os.WNOHANG)
            if observed == child:
                reaped, status = True, value
                # A completed child has closed its sole public output descriptor.
                try:
                    original.extend(os.read(read_fd, _PACKET.size + 1 - len(original)))
                except BlockingIOError:
                    pass
                break
            time.sleep(0.005)
        require(status is not None and len(original) == _PACKET.size,
                "private process rejection probe expired or omitted original evidence")
        magic, observed_target, observed_uid, observed_child, controls, denials = _PACKET.unpack(original)
        require(magic == b"DPV1" and (observed_target, observed_uid, observed_child) == (target, uid, child)
                and controls == denials == 7
                and os.WIFSIGNALED(status) and os.WTERMSIG(status) == signal.SIGSEGV
                and not os.WCOREDUMP(status),
                "private process actual ptrace/task rejection unavailable")
    finally:
        if child > 0 and not reaped:
            # Only the still-owned unreaped child is signalled; no offered PID,
            # process group, external process or production role is a probe target.
            try:
                os.kill(child, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.waitpid(child, 0)
        os.close(read_fd)
        if write_fd >= 0:
            os.close(write_fd)


def protect_darwin_process() -> None:
    """Enforce and observe actual macOS protection before any private role intake."""
    require(sys.platform == "darwin" and os.getuid() != 0
            and os.getuid() == os.geteuid() and os.getgid() == os.getegid(),
            "private macOS process identity unavailable")
    disable_core_dumps()
    library = _darwin_library()
    require(signal.getsignal(signal.SIGCHLD) == signal.SIG_DFL,
            "private macOS process child ownership unavailable")
    _single_darwin_thread(library)
    require(library.ptrace(31, 0, None, 0) == 0,
            "private macOS process deny-attach unavailable")
    _darwin_rejection_probe(library)
    require(resource.getrlimit(resource.RLIMIT_CORE) == (0, 0),
            "private macOS process core protection changed")


def close_unrelated_worker_descriptors() -> frozenset[int]:
    """Retain only the closed Native worker FD grammar, then reobserve closure.

    Role identities/pins are still admitted by the actual installed parent. This
    roster observation is data; it grants no signing or Native source capability.
    """
    opened = _descriptor_inventory()
    roles = opened & (_WORKER_REQUIRED_FDS | frozenset((13,)))
    require(_WORKER_REQUIRED_FDS <= roles, "private Native worker descriptor roles absent")
    keep = roles | (opened & frozenset((0, 1, 2)))
    _close_except(keep)
    for descriptor in roles:
        os.set_inheritable(descriptor, False)
    require(all(not os.get_inheritable(descriptor) for descriptor in roles),
            "private Native worker descriptors remain inheritable")
    return roles


def require_worker_role_originals(roles: frozenset[int], google_present: bool) -> None:
    """Observe actual held FD kinds/owners/modes before duplicating private inputs."""
    expected = _WORKER_REQUIRED_FDS | (frozenset((13,)) if google_present else frozenset())
    require(type(google_present) is bool and roles == expected,
            "private Native worker Google descriptor role changed")
    for descriptor, access in ((9, os.O_RDONLY), (10, os.O_WRONLY)):
        value = os.fstat(descriptor)
        require(stat.S_ISFIFO(value.st_mode)
                and value.st_uid in (0, os.getuid())
                and fcntl.fcntl(descriptor, fcntl.F_GETFL) & os.O_ACCMODE == access,
                "private Native worker channel descriptor differs")
    for descriptor in roles - frozenset((9, 10, 17)):
        value = os.fstat(descriptor)
        require(stat.S_ISREG(value.st_mode) and value.st_uid == 0
                and not value.st_mode & 0o022
                and fcntl.fcntl(descriptor, fcntl.F_GETFL) & os.O_ACCMODE == os.O_RDONLY,
                "private Native worker code/credential original differs")
        if descriptor in (12, 13):
            require(not value.st_mode & 0o077
                    and (value.st_size == 32 if descriptor == 12 else 0 < value.st_size <= 32 * 1024),
                    "private Native worker protected role original differs")
        else:
            require(0 < value.st_size <= 16 * 1024 * 1024
                    and (bool(value.st_mode & 0o111) if descriptor in (14, 18) else True),
                    "private Native worker executable/archive original differs")
    store = os.fstat(17)
    require(stat.S_ISDIR(store.st_mode) and store.st_uid == os.getuid()
            and store.st_mode & 0o7777 == 0o700,
            "private Native worker store original differs")
