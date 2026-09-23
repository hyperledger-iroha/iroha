"""Private held-file primitive for the fixed JavaScript parent input owner.

POSIX only; no imports, process, archive parser or qualification authority.
Original regular files and output fd remain held with original parent lineage.
Reads are streamed in64KiB; ambiguous closes detach and are never retried.
"""
from __future__ import annotations
import hashlib
import os
from pathlib import Path
import stat

from release_manifest_signing import _open_release_output_parent
from sorafs_javascript_archive import ArchiveError, _require

CHUNK = 64 * 1024
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


def absolute(path: Path) -> Path:
    """Admit only an original bounded canonical POSIX spelling."""
    try:
        encoded = str(path).encode('utf-8', errors='strict')
    except UnicodeError as error:
        raise ArchiveError('child input path is not valid Unicode') from error
    _require(type(path) is type(Path()) and path.is_absolute() and path != Path('/')
             and str(path) == os.path.normpath(path) and '\0' not in str(path)
             and len(str(path)) <= 4096 and len(encoded) <= 4096
             and len(path.parts) <= 64, 'child input path is not canonical or bounded')
    return path


def seal(info) -> tuple[int, ...]:
    """Full observable file seal, including ctime and both ownership fields."""
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def drain(descriptors: tuple[int, ...]) -> None:
    """Attempt every detached original descriptor once and retain all failures."""
    errors = []
    for fd in reversed(descriptors):
        try:
            os.close(fd)
        except BaseException as error:
            errors.append(error)
    if errors:
        error = ArchiveError('child input descriptor cleanup failed')
        error.cleanup_errors = tuple(errors)
        raise error from errors[0]


def record_cleanup(error: BaseException, cleanup: BaseException) -> None:
    """Append one cleanup diagnostic without discarding prior exact failures."""
    previous = getattr(error, 'cleanup_errors', ())
    error.cleanup_errors = (previous if type(previous) is tuple else (previous,)) + (cleanup,)


def cleanup_preserving(error: BaseException, owner) -> None:
    """Retain the original acquisition/refusal exception during cleanup."""
    try:
        owner.close()
    except BaseException as cleanup:
        record_cleanup(error, cleanup)


class HeldInputFile:
    """Internal one-shot original file; does not authenticate candidate claims."""
    def __init__(self, path: Path, maximum: int, *, payload: bytes | None = None,
                 retain_bytes: bool = True, mode: int | None = None):
        self.path = absolute(path)
        self._fd = self._parent = None
        self._failed = self._closed = self._checking = False
        self._seal = self._digest = self._raw = None
        _require(type(maximum) is int and 0 < maximum <= 1024**3
                 and type(retain_bytes) is bool and (not retain_bytes or maximum <= 16*1024**2),
                 'child input file bound differs')
        self._maximum, self._retain, self._mode = maximum, retain_bytes, mode
        if payload is not None:
            _require(type(payload) is bytes and 0 < len(payload) <= maximum and mode == 0o600,
                     'child input output bytes or mode differ')
        try:
            self._parent = _open_release_output_parent(path.parent)
            flags = FLAGS if payload is None else os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
            self._fd = os.open(path.name, flags, 0o600, dir_fd=self._parent[0])
            opened = os.fstat(self._fd)
            self._validate(opened)
            if payload is not None:
                view = memoryview(payload)
                while view:
                    count = os.write(self._fd, view[:CHUNK])
                    _require(count > 0, 'child input output write made no progress')
                    view = view[count:]
                os.fsync(self._fd)
            self._seal = seal(os.fstat(self._fd))
            self._digest, self._raw = self._read()
            if payload is not None:
                _require(self._raw == payload, 'child input output bytes changed during creation')
            self.recheck()
        except BaseException as error:
            self._failed = True
            cleanup_preserving(error, self)
            raise

    def _validate(self, info) -> None:
        _require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_uid == os.getuid()
                 and 0 <= info.st_size <= self._maximum and not stat.S_IMODE(info.st_mode) & 0o022
                 and (self._mode is None or stat.S_IMODE(info.st_mode) == self._mode),
                 'child input is not its bounded single-link regular file')

    def _lineage(self) -> None:
        _fd, lineage, descriptors = _open_release_output_parent(self.path.parent)
        primary = None
        try:
            _require(self._parent is not None and lineage == self._parent[1]
                     and tuple((os.fstat(fd).st_dev, os.fstat(fd).st_ino) for fd in self._parent[2]) == lineage,
                     'child input original ancestor changed')
        except BaseException as error:
            primary = error
            raise
        finally:
            try:
                drain(descriptors)
            except BaseException as cleanup:
                if primary is None:
                    raise
                record_cleanup(primary, cleanup)

    def _read(self) -> tuple[str, bytes | None]:
        _require(not self._closed and self._fd is not None, 'child input file is closed')
        self._lineage()
        before = os.fstat(self._fd)
        self._validate(before)
        _require(seal(before) == self._seal, 'child input original descriptor changed')
        digest, size, chunks = hashlib.sha256(), 0, []
        while True:
            raw = os.pread(self._fd, min(CHUNK, before.st_size - size + 1), size)
            if not raw:
                break
            size += len(raw)
            _require(size <= before.st_size, 'child input file exceeded original extent')
            digest.update(raw)
            if self._retain:
                chunks.append(raw)
        _require(self._parent is not None and size == before.st_size
                 and seal(os.fstat(self._fd)) == self._seal
                 == seal(os.stat(self.path.name, dir_fd=self._parent[0], follow_symlinks=False)),
                 'child input file changed during its original read')
        self._lineage()
        return digest.hexdigest(), b''.join(chunks) if self._retain else None

    def recheck(self) -> None:
        """Recheck original descriptor, pathname, ancestry and all original bytes."""
        if self._checking:
            self._failed = True
        _require(not self._failed and not self._closed and not self._checking,
                 'child input file is refused or reentrant')
        self._checking = True
        try:
            observed = self._read()
            _require(not self._failed and not self._closed and observed == (self._digest, self._raw),
                     'child input file changed during recheck')
        except BaseException:
            self._failed = True
            raise
        finally:
            self._checking = False

    @property
    def descriptor(self) -> int:
        """Borrow the retained fd; callers must not close or replace its ownership."""
        if self._checking:
            self._failed = True
        _require(not self._closed and not self._failed and not self._checking and self._fd is not None,
                 'child input file has no active descriptor')
        return self._fd

    @property
    def raw(self) -> bytes:
        """Read the original bounded captured bytes, never a replacement path."""
        self.descriptor
        _require(self._retain and self._raw is not None, 'streamed original has no retained byte buffer')
        return self._raw

    @property
    def identity(self) -> tuple[str, int]:
        """Return only byte identity, not process/native verification authority."""
        self.descriptor
        return self._digest, self._seal[6]

    def close(self) -> None:
        """Detach all ownership before once-only cleanup; never unlink an input."""
        self._closed = True
        if self._checking:
            self._failed = True
        fd, parent = self._fd, self._parent
        self._fd = self._parent = None
        descriptors = (() if parent is None else parent[2]) + (() if fd is None else (fd,))
        drain(descriptors)
