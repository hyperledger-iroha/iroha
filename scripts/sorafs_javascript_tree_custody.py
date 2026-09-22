"""One bounded POSIX physical tree owner shared by JavaScript input relations.

Root and ancestor descriptors stay held; leaf/subdirectory descriptors are
transient. Complete initial/final seals cannot detect a change fully restored
between observations or attest loaded code. No extraction or execution occurs.
"""
from __future__ import annotations

from contextlib import AbstractContextManager
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import stat

from release_manifest_signing import _open_release_output_parent
from sorafs_javascript_archive import ArchiveError, MAX_NAME_BYTES, NpmPathInventory, _require
from sorafs_sdk_artifact_index import _identity

_READ_BYTES = 64 * 1024
MAX_TREE_FILES = 8192
MAX_TREE_BYTES = 192 * 1024 * 1024
MAX_TREE_FILE_BYTES = 32 * 1024 * 1024
MAX_TREE_DEPTH = 64


@dataclass(frozen=True)
class TreeMember:
    """Immutable physical expectation; conveys no original-input authority."""
    path: str
    content: bytes
    mode: int


def _validate_members(members: tuple[TreeMember, ...]) -> None:
    """Bound names and bytes before allocating indexes or hashing contents."""
    _require(type(members) is tuple and 0 < len(members) <= MAX_TREE_FILES,
             "tree member count exceeds its fixed bound")
    total = 0
    for row in members:
        _require(type(row) is TreeMember and type(row.path) is str
                 and 0 < len(row.path) <= MAX_NAME_BYTES - len("package/")
                 and row.path.count("/") < MAX_TREE_DEPTH
                 and type(row.content) is bytes and len(row.content) <= MAX_TREE_FILE_BYTES
                 and type(row.mode) is int and row.mode in (0o644, 0o755),
                 "tree member shape or per-file bound differs")
        total += len(row.content)
        _require(total <= MAX_TREE_BYTES, "tree total byte bound exceeded")
    inventory = NpmPathInventory()
    for row in members:
        inventory.admit(row.path)


def _close_descriptors(descriptors: tuple[int, ...]) -> None:
    """Attempt each close once; errors never abandon later owned descriptors.

    A failed close may already have freed/reused its numeric fd. Never retry it.
    Propagate the first failure after draining the other original handles.
    """
    first = None
    for descriptor in reversed(descriptors):
        try:
            os.close(descriptor)
        except BaseException as error:
            if first is None:
                first = error
    if first is not None:
        raise first


class OriginalTree(AbstractContextManager):
    """Retain a closed physical tree from immutable expected member bytes.

    This class checks physical/content equality only. A source-specific caller
    must validate its original projection before constructing these expectations.
    It establishes no candidate, package, signer or execution authority.
    """
    def __init__(self, root: Path, members: tuple[TreeMember, ...]):
        _require(os.name == "posix" and all(hasattr(os, name) for name in
                 ("O_NOFOLLOW", "O_NONBLOCK", "O_CLOEXEC", "O_DIRECTORY")),
                 "tree custody requires POSIX descriptor-relative support")
        _require(type(root) is type(Path()) and root.is_absolute()
                 and str(root) == os.path.normpath(root) and "\0" not in str(root)
                 and len(str(root)) <= 4096 and len(root.parts) <= 64,
                 "tree root must be a bounded canonical absolute path")
        _validate_members(members)
        self.root = root
        self._members = {row.path: row for row in members}
        self._directories = {""}
        for path in self._members:
            parts = path.split("/")
            self._directories.update("/".join(parts[:end]) for end in range(1, len(parts)))
        self._hashes = {path: hashlib.sha256(row.content).digest() for path, row in self._members.items()}
        self._parent = None
        self._state = None
        self._entered = self._closed = self._failed = self._checking = False

    def _lineage(self) -> None:
        _fd, lineage, descriptors = _open_release_output_parent(self.root)
        try:
            _require(lineage == self._parent[1], "tree root or ancestor was replaced")
            _require(tuple((os.fstat(fd).st_dev, os.fstat(fd).st_ino) for fd in self._parent[2])
                     == lineage, "held tree ancestor descriptor changed")
        finally:
            _close_descriptors(descriptors)

    def _file(self, parent: int, name: str, path: str, metadata) -> tuple:
        row = self._members[path]
        expected = _identity(metadata)
        descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                             dir_fd=parent)
        try:
            opened = os.fstat(descriptor)
            _require(stat.S_ISREG(opened.st_mode) and opened.st_nlink == 1
                     and opened.st_uid == os.getuid() and stat.S_IMODE(opened.st_mode) == row.mode
                     and opened.st_size == len(row.content) and _identity(opened) == expected,
                     "tree leaf is not its exact single-link regular file")
            digest, size = hashlib.sha256(), 0
            while True:
                data = os.read(descriptor, min(_READ_BYTES, len(row.content) - size + 1))
                if not data:
                    break
                size += len(data)
                _require(size <= len(row.content), "tree leaf exceeded its original byte bound")
                digest.update(data)
            _require(size == len(row.content) and digest.digest() == self._hashes[path],
                     "tree leaf differs from original content")
            _require(_identity(os.fstat(descriptor)) == expected
                     == _identity(os.stat(name, dir_fd=parent, follow_symlinks=False)),
                     "tree leaf changed while read")
            return expected
        finally:
            os.close(descriptor)

    def _scan(self) -> dict[str, tuple]:
        state, physical = {}, set()
        def walk(descriptor: int, relative: str) -> None:
            before = os.fstat(descriptor)
            _require(stat.S_ISDIR(before.st_mode) and before.st_uid == os.getuid()
                     and stat.S_IMODE(before.st_mode) in (0o700, 0o755),
                     "tree directory ownership or mode differs")
            state["d:" + relative] = _identity(before)
            with os.scandir(descriptor) as entries:
                for entry in entries:
                    path = relative + "/" + entry.name if relative else entry.name
                    _require(path in self._directories or path in self._members,
                             "tree tree contains an unowned entry")
                    metadata = os.stat(entry.name, dir_fd=descriptor, follow_symlinks=False)
                    if path in self._directories:
                        _require(stat.S_ISDIR(metadata.st_mode), "tree directory changed type")
                        child = os.open(entry.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
                                        | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=descriptor)
                        try:
                            _require(_identity(os.fstat(child)) == _identity(metadata),
                                     "tree directory changed before open")
                            walk(child, path)
                        finally:
                            os.close(child)
                    else:
                        seal = self._file(descriptor, entry.name, path, metadata)
                        _require(seal[:2] not in physical, "tree paths alias a physical file")
                        physical.add(seal[:2])
                        state["f:" + path] = seal
            _require(_identity(os.fstat(descriptor)) == state["d:" + relative],
                     "tree directory changed while scanned")
        self._lineage()
        walk(self._parent[0], "")
        _require(set(state) == {"d:" + path for path in self._directories}
                 | {"f:" + path for path in self._members}, "tree tree omits original members")
        self._lineage()
        return state

    def __enter__(self):
        if self._checking:
            self._failed = True
            raise ArchiveError("tree custody operation is reentrant")
        _require(not self._entered and not self._closed, "tree owner is one-shot or closed")
        self._entered = self._checking = True
        try:
            self._parent = _open_release_output_parent(self.root)
            observed = self._scan()
            _require(not self._closed and not self._failed,
                     "tree custody ended during initial capture")
            self._state = observed
            return self
        except BaseException:
            self._failed = True
            self.close()
            raise
        finally:
            self._checking = False

    def recheck(self) -> None:
        """Rehash all named leaves and compare original file/directory identities."""
        if self._checking:
            self._failed = True
            raise ArchiveError("tree custody operation is reentrant")
        _require(self._state is not None and not self._closed and not self._failed,
                 "tree owner is inactive or previously failed")
        self._checking = True
        try:
            observed = self._scan()
            _require(not self._closed and not self._failed,
                     "tree custody ended during recheck")
            _require(observed == self._state, "tree original tree identity changed")
        except BaseException:
            self._failed = True
            raise
        finally:
            self._checking = False

    def close(self) -> None:
        """Release held descriptors once; never mutate tree content."""
        self._closed = True
        parent, self._parent = self._parent, None
        if parent is not None:
            try:
                _close_descriptors(parent[2])
            except BaseException:
                self._failed = True
                raise

    def __exit__(self, exc_type, exc_value, traceback):
        try:
            if exc_type is None:
                self.recheck()
        finally:
            self.close()
        return False
