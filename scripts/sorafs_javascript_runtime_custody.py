"""Retain original Node runtime image, alias and absence inputs.

This physical owner complements the pure Node runtime graph. It opens exact
original images and observes literal aliases, but does not launch Node or
attest dyld's selected libraries.
"""
from __future__ import annotations

import os
from pathlib import Path
import stat

from release_manifest_signing import _open_release_output_parent
from sorafs_javascript_input_files import (
    HeldInputFile, cleanup_preserving, drain, record_cleanup, seal,
)
from sorafs_javascript_runtime_graph import RuntimeInputError, absolute_path, require, text_path
from sorafs_javascript_runtime_inputs import (
    MAX_ALIASES, MAX_CANDIDATES, MAX_IMAGES, NodeRuntimeManifest,
    parse_node_runtime_bundle,
)

MAX_RUNTIME_HELD_DESCRIPTORS = 2048

# TODO: Join this complete physical input set to the fixed child and retain it
# through process EOF/exit. Alias resolution and dyld's actual mapped-image
# selection still need the separate process observation.


class _HeldRuntimeLeaf:
    """Shared one-shot no-follow parent owner for exact leaf input facts."""

    def __init__(self, path: Path, expected_target: str | None):
        require(type(path) is type(Path()), "runtime leaf path type differs")
        self.path = Path(absolute_path(str(path)))
        self._target = expected_target
        self._parent = None
        self._seals = None
        self._leaf_seal = None
        self._failed = self._closed = self._checking = False
        try:
            self._parent = _open_release_output_parent(self.path.parent)
            self._seals = tuple(seal(os.fstat(fd)) for fd in self._parent[2])
            require(all(stat.S_ISDIR(row[2]) and row[3] in (0, os.getuid())
                        and not stat.S_IMODE(row[2]) & 0o022 for row in self._seals),
                    "runtime leaf parent policy differs")
            self.recheck()
        except BaseException as error:
            self._failed = True
            cleanup_preserving(error, self)
            raise

    def _lineage(self) -> None:
        require(self._parent is not None and self._seals is not None,
                "runtime leaf parent is unavailable")
        _fd, observed, descriptors = _open_release_output_parent(self.path.parent)
        primary = None
        try:
            require(observed == self._parent[1]
                    and tuple(seal(os.fstat(fd)) for fd in self._parent[2]) == self._seals
                    and tuple(seal(os.fstat(fd)) for fd in descriptors) == self._seals,
                    "runtime leaf parent lineage changed")
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

    def recheck(self) -> None:
        """Reobserve the same parent and exact absent or symlink leaf fact."""
        if self._checking:
            self._failed = True
        require(not self._closed and not self._failed and not self._checking,
                "runtime leaf owner is inactive or reentrant")
        self._checking = True
        try:
            self._lineage()
            try:
                observed = os.stat(self.path.name, dir_fd=self._parent[0],
                                   follow_symlinks=False)
            except FileNotFoundError:
                if self._target is not None:
                    raise RuntimeInputError("runtime alias leaf disappeared") from None
            except OSError as error:
                raise RuntimeInputError("runtime leaf cannot be observed") from error
            else:
                if self._target is None:
                    raise RuntimeInputError("runtime absent leaf became present")
                require(stat.S_ISLNK(observed.st_mode) and observed.st_uid in (0, os.getuid())
                        and observed.st_nlink == 1,
                        "runtime alias leaf type or owner differs")
                try:
                    actual_target = os.readlink(self.path.name, dir_fd=self._parent[0])
                except OSError as error:
                    raise RuntimeInputError("runtime alias target cannot be observed") from error
                require(actual_target == self._target,
                        "runtime alias target differs from original bytes")
                try:
                    final = os.stat(self.path.name, dir_fd=self._parent[0],
                                    follow_symlinks=False)
                except OSError as error:
                    raise RuntimeInputError("runtime alias leaf changed during observation") from error
                current_seal = seal(observed)
                require(seal(final) == current_seal,
                        "runtime alias leaf changed during observation")
                if self._leaf_seal is None:
                    self._leaf_seal = current_seal
                else:
                    require(self._leaf_seal == current_seal,
                            "runtime alias original inode changed")
            self._lineage()
            require(not self._closed and not self._failed,
                    "runtime leaf owner ended during observation")
        except BaseException:
            self._failed = True
            raise
        finally:
            self._checking = False

    def close(self) -> None:
        """Detach all held descriptors before one cleanup attempt."""
        self._closed = True
        if self._checking:
            self._failed = True
        parent, self._parent = self._parent, None
        self._seals = None
        self._leaf_seal = None
        if parent is not None:
            drain(parent[2])


class HeldRuntimeAbsentLeaf(_HeldRuntimeLeaf):
    """Reject one candidate leaf that becomes present beneath its no-follow parent.

    A path through a symlink needs a separately held and authenticated alias
    before this owner is constructed at the canonical parent. Absence alone
    never establishes dyld selection or runtime approval.
    """

    def __init__(self, path: Path):
        super().__init__(path, None)


class HeldRuntimeAlias(_HeldRuntimeLeaf):
    """Retain one exact symlink spelling and literal target without following it.

    The pure runtime namespace verifies its resolved target; the caller must
    separately own the referenced image/directory and connect both owners.
    """

    def __init__(self, path: Path, target: str):
        super().__init__(path, text_path(target))


class OriginalNodeRuntimeInputs:
    """Hold a parsed runtime's original files, aliases and explicit absent leaves.

    The independent manifest pin is an input, not approval. This owner gives no
    process, native-addon or dyld selection authority. It requires all absent
    candidate parents to be directly canonical (no symlink traversal).
    """

    def __init__(self, raw_bundle: bytes, *, expected_manifest_sha256: str):
        self._owners = []
        self._images = {}
        self._bundle = None
        self._closed = self._failed = self._checking = self._entered = False
        bundle = parse_node_runtime_bundle(raw_bundle,
                                           expected_manifest_sha256=expected_manifest_sha256)
        manifest = bundle.manifest
        absent = sorted({slot.path for edge in manifest.edges
                         for slot in edge.candidates if slot.resolved is None})
        require(len(manifest.images) <= MAX_IMAGES and len(manifest.aliases) <= MAX_ALIASES
                and len(absent) <= MAX_CANDIDATES, "runtime physical owner count differs")
        paths = [row.path for row in manifest.images]
        paths.extend(row.path for row in manifest.aliases)
        paths.extend(absent)
        handles = sum(len(Path(row.path).parts) for row in manifest.images)
        handles += sum(len(Path(row.path).parts) - 1 for row in manifest.aliases)
        handles += sum(len(Path(path).parts) - 1 for path in absent)
        # Rechecking one owner briefly opens a second parent lineage while all
        # original handles remain held; reserve that transient peak up front.
        transient = max((len(Path(path).parts) - 1 for path in paths), default=0)
        require(handles + transient <= MAX_RUNTIME_HELD_DESCRIPTORS,
                "runtime physical descriptor admission bound")
        try:
            for row in manifest.images:
                owner = self._add(HeldInputFile(Path(row.path), row.size,
                                                retain_bytes=False, mode=row.mode))
                require(owner.identity == (row.sha256, row.size),
                        "runtime physical image bytes differ from pinned originals")
                self._images[row.path] = owner
            for row in manifest.aliases:
                self._add(HeldRuntimeAlias(Path(row.path), row.target))
            for path in absent:
                self._add(HeldRuntimeAbsentLeaf(Path(path)))
            self._bundle = bundle
            self.recheck()
        except BaseException as error:
            self._failed = True
            cleanup_preserving(error, self)
            raise

    def _add(self, owner):
        try:
            self._owners.append(owner)
        except BaseException as error:
            if not any(item is owner for item in self._owners):
                cleanup_preserving(error, owner)
            raise
        return owner

    @property
    def manifest(self) -> NodeRuntimeManifest:
        """Borrow the exact parsed input claims without conferring approval."""
        require(not self._closed and not self._failed and not self._checking
                and self._bundle is not None, "runtime physical inputs are inactive")
        return self._bundle.manifest

    @property
    def executable_descriptor(self) -> int:
        """Borrow the original executable fd; the owner retains cleanup custody."""
        return self._images[self.manifest.executable].descriptor

    def recheck(self) -> None:
        """Recheck every retained original against the same pinned content graph."""
        if self._checking:
            self._failed = True
        require(not self._closed and not self._failed and not self._checking
                and self._bundle is not None, "runtime physical inputs are inactive or reentrant")
        self._checking = True
        try:
            for owner in tuple(self._owners):
                owner.recheck()
            for row in self._bundle.manifest.images:
                require(self._images[row.path].identity == (row.sha256, row.size),
                        "runtime physical image bytes changed")
            require(not self._closed and not self._failed,
                    "runtime physical input owner ended during observation")
        except BaseException:
            self._failed = True
            raise
        finally:
            self._checking = False

    def close(self) -> None:
        """Detach all original owners and attempt each cleanup once."""
        self._closed = True
        if self._checking:
            self._failed = True
        owners, self._owners = tuple(self._owners), []
        self._images = {}
        self._bundle = None
        errors = []
        for owner in reversed(owners):
            try:
                owner.close()
            except BaseException as error:
                errors.append(error)
        if errors:
            failure = RuntimeInputError("runtime physical input cleanup failed")
            failure.cleanup_errors = tuple(errors)
            raise failure from errors[0]

    def __enter__(self):
        """Retain the already-acquired original inputs for one execution scope."""
        require(not self._entered, "runtime physical input scope is one-shot")
        try:
            self.recheck()
        except BaseException as error:
            # A failed __enter__ has no matching __exit__; release the already
            # acquired original FDs without hiding the exact refusal.
            cleanup_preserving(error, self)
            raise
        self._entered = True
        return self

    def __exit__(self, kind, value, traceback):
        """Recheck on clean exit, then detach every original owner."""
        if kind is not None:
            cleanup_preserving(value, self)
            return False
        try:
            self.recheck()
        except BaseException as error:
            cleanup_preserving(error, self)
            raise
        self.close()
        return False
