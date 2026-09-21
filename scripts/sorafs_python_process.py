"""Bounded real Python subprocess custody for the offline SoraFS producer.

The caller owns fixed reviewed argv and original executable/input bytes. This
module never merges ambient environment, fabricates reports, or interprets an
exit status as qualification. It retains separate fresh logs under pinned real
parents. POSIX hosts are required; no shell or pathname-based output fallback.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import math
import os
from pathlib import Path
import selectors
import signal
import stat
import subprocess
import time

from release_manifest_signing import _open_release_output_parent

MAX_OUTPUT_BYTES = 128 * 1024 * 1024
MAX_TIMEOUT_SECONDS = 1200
READ_BYTES = 64 * 1024


class ProcessError(RuntimeError):
    """The owned subprocess or its retained output violated a runtime bound."""


@dataclass(frozen=True)
class ProcessFile:
    path: Path
    sha256: str
    size: int


@dataclass(frozen=True)
class ProcessResult:
    returncode: int
    stdout: ProcessFile
    stderr: ProcessFile


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ProcessError(message)


def _absolute(path: Path) -> Path:
    _require(type(path) is type(Path()) and path.is_absolute()
             and str(path) == os.path.normpath(path) and "\x00" not in str(path),
             "process paths must be absolute canonical Path values")
    return path


def private_environment(home: Path, temporary: Path) -> dict[str, str]:
    """Construct the entire environment; preserve no caller ambient settings."""
    for path in (home, temporary):
        _absolute(path)
        _require(path.resolve(strict=True) == path and path.is_dir(), "private runtime directory aliases")
    _require(home != temporary, "private home and temporary directories must differ")
    return {"PATH": "/usr/bin:/bin", "HOME": str(home), "TMPDIR": str(temporary),
            "LC_ALL": "C", "PYTHONDONTWRITEBYTECODE": "1", "PYTHONNOUSERSITE": "1",
            "PYTEST_DISABLE_PLUGIN_AUTOLOAD": "1", "PIP_CONFIG_FILE": os.devnull}


class _Log:
    def __init__(self, path: Path, limit: int):
        self.path, self.limit = _absolute(path), limit
        self.parent, self.lineage, self.descriptors = _open_release_output_parent(path.parent)
        self.fd = None
        self.size = 0
        self.digest = hashlib.sha256()
        try:
            self.fd = os.open(path.name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
                              | os.O_CLOEXEC, 0o600, dir_fd=self.parent)
            before = os.fstat(self.fd)
            _require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1, "log is not a unique regular file")
            self.physical = before.st_dev, before.st_ino
        except BaseException:
            self.close()
            raise

    def append(self, raw: bytes) -> None:
        allowed = min(len(raw), self.limit - self.size)
        payload = raw[:allowed]
        view = memoryview(payload)
        while view:
            written = os.write(self.fd, view)
            _require(written > 0, "retained process output had a short write")
            view = view[written:]
        self.size += len(payload)
        self.digest.update(payload)
        _require(allowed == len(raw), "process output exceeded its byte limit")

    def finish(self) -> ProcessFile:
        os.fsync(self.fd)
        metadata = os.fstat(self.fd)
        named = os.stat(self.path.name, dir_fd=self.parent, follow_symlinks=False)
        identity = lambda st: (st.st_dev, st.st_ino, st.st_mode, st.st_nlink, st.st_size, st.st_mtime_ns, st.st_ctime_ns)
        _require(stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1
                 and stat.S_IMODE(metadata.st_mode) == 0o600
                 and (metadata.st_dev, metadata.st_ino) == self.physical
                 and metadata.st_size == self.size and identity(named) == identity(metadata),
                 "retained process output lost its original file identity")
        os.lseek(self.fd, 0, os.SEEK_SET)
        digest, size = hashlib.sha256(), 0
        while raw := os.read(self.fd, READ_BYTES):
            size += len(raw)
            _require(size <= self.size, "retained process output grew during recheck")
            digest.update(raw)
        _require(size == self.size and digest.digest() == self.digest.digest()
                 and identity(os.fstat(self.fd)) == identity(metadata), "retained process bytes changed")
        _, lineage, descriptors = _open_release_output_parent(self.path.parent)
        try:
            _require(lineage == self.lineage, "process output parent was replaced")
        finally:
            for descriptor in reversed(descriptors):
                os.close(descriptor)
        return ProcessFile(self.path, digest.hexdigest(), size)

    def close(self) -> None:
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None
        for descriptor in reversed(self.descriptors):
            os.close(descriptor)
        self.descriptors = ()


def _stop_owned_process(process: subprocess.Popen) -> None:
    """Terminate only the new session created for this producer's Python command."""
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=0.5)
    except subprocess.TimeoutExpired:
        pass
    # An already-exited direct child can leave its own Python descendants holding
    # pipe descriptors. The private session remains ours until collection ends.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait(timeout=5)


def run_python_process(command: tuple[str, ...], *, cwd: Path, stdout_path: Path,
                       stderr_path: Path, home: Path, temporary: Path, stdout_limit: int,
                       stderr_limit: int, timeout_seconds: float) -> ProcessResult:
    """Run actual fixed argv, draining two pipes without queues or unbounded reads.

    Nonzero exit codes are returned intact for the producer to reject. Overflow,
    timeout, read/write errors and output substitution raise after terminating
    and reaping this owned process. Failure logs remain bounded partial prefixes.
    The producer must use isolated Python flags and may not pass build commands.
    """
    _require(os.name == "posix", "Python process custody requires POSIX descriptor support")
    _require(type(command) is tuple and 1 <= len(command) <= 128
             and all(type(arg) is str and "\x00" not in arg and len(arg.encode()) <= 65536 for arg in command)
             and bool(command[0]), "process command is not bounded fixed argv")
    _absolute(Path(command[0])); _absolute(cwd)
    _require(cwd.resolve(strict=True) == cwd and cwd.is_dir(), "process cwd aliases")
    _require(stdout_path != stderr_path, "stdout and stderr must have distinct owners")
    _require(all(type(limit) is int and 0 <= limit <= MAX_OUTPUT_BYTES for limit in (stdout_limit, stderr_limit))
             and stdout_limit + stderr_limit <= MAX_OUTPUT_BYTES, "process output bounds are invalid")
    _require(type(timeout_seconds) in (int, float) and math.isfinite(timeout_seconds)
             and 0 < timeout_seconds <= MAX_TIMEOUT_SECONDS, "process wall limit is invalid")
    environment = private_environment(home, temporary)
    logs, process = [], None
    selector = selectors.DefaultSelector()
    try:
        logs.append(_Log(stdout_path, stdout_limit))
        logs.append(_Log(stderr_path, stderr_limit))
        deadline = time.monotonic() + timeout_seconds
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, cwd=cwd, env=environment,
                                   close_fds=True, start_new_session=True)
        for stream, log in zip((process.stdout, process.stderr), logs, strict=True):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, log)
        while selector.get_map() or process.poll() is None:
            remaining = deadline - time.monotonic()
            _require(remaining > 0, "process exceeded its wall-clock limit")
            for key, _ in selector.select(min(0.1, remaining)):
                log = key.data
                try:
                    raw = os.read(key.fileobj.fileno(), min(READ_BYTES, log.limit - log.size + 1))
                except (BlockingIOError, InterruptedError):
                    continue
                if raw:
                    log.append(raw)
                else:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
        result = ProcessResult(process.wait(timeout=0), logs[0].finish(), logs[1].finish())
        return result
    except BaseException as error:
        if process is not None and (process.poll() is None or selector.get_map()):
            try:
                _stop_owned_process(process)
            except BaseException as cleanup:
                raise ProcessError("owned Python process could not be reaped") from cleanup
        if isinstance(error, (KeyboardInterrupt, SystemExit, ProcessError)):
            raise
        raise ProcessError("owned Python process or retained output failed") from error
    finally:
        selector.close()
        if process is not None:
            for stream in (process.stdout, process.stderr):
                if stream is not None:
                    stream.close()
        for log in reversed(logs):
            log.close()
