"""Keep the canonical workspace census originals alive during release publication.

This internal owner requires Python 3.11+ and a canonical existing source root.
The caller supplies the census and hashes it with compute_workspace_source_manifest;
this module never chooses paths, defines an aggregate, repairs permissions, or
confers Cargo/dependency provenance. All generated outputs must be outside source.
"""

from __future__ import annotations

from contextlib import contextmanager
import importlib.util
import os
from pathlib import Path
import stat
from typing import Iterable


class SourceCustodyError(RuntimeError):
    """An original source member or its retained namespace changed."""


def _stamp(info: os.stat_result) -> tuple[int, ...]:
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _directory_identity(info: os.stat_result) -> tuple[int, ...]:
    return info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid


class _PosixOriginal:
    def __init__(self, path: Path, *, ancestor: bool = False):
        self.path = path
        self.descriptor = -1
        self.ancestor = ancestor
        before = path.lstat()
        if stat.S_ISDIR(before.st_mode):
            flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
            self.kind = "directory"
        elif stat.S_ISREG(before.st_mode):
            flags = os.O_RDONLY | os.O_NOFOLLOW
            self.kind = "file"
            if before.st_nlink != 1:
                raise SourceCustodyError("source regular input has hard-link aliases")
        elif stat.S_ISLNK(before.st_mode) and not ancestor:
            self.kind = "link"
            if before.st_nlink != 1:
                raise SourceCustodyError("source symlink has hard-link aliases")
            if hasattr(os, "O_PATH"):
                flags = os.O_PATH | os.O_NOFOLLOW
            elif hasattr(os, "O_SYMLINK"):
                flags = os.O_RDONLY | os.O_SYMLINK
            else:
                raise SourceCustodyError("host cannot retain a source symlink original")
        else:
            raise SourceCustodyError("source input is indirect or has an unsupported kind")
        # A regular-file name can become a FIFO after lstat. Open without
        # blocking, then reject its original type/identity before reading.
        descriptor = os.open(path, flags | os.O_NONBLOCK | getattr(os, "O_CLOEXEC", 0))
        try:
            opened = os.fstat(descriptor)
            if _stamp(before) != _stamp(opened):
                raise SourceCustodyError("source input changed before retention")
            if ancestor and self.kind != "directory":
                raise SourceCustodyError("source ancestry is not a real directory")
            self.before = _stamp(opened)
            self.identity = opened.st_dev, opened.st_ino
            self.descriptor = descriptor
            self.target = self._link_target() if self.kind == "link" else None
            self.revalidate()
        except BaseException:
            os.close(descriptor)
            self.descriptor = -1
            raise

    def _link_target(self) -> bytes:
        if hasattr(os, "O_PATH"):
            return os.fsencode(os.readlink("", dir_fd=self.descriptor))
        # Darwin's O_SYMLINK retains the original; the held and named snapshots
        # surround this read, and the enclosing keeper retains its real parents.
        return os.fsencode(os.readlink(self.path))

    def revalidate(self) -> None:
        if self.descriptor < 0:
            raise SourceCustodyError("source original is already closed")
        held, named = os.fstat(self.descriptor), self.path.lstat()
        if self.ancestor:
            expected = self.before[:5]
            unchanged = _directory_identity(held) == expected == _directory_identity(named)
        else:
            unchanged = _stamp(held) == self.before == _stamp(named)
        if not unchanged:
            raise SourceCustodyError("source original or selected name changed")
        if self.kind == "link" and self._link_target() != self.target:
            raise SourceCustodyError("original source symlink changed")

    def duplicate_reader(self) -> int:
        if self.kind != "file":
            raise SourceCustodyError("source input is not a regular file")
        self.revalidate()
        descriptor = os.dup(self.descriptor)
        try:
            os.lseek(descriptor, 0, os.SEEK_SET)
        except BaseException:
            os.close(descriptor)
            raise
        return descriptor

    def close(self) -> None:
        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


class SourceKeeper:
    """Retain one supplied canonical census until its publication acknowledgement.

    Windows additionally latches every native subtree and parent change. The
    parent latch is conservative: an unrelated sibling change also refuses the
    session. Use a dedicated source parent and keep outputs outside this tree.
    POSIX retains every original and checks its complete change-time snapshot.
    This owner is synchronous; overlapping readers are refused.
    """

    def __init__(self, root: Path, *, maximum_members: int = 1_000_000, native_module=None):
        self.root = Path(root)
        if not self.root.is_absolute() or Path(os.path.abspath(root)) != self.root:
            raise SourceCustodyError("source root must be absolute and normalized")
        self.maximum_members = maximum_members
        self.originals = {}
        self.members = {}
        self.absent = set()
        self.watches = []
        self.closed = False
        self.reading = False
        self.failed = False
        self.captured = False
        self._original_type = _PosixOriginal
        self._native = None
        if os.name == "nt":
            if native_module is None:
                path = Path(__file__).with_name("workspace_source_keeper_windows.py")
                spec = importlib.util.spec_from_file_location("_iroha_source_keeper_windows", path)
                if spec is None or spec.loader is None:
                    raise SourceCustodyError("native source keeper is unavailable")
                module = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(module)
            else:
                module = native_module
            self._native = module
            self._original_type = module.WindowsOriginal
        try:
            # Retain every ancestor without following a directory symlink. Only
            # the source subtree's timestamps are frozen; unrelated /tmp activity
            # must not invalidate POSIX ancestors.
            for path in reversed((self.root, *self.root.parents)):
                if self._retain(path, ancestor=path != self.root).kind != "directory":
                    raise SourceCustodyError("source ancestry is not a real directory")
            if self._native is not None:
                self._native.require_local_ntfs(self.root, self.originals[self.root])
                # These handles are never reset. The parent covers root-only ACL
                # or metadata changes, which a subtree watch does not report.
                self.watches.append(self._native.ChangeLatch(self.root, recursive=True))
                self.watches.append(self._native.ChangeLatch(self.root.parent, recursive=False))
            self.revalidate()
        except BaseException:
            self.close()
            raise

    def _retain(self, path: Path, *, ancestor: bool = False):
        if path in self.originals:
            return self.originals[path]
        if len(self.originals) >= self.maximum_members * 2 + len(self.root.parents) + 1:
            raise SourceCustodyError("source original ancestry exceeds its bounded inventory")
        original = self._original_type(path, ancestor=ancestor)
        self.originals[path] = original
        return original

    def _path(self, relative: str) -> Path:
        path = Path(relative)
        if (path.is_absolute() or not path.parts or ".." in path.parts
                or "." in path.parts or "\x00" in relative):
            raise SourceCustodyError("source member escapes its retained root")
        selected = self.root / path
        if selected == self.root or not selected.is_relative_to(self.root):
            raise SourceCustodyError("source member escapes its retained root")
        return selected

    def capture(self, paths: Iterable[str]) -> None:
        """Retain the supplied path set; this does not choose or hash a census."""
        if self.closed or self.failed or self.captured:
            raise SourceCustodyError("source census is closed, refused or already captured")
        self.captured = True
        identities = set()
        try:
            selected = set()
            for relative in paths:
                selected.add(relative)
                if len(selected) > self.maximum_members:
                    raise SourceCustodyError("source census exceeds its member bound")
            for relative in sorted(selected, key=os.fsencode):
                path = self._path(relative)
                missing_parent = False
                for parent in reversed(path.parents):
                    if parent.is_relative_to(self.root):
                        try:
                            if self._retain(parent).kind != "directory":
                                raise SourceCustodyError("source parent is not a real directory")
                        except FileNotFoundError:
                            self.absent.add(parent)
                            missing_parent = True
                            break
                if missing_parent:
                    self.absent.add(path)
                    continue
                try:
                    original = self._retain(path)
                except FileNotFoundError:
                    self.absent.add(path)
                    continue
                if original.kind != "directory":
                    if original.identity in identities:
                        raise SourceCustodyError("source census aliases one original input")
                    identities.add(original.identity)
                self.members[path] = original
            self.revalidate()
        except BaseException:
            self.failed = True
            raise

    def contains(self, path: Path) -> bool:
        return Path(path) in self.members

    @contextmanager
    def reader(self, path: Path):
        """Read a duplicate of the held regular original, never a fresh name."""
        if self.closed or self.failed or self.reading:
            raise SourceCustodyError("source keeper cannot enter this read")
        original = self.members.get(Path(path))
        if original is None:
            raise SourceCustodyError("source read was not in the retained census")
        self.reading = True
        try:
            descriptor = original.duplicate_reader()
            with os.fdopen(descriptor, "rb") as stream:
                yield stream
            original.revalidate()
            self._check_latches()
        except BaseException:
            self.failed = True
            raise
        finally:
            self.reading = False

    def readlink(self, path: Path) -> str:
        """Return the original link spelling used by the existing canonical owner."""
        original = self.members.get(Path(path))
        if self.closed or self.failed or original is None or original.kind != "link":
            raise SourceCustodyError("source link was not in the retained census")
        try:
            original.revalidate()
            self._check_latches()
            return os.fsdecode(original.target)
        except BaseException:
            self.failed = True
            raise

    def _check_latches(self) -> None:
        for watch in self.watches:
            watch.revalidate()

    def revalidate(self) -> None:
        """Reject any lost original or lifetime change; a refusal remains latched."""
        if self.closed or self.failed:
            raise SourceCustodyError("source keeper is closed or permanently refused")
        try:
            self._check_latches()
            for original in self.originals.values():
                original.revalidate()
            for path in self.absent:
                if os.path.lexists(path):
                    raise SourceCustodyError("an absent source member appeared")
            self._check_latches()
        except BaseException:
            self.failed = True
            raise

    def close(self) -> None:
        if self.closed:
            return
        self.closed = True
        for watch in reversed(self.watches):
            watch.close()
        for original in reversed(tuple(self.originals.values())):
            original.close()

    def __enter__(self):
        return self

    def __exit__(self, kind, _error, _traceback):
        try:
            if kind is None:
                self.revalidate()
        finally:
            self.close()
