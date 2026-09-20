"""Original bounded namespace ownership for the fixed experiment's two roots."""
from __future__ import annotations

import os
from pathlib import Path
import stat

from resource_bundle import _DIRECTORY_FLAGS, _directory_owner, _identity, _root_path
from scaling_experiment_plan import RUN_KEYS


class ExperimentDirectoryError(ValueError):
    """Closed original-directory failure without runtime path material."""


def require(condition):
    if not condition: raise ExperimentDirectoryError('fixed_experiment_directory_invalid')


class ExperimentDirectories:
    """Create exact public/private roots; retain every original named ancestor.

    Per-trial native/genesis/capture descendants retain their own original
    owners. This root never scans mutable private stores or claims a live quota.
    """
    def __init__(self, evidence: Path, runtime: Path):
        self._rows, self._managed = {}, {}
        self._closed, self._failed = False, False
        self._next, self._active_capture = 0, None
        try:
            for root in (evidence, runtime): _root_path(root)
            require(evidence != runtime and evidence not in runtime.parents and runtime not in evidence.parents)
            self.evidence, self.runtime = evidence, runtime
            self._scope = (str(evidence), str(runtime))
            for root in (evidence, runtime):
                self._retain_ancestors(root.parent)
                self._create(root.parent, root.name)
            self._create(evidence, 'runs'); self._create(evidence, 'resources')
            self.validate()
            # FixedExperimentFiles owns the evidence-root control namespace,
            # including its one authorized publication temporary. This owner
            # retains root identity and exact runs/resources descendant names.
            del self._managed[evidence]
        except BaseException:
            self._failed = True
            self.close()
            raise

    def _retain(self, path, parent, name):
        require(path not in self._rows and len(self._rows) < 256)
        fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent)
        created = None
        try:
            info = os.fstat(fd); created = _directory_owner(info)
            require(stat.S_ISDIR(info.st_mode)
                    and _directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == created)
            require(not os.get_inheritable(fd))
            self._rows[path] = (fd, parent, name, created)
        except BaseException:
            if created is not None:
                try:
                    if _identity(os.fstat(fd))[:2] == created[:2]: os.close(fd)
                except OSError: pass
            raise

    def _retain_ancestors(self, path):
        parent, current = None, Path('/')
        for index, component in enumerate(path.parts):
            current = Path('/') if index == 0 else current/component
            name = '/' if index == 0 else component
            if current not in self._rows: self._retain(current, parent, name)
            parent = self._rows[current][0]

    def _create(self, parent, name):
        fd = self._rows[parent][0]
        os.mkdir(name, mode=0o700, dir_fd=fd)
        path = parent/name
        self._retain(path, fd, name)
        info = os.fstat(self._rows[path][0])
        require(info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700)
        os.fsync(fd)
        if parent in self._managed: self._managed[parent].add(name)
        self._managed[path] = set()
        return path

    def create_run(self, pair, variant):
        """Create only the fixed next slot's public/private parents and capture parent."""
        try:
            self.validate()
            require(self._active_capture is None and self._next < len(RUN_KEYS)
                    and type(pair) is int and type(variant) is str
                    and (pair, variant) == RUN_KEYS[self._next])
            name = f'pair-{pair:02}'
            for parent in (self.evidence/'runs', self.evidence/'resources', self.runtime):
                if parent/name not in self._rows: self._create(parent, name)
            self._create(self.evidence/'runs'/name, variant)
            self._create(self.runtime/name, variant)
            # These leaves are producer namespaces, owned by actual trial/file
            # owners; the outer hierarchy itself remains an exact bounded tree.
            del self._managed[self.evidence/'runs'/name/variant]
            del self._managed[self.runtime/name/variant]
            self._managed[self.evidence/'resources'/name].add(variant)
            self._active_capture = self.evidence/'resources'/name/variant
            self.validate()
        except BaseException:
            self._failed = True
            raise

    def finish_run(self):
        """Close the one original capture-creation window before the next slot."""
        try:
            self.validate()
            path = self._active_capture
            require(path is not None)
            info = os.stat(path.name, dir_fd=self._rows[path.parent][0], follow_symlinks=False)
            require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid()
                    and stat.S_IMODE(info.st_mode) == 0o700)
            self._active_capture = None
            self._next += 1
            self.validate()
        except BaseException:
            self._failed = True
            raise

    def validate(self):
        """Check original edges and exact outer names without opening descendants."""
        try:
            require(not self._closed and not self._failed
                    and (str(self.evidence), str(self.runtime)) == self._scope)
            for fd, parent, name, identity in self._rows.values():
                require(not os.get_inheritable(fd) and _directory_owner(os.fstat(fd)) == identity
                        and _directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity)
            before = tuple((path, _identity(os.fstat(self._rows[path][0]))) for path in self._managed)
            for path, expected in self._managed.items():
                allowed = expected
                names = []
                with os.scandir(self._rows[path][0]) as entries:
                    for entry in entries:
                        require(len(names) < len(allowed))
                        names.append(entry.name)
                actual = set(names)
                if self._active_capture is not None and self._active_capture.parent == path:
                    require(expected - {self._active_capture.name} <= actual <= expected)
                else: require(actual == expected)
            for fd, parent, name, identity in self._rows.values():
                require(not os.get_inheritable(fd) and _directory_owner(os.fstat(fd)) == identity
                        and _directory_owner(os.stat(name, dir_fd=parent, follow_symlinks=False)) == identity)
            require(tuple((path, _identity(os.fstat(self._rows[path][0]))) for path in self._managed) == before)
        except BaseException:
            self._failed = True
            raise

    def descriptor(self, path):
        """Borrow one exact root/parent which this owner originally retained."""
        try:
            self.validate()
            require(path in self._rows)
            return self._rows[path][0]
        except BaseException:
            self._failed = True
            raise

    def close(self):
        """Release only still-original descriptors, preserving all files."""
        if self._closed: return
        self._closed, self._failed = True, True
        for fd, _, _, identity in reversed(tuple(self._rows.values())):
            try:
                if _identity(os.fstat(fd))[:2] == identity[:2]: os.close(fd)
            except OSError: pass
        self._rows.clear(); self._managed.clear()
