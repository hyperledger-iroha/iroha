"""Stage and publish the Python producer's two actual artifacts without replacement.

The caller owns semantic verification and the final original-input recheck. This
POSIX-only context retains its pending descriptors and parent ancestry through
that check and publication. Failed attempts keep bounded pending diagnostics;
no final name is created before the final callback succeeds. This is artifact
custody, not a signing or execution-qualification authority.
"""
from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import stat
from typing import Callable

from release_manifest_signing import _open_release_output_parent
from sorafs_python_runtime_inputs import MAX_BUNDLE_BYTES
from sorafs_sdk_artifact_index import MAX_FILE_BYTES

_CHUNK = 1024 * 1024


class PublicationError(RuntimeError):
    """An owned pending artifact or its no-replace publication failed."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise PublicationError(message)


def _metadata(value: os.stat_result) -> tuple:
    return (value.st_dev, value.st_ino, value.st_mode, value.st_uid, value.st_nlink,
            value.st_size, value.st_mtime_ns, value.st_ctime_ns)


@dataclass(frozen=True)
class PublishedFile:
    path: Path
    sha256: str
    size: int


@dataclass(frozen=True)
class PublishedArtifacts:
    runtime_bundle: PublishedFile
    execution_archive: PublishedFile


class _Pending:
    def __init__(self, owner, name: str, maximum: int):
        self.owner, self.name, self.maximum = owner, name, maximum
        self.pending = "." + name + ".pending"
        self.size = 0
        self.closed = False
        self.expected = None
        self.sealed_metadata = None
        self.fd = os.open(self.pending, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
                          | os.O_CLOEXEC, 0o600, dir_fd=owner.parent)
        metadata = os.fstat(self.fd)
        self.physical = metadata.st_dev, metadata.st_ino

    def write(self, raw: bytes) -> int:
        _require(not self.closed and self.expected is None and type(raw) is bytes,
                 "pending stream is closed or does not contain immutable bytes")
        _require(self.size + len(raw) <= self.maximum, "pending artifact exceeded its byte bound")
        position = 0
        while position < len(raw):
            count = os.write(self.fd, memoryview(raw)[position:])
            _require(count > 0, "pending artifact write made no progress")
            position += count
        self.size += len(raw)
        return len(raw)

    def finish_stream(self) -> None:
        os.fsync(self.fd)
        self.closed = True

    def readback(self, expected_sha256: str, expected_size: int, *, capture: bool = False) -> bytes | None:
        _require(self.closed and type(expected_sha256) is str
                 and re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is not None
                 and expected_sha256 != "0" * 64 and type(expected_size) is int
                 and 0 < expected_size <= self.maximum and self.size == expected_size,
                 "pending artifact identity or size differs")
        _require(self.expected is None or (self.expected.sha256, self.expected.size)
                 == (expected_sha256, expected_size), "pending artifact cannot be rebound to another identity")
        self.owner._lineage()
        before = os.fstat(self.fd)
        _require(self.sealed_metadata is None or _metadata(before) == self.sealed_metadata,
                 "pending artifact metadata changed after readback")
        named = os.stat(self.pending, dir_fd=self.owner.parent, follow_symlinks=False)
        _require(stat.S_ISREG(before.st_mode) and stat.S_IMODE(before.st_mode) == 0o600
                 and before.st_nlink == 1 and before.st_size == expected_size
                 and (before.st_dev, before.st_ino) == self.physical
                 and _metadata(before) == _metadata(named), "pending artifact lost its original file owner")
        raw = self._read_exact(expected_sha256, expected_size, before, capture=capture)
        self.expected = PublishedFile(self.owner.work / self.name, expected_sha256, expected_size)
        self.sealed_metadata = _metadata(before)
        return raw

    def _read_exact(self, expected_sha256: str, expected_size: int, before: os.stat_result,
                    *, capture: bool = False) -> bytes | None:
        os.lseek(self.fd, 0, os.SEEK_SET)
        digest, size, chunks = hashlib.sha256(), 0, []
        while raw := os.read(self.fd, min(_CHUNK, expected_size - size + 1)):
            size += len(raw)
            _require(size <= expected_size, "pending artifact grew during readback")
            digest.update(raw)
            if capture:
                chunks.append(raw)
        _require(size == expected_size and digest.hexdigest() == expected_sha256
                 and _metadata(os.fstat(self.fd)) == _metadata(before), "pending artifact bytes changed")
        return b"".join(chunks) if capture else None

    def publish(self) -> None:
        # link creates the new final name atomically and refuses an existing name.
        self.owner._linked.append(self)
        os.link(self.pending, self.name, src_dir_fd=self.owner.parent,
                dst_dir_fd=self.owner.parent, follow_symlinks=False)
        current = os.fstat(self.fd)
        named = os.stat(self.name, dir_fd=self.owner.parent, follow_symlinks=False)
        pending = os.stat(self.pending, dir_fd=self.owner.parent, follow_symlinks=False)
        _require((current.st_dev, current.st_ino) == self.physical and current.st_nlink == 2
                 and _metadata(current) == _metadata(named) == _metadata(pending),
                 "published artifact lost its original pending owner")
        # Linking legitimately changes ctime and nlink; all other sealed
        # metadata and the exact descriptor bytes must still match.
        _require(all(_metadata(current)[index] == self.sealed_metadata[index]
                     for index in (0, 1, 2, 3, 5, 6)), "pending artifact changed during publication")
        self._read_exact(self.expected.sha256, self.expected.size, current)
        self.owner._lineage()
        os.unlink(self.pending, dir_fd=self.owner.parent)
        current = os.fstat(self.fd)
        named = os.stat(self.name, dir_fd=self.owner.parent, follow_symlinks=False)
        _require(current.st_nlink == 1 and _metadata(current) == _metadata(named),
                 "published artifact has an unexpected additional owner")

    def close(self) -> None:
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None


class PythonArtifactPublication:
    """One-shot staged runtime bundle plus execution ZIP; ZIP is published last."""
    def __init__(self, work: Path):
        self.work = work
        self.parent = None
        self.handles = ()
        self.runtime = self.archive = None
        self._linked = []
        self._entered = self._closed = self._published = self._publishing = False

    def __enter__(self):
        _require(not self._entered and not self._closed, "publication owner cannot be reopened")
        _require(isinstance(self.work, Path) and self.work.is_absolute()
                 and str(self.work) == os.path.normpath(self.work), "publication parent is not canonical")
        _require(os.name == "posix" and os.link in os.supports_dir_fd
                 and os.link in os.supports_follow_symlinks, "no-replace descriptor publication is unavailable")
        self._entered = True
        try:
            self.parent, self.lineage, self.handles = _open_release_output_parent(self.work)
            self._absent_finals()
            return self
        except BaseException:
            self.close()
            raise

    def _active(self) -> None:
        _require(self._entered and not self._closed and not self._published and not self._publishing,
                 "publication owner is not open for staging")

    def _lineage(self) -> None:
        _, lineage, handles = _open_release_output_parent(self.work)
        try:
            _require(lineage == self.lineage, "publication parent was replaced")
        finally:
            for descriptor in reversed(handles):
                os.close(descriptor)

    def _absent_finals(self) -> None:
        for name in ("python-runtime-inputs.bundle", "python-consumer.zip"):
            try:
                os.stat(name, dir_fd=self.parent, follow_symlinks=False)
            except FileNotFoundError:
                continue
            raise PublicationError("completed artifact name already exists")

    @contextmanager
    def runtime_stream(self):
        """Expose only a bounded writer; retain its original descriptor after exit."""
        self._active()
        _require(self.runtime is None, "runtime bundle may be staged only once")
        self.runtime = _Pending(self, "python-runtime-inputs.bundle", MAX_BUNDLE_BYTES)
        try:
            yield self.runtime
        finally:
            self.runtime.finish_stream()

    def read_runtime_bundle(self, *, expected_sha256: str, expected_size: int) -> bytes:
        """Recheck/read the original staged descriptor, never reopen its path."""
        self._active()
        _require(self.runtime is not None, "runtime bundle has not been staged")
        return self.runtime.readback(expected_sha256, expected_size, capture=True)

    def stage_execution_archive(self, raw: bytes) -> PublishedFile:
        """Write and verify the original complete ZIP bytes under a pending name."""
        self._active()
        _require(self.archive is None and type(raw) is bytes and 0 < len(raw) <= MAX_FILE_BYTES,
                 "execution archive is absent, repeated or exceeds its bound")
        self.archive = _Pending(self, "python-consumer.zip", MAX_FILE_BYTES)
        for offset in range(0, len(raw), _CHUNK):
            self.archive.write(raw[offset:offset + _CHUNK])
        self.archive.finish_stream()
        self.archive.readback(hashlib.sha256(raw).hexdigest(), len(raw))
        return self.archive.expected

    def publish(self, *, check_originals: Callable[[], None]) -> PublishedArtifacts:
        """Run final original checks before any final name, then publish ZIP last.

        The caller must close its input owners without another automatic recheck
        after this returns. Keep those owners live through this call; this method
        provides no signature, case-execution or producer-approval assertion.
        """
        self._active()
        _require(callable(check_originals) and self.runtime is not None and self.archive is not None
                 and self.runtime.expected is not None and self.archive.expected is not None,
                 "publication requires both original readbacks and a final input check")
        self._publishing = True
        try:
            check_originals()
            self._lineage()
            self._absent_finals()
            for pending in (self.runtime, self.archive):
                pending.readback(pending.expected.sha256, pending.expected.size)
            for pending in (self.runtime, self.archive):
                pending.publish()
            self._published = True
            return PublishedArtifacts(self.runtime.expected, self.archive.expected)
        except BaseException:
            self._rollback_links()
            raise

    def _rollback_links(self) -> None:
        for pending in reversed(self._linked):
            try:
                named = os.stat(pending.name, dir_fd=self.parent, follow_symlinks=False)
            except FileNotFoundError:
                continue
            if (named.st_dev, named.st_ino) == pending.physical:
                os.unlink(pending.name, dir_fd=self.parent)
        self._linked.clear()

    def close(self) -> None:
        """Close only; completed publication has no post-publication recheck."""
        if self._closed:
            return
        self._closed = True
        for pending in (self.runtime, self.archive):
            if pending is not None:
                pending.close()
        for descriptor in reversed(self.handles):
            os.close(descriptor)
        self.handles = ()

    def __exit__(self, _exc_type, _exc_value, _traceback):
        self.close()
        return False
