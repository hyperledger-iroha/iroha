"""Bounded reads over a complete immutable, owner-only evidence inventory.

Hash every declared file once using bounded buffers, then retain metadata and
bindings rather than capture contents. Empty and large opaque logs remain in
the inventory. Protocol consumers independently enforce nonempty record types.
No path follows a symbolic link and no method mutates the evidence tree.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat

import private_settlement_session_control as control

MAX_RECORD_BYTES = control.MAX_FRAME_BYTES + 4
MAX_ENTRIES = 1_000_000
MAX_DEPTH = 64
HASH_BUFFER_BYTES = 256 * 1024


def generic_reference(value):
    """Validate a canonical physical-file reference, including an empty log."""
    control.exact(value, {'path', 'sha256', 'bytes'}, 'physical evidence reference')
    control.reference({'path': value['path'], 'sha256': value['sha256'], 'bytes': 1})
    control.unsigned(value['bytes'])
    return value


def metadata(info):
    """Bind object identity, type, ownership, links, size and change times."""
    return tuple(getattr(info, field) for field in ('st_dev', 'st_ino', 'st_mode', 'st_uid',
        'st_gid', 'st_nlink', 'st_size', 'st_mtime_ns', 'st_ctime_ns'))


class RetainedRecordProvider:
    """Hold one root descriptor and read a fixed inventory with bounded memory.

``relative_roots`` enumerates the complete physical record namespaces owned by
the registered collector (for example sessions and benchmark attempt roots).
It may also include individual foundation files. It cannot overlap or include
symlinks. The root and all declared objects must belong exclusively to the
current user. Call close only when every reducer has finished using the owner.
"""

    def __init__(self, root, relative_roots):
        self.path, self.fd, self.closed = Path(root), -1, False
        self.files, self.directories, self.references = {}, {}, {}
        control.require(self.path.is_absolute() and self.path.resolve(strict=True) == self.path,
                        'evidence root is not its canonical absolute path')
        control.require(type(relative_roots) in (list, tuple) and bool(relative_roots)
                        and len(relative_roots) <= MAX_ENTRIES, 'evidence namespaces are missing or unbounded')
        self.roots = tuple(sorted(relative_roots))
        for path in self.roots:
            generic_reference({'path': path, 'sha256': '1'*64, 'bytes': 0})
        names = set(self.roots)
        control.require(len(names) == len(self.roots)
                        and all(str(parent) not in names for path in self.roots
                                for parent in Path(path).parents if str(parent) != '.'),
                        'evidence namespaces repeat or overlap')
        try:
            self.fd = os.open(self.path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
            root_info = os.fstat(self.fd); self._private(root_info, directory=True)
            self.root_metadata = metadata(root_info)
            self._root_stable()
            self._scan(initial=True)
        except BaseException:
            self.close()
            raise

    def _private(self, info, *, directory):
        control.require((stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode))
                        and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) & 0o077 == 0
                        and (directory or info.st_nlink == 1),
                        'evidence is not an exclusive owned regular file or directory')

    def _root_stable(self):
        control.require(not self.closed and self.fd >= 0
                        and metadata(os.fstat(self.fd)) == self.root_metadata
                        == metadata(self.path.lstat()), 'evidence root changed or owner is closed')

    def _open_parent(self, path, *, initial=False):
        parts = Path(path).parts
        control.require(len(parts) <= MAX_DEPTH, 'evidence path exceeds the depth bound')
        current = os.dup(self.fd); prefix = []
        try:
            for part in parts[:-1]:
                child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=current)
                try:
                    info = os.fstat(child); self._private(info, directory=True)
                    control.require(metadata(info) == metadata(os.stat(part, dir_fd=current, follow_symlinks=False)),
                                    'evidence ancestor changed during open')
                    prefix.append(part); name = '/'.join(prefix)
                    if initial and name not in self.directories:
                        control.require(len(self.directories)+len(self.files) < MAX_ENTRIES,
                                        'evidence ancestor inventory exceeds its bound')
                        self.directories[name] = metadata(info)
                    control.require(self.directories.get(name) == metadata(info), 'evidence ancestor changed')
                except BaseException:
                    os.close(child); raise
                os.close(current); current = child
            return current, parts[-1]
        except BaseException:
            os.close(current); raise

    def _file(self, parent, leaf, path, *, retain, output=None):
        descriptor = os.open(leaf, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=parent)
        try:
            before = os.fstat(descriptor); self._private(before, directory=False)
            control.require(metadata(before) == metadata(os.stat(leaf, dir_fd=parent, follow_symlinks=False)),
                            'evidence file was replaced while opening')
            control.unsigned(before.st_size)
            control.require(not retain or before.st_size <= MAX_RECORD_BYTES,
                            'requested record exceeds the bounded read size')
            digest = hashlib.sha256(); chunks = [] if retain else None; count = 0
            while count < before.st_size:
                raw = os.read(descriptor, min(HASH_BUFFER_BYTES, before.st_size-count))
                control.require(bool(raw), 'evidence file became shorter while reading')
                digest.update(raw); count += len(raw)
                if retain: chunks.append(raw)
                if output is not None: output.write(raw)
            control.require(os.read(descriptor, 1) == b''
                            and metadata(before) == metadata(os.fstat(descriptor))
                            == metadata(os.stat(leaf, dir_fd=parent, follow_symlinks=False)),
                            'evidence file changed during its bounded read')
            return metadata(before), {'path': path, 'sha256': digest.hexdigest(), 'bytes': count}, (b''.join(chunks) if retain else None)
        finally:
            os.close(descriptor)

    def _scan(self, *, initial):
        self._root_stable()
        found_files, found_directories = set(), set()

        def visit(parent, leaf, path, depth):
            control.require(depth <= MAX_DEPTH and len(found_files)+len(found_directories) < MAX_ENTRIES,
                            'evidence inventory exceeds its entry/depth bound')
            info = os.stat(leaf, dir_fd=parent, follow_symlinks=False)
            if stat.S_ISDIR(info.st_mode):
                self._private(info, directory=True)
                child = os.open(leaf, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=parent)
                try:
                    original = metadata(info)
                    control.require(metadata(os.fstat(child)) == original, 'evidence directory changed during open')
                    if initial:
                        control.require(path in self.directories or len(self.directories)+len(self.files) < MAX_ENTRIES,
                                        'evidence directory inventory exceeds its bound')
                        self.directories[path] = original
                    control.require(self.directories.get(path) == original, 'evidence directory changed')
                    found_directories.add(path)
                    with os.scandir(child) as entries:
                        names = sorted(entry.name for entry in entries)
                    for name in names:
                        child_path = path+'/'+name
                        generic_reference({'path': child_path, 'sha256': '1'*64, 'bytes': 0})
                        visit(child, name, child_path, depth+1)
                    control.require(metadata(os.fstat(child)) == original
                                    == metadata(os.stat(leaf, dir_fd=parent, follow_symlinks=False)),
                                    'evidence directory changed during inventory')
                finally:
                    os.close(child)
            else:
                self._private(info, directory=False)
                if initial:
                    control.require(len(self.directories)+len(self.files) < MAX_ENTRIES,
                                    'evidence file inventory exceeds its bound')
                    attributes, reference, _ = self._file(parent, leaf, path, retain=False)
                    self.files[path], self.references[path] = attributes, reference
                control.require(self.files.get(path) == metadata(info), 'evidence file metadata changed')
                found_files.add(path)

        for root in self.roots:
            parent, leaf = self._open_parent(root, initial=initial)
            try:
                visit(parent, leaf, root, len(Path(root).parts))
            finally:
                os.close(parent)
        control.require(found_files == set(self.files), 'evidence files appeared or disappeared')
        self._root_stable()

    def inventory(self):
        """Recheck the immutable physical inventory without rereading file bodies."""
        self._scan(initial=False)
        return {path: dict(reference) for path, reference in self.references.items()}

    def read(self, reference):
        """Read one admitted bounded file, rehashing its complete contents."""
        generic_reference(reference); self._root_stable()
        expected = self.references.get(reference['path'])
        control.require(expected is not None and control.canonical(expected) == control.canonical(reference),
                        'requested record is outside or differs from the frozen inventory')
        parent, leaf = self._open_parent(reference['path'])
        try:
            attributes, observed, raw = self._file(parent, leaf, reference['path'], retain=True)
            control.require(attributes == self.files[reference['path']] and observed == expected,
                            'requested evidence changed from its immutable binding')
        finally:
            os.close(parent)
        # Reopen the complete named ancestry after the held-descriptor read;
        # a deep ancestor replacement need not change the root's metadata.
        parent, leaf = self._open_parent(reference['path'])
        try:
            control.require(metadata(os.stat(leaf, dir_fd=parent, follow_symlinks=False)) == attributes,
                            'record endpoint changed after its held-descriptor read')
        finally:
            os.close(parent)
        self._root_stable()
        return raw

    def copy_to(self, reference, destination):
        """Stream a bound file to an exclusive destination with the same read guards."""
        generic_reference(reference); self._root_stable()
        expected = self.references.get(reference['path'])
        control.require(expected == reference, 'archive reference differs from held inventory')
        destination = Path(destination)
        parent, leaf = self._open_parent(reference['path'])
        try:
            fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
            with os.fdopen(fd, 'wb') as output:
                attributes, observed, _ = self._file(parent, leaf, reference['path'], retain=False, output=output)
                output.flush(); os.fsync(output.fileno())
            control.require(attributes == self.files[reference['path']] and observed == expected,
                            'archive source changed from its bound identity')
        finally:
            os.close(parent)
        parent, leaf = self._open_parent(reference['path'])
        try:
            control.require(metadata(os.stat(leaf, dir_fd=parent, follow_symlinks=False)) == attributes,
                            'archive source endpoint changed after copy')
        finally:
            os.close(parent)
        self._root_stable()

    def close(self):
        """Release only the owned root descriptor, with no evidence mutation."""
        if self.fd >= 0:
            os.close(self.fd); self.fd = -1
        self.closed = True

    def __enter__(self):
        return self

    def __exit__(self, *unused):
        self.close()
