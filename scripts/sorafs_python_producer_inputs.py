"""Bounded original input and installed-wheel joins for the Python producer.

Requires the repository's sole wheel verifier and fixed observation child. These
helpers do not install packages, execute assertions or grant release approval.
"""
from __future__ import annotations

from dataclasses import asdict
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import stat
import zipfile

from release_manifest_signing import _open_release_output_parent

from sorafs_python_consumer_artifact import (
    ArtifactError, FIXED_SOURCES, MAX_SOURCE_BYTES, MAX_SOURCE_FILES,
    MAX_SOURCE_FILE_BYTES, SOURCE_PREFIXES, _VERIFIER as verifier,
)

# The POSIX release package is the pyo3 abi3-py39 artifact. Producer and remote
# replay use this source-owned profile, never the verifier host's suffix list.
POSIX_EXTENSION_SUFFIXES = (".abi3.so",)

_CHILD_PATH = Path(__file__).resolve().parent / "fixtures/SorafsPythonConsumerQualificationRunner.py"
_spec = importlib.util.spec_from_file_location("_sorafs_python_producer_child", _CHILD_PATH)
if _spec is None or _spec.loader is None:
    raise ArtifactError("fixed Python child owner is missing")
child = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(child)


def identity(raw: bytes) -> dict:
    """Derive a content identity from actual captured bytes."""
    return {"sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)}


def write_fresh(path: Path, raw: bytes, *, executable: bool = False) -> None:
    """Create a private output exclusively; retain partial output on failure."""
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.parent.resolve(strict=True) != path.parent:
        raise ArtifactError("output parent is not canonical")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
    descriptor = os.open(path, flags, 0o700 if executable else 0o600)
    try:
        position = 0
        while position < len(raw):
            count = os.write(descriptor, memoryview(raw)[position:])
            if count <= 0:
                raise ArtifactError("output write made no progress")
            position += count
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


class OriginalInputs:
    """Own primary descriptors; streamed trees retain before/after seals only.

    Use ``with`` or explicit ``close``. Inputs passed with ``hold=True`` remain
    open with their original directory ancestry until producer completion.
    Source/runtime tree scans may use ``hold=False`` to bound descriptor count;
    those scans do not acquire the primary held-descriptor guarantee.
    """

    MAX_HELD_FILES = 256
    MAX_HELD_BYTES = 8 * 1024 * 1024 * 1024
    MAX_FILE_BYTES = 1024 * 1024 * 1024

    def __init__(self):
        self.files: dict[Path, tuple[bytes, tuple]] = {}
        self.links: dict[Path, tuple] = {}
        self._held: dict[Path, tuple[int, tuple]] = {}
        self._parents = {}
        self._closed = self._entered = False

    def __enter__(self):
        if self._closed or self._entered:
            raise ArtifactError("original input owner cannot be reopened")
        self._entered = True
        return self

    def _held_read(self, path: Path, maximum: int) -> tuple[bytes, tuple]:
        descriptor, expected = self._held[path]
        if child._stat_identity(os.fstat(descriptor)) != expected:
            raise ArtifactError("held original input metadata changed")
        os.lseek(descriptor, 0, os.SEEK_SET)
        blocks, size, digest = [], 0, hashlib.sha256()
        while raw := os.read(descriptor, min(1024 * 1024, maximum - size + 1)):
            size += len(raw)
            if size > maximum:
                raise ArtifactError("held original input exceeds its byte bound")
            digest.update(raw); blocks.append(raw)
        parent = self._parents[path.parent][0]
        if (child._stat_identity(os.fstat(descriptor)) != expected
                or child._stat_identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False)) != expected
                or size != expected[2]):
            raise ArtifactError("held original input lost its original file owner")
        _, lineage, descriptors = _open_release_output_parent(path.parent)
        try:
            if lineage != self._parents[path.parent][1]:
                raise ArtifactError("held original input ancestor changed")
        finally:
            for fd in reversed(descriptors):
                os.close(fd)
        return b"".join(blocks), (*expected[:6], digest.hexdigest())

    def _hold(self, path: Path, maximum: int) -> None:
        if len(self._held) >= self.MAX_HELD_FILES:
            raise ArtifactError("held original input file count exceeds its bound")
        if path.parent not in self._parents:
            self._parents[path.parent] = _open_release_output_parent(path.parent)
        descriptor = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                             dir_fd=self._parents[path.parent][0])
        try:
            metadata = os.fstat(descriptor)
            expected = child._stat_identity(metadata)
            if (not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1
                    or metadata.st_size > maximum
                    or sum(value[1][2] for value in self._held.values()) + metadata.st_size > self.MAX_HELD_BYTES):
                raise ArtifactError("held input is not a bounded single-link regular file")
            if any(value[1][:2] == expected[:2] for value in self._held.values()):
                raise ArtifactError("held original input paths alias a physical file")
            self._held[path] = descriptor, expected
        except BaseException:
            os.close(descriptor)
            raise

    def read(self, path: Path, maximum: int, *, hold: bool = False) -> bytes:
        """Capture once; held primary reads never reopen a substitute leaf."""
        if self._closed:
            raise ArtifactError("original input owner is closed")
        if (type(maximum) is not int or not 0 <= maximum <= self.MAX_FILE_BYTES
                or type(hold) is not bool or not isinstance(path, Path)
                or not path.is_absolute() or str(path) != os.path.normpath(path)):
            raise ArtifactError("original input path or byte bound is invalid")
        if hold and path not in self._held:
            self._hold(path, maximum)
        raw, seal = (self._held_read(path, maximum) if path in self._held
                     else child.read_stable(path, limit=maximum))
        if path in self.files and self.files[path] != (raw, seal):
            raise ArtifactError("original producer input changed")
        self.files[path] = (raw, seal)
        return raw

    def recheck(self) -> None:
        """Recheck held owners and independently rescan all streamed file seals."""
        if self._closed:
            raise ArtifactError("original input owner is closed")
        for path, expected in self.files.items():
            observed = (self._held_read(path, len(expected[0])) if path in self._held
                        else child.read_stable(path, limit=max(1, len(expected[0]))))
            if observed != expected:
                raise ArtifactError("original producer input changed during execution")
        for path, expected in self.links.items():
            if (child._stat_identity(path.lstat()), os.readlink(path)) != expected:
                raise ArtifactError("original environment directory alias changed")

    def directory_link(self, path: Path, target: str) -> None:
        """Retain one explicitly modeled private-environment directory alias."""
        if self._closed or len(self.links) >= 64 and path not in self.links:
            raise ArtifactError("environment directory alias owner is closed or excessive")
        if path.parent.resolve(strict=True) != path.parent or not path.is_symlink():
            raise ArtifactError("environment directory alias is not canonical")
        before = path.lstat()
        observed = (child._stat_identity(before), os.readlink(path))
        if (observed[1] != target or path.resolve(strict=True) != path.parent / target
                or not path.resolve(strict=True).is_dir()
                or child._stat_identity(path.lstat()) != observed[0]):
            raise ArtifactError("environment directory alias target differs")
        if path in self.links and self.links[path] != observed:
            raise ArtifactError("original environment directory alias changed")
        self.links[path] = observed

    def close(self) -> None:
        """Close every original primary and ancestor descriptor exactly once."""
        self._closed = True
        for descriptor, _expected in self._held.values():
            os.close(descriptor)
        self._held.clear()
        for _parent, _lineage, descriptors in self._parents.values():
            for descriptor in reversed(descriptors):
                os.close(descriptor)
        self._parents.clear()

    def __exit__(self, exc_type, _exc_value, _traceback):
        try:
            if exc_type is None:
                self.recheck()
        finally:
            self.close()
        return False


def capture_tree(root: Path, originals: OriginalInputs, *, maximum: int,
                 maximum_file: int, maximum_entries: int,
                 directory_links: dict[str, str] | None = None) -> dict[str, bytes]:
    """Capture a complete bounded ordinary-file tree with no ignored entries."""
    if root.resolve(strict=True) != root or not root.is_dir():
        raise ArtifactError("source tree must be a canonical directory")
    allowed_links = {} if directory_links is None else directory_links
    seen_links = set()
    pending, result, directories = [root], {}, {}
    count = total = 0
    while pending:
        directory = pending.pop()
        before = directory.lstat()
        directories[directory] = child._stat_identity(before)
        if not stat.S_ISDIR(before.st_mode):
            raise ArtifactError("source directory changed type")
        with os.scandir(directory) as stream:
            for entry in stream:
                count += 1
                if count > maximum_entries:
                    raise ArtifactError("tree entry bound or symbolic link")
                path = Path(entry.path)
                name = path.relative_to(root).as_posix()
                if entry.is_symlink():
                    if name not in allowed_links:
                        raise ArtifactError("unmodeled source tree symbolic link")
                    originals.directory_link(path, allowed_links[name])
                    seen_links.add(name)
                    continue
                if entry.is_dir(follow_symlinks=False):
                    pending.append(path)
                    continue
                child._relative(name)
                raw = originals.read(path, min(maximum_file, maximum - total))
                result[name] = raw
                total += len(raw)
    if any(child._stat_identity(path.lstat()) != seal for path, seal in directories.items()):
        raise ArtifactError("source tree changed while captured")
    if seen_links != set(allowed_links):
        raise ArtifactError("modeled directory alias inventory differs")
    return dict(sorted(result.items()))


def source_snapshot(root: Path, originals: OriginalInputs) -> dict[str, bytes]:
    """Capture the exact fixed test/tools and complete three dependency trees."""
    result = {name: originals.read(root / name, MAX_SOURCE_FILE_BYTES)
              for name in sorted(FIXED_SOURCES)}
    for prefix in SOURCE_PREFIXES:
        tree = capture_tree(root / prefix.rstrip("/"), originals,
                            maximum=MAX_SOURCE_BYTES - sum(map(len, result.values())),
                            maximum_file=MAX_SOURCE_FILE_BYTES,
                            maximum_entries=MAX_SOURCE_FILES - len(result))
        result.update({prefix + name: raw for name, raw in tree.items()})
    if len(result) > MAX_SOURCE_FILES or sum(map(len, result.values())) > MAX_SOURCE_BYTES:
        raise ArtifactError("source snapshot exceeds its admitted bounds")
    return dict(sorted(result.items()))


def native_member(raw: bytes, archive) -> bytes:
    """Read only the sole parser's already validated native member."""
    if archive.owner != verifier.NATIVE_OWNER or archive.native_member is None:
        raise ArtifactError("native wheel does not own an extension")
    expected = next(member for member in archive.package_members
                    if member.name == archive.native_member)
    with zipfile.ZipFile(io.BytesIO(raw)) as wheel:
        body = verifier._read_member_bytes(wheel, wheel.getinfo(expected.name), label="native wheel extension")
    if identity(body) != {"sha256": expected.sha256, "size": expected.size}:
        raise ArtifactError("native member differs from the sole parsed wheel")
    return body


def installed_wheel_join(observation, wheel, originals: OriginalInputs) -> dict[str, bytes]:
    """Join every observed installed file to original wheel bytes and pip metadata.

    Retain the actual RECORD/direct_url bytes for the later original-index
    adapter. Package files derive from the sole wheel parser, never a report's
    asserted member inventory.
    """
    if observation.owner != wheel.owner.package or observation.version != wheel.metadata_version:
        raise ArtifactError("observed installed wheel owner differs")
    if observation.path != str(wheel.path) or asdict(observation.seal) != asdict(wheel.seal):
        raise ArtifactError("observed original wheel seal differs")
    initializer = wheel.package_member
    matches = [row for row in observation.installed_files if row.path.endswith("/" + initializer)]
    if len(matches) != 1:
        raise ArtifactError("installed wheel has no unique package root")
    site = Path(matches[0].path).parent.parent
    # Restrict capture to original member locations before any filesystem read.
    # Content, generated metadata and RECORD policy remain the sole verifier's.
    names = {member.name for member in (*wheel.package_members, *wheel.dist_info_members)}
    names.update(wheel.dist_info_root + "/" + name for name in verifier.PIP_GENERATED_DIST_INFO_FILES)
    observed = {row.path: row for row in observation.installed_files}
    if (len(observation.installed_files) != len(observed)
            or set(observed) != {str(site / name) for name in names}):
        raise ArtifactError("observed installed file inventory differs from original wheel")
    captured, total = {}, 0
    for row in observation.installed_files:
        path = Path(row.path)
        if not path.is_relative_to(site):
            raise ArtifactError("observed installed file escapes original site root")
        name = path.relative_to(site).as_posix()
        if name in captured:
            raise ArtifactError("observed installed file repeats a member")
        raw = originals.read(path, min(verifier.MAX_MEMBER_BYTES,
                                       verifier.MAX_TOTAL_UNCOMPRESSED_BYTES - total))
        total += len(raw)
        device, inode, actual_size, mtime, ctime, mode, actual_digest = originals.files[path][1]
        seal = verifier.FileSeal(actual_digest, device, inode, actual_size, mtime, ctime,
                                 stat.S_IMODE(mode))
        if asdict(seal) != asdict(row.seal):
            raise ArtifactError("installed report differs from original consumed bytes")
        captured[name] = raw
    try:
        content = verifier.verify_installed_wheel_bytes(
            wheel, source_uri=wheel.path.as_uri(), wheel_sha256=wheel.seal.sha256,
            installed_files=captured)
    except verifier.VerificationError as error:
        raise ArtifactError(str(error)) from error
    if set(observed) != {str(site / member.name) for member in content.files}:
        raise ArtifactError("observed installed file inventory differs from original wheel")
    return {wheel.dist_info_root + "/" + name: captured[wheel.dist_info_root + "/" + name]
            for name in ("direct_url.json", "RECORD")}
