"""Bounded original direct-child ownership for fixed scaling commands.

Requires retained ExecutableImage, its kernel process reader, and an explicit
original input/runtime verifier. Only internal fixed command owners construct
argv. This module has no user-facing command parser. Every failed child stays
owned for explicit bounded cleanup; a timeout never becomes a success receipt.
"""
from __future__ import annotations

from dataclasses import dataclass
import fcntl
import hashlib
import os
import selectors
import stat
import subprocess
import time
from typing import Callable

from resource_process import ExecutableImage, PinnedProcess, ProcessIdentity, _file_identity

MAX_TRIAL_NS = 7200 * 1_000_000_000
MAX_OUTPUT_BYTES = 24 * 1024 * 1024
MAX_COMMANDS = 72004
MAX_ARGV_ITEMS = 256
MAX_STREAM_BYTES = 256 * 1024 * 1024
MAX_CHUNK_BYTES = 65536


class CommandError(ValueError):
    """Fixed public error code with no private argv, child output or inputs."""


def _require(value, code):
    if not value:
        raise CommandError(code)


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise CommandError('scaling_command_failed') from None


def _input_identity(info):
    """Keep pipe ownership stable while its bounded payload is consumed."""
    if stat.S_ISFIFO(info.st_mode):
        return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid, info.st_nlink)
    return _file_identity(info)


def _remaining(end):
    ns = end - time.monotonic_ns()
    _require(ns > 0, 'scaling_command_deadline_exceeded')
    return ns / 1_000_000_000


@dataclass(frozen=True, slots=True)
class CommandResult:
    """Bounded stdout from one successfully reaped original executable image."""
    stdout: bytes
    process: ProcessIdentity


@dataclass(frozen=True, slots=True)
class StreamCommandResult:
    """Transport completion; the caller must still finish its semantic parser."""
    process: ProcessIdentity
    stdout_bytes: int
    stdout_sha256: str


@dataclass(slots=True)
class _Child:
    role: str
    process: subprocess.Popen
    handle: subprocess.Popen
    stdout: object = None
    stderr: object = None
    pinned: PinnedProcess | None = None
    identity: ProcessIdentity | None = None
    pid: int | None = None


class BoundedCommand:
    """One original image/deadline, sequential fixed invocations and cleanup.

    Result bytes remain valid evidence only while the caller retains and checks
    the original inputs/runtime closure. A result is transport evidence, not a
    semantic proof. Failed or reentrant invocations permanently close admission.
    The first kernel image pin is mandatory even for a short-lived CLI.
    """

    def __init__(self, image: ExecutableImage, reader, deadline_ns: int,
                 verify_inputs: Callable[[], None]):
        _require(not hasattr(self, '_image'), 'scaling_command_readmission')
        _require(isinstance(image, ExecutableImage) and callable(getattr(reader, 'sample', None))
                 and callable(verify_inputs), 'scaling_command_owner_invalid')
        _require(type(deadline_ns) is int
                 and 0 < deadline_ns - time.monotonic_ns() <= MAX_TRIAL_NS,
                 'scaling_command_deadline_invalid')
        image.validate()
        self._image, self._reader = image, reader
        self._binding = (image.path, image.fd, image.identity, image.sha256, image.uuids)
        self._end, self._guard = deadline_ns, verify_inputs
        self._admitted_end = deadline_ns
        self._phase, self._children, self._fds = 'idle', [], ()

    @property
    def deadline_ns(self):
        """The original absolute bound, never refreshed between commands."""
        return self._admitted_end

    def _verify_image(self):
        image = self._image
        _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == self._binding,
                 'scaling_command_image_binding_changed')
        image.validate()
        for fd, identity, flags in self._fds:
            _require(_input_identity(os.fstat(fd)) == identity
                     and fcntl.fcntl(fd, fcntl.F_GETFL) == flags,
                     'scaling_command_input_descriptor_changed')

    def _verify(self):
        _require(self._phase == 'busy', 'scaling_command_phase_invalid')
        _require(self._end == self._admitted_end, 'scaling_command_deadline_changed')
        _remaining(self._end)
        self._verify_image()
        self._guard()
        _require(self._phase == 'busy', 'scaling_command_phase_invalid')
        self._verify_image()
        _require(self._end == self._admitted_end, 'scaling_command_deadline_changed')
        _remaining(self._end)

    def _bound(self, child):
        _require(child.process is child.handle and child.process.pid == child.pid and child.process.stdout is child.stdout
                 and child.process.stderr is child.stderr, 'scaling_command_child_binding_changed')
        if child.pinned is not None:
            pinned = child.pinned
            _require(pinned.pid == child.pid and pinned.identity == child.identity
                     and pinned.peer_id == child.role and pinned.image is self._image
                     and pinned.reader is self._reader, 'scaling_command_child_binding_changed')

    def run(self, role: str, argv: tuple[str, ...], pass_fds: tuple[int, ...],
            stdout_limit: int, stderr_limit: int = 16384) -> CommandResult:
        """Collect bounded stdout from fixed argv and require terminal zero."""
        return self._execute(role, argv, pass_fds, stdout_limit, stderr_limit, None)

    def run_stream(self, role: str, argv: tuple[str, ...], pass_fds: tuple[int, ...],
                   stdout_limit: int, consume: Callable[[bytes], None],
                   stderr_limit: int = 16384) -> StreamCommandResult:
        """Deliver bounded chunks to an internal unpublished semantic consumer.

        At most 256 MiB is delivered in chunks no larger than 64 KiB. The
        consumer must not publish evidence during delivery: later I/O, deadline,
        exit or custody checks may still fail. A successful transport result
        does not close a partial JSON row; the caller must finish its parser
        and all semantic checks before creating a proof receipt.
        """
        try:
            _require(callable(consume), 'scaling_command_consumer_invalid')
            return self._execute(role, argv, pass_fds, stdout_limit, stderr_limit, consume)
        except BaseException as error:
            self._phase = 'failed'
            _failure(error)

    def _execute(self, role, argv, pass_fds, stdout_limit, stderr_limit, consume):
        try:
            _require(self._phase == 'idle', 'scaling_command_phase_invalid')
            self._phase = 'busy'
            _require(type(role) is str and 0 < len(role) <= 64
                     and all(c in 'abcdefghijklmnopqrstuvwxyz0123456789_-' for c in role),
                     'scaling_command_role_invalid')
            _require(type(argv) is tuple and 1 <= len(argv) <= MAX_ARGV_ITEMS
                     and all(type(arg) is str and '\x00' not in arg and len(os.fsencode(arg)) <= 8192 for arg in argv)
                     and sum(len(os.fsencode(arg)) for arg in argv) <= 65536
                     and argv[0] == str(self._binding[0]), 'scaling_command_argv_invalid')
            _require(type(pass_fds) is tuple and len(pass_fds) <= 128
                     and all(type(fd) is int and 3 <= fd <= 65535 for fd in pass_fds)
                     and len(set(pass_fds)) == len(pass_fds), 'scaling_command_descriptors_invalid')
            _require(type(stdout_limit) is int and 0 < stdout_limit <= (MAX_OUTPUT_BYTES if consume is None else MAX_STREAM_BYTES)
                     and type(stderr_limit) is int and 0 < stderr_limit <= MAX_OUTPUT_BYTES,
                     'scaling_command_output_bound_invalid')
            _require(len(self._children) < MAX_COMMANDS, 'scaling_command_count_exceeded')
            descriptors = []
            for fd in pass_fds:
                info, flags = os.fstat(fd), fcntl.fcntl(fd, fcntl.F_GETFL)
                seed_pipe = (role == 'generator' and pass_fds == (fd,)
                    and argv.count('--seed-fd') == 1 and '--seed' not in argv
                    and argv.index('--seed-fd') + 1 < len(argv)
                    and argv[argv.index('--seed-fd') + 1] == str(fd)
                    and stat.S_ISFIFO(info.st_mode)
                    and 0 <= info.st_size <= 64 and flags & os.O_NONBLOCK)
                _require((seed_pipe or (stat.S_ISREG(info.st_mode) and info.st_nlink == 1
                         and stat.S_IMODE(info.st_mode) in (0o400, 0o600)))
                         and info.st_uid == os.geteuid()
                         and flags & os.O_ACCMODE == os.O_RDONLY, 'scaling_command_descriptor_invalid')
                descriptors.append((fd, _input_identity(info), flags))
            self._fds = tuple(descriptors)
            self._verify()
            result = self._run(role, argv, pass_fds, stdout_limit, stderr_limit, consume)
            self._verify()
            self._phase, self._fds = 'idle', ()
            return result
        except BaseException as error:
            self._phase = 'failed'
            _failure(error)

    def _run(self, role, argv, pass_fds, stdout_limit, stderr_limit, consume):
        selector = selectors.DefaultSelector()
        child = None
        buffers = {'stdout': bytearray(), 'stderr': bytearray()}
        limits = {'stdout': stdout_limit, 'stderr': stderr_limit}
        counts = {'stdout': 0, 'stderr': 0}
        digest = hashlib.sha256()
        try:
            self._verify()
            process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, cwd='/', env={}, close_fds=True, pass_fds=pass_fds,
                shell=False, start_new_session=False, bufsize=0)
            child = _Child(role, process, process)
            self._children.append(child)
            child.stdout, child.stderr, child.pid = process.stdout, process.stderr, process.pid
            _require(type(child.pid) is int and 1 < child.pid <= (1 << 31) - 1
                     and child.stdout is not None and child.stderr is not None,
                     'scaling_command_child_invalid')
            child.pinned = PinnedProcess(role, child.pid, self._image, self._reader)
            child.identity = child.pinned.identity
            self._verify()
            for label, stream in (('stdout', child.stdout), ('stderr', child.stderr)):
                os.set_blocking(stream.fileno(), False)
                selector.register(stream.fileno(), selectors.EVENT_READ, label)
            while selector.get_map():
                self._bound(child)
                self._verify()
                if process.poll() is None:
                    _require(child.pinned.sample().identity == child.identity,
                             'scaling_command_child_binding_changed')
                self._bound(child)
                self._verify()
                for key, _ in selector.select(min(_remaining(self._end), 0.05)):
                    self._bound(child)
                    self._verify()
                    label = key.data
                    try:
                        raw = os.read(key.fd, min(MAX_CHUNK_BYTES, limits[label] + 1 - counts[label]))
                    except BlockingIOError:
                        continue
                    if not raw:
                        selector.unregister(key.fd)
                        continue
                    counts[label] += len(raw)
                    _require(counts[label] <= limits[label], 'scaling_command_output_exceeded')
                    if label == 'stdout' and consume is not None:
                        digest.update(raw)
                        self._bound(child)
                        self._verify()
                        consume(raw)
                        self._bound(child)
                        self._verify()
                    else:
                        buffers[label].extend(raw)
            self._bound(child)
            status = process.wait(timeout=_remaining(self._end))
            _require(type(status) is int and status == 0 and process.returncode == 0,
                     'scaling_command_exit_failed')
            self._bound(child)
            self._verify()
            if consume is not None:
                return StreamCommandResult(child.identity, counts['stdout'], digest.hexdigest())
            return CommandResult(bytes(buffers['stdout']), child.identity)
        finally:
            selector.close()
            if child is not None:
                for stream in (child.stdout, child.stderr):
                    if stream is not None:
                        stream.close()

    def cleanup(self, deadline_ns: int) -> tuple[str, ...]:
        """Permanently fail and attempt bounded reap using original Popen only.

        No force kill is used. Pending roles remain owned for caller resolution.
        Cleanup's separate bound never extends the trial or permits another run.
        """
        self._phase = 'failed'
        _require(type(deadline_ns) is int
                 and 0 < deadline_ns - time.monotonic_ns() <= 300 * 1_000_000_000,
                 'scaling_command_cleanup_deadline_invalid')
        pending = []
        for child in reversed(self._children):
            try:
                self._bound(child)
                if child.process.poll() is None:
                    _remaining(deadline_ns)
                    self._bound(child)
                    child.process.terminate()
                timeout = _remaining(deadline_ns)
                self._bound(child)
                child.process.wait(timeout=timeout)
                self._bound(child)
            except BaseException as error:
                if isinstance(error, Exception): pending.append(child.role)
                else: _failure(error)
        return tuple(pending)
