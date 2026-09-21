"""Own the exact six-consumer file graph without asserting package execution.

Every producer needs its concrete executed-input verifier before this inventory
can participate in SF11. Parsing an index or matching its digests is deliberately
not a qualification result. The independently authenticated signed aggregate and
reviewed producer/operator remain the authority for process observations.
"""
from __future__ import annotations

from contextlib import AbstractContextManager
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
from typing import BinaryIO

from release_manifest_signing import _open_release_output_parent
from sorafs_evidence_json import decode_evidence_json

SCHEMA = "sorafs.reference_sdk.package_index.v1"
CONSUMERS = ("javascript", "python", "kotlin_jvm", "java_source_kotlin", "swift", "csharp")
SUFFIXES = (".tgz", ".whl", ".zip", ".zip", ".zip", ".nupkg")
MAX_INDEX_BYTES = 2 * 1024 * 1024
MAX_FILES = 256
MAX_FILE_BYTES = 1024 * 1024 * 1024
MAX_TOTAL_BYTES = 8 * 1024 * 1024 * 1024
_DIGEST = re.compile(r"[0-9a-f]{64}\Z")
_COMMIT = re.compile(r"[0-9a-f]{40}\Z")
_VERSION = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?(?:\+[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?\Z")


class IndexError(ValueError):
    """The exact index or an original indexed file failed its custody contract."""


def _object(value: object, fields: set[str], label: str) -> dict:
    if type(value) is not dict or set(value) != fields:
        raise IndexError(f"{label} fields do not match the closed V1 contract")
    return value


def _digest(value: object) -> str:
    if type(value) is not str or _DIGEST.fullmatch(value) is None or value == "0" * 64:
        raise IndexError("source digest must be canonical nonzero SHA-256")
    return value


def _path(value: object) -> str:
    if (type(value) is not str or not value or len(value) > 1024
            or re.fullmatch(r"[A-Za-z0-9_.\-/]+", value) is None
            or value.startswith("/") or any(part in ("", ".", "..") for part in value.split("/"))
            or PurePosixPath(value).as_posix() != value):
        raise IndexError("indexed path must be a canonical bounded relative file path")
    return value


@dataclass(frozen=True)
class FileReference:
    """An exact expected file identity; this value alone authenticates no file."""
    path: str
    sha256: str
    size: int


@dataclass(frozen=True)
class Consumer:
    """One canonical consumer and the actual original files its adapter must use."""
    name: str
    version: str
    artifact: str
    execution: str
    inputs: tuple[str, ...]


@dataclass(frozen=True)
class PackageIndex:
    """Immutable parsed structure with no execution or release approval claim."""
    source_commit: str
    workspace_source_manifest_sha256: str
    files: tuple[FileReference, ...]
    consumers: tuple[Consumer, ...]
    sha256: str
    size: int

    def file(self, path: str) -> FileReference:
        """Return the one indexed identity or reject an unindexed source."""
        for reference in self.files:
            if reference.path == path:
                return reference
        raise IndexError("file is not in the original index")

    def consumer(self, name: str) -> Consumer:
        """Return the one canonical consumer without accepting retired aliases."""
        for consumer in self.consumers:
            if consumer.name == name:
                return consumer
        raise IndexError("consumer is absent or retired")


def parse_index(raw: bytes, *, expected_source_commit: str, expected_source_manifest_sha256: str) -> PackageIndex:
    """Parse one closed six-consumer dependency graph from original captured bytes."""
    if not raw or len(raw) > MAX_INDEX_BYTES:
        raise IndexError("package index byte limit exceeded")
    value = _object(decode_evidence_json(raw), {"schema", "candidate", "files", "consumers"}, "index")
    if (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False,
                   allow_nan=False) + "\n").encode("utf-8") != raw:
        raise IndexError("package index must use the canonical V1 JSON encoding")
    if value["schema"] != SCHEMA:
        raise IndexError("package index schema is unsupported")
    candidate = _object(value["candidate"], {"source_commit", "workspace_source_manifest_sha256"}, "candidate")
    if type(candidate["source_commit"]) is not str or _COMMIT.fullmatch(candidate["source_commit"]) is None:
        raise IndexError("candidate commit must be full canonical Git identity")
    if candidate["source_commit"] != expected_source_commit or _digest(candidate["workspace_source_manifest_sha256"]) != _digest(expected_source_manifest_sha256):
        raise IndexError("index does not belong to the independently selected candidate")
    inventory = value["files"]
    if type(inventory) is not dict or not 0 < len(inventory) <= MAX_FILES or list(inventory) != sorted(inventory):
        raise IndexError("file inventory must be nonempty, bounded and sorted")
    files = []
    for path, reference in inventory.items():
        path = _path(path)
        reference = _object(reference, {"sha256", "size"}, "file reference")
        size = reference["size"]
        if type(size) is not int or not 0 < size <= MAX_FILE_BYTES:
            raise IndexError("indexed file size is outside its exact positive bound")
        files.append(FileReference(path, _digest(reference["sha256"]), size))
    if sum(reference.size for reference in files) > MAX_TOTAL_BYTES:
        raise IndexError("indexed sources exceed their combined size bound")
    rows = value["consumers"]
    if type(rows) is not list or len(rows) != len(CONSUMERS):
        raise IndexError("index must contain exactly six canonical consumers")
    consumers, covered, artifacts = [], set(), set()
    for name, suffix, row in zip(CONSUMERS, SUFFIXES, rows, strict=True):
        row = _object(row, {"consumer", "version", "artifact", "execution", "inputs"}, "consumer")
        if row["consumer"] != name:
            raise IndexError("consumer order and names must match the sole V1 inventory")
        if type(row["version"]) is not str or _VERSION.fullmatch(row["version"]) is None:
            raise IndexError("consumer version is not canonical SemVer")
        artifact, execution = _path(row["artifact"]), _path(row["execution"])
        if not artifact.endswith(suffix) or not execution.endswith(".zip"):
            raise IndexError("consumer artifact/execution format is not its producer format")
        if name == "java_source_kotlin" and artifact != execution:
            raise IndexError("Java consumer owns its one original qualification archive")
        if name != "java_source_kotlin" and artifact == execution:
            raise IndexError("SDK package cannot substitute for its execution observations")
        inputs = row["inputs"]
        if type(inputs) is not list or not inputs or any(type(path) is not str for path in inputs) or inputs != sorted(set(inputs)):
            raise IndexError("consumer inputs must be nonempty, sorted and unique")
        for path in inputs:
            _path(path)
        references = {artifact, execution, *inputs}
        if not references <= inventory.keys() or artifact in inputs or execution in inputs:
            raise IndexError("consumer references unindexed or duplicated own artifacts")
        covered |= references
        digest = inventory[artifact]["sha256"]
        if digest in artifacts:
            raise IndexError("different consumer artifacts must have distinct original bytes")
        artifacts.add(digest)
        consumers.append(Consumer(name, row["version"], artifact, execution, tuple(inputs)))
    if covered != inventory.keys():
        raise IndexError("index contains an unconsumed file")
    if consumers[2].version != consumers[3].version or consumers[2].artifact not in consumers[3].inputs:
        raise IndexError("Java consumer must bind the canonical Kotlin package/version")
    return PackageIndex(candidate["source_commit"], candidate["workspace_source_manifest_sha256"], tuple(files), tuple(consumers), hashlib.sha256(raw).hexdigest(), len(raw))


def _identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (metadata.st_dev, metadata.st_ino, metadata.st_mode, metadata.st_uid,
            metadata.st_nlink, metadata.st_size, metadata.st_mtime_ns, metadata.st_ctime_ns)


class OpenedIndexFiles(AbstractContextManager):
    """Keep original descriptors/ancestor identities through one verification operation.

    Each file is authenticated when opened and after the complete operation.
    An adapter reads immutable bytes from that same held descriptor. No parser
    obtains a replacement path owner. Buffers belong to the adapter, not a global
    cache; hashing streams in fixed chunks even for the largest package inputs.
    """
    def __init__(self, root: Path, index: PackageIndex):
        if not root.is_absolute() or root != Path(os.path.normpath(root)):
            raise IndexError("indexed source root must be an absolute canonical directory")
        self.root, self.index = root, index
        self._parents = {}
        self._files: dict[str, tuple[BinaryIO, tuple[int, ...]]] = {}
        self._active = False
        self._entered = False

    def _check(self, reference: FileReference, handle: BinaryIO, expected: tuple[int, ...]) -> None:
        if _identity(os.fstat(handle.fileno())) != expected:
            raise IndexError("original indexed file changed during verification")
        handle.seek(0)
        digest, size = hashlib.sha256(), 0
        while chunk := handle.read(1024 * 1024):
            size += len(chunk)
            if size > reference.size:
                raise IndexError("indexed file grew beyond its declared allocation")
            digest.update(chunk)
        if size != reference.size or digest.hexdigest() != reference.sha256 or _identity(os.fstat(handle.fileno())) != expected:
            raise IndexError("original indexed file does not match its exact bytes")

    def __enter__(self):
        if self._entered:
            raise IndexError("an indexed input owner may be entered only once")
        self._entered = True
        physical_files = set()
        try:
            for reference in self.index.files:
                path = self.root / reference.path
                parent = path.parent
                if parent not in self._parents:
                    _, lineage, descriptors = _open_release_output_parent(parent)
                    self._parents[parent] = (lineage, descriptors)
                descriptors = self._parents[parent][1]
                descriptor = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
                                     | getattr(os, "O_CLOEXEC", 0), dir_fd=descriptors[-1])
                handle = os.fdopen(descriptor, "rb")
                metadata = os.fstat(handle.fileno())
                self._files[reference.path] = handle, _identity(metadata)
                if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or metadata.st_size != reference.size:
                    raise IndexError("indexed input must be the original single-link regular file of its exact size")
                physical = (metadata.st_dev, metadata.st_ino)
                if physical in physical_files:
                    raise IndexError("indexed paths alias the same physical input")
                physical_files.add(physical)
                self._check(reference, handle, _identity(metadata))
            self._active = True
            return self
        except BaseException:
            self.close()
            raise

    def read(self, path: str, maximum: int) -> bytes:
        """Read one admitted exact immutable byte sequence from its original owner."""
        if not self._active:
            raise IndexError("indexed input owner is not active")
        reference = self.index.file(path)
        if type(maximum) is not int or maximum <= 0 or reference.size > maximum:
            raise IndexError("adapter input exceeds its own byte bound")
        handle, expected = self._files[path]
        self._check(reference, handle, expected)
        handle.seek(0)
        raw = handle.read(reference.size + 1)
        if len(raw) != reference.size or hashlib.sha256(raw).hexdigest() != reference.sha256 or _identity(os.fstat(handle.fileno())) != expected:
            raise IndexError("captured indexed bytes changed during their read")
        return raw

    def recheck(self) -> None:
        """Authenticate original descriptors and paths before publishing any result."""
        if not self._active:
            raise IndexError("indexed input owner is not active")
        for reference in self.index.files:
            handle, expected = self._files[reference.path]
            self._check(reference, handle, expected)
            path = self.root / reference.path
            descriptors = self._parents[path.parent][1]
            if _identity(os.stat(path.name, dir_fd=descriptors[-1], follow_symlinks=False)) != expected:
                raise IndexError("indexed source path lost its original physical owner")
        for parent, (expected, _) in self._parents.items():
            _, observed, descriptors = _open_release_output_parent(parent)
            for descriptor in reversed(descriptors):
                os.close(descriptor)
            if expected != observed:
                raise IndexError("indexed source ancestor changed during verification")

    def close(self) -> None:
        """Release every held file and ancestor on success, failure or cancellation."""
        self._active = False
        for handle, _ in self._files.values():
            handle.close()
        self._files.clear()
        for _, descriptors in self._parents.values():
            for descriptor in reversed(descriptors):
                os.close(descriptor)
        self._parents.clear()

    def __exit__(self, exc_type, exc_value, traceback):
        try:
            if exc_type is None:
                self.recheck()
        finally:
            self.close()
        return False
