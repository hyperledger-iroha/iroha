"""Stream/recheck original POSIX CPython inputs and produce a bounded byte bundle.

Library only. The parent supplies an independently pinned manifest, owns the
fresh output stream and eventual process lifetime, and must retain this context
through execution. No installation, discovery, subprocess or network is hidden
here. Full normal OS/native-library and actual loader binding stays with the
parent's fixed runtime probe and platform qualification.
"""
from __future__ import annotations

from contextlib import AbstractContextManager
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path, PurePosixPath
import stat
import struct
from typing import BinaryIO

from release_manifest_signing import _open_release_output_parent
from sorafs_sdk_artifact_index import _identity
from sorafs_python_consumer_artifact import ArtifactError, _require
from sorafs_python_runtime_inputs import MAGIC, MAX_BUNDLE_BYTES, MAX_FILES, RuntimeManifest, parse_runtime_manifest

_CHUNK = 1024 * 1024
_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


@dataclass(frozen=True)
class FileDigest:
    """Exact produced bytes, not an original path or execution qualification token."""
    sha256: str
    size: int


class OriginalPythonRuntime(AbstractContextManager):
    """Own bounded original directory lineages and complete before/after file seals.

    File descriptors are transient and never retained per stdlib member: this is
    ordinary signed-producer toolchain custody, not proof against a malicious OS.
    Streamed file digest/identity checks and exact tree rechecks surround work;
    original configured runtime files are neither relocated nor rewritten.
    """
    def __init__(self, manifest: RuntimeManifest):
        _require(type(manifest) is RuntimeManifest, "runtime requires a parsed inventory")
        _require(parse_runtime_manifest(manifest.raw, expected_sha256=manifest.sha256) == manifest,
                 "runtime inventory projection differs from its pinned bytes")
        self.manifest = manifest
        self._parents = {}
        self._state = None
        self._entered = False
        self._active = False

    def _parent(self, path: Path) -> int:
        if path not in self._parents:
            _require(len(self._parents) < 20, "runtime parent descriptor bound")
            self._parents[path] = _open_release_output_parent(path)
        return self._parents[path][0]

    def _lineages(self) -> None:
        for path, (_fd, expected, _held) in self._parents.items():
            _observed_fd, observed, handles = _open_release_output_parent(path)
            try:
                _require(observed == expected, "runtime ancestor lost its original identity")
            finally:
                for fd in reversed(handles):
                    os.close(fd)

    @staticmethod
    def _read_file(parent: int, name: str, reference, emit=None) -> tuple:
        fd = os.open(name, _FLAGS, dir_fd=parent)
        try:
            before = os.fstat(fd)
            _require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                     and before.st_size == reference.size, "runtime file must have exact single-link regular ownership")
            digest, size = hashlib.sha256(), 0
            while True:
                data = os.read(fd, min(_CHUNK, reference.size - size + 1))
                if not data:
                    break
                size += len(data)
                _require(size <= reference.size, "runtime file exceeded declared allocation")
                digest.update(data)
                if emit is not None:
                    emit(data)
            identity = _identity(before)
            _require(size == reference.size and digest.hexdigest() == reference.sha256,
                     "runtime file bytes differ from independent pin")
            _require(_identity(os.fstat(fd)) == identity
                     == _identity(os.stat(name, dir_fd=parent, follow_symlinks=False)),
                     "runtime file changed while consumed")
            return identity
        finally:
            os.close(fd)

    def _outside(self, reference, emit=None) -> tuple:
        path = Path(reference.path)
        return self._read_file(self._parent(path.parent), path.name, reference, emit)

    def _scan(self) -> dict[str, tuple]:
        manifest = self.manifest
        state = {}
        for reference in (manifest.executable, *manifest.shared_runtime):
            state["f:" + reference.path] = self._outside(reference)
        _require(state["f:" + manifest.executable.path][2] & 0o111, "Python executable has no execution mode")
        zip_path = Path(manifest.zip_path)
        zip_parent = self._parent(zip_path.parent)
        if manifest.zip_file is None:
            try:
                os.stat(zip_path.name, dir_fd=zip_parent, follow_symlinks=False)
            except FileNotFoundError:
                state["absent:" + manifest.zip_path] = ()
            else:
                raise ArtifactError("stdlib zip was declared absent but exists")
        else:
            state["f:" + manifest.zip_path] = self._outside(manifest.zip_file)
        root = Path(manifest.stdlib_root)
        files = {value.path: value for value in manifest.stdlib_files}
        links = {value.path: value for value in manifest.stdlib_links}
        directories = set(manifest.stdlib_directories)
        observed_files, observed_links, observed_dirs = set(), set(), set()
        entries = 0

        def walk(fd: int, relative: str) -> None:
            nonlocal entries
            before = _identity(os.fstat(fd))
            state["d:" + str(root / relative)] = before
            with os.scandir(fd) as children:
                for entry in children:
                    entries += 1
                    _require(entries <= 2 * MAX_FILES + 1, "runtime tree entry bound")
                    name = str(PurePosixPath(relative) / entry.name) if relative else entry.name
                    metadata = os.stat(entry.name, dir_fd=fd, follow_symlinks=False)
                    if name == "site-packages":
                        kind = ("symlink" if stat.S_ISLNK(metadata.st_mode) else
                                "directory" if stat.S_ISDIR(metadata.st_mode) else "invalid")
                        target = os.readlink(entry.name, dir_fd=fd) if kind == "symlink" else None
                        _require((kind, target) == (manifest.site_packages_kind, manifest.site_packages_target),
                                 "excluded site-packages entry differs")
                        state["excluded:" + str(root / name)] = (*_identity(metadata), target)
                    elif stat.S_ISDIR(metadata.st_mode):
                        _require(name in directories, "unrecorded runtime directory")
                        observed_dirs.add(name)
                        child = os.open(entry.name, _FLAGS | os.O_DIRECTORY, dir_fd=fd)
                        try:
                            _require(_identity(os.fstat(child)) == _identity(metadata), "runtime directory changed before open")
                            walk(child, name)
                        finally:
                            os.close(child)
                    elif stat.S_ISLNK(metadata.st_mode):
                        _require(name in links, "unrecorded runtime symlink")
                        link = links[name]
                        target = os.readlink(entry.name, dir_fd=fd)
                        _require(target == link.target and str((root / name).resolve(strict=True)) == link.resolved,
                                 "runtime symlink differs from pinned target")
                        observed_links.add(name)
                        state["l:" + str(root / name)] = (*_identity(metadata), target)
                    else:
                        _require(name in files, "unrecorded runtime file")
                        observed_files.add(name)
                        state["f:" + str(root / name)] = self._read_file(fd, entry.name, files[name])
            _require(_identity(os.fstat(fd)) == before, "runtime directory changed during scan")

        walk(self._parent(root), "")
        _require(observed_files == set(files) and observed_links == set(links)
                 and observed_dirs == directories, "runtime tree lost declared entries")
        _require(("excluded:" + str(root / "site-packages") in state)
                 == (manifest.site_packages_kind != "absent"), "excluded site-packages presence differs")
        physical = [(value[0], value[1]) for key, value in state.items() if key.startswith("f:")]
        _require(len(set(physical)) == len(physical), "runtime regular files alias physical ownership")
        self._lineages()
        return state

    def __enter__(self):
        _require(not self._entered, "runtime owner may only be entered once")
        self._entered = True
        try:
            self._state = self._scan()
            self._active = True
            return self
        except BaseException:
            self.close()
            raise

    def recheck(self) -> None:
        """Rehash actual originals and refuse any file/tree/absence/lineage change."""
        _require(self._active, "runtime owner is not active")
        _require(self._scan() == self._state, "runtime original identity changed")

    def write_bundle(self, destination: BinaryIO) -> FileDigest:
        """Stream exact originals into the parent's fresh unpublished binary owner."""
        self.recheck()
        digest, size = hashlib.sha256(), 0
        def emit(raw: bytes) -> None:
            nonlocal size
            _require(size + len(raw) <= MAX_BUNDLE_BYTES, "runtime bundle allocation bound")
            offset = 0
            while offset < len(raw):
                count = destination.write(raw[offset:])
                _require(type(count) is int and 0 < count <= len(raw) - offset, "runtime bundle write made invalid progress")
                offset += count
            size += len(raw)
            digest.update(raw)
        emit(MAGIC + struct.pack(">Q", len(self.manifest.raw)) + self.manifest.raw)
        root = Path(self.manifest.stdlib_root)
        for reference in self.manifest.files():
            path = Path(reference.path)
            if path.is_relative_to(root):
                # Walk each original relative ancestor without retaining a per-file descriptor set.
                relative = path.relative_to(root)
                parent = self._parent(root)
                transient = []
                try:
                    for part in relative.parts[:-1]:
                        parent = os.open(part, _FLAGS | os.O_DIRECTORY, dir_fd=parent)
                        transient.append(parent)
                    identity = self._read_file(parent, path.name, reference, emit)
                finally:
                    for fd in reversed(transient):
                        os.close(fd)
            else:
                identity = self._outside(reference, emit)
            _require(identity == self._state["f:" + reference.path], "streamed member lost its original identity")
        self.recheck()
        return FileDigest(digest.hexdigest(), size)

    def close(self) -> None:
        """Release only this owner's bounded directory handles, including on failure."""
        self._active = False
        for _fd, _lineage, handles in self._parents.values():
            for descriptor in reversed(handles):
                os.close(descriptor)
        self._parents.clear()

    def __exit__(self, exc_type, exc_value, traceback):
        try:
            if exc_type is None:
                self.recheck()
        finally:
            self.close()
        return False
