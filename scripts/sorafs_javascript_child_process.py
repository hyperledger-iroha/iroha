"""One-shot, bounded process custody for the fixed JavaScript qualification child.

The caller supplies the existing original child-input and Node runtime owners.
This module keeps both alive through stdout/stderr EOF and child exit, then
returns exact captured bytes. A successful component observation is not an SDK,
native-addon, mapped-image, candidate, signer, or release qualification receipt.
"""
from __future__ import annotations

import base64
import binascii
from dataclasses import dataclass
import hashlib
import math
import os
from pathlib import Path
import selectors
import signal
import sys
import threading
import time

from sorafs_evidence_json import decode_evidence_json
from sorafs_javascript_input_files import absolute, record_cleanup
from sorafs_javascript_parent_input import MAX_INPUT_BYTES, OriginalJavascriptChildInput
from sorafs_javascript_runtime_custody import OriginalNodeRuntimeInputs


FRAME_PREFIX = b"SORAFS_JAVASCRIPT_CHILD_V1 "
MAX_OBSERVATION_BYTES = 8 * 1024 * 1024
MAX_STDOUT_BYTES = len(FRAME_PREFIX) + 4 * ((MAX_OBSERVATION_BYTES + 2) // 3) + 1
MAX_STDERR_BYTES = 4 * 1024 * 1024
MAX_TIMEOUT_SECONDS = 1200
MAX_PARENT_OPEN_FDS = 16384
READ_BYTES = 64 * 1024


class ChildProcessError(RuntimeError):
    """The fixed owned child, its original inputs, or a pipe bound failed."""


@dataclass(frozen=True)
class ChildProcessCapture:
    """Exact local process bytes; no release or verifier authority."""

    exit_code: int
    stdout: bytes
    stderr: bytes
    observation: bytes


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ChildProcessError(message)


def _close_fds(fds: tuple[int, ...]) -> None:
    """Close every detached pipe end once, retaining all cleanup failures."""
    errors = []
    for fd in reversed(fds):
        try:
            os.close(fd)
        except OSError as error:
            errors.append(error)
    if errors:
        failure = ChildProcessError("fixed child pipe cleanup failed")
        failure.cleanup_errors = tuple(errors)
        raise failure from errors[0]


def _wait_owned(pid: int, deadline: float) -> int:
    """Reap only the exact child PID; never wait for unrelated processes."""
    while True:
        try:
            observed, status = os.waitpid(pid, os.WNOHANG)
        except InterruptedError:
            continue
        if observed == pid:
            return os.waitstatus_to_exitcode(status)
        remaining = deadline - time.monotonic()
        _require(remaining > 0, "fixed child exceeded its wall-clock limit")
        time.sleep(min(remaining, 0.05))


def _stop_owned(pid: int) -> None:
    """Stop only the private process group created for this owned child."""
    try:
        os.killpg(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    # Keep the PID unreaped until after the signal, so its group identity cannot
    # be reused for another process. A kernel-stuck child remains a hard error.
    _wait_owned(pid, time.monotonic() + 5)


def _frame(stdout: bytes) -> bytes:
    _require(stdout.startswith(FRAME_PREFIX) and stdout.endswith(b"\n")
             and len(stdout) <= MAX_STDOUT_BYTES,
             "fixed child omitted its complete final frame")
    encoded = stdout[len(FRAME_PREFIX):-1]
    _require(bool(encoded) and b"\n" not in encoded and b"\r" not in encoded,
             "fixed child emitted an extra or malformed frame")
    try:
        raw = base64.b64decode(encoded, validate=True)
    except binascii.Error as error:
        raise ChildProcessError("fixed child frame base64 differs") from error
    _require(0 < len(raw) <= MAX_OBSERVATION_BYTES and raw.endswith(b"\n")
             and base64.b64encode(raw) == encoded,
             "fixed child observation bytes or frame encoding differ")
    return raw


def _observed_open_fds() -> set[int]:
    """Snapshot inheritable parent FDs for explicit child close actions.

    This is a sole-Python-thread component boundary, not a proof that native
    threads or the OS cannot race between enumeration and posix_spawn.
    """
    _require(threading.active_count() == 1,
             "fixed child launch requires the sole Python thread")
    try:
        names = set()
        with os.scandir("/dev/fd") as entries:
            for entry in entries:
                _require(len(names) < MAX_PARENT_OPEN_FDS,
                         "fixed child parent descriptor census exceeds its bound")
                names.add(entry.name)
    except OSError as error:
        raise ChildProcessError("fixed child cannot enumerate inherited descriptors") from error
    observed = set()
    for name in names:
        if not name.isascii() or not name.isdecimal():
            continue
        fd = int(name)
        try:
            if not os.get_inheritable(fd):
                continue
        except OSError:
            continue
        observed.add(fd)
    return observed


def _spawn_fixed(executable: Path, script: Path, input_sha256: str, input_fd: int,
                 environment: dict[str, str], timeout_seconds: float) -> ChildProcessCapture:
    """Run one exact executable/script/digest argv with original fd3 and two pipes.

    This private primitive is separately exercised with an inert Python script;
    only OriginalJavascriptChildProcess selects the production arguments.
    """
    _require(os.name == "posix" and hasattr(os, "posix_spawn")
             and hasattr(os, "pipe2"), "fixed child needs POSIX descriptor custody")
    absolute(executable)
    absolute(script)
    _require(type(input_sha256) is str and len(input_sha256) == 64
             and all(char in "0123456789abcdef" for char in input_sha256)
             and input_sha256 != "0" * 64,
             "fixed child input digest selection differs")
    _require(type(input_fd) is int and input_fd >= 3, "fixed child original fd differs")
    os.fstat(input_fd)
    _require(type(timeout_seconds) in (int, float) and math.isfinite(timeout_seconds)
             and 0 < timeout_seconds <= MAX_TIMEOUT_SECONDS,
             "fixed child wall-clock bound differs")
    _require(type(environment) is dict and set(environment) ==
             {"PATH", "HOME", "TMPDIR", "LC_ALL", "TZ"}
             and all(type(key) is str and type(value) is str and "\0" not in value
                     for key, value in environment.items()),
             "fixed child environment is not closed")

    pipes = []
    pid = None
    reaped = False
    selector = selectors.DefaultSelector()
    stdout, stderr = bytearray(), bytearray()
    try:
        for _ in range(2):
            pipes.extend(os.pipe2(os.O_CLOEXEC))
        out_read, out_write, err_read, err_write = pipes
        _require(all(fd >= 3 for fd in pipes),
                 "fixed child needs reserved standard parent descriptors")
        # Map the outputs first: a pipe endpoint can otherwise occupy fd3 before
        # the inherited original request is remapped there.
        actions = [(os.POSIX_SPAWN_OPEN, 0, os.devnull, os.O_RDONLY, 0),
                   (os.POSIX_SPAWN_CLOSE, out_read),
                   (os.POSIX_SPAWN_CLOSE, err_read),
                   (os.POSIX_SPAWN_DUP2, out_write, 1),
                   (os.POSIX_SPAWN_DUP2, err_write, 2),
                   (os.POSIX_SPAWN_DUP2, input_fd, 3)]
        actions.extend((os.POSIX_SPAWN_CLOSE, fd) for fd in
                       (out_write, err_write, input_fd) if fd not in (0, 1, 2, 3))
        reserved = {0, 1, 2, 3, *pipes, input_fd}
        # /dev/fd enumeration may itself briefly use a descriptor. Reprobe
        # after that call returns so a closed enumeration fd is never passed
        # as a POSIX_SPAWN_CLOSE action (Darwin rejects EBADF file actions).
        observed = _observed_open_fds() - reserved
        for fd in sorted(observed):
            try:
                if not os.get_inheritable(fd):
                    continue
            except OSError:
                continue
            actions.append((os.POSIX_SPAWN_CLOSE, fd))
        argv = (str(executable), str(script), input_sha256)
        deadline = time.monotonic() + timeout_seconds
        pid = os.posix_spawn(str(executable), argv, environment,
                             file_actions=actions, setpgroup=0)
        # Detach each write end only after its close succeeded. A failed close
        # leaves that end in the owned set for final cleanup after reaping.
        for fd in (out_write, err_write):
            os.close(fd)
            pipes.remove(fd)
        for fd, buffer, limit in ((out_read, stdout, MAX_STDOUT_BYTES),
                                  (err_read, stderr, MAX_STDERR_BYTES)):
            os.set_blocking(fd, False)
            selector.register(fd, selectors.EVENT_READ, (buffer, limit))
        while selector.get_map():
            remaining = deadline - time.monotonic()
            _require(remaining > 0, "fixed child exceeded its wall-clock limit")
            for key, _ in selector.select(min(remaining, 0.1)):
                buffer, limit = key.data
                try:
                    chunk = os.read(key.fd, min(READ_BYTES, limit - len(buffer) + 1))
                except (BlockingIOError, InterruptedError):
                    continue
                if chunk:
                    _require(len(buffer) + len(chunk) <= limit,
                             "fixed child pipe exceeded its byte limit")
                    buffer.extend(chunk)
                else:
                    selector.unregister(key.fd)
                    os.close(key.fd)
                    pipes.remove(key.fd)
        code = _wait_owned(pid, deadline)
        reaped = True
        _require(code == 0, "fixed child exited without success")
        _require(not stderr, "fixed child wrote stderr on successful exit")
        output = bytes(stdout)
        return ChildProcessCapture(code, output, bytes(stderr), _frame(output))
    except BaseException as error:
        error.captured_stdout = bytes(stdout)
        error.captured_stderr = bytes(stderr)
        if pid is not None and not reaped:
            try:
                _stop_owned(pid)
            except BaseException as cleanup:
                record_cleanup(error, cleanup)
        raise
    finally:
        primary = sys.exc_info()[1]
        cleanup_errors = []
        try:
            selector.close()
        except BaseException as cleanup:
            cleanup_errors.append(cleanup)
        try:
            _close_fds(tuple(pipes))
        except BaseException as cleanup:
            cleanup_errors.append(cleanup)
        if primary is not None:
            for cleanup in cleanup_errors:
                record_cleanup(primary, cleanup)
        elif cleanup_errors:
            failure = ChildProcessError("fixed child cleanup failed")
            failure.cleanup_errors = tuple(cleanup_errors)
            raise failure from cleanup_errors[0]


def _original_input_context(owner: OriginalJavascriptChildInput) -> tuple[Path, Path]:
    """Derive fixed private child paths from the held original fd3 bytes."""
    fd = owner.descriptor
    raw = os.pread(fd, MAX_INPUT_BYTES + 1, 0)
    _require(0 < len(raw) <= MAX_INPUT_BYTES
             and hashlib.sha256(raw).hexdigest() == owner.sha256,
             "fixed child original request bytes differ")
    value = decode_evidence_json(raw)
    _require(value.get("schema") == "sorafs.javascript.child_input.v1"
             and type(value.get("environmentRoot")) is str
             and type(value.get("temporaryRoot")) is str,
             "fixed child original request context differs")
    environment_root = absolute(Path(value["environmentRoot"]))
    temporary_root = absolute(Path(value["temporaryRoot"]))
    return environment_root / "qualification/tools/sorafs_javascript_child.mjs", temporary_root


class OriginalJavascriptChildProcess:
    """Own one fixed child run and keep both original owners until final EOF/exit.

    The owners are consumed exactly once, including failed entry or spawn. A
    captured frame remains non-authorizing until separate source, runtime,
    native, original-index and signed candidate verifiers complete.
    """

    def __init__(self, child_input: OriginalJavascriptChildInput,
                 runtime: OriginalNodeRuntimeInputs, *, timeout_seconds: float):
        _require(type(child_input) is OriginalJavascriptChildInput
                 and type(runtime) is OriginalNodeRuntimeInputs,
                 "fixed child needs exact original input and runtime owners")
        self._input = child_input
        self._runtime = runtime
        self._timeout = timeout_seconds
        self._used = False

    def run(self) -> ChildProcessCapture:
        """Launch the selected original Node path with no ambient Node options."""
        _require(not self._used, "fixed child process owner is one-shot")
        self._used = True
        # Runtime originals are acquired in their constructor. Enter them
        # first so a failing child-input __enter__ still closes those FDs.
        with self._runtime as runtime, self._input as child_input:
            child_input.recheck()
            runtime.recheck()
            script, temporary = _original_input_context(child_input)
            environment_root = script.parents[2]
            selected = absolute(Path(runtime.manifest.selected_executable))
            environment = {"PATH": "/usr/bin:/bin", "HOME": str(environment_root),
                           "TMPDIR": str(temporary), "LC_ALL": "C", "TZ": "UTC"}
            capture = _spawn_fixed(selected, script, child_input.sha256,
                                   child_input.descriptor, environment, self._timeout)
            # Both owners are still held after the two pipe EOFs and direct
            # child exit. Their context exits repeat these original rechecks.
            child_input.recheck()
            runtime.recheck()
            return capture
