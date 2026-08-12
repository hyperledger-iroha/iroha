#!/usr/bin/env python3
"""Run a reproducible Cargo build profile and emit a machine-readable report."""

from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import json
import os
import platform
import resource
import secrets
import shutil
import stat
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence


SCHEMA_VERSION = 3
INPUT_DRIFT_EXIT_CODE = 3
PROFILE_ENV_KEYS = (
    "CARGO_INCREMENTAL",
    "CARGO_PROFILE_DEV_CODEGEN_UNITS",
    "CARGO_PROFILE_DEV_DEBUG",
    "CARGO_PROFILE_DEV_INCREMENTAL",
    "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
    "CARGO_PROFILE_RELEASE_DEBUG",
    "CARGO_PROFILE_RELEASE_INCREMENTAL",
    "MACOSX_DEPLOYMENT_TARGET",
    "SOURCE_DATE_EPOCH",
)
_HOST_ENV_KEYS = (
    "PATH",
    "SYSTEMROOT",
)
_MAX_TREE_RECORDS = 250_000
_MAX_TREE_FILE_BYTES = 4 * 1024 * 1024 * 1024
_MAX_TREE_TOTAL_BYTES = 64 * 1024 * 1024 * 1024
_MIN_FREE_BYTES_AFTER_COPY = 1024 * 1024 * 1024
_MAX_TREE_DEPTH = 128
_MAX_TREE_PATH_BYTES = 4096
_STABLE_STAT_FIELDS = (
    "st_dev",
    "st_ino",
    "st_mode",
    "st_uid",
    "st_gid",
    "st_nlink",
    "st_size",
    "st_mtime_ns",
    "st_ctime_ns",
)


@dataclass(frozen=True)
class SourceFingerprint:
    """Digest and size of the non-ignored repository input tree."""

    sha256: str
    files: int
    bytes: int
    deleted: int


@dataclass(frozen=True)
class TreeFingerprint:
    """Canonical content identity for one bounded directory tree."""

    sha256: str
    records: int
    files: int
    bytes: int


@dataclass
class PrivateState:
    """Invocation-private roots and the identity used for safe cleanup."""

    root: Path
    identity: tuple[int, int]
    parent_fd: int
    root_fd: int
    name: str
    home: Path
    temporary: Path
    tools: Path
    source: Path
    cargo_home: Path
    rustup_home: Path
    closed: bool = False


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description=(
            "Profile a locked Cargo build with a stable source/toolchain input "
            "fingerprint. The target directory must be outside the repository."
        )
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Repository root (default: inferred from this script).",
    )
    parser.add_argument(
        "--target-dir",
        type=Path,
        required=True,
        help="External Cargo target directory used for the measured build.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        required=True,
        help="JSON report path. A .jsonl message log and .stderr.log are adjacent.",
    )
    parser.add_argument(
        "--cargo-home",
        type=Path,
        required=True,
        help="Canonical external, caller-private Cargo cache root.",
    )
    parser.add_argument(
        "--rustup-home",
        type=Path,
        required=True,
        help="Canonical external, caller-private Rustup toolchain root.",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        default=1,
        help="Cargo build jobs (default: 1 for comparable measurements).",
    )
    parser.add_argument(
        "--label",
        default="cargo-build-profile",
        help="Stable caller-supplied label stored in the report.",
    )
    parser.add_argument(
        "--reuse-target",
        action="store_true",
        help="Allow a non-empty target directory for an explicit warm build.",
    )
    parser.add_argument(
        "cargo_args",
        nargs=argparse.REMAINDER,
        help="Cargo command after `--` (default: `build --workspace`).",
    )
    args = parser.parse_args(argv)
    if args.jobs <= 0:
        parser.error("--jobs must be greater than zero")
    return args


def sha256_bytes(payload: bytes) -> str:
    """Return a lowercase SHA-256 digest."""
    return hashlib.sha256(payload).hexdigest()


def canonical_json_bytes(value: Any) -> bytes:
    """Serialize a value into a stable compact JSON representation."""
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def command_output(
    command: Sequence[str], cwd: Path, environment: dict[str, str]
) -> str:
    """Run a read-only identity command and return normalized stdout."""
    return subprocess.check_output(
        command, cwd=cwd, env=environment, text=True
    ).strip()


def resolve_inside(path: Path, parent: Path) -> bool:
    """Return whether `path` resolves inside `parent`."""
    try:
        path.resolve().relative_to(parent.resolve())
    except ValueError:
        return False
    return True


def paths_overlap(left: Path, right: Path) -> bool:
    """Return whether either resolved path contains the other."""

    return resolve_inside(left, right) or resolve_inside(right, left)


def _stat_unchanged(before: os.stat_result, after: os.stat_result) -> bool:
    return all(
        getattr(before, field) == getattr(after, field)
        for field in _STABLE_STAT_FIELDS
    )


def _bounded_relative_path(relative: str) -> None:
    candidate = Path(relative)
    if (
        not relative
        or candidate.is_absolute()
        or ".." in candidate.parts
        or len(candidate.parts) > _MAX_TREE_DEPTH
        or len(relative.encode("utf-8", "surrogateescape")) > _MAX_TREE_PATH_BYTES
    ):
        raise ValueError(f"unsafe or over-limit snapshot path: {relative!r}")


_DIRECTORY_FLAGS = (
    os.O_RDONLY
    | getattr(os, "O_DIRECTORY", 0)
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)


def _identity(metadata: os.stat_result) -> tuple[int, int]:
    return metadata.st_dev, metadata.st_ino


def _path_components(path: Path) -> tuple[Path, tuple[str, ...]]:
    absolute = path.absolute()
    if not absolute.is_absolute() or not absolute.anchor:
        raise ValueError(f"path must be absolute: {path}")
    components = tuple(absolute.parts[1:])
    if any(component in ("", ".", "..") for component in components):
        raise ValueError(f"path is not lexically canonical: {path}")
    return Path(absolute.anchor), components


def _open_directory_anchored(path: Path, *, create: bool = False) -> int:
    """Open every directory component relative to its already-open parent."""

    anchor, components = _path_components(path)
    descriptor = os.open(anchor, _DIRECTORY_FLAGS)
    try:
        for component in components:
            try:
                child = os.open(component, _DIRECTORY_FLAGS, dir_fd=descriptor)
            except FileNotFoundError:
                if not create:
                    raise
                os.mkdir(component, 0o700, dir_fd=descriptor)
                created = os.stat(component, dir_fd=descriptor, follow_symlinks=False)
                child = os.open(component, _DIRECTORY_FLAGS, dir_fd=descriptor)
                if _identity(os.fstat(child)) != _identity(created):
                    os.close(child)
                    raise ValueError(
                        f"directory was replaced while created: {path}"
                    )
            os.close(descriptor)
            descriptor = child
        metadata = os.fstat(descriptor)
        if not stat.S_ISDIR(metadata.st_mode):
            raise ValueError(f"path is not a real directory: {path}")
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _open_parent_anchored(path: Path, *, create: bool = False) -> tuple[int, str]:
    absolute = path.absolute()
    if not absolute.name:
        raise ValueError(f"path must name a non-root entry: {path}")
    return _open_directory_anchored(absolute.parent, create=create), absolute.name


def _directory_identity(path: Path) -> tuple[int, int]:
    descriptor = _open_directory_anchored(path)
    try:
        return _identity(os.fstat(descriptor))
    finally:
        os.close(descriptor)


def _directory_names_fd(descriptor: int) -> tuple[str, ...]:
    return tuple(sorted(os.listdir(descriptor)))


def _lstat_at(parent_fd: int, name: str) -> os.stat_result:
    return os.stat(name, dir_fd=parent_fd, follow_symlinks=False)


def _open_child_directory_at(
    parent_fd: int,
    name: str,
    before: os.stat_result | None = None,
    *,
    display: Path | str,
) -> int:
    descriptor = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent_fd)
    opened = os.fstat(descriptor)
    if not stat.S_ISDIR(opened.st_mode) or (
        before is not None and not _stat_unchanged(before, opened)
    ):
        os.close(descriptor)
        raise ValueError(f"snapshot directory changed while opened: {display}")
    return descriptor


def _open_relative_parent_fd(root_fd: int, relative: str) -> tuple[int, str]:
    _bounded_relative_path(relative)
    parts = Path(relative).parts
    descriptor = os.dup(root_fd)
    try:
        for component in parts[:-1]:
            child = _open_child_directory_at(
                descriptor, component, display=relative
            )
            os.close(descriptor)
            descriptor = child
        return descriptor, parts[-1]
    except BaseException:
        os.close(descriptor)
        raise


def _ensure_relative_parent_fd(root_fd: int, relative: str) -> tuple[int, str]:
    _bounded_relative_path(relative)
    parts = Path(relative).parts
    descriptor = os.dup(root_fd)
    try:
        for component in parts[:-1]:
            try:
                child = _open_child_directory_at(
                    descriptor, component, display=relative
                )
            except FileNotFoundError:
                os.mkdir(component, 0o700, dir_fd=descriptor)
                created = _lstat_at(descriptor, component)
                child = _open_child_directory_at(
                    descriptor, component, created, display=relative
                )
            os.close(descriptor)
            descriptor = child
        return descriptor, parts[-1]
    except BaseException:
        os.close(descriptor)
        raise


def _read_regular_stable_at(
    parent_fd: int,
    name: str,
    before: os.stat_result,
    display: Path | str,
) -> bytes:
    """Read a regular file through a held parent directory descriptor."""

    if before.st_size > _MAX_TREE_FILE_BYTES:
        raise ValueError(f"snapshot file exceeds its size limit: {display}")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(name, flags, dir_fd=parent_fd)
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or not _stat_unchanged(before, opened):
            raise ValueError(f"snapshot file changed while opened: {display}")
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > _MAX_TREE_FILE_BYTES:
                raise ValueError(f"snapshot file exceeds its size limit: {display}")
            chunks.append(chunk)
        after = os.fstat(descriptor)
        if total != opened.st_size or not _stat_unchanged(opened, after):
            raise ValueError(f"snapshot file changed while read: {display}")
        if not _stat_unchanged(after, _lstat_at(parent_fd, name)):
            raise ValueError(f"snapshot file was replaced while read: {display}")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def _read_regular_stable(path: Path, before: os.stat_result) -> bytes:
    parent_fd, name = _open_parent_anchored(path)
    try:
        return _read_regular_stable_at(parent_fd, name, before, path)
    finally:
        os.close(parent_fd)


def _read_symlink_stable_at(
    parent_fd: int,
    name: str,
    before: os.stat_result,
    display: Path | str,
) -> str:
    target = os.readlink(name, dir_fd=parent_fd)
    after = _lstat_at(parent_fd, name)
    if (
        not stat.S_ISLNK(after.st_mode)
        or not _stat_unchanged(before, after)
        or os.readlink(name, dir_fd=parent_fd) != target
    ):
        raise ValueError(f"source symlink changed while read: {display}")
    return target


def _relative_target_components(
    relative: str, target: str, display: Path | str
) -> tuple[str, ...]:
    if Path(target).is_absolute():
        raise ValueError(f"snapshot symlink has an absolute target: {display}")
    components = list(Path(relative).parent.parts)
    for component in Path(target).parts:
        if component in ("", "."):
            continue
        if component == "..":
            if not components:
                raise ValueError(f"snapshot symlink escapes its input root: {display}")
            components.pop()
        else:
            components.append(component)
    return tuple(components)


def _internal_symlink_target(relative: str, target: str, display: Path | str) -> str:
    _relative_target_components(relative, target, display)
    return target


def _verify_symlink_target_fd(
    root_fd: int,
    relative: str,
    target: str,
    display: Path | str,
) -> None:
    """Resolve a symlink below a held root without following pathname ancestors."""

    pending = list(_relative_target_components(relative, target, display))
    descriptor = os.dup(root_fd)
    resolved: list[str] = []
    symlinks = 0
    try:
        while pending:
            component = pending.pop(0)
            metadata = _lstat_at(descriptor, component)
            if stat.S_ISLNK(metadata.st_mode):
                symlinks += 1
                if symlinks > 40:
                    raise ValueError(
                        f"snapshot symlink chain is too deep: {display}"
                    )
                nested_target = _read_symlink_stable_at(
                    descriptor, component, metadata, display
                )
                nested_relative = "/".join((*resolved, component))
                pending = list(
                    _relative_target_components(
                        nested_relative, nested_target, display
                    )
                ) + pending
                os.close(descriptor)
                descriptor = os.dup(root_fd)
                resolved.clear()
                continue
            if pending:
                if not stat.S_ISDIR(metadata.st_mode):
                    raise ValueError(
                        f"snapshot symlink target is unavailable: {display}"
                    )
                child = _open_child_directory_at(
                    descriptor, component, metadata, display=display
                )
                os.close(descriptor)
                descriptor = child
                resolved.append(component)
            elif not (
                stat.S_ISDIR(metadata.st_mode) or stat.S_ISREG(metadata.st_mode)
            ):
                raise ValueError(
                    f"snapshot symlink target has an unsupported type: {display}"
                )
    except (FileNotFoundError, NotADirectoryError) as error:
        raise ValueError(
            f"snapshot symlink target is unavailable: {display}"
        ) from error
    finally:
        os.close(descriptor)


def _safe_symlink_target_at(
    root_fd: int,
    parent_fd: int,
    name: str,
    before: os.stat_result,
    relative: str,
    display: Path | str,
) -> str:
    target = _read_symlink_stable_at(parent_fd, name, before, display)
    _internal_symlink_target(relative, target, display)
    _verify_symlink_target_fd(root_fd, relative, target, display)
    return target


def _safe_symlink_target(path: Path, before: os.stat_result, root: Path) -> str:
    relative = str(path.relative_to(root))
    root_fd = _open_directory_anchored(root)
    parent_fd, name = _open_parent_anchored(path)
    try:
        return _safe_symlink_target_at(
            root_fd, parent_fd, name, before, relative, path
        )
    finally:
        os.close(parent_fd)
        os.close(root_fd)


def _read_symlink_stable(path: Path, before: os.stat_result) -> str:
    parent_fd, name = _open_parent_anchored(path)
    try:
        return _read_symlink_stable_at(parent_fd, name, before, path)
    finally:
        os.close(parent_fd)


def _path_still_names(path: Path, identity: tuple[int, int]) -> bool:
    try:
        parent_fd, name = _open_parent_anchored(path)
    except (FileNotFoundError, NotADirectoryError, OSError, ValueError):
        return False
    try:
        try:
            metadata = _lstat_at(parent_fd, name)
        except FileNotFoundError:
            return False
        return _identity(metadata) == identity
    finally:
        os.close(parent_fd)


def _cleanup_name() -> str:
    return f".iroha-profile-cleanup-{secrets.token_hex(16)}"


def _rename_with_flags_at(
    parent_fd: int, source: str, destination: str, flags: int
) -> None:
    """Rename within one directory with platform-native atomic flags."""

    library = ctypes.CDLL(None, use_errno=True)
    if sys.platform == "darwin":
        rename = library.renameatx_np
        rename.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        rename.restype = ctypes.c_int
        result = rename(
            parent_fd,
            os.fsencode(source),
            parent_fd,
            os.fsencode(destination),
            flags,
        )
    elif sys.platform.startswith("linux") and hasattr(library, "renameat2"):
        rename = library.renameat2
        rename.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        rename.restype = ctypes.c_int
        result = rename(
            parent_fd,
            os.fsencode(source),
            parent_fd,
            os.fsencode(destination),
            flags,
        )
    else:
        raise OSError(
            errno.ENOTSUP,
            "atomic flagged rename is unavailable",
        )
    if result != 0:
        error_number = ctypes.get_errno()
        raise OSError(error_number, os.strerror(error_number), destination)


def _rename_noreplace_at(parent_fd: int, source: str, destination: str) -> None:
    """Rename within one directory without replacing the destination entry."""

    # RENAME_EXCL on Darwin and RENAME_NOREPLACE on Linux.
    _rename_with_flags_at(
        parent_fd,
        source,
        destination,
        0x00000004 if sys.platform == "darwin" else 1,
    )


def _rename_swap_at(parent_fd: int, left: str, right: str) -> None:
    """Atomically exchange two entries in one directory."""

    # RENAME_SWAP on Darwin and RENAME_EXCHANGE on Linux.
    _rename_with_flags_at(
        parent_fd,
        left,
        right,
        0x00000002 if sys.platform == "darwin" else 2,
    )


def _discard_quarantined_entry_at(
    parent_fd: int,
    quarantine: str,
    identity: tuple[int, int],
    *,
    directory: bool,
) -> None:
    """Delete an owned quarantine without a check-then-delete pathname race."""

    tombstone = _cleanup_name()
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(tombstone, flags, 0o600, dir_fd=parent_fd)
    os.close(descriptor)
    tombstone_identity = _identity(_lstat_at(parent_fd, tombstone))
    swapped = False
    try:
        _rename_swap_at(parent_fd, quarantine, tombstone)
        swapped = True
        moved = _lstat_at(parent_fd, tombstone)
        if _identity(moved) != identity:
            try:
                _rename_swap_at(parent_fd, quarantine, tombstone)
                swapped = False
            except OSError as rollback:
                raise ValueError(
                    "cleanup quarantine changed during atomic removal; "
                    f"foreign entry retained as {tombstone!r} and cleanup "
                    f"tombstone retained as {quarantine!r}"
                ) from rollback
            raise ValueError(
                "cleanup quarantine changed during atomic removal; "
                "foreign entry restored"
            )
        if directory:
            os.rmdir(tombstone, dir_fd=parent_fd)
        else:
            os.unlink(tombstone, dir_fd=parent_fd)
        swapped = False
        # The original quarantine name now names only our one-link tombstone.
        if _identity(_lstat_at(parent_fd, quarantine)) != tombstone_identity:
            raise ValueError("cleanup tombstone changed identity")
        os.unlink(quarantine, dir_fd=parent_fd)
    except BaseException:
        if not swapped:
            try:
                if _identity(_lstat_at(parent_fd, tombstone)) == tombstone_identity:
                    os.unlink(tombstone, dir_fd=parent_fd)
            except (FileNotFoundError, OSError):
                pass
        raise


def _remove_directory_contents_fd(descriptor: int) -> None:
    for name in _directory_names_fd(descriptor):
        metadata = _lstat_at(descriptor, name)
        _remove_owned_entry_at(descriptor, name, _identity(metadata))


def _remove_owned_entry_at(
    parent_fd: int,
    name: str,
    identity: tuple[int, int],
) -> bool:
    """Quarantine, verify, then remove only the named inode identity."""

    try:
        current = _lstat_at(parent_fd, name)
    except FileNotFoundError:
        return False
    if _identity(current) != identity:
        return False
    if stat.S_ISDIR(current.st_mode) and not stat.S_ISLNK(current.st_mode):
        quarantine = _cleanup_name()
        os.rename(
            name,
            quarantine,
            src_dir_fd=parent_fd,
            dst_dir_fd=parent_fd,
        )
        moved = _lstat_at(parent_fd, quarantine)
        if _identity(moved) != identity:
            try:
                _rename_noreplace_at(parent_fd, quarantine, name)
            except OSError as rollback:
                raise ValueError(
                    "cleanup directory changed while quarantined; replacement "
                    f"retained as {quarantine!r} because {name!r} could not be "
                    "restored without replacing another entry"
                ) from rollback
            restored = _lstat_at(parent_fd, name)
            if _identity(restored) != _identity(moved):
                raise ValueError("cleanup directory rollback changed identity")
            raise ValueError(
                "cleanup directory changed before quarantine; replacement restored"
            )
        child = _open_child_directory_at(
            parent_fd, quarantine, moved, display=quarantine
        )
        try:
            os.fchmod(child, 0o700)
            _remove_directory_contents_fd(child)
        finally:
            os.close(child)
        _discard_quarantined_entry_at(
            parent_fd, quarantine, identity, directory=True
        )
        return True

    guard = _cleanup_name()
    os.link(
        name,
        guard,
        src_dir_fd=parent_fd,
        dst_dir_fd=parent_fd,
        follow_symlinks=False,
    )
    linked = _lstat_at(parent_fd, guard)
    if _identity(linked) != identity:
        os.unlink(guard, dir_fd=parent_fd)
        return False
    try:
        original = _lstat_at(parent_fd, name)
    except FileNotFoundError:
        original = None
    if original is None or _identity(original) != identity:
        os.unlink(guard, dir_fd=parent_fd)
        return False
    quarantine = _cleanup_name()
    os.rename(
        name,
        quarantine,
        src_dir_fd=parent_fd,
        dst_dir_fd=parent_fd,
    )
    moved = _lstat_at(parent_fd, quarantine)
    if _identity(moved) != identity:
        try:
            _rename_noreplace_at(parent_fd, quarantine, name)
        except OSError as rollback:
            if _identity(_lstat_at(parent_fd, guard)) == identity:
                os.unlink(guard, dir_fd=parent_fd)
            raise ValueError(
                "cleanup file changed while quarantined; replacement retained "
                f"as {quarantine!r} because {name!r} could not be restored "
                "without replacing another entry"
            ) from rollback
        if _identity(_lstat_at(parent_fd, name)) != _identity(moved):
            raise ValueError("cleanup file rollback changed identity")
        if _identity(_lstat_at(parent_fd, guard)) == identity:
            os.unlink(guard, dir_fd=parent_fd)
        raise ValueError(
            "cleanup file changed before quarantine; replacement restored"
        )
    if _identity(_lstat_at(parent_fd, guard)) != identity:
        raise ValueError("cleanup file guard changed identity")
    os.unlink(guard, dir_fd=parent_fd)
    _discard_quarantined_entry_at(
        parent_fd, quarantine, identity, directory=False
    )
    return True


def _remove_owned_path(path: Path, identity: tuple[int, int]) -> bool:
    parent_fd, name = _open_parent_anchored(path)
    try:
        return _remove_owned_entry_at(parent_fd, name, identity)
    finally:
        os.close(parent_fd)


def _bounded_tree_fingerprint_fd(
    root_fd: int,
    root_display: Path,
    roots: Sequence[str] | None = None,
    *,
    reject_hardlinks: bool = False,
) -> TreeFingerprint:
    digest = hashlib.sha256()
    budget = {"records": 0, "files": 0, "bytes": 0}

    def add_record(record: dict[str, Any]) -> None:
        budget["records"] += 1
        if budget["records"] > _MAX_TREE_RECORDS:
            raise ValueError("snapshot input contains too many entries")
        digest.update(canonical_json_bytes(record))
        digest.update(b"\n")

    def visit(parent_fd: int, name: str, relative: str) -> None:
        _bounded_relative_path(relative)
        before = _lstat_at(parent_fd, name)
        display = root_display / relative
        if stat.S_ISDIR(before.st_mode) and not stat.S_ISLNK(before.st_mode):
            directory = _open_child_directory_at(
                parent_fd, name, before, display=display
            )
            try:
                names = _directory_names_fd(directory)
                add_record({"kind": "directory", "path": relative})
                for child_name in names:
                    visit(directory, child_name, f"{relative}/{child_name}")
                if (
                    not _stat_unchanged(before, os.fstat(directory))
                    or names != _directory_names_fd(directory)
                ):
                    raise ValueError(f"snapshot directory changed while read: {display}")
            finally:
                os.close(directory)
            if not _stat_unchanged(before, _lstat_at(parent_fd, name)):
                raise ValueError(f"snapshot directory was replaced: {display}")
            return
        if stat.S_ISREG(before.st_mode):
            if reject_hardlinks and before.st_nlink != 1:
                raise ValueError(
                    f"snapshot input contains a hard-linked file: {display}"
                )
            payload = _read_regular_stable_at(parent_fd, name, before, display)
            budget["files"] += 1
            budget["bytes"] += len(payload)
            if budget["bytes"] > _MAX_TREE_TOTAL_BYTES:
                raise ValueError("snapshot input exceeds its total byte limit")
            add_record(
                {
                    "bytes": len(payload),
                    "executable": bool(before.st_mode & 0o111),
                    "kind": "file",
                    "path": relative,
                    "sha256": sha256_bytes(payload),
                }
            )
            return
        if stat.S_ISLNK(before.st_mode):
            add_record(
                {
                    "kind": "symlink",
                    "path": relative,
                    "target": _safe_symlink_target_at(
                        root_fd, parent_fd, name, before, relative, display
                    ),
                }
            )
            return
        raise ValueError(f"snapshot input contains a special file: {display}")

    selected = tuple(sorted(roots)) if roots is not None else _directory_names_fd(root_fd)
    for relative in selected:
        _bounded_relative_path(relative)
        parent_fd, name = _open_relative_parent_fd(root_fd, relative)
        try:
            try:
                _lstat_at(parent_fd, name)
            except FileNotFoundError:
                add_record({"kind": "missing", "path": relative})
            else:
                visit(parent_fd, name, relative)
        finally:
            os.close(parent_fd)
    return TreeFingerprint(
        sha256=digest.hexdigest(),
        records=budget["records"],
        files=budget["files"],
        bytes=budget["bytes"],
    )


def bounded_tree_fingerprint(
    root: Path,
    roots: Sequence[str] | None = None,
    *,
    expected_identity: tuple[int, int] | None = None,
    reject_hardlinks: bool = False,
) -> TreeFingerprint:
    """Hash a bounded tree without following symlinks or accepting special files."""

    root = root.absolute()
    descriptor = _open_directory_anchored(root)
    identity = _identity(os.fstat(descriptor))
    try:
        if expected_identity is not None and identity != expected_identity:
            raise ValueError(f"snapshot input root identity changed: {root}")
        result = _bounded_tree_fingerprint_fd(
            descriptor,
            root,
            roots,
            reject_hardlinks=reject_hardlinks,
        )
        if not _path_still_names(root, identity):
            raise ValueError(f"snapshot input root was replaced: {root}")
        return result
    finally:
        os.close(descriptor)


def copy_bounded_tree(
    source: Path,
    destination: Path,
    *,
    roots: Sequence[str] | None = None,
    expected_source_identity: tuple[int, int] | None = None,
    reject_source_hardlinks: bool = False,
) -> TreeFingerprint:
    """Create an inode-independent bounded tree copy and verify both endpoints."""

    source = source.absolute()
    destination = destination.absolute()
    if os.path.lexists(destination):
        raise ValueError(f"snapshot destination already exists: {destination}")
    if paths_overlap(source, destination):
        raise ValueError("snapshot source and destination must be disjoint")
    source_fd = _open_directory_anchored(source)
    source_identity = _identity(os.fstat(source_fd))
    if (
        expected_source_identity is not None
        and source_identity != expected_source_identity
    ):
        os.close(source_fd)
        raise ValueError(f"snapshot input root identity changed: {source}")
    try:
        before = _bounded_tree_fingerprint_fd(
            source_fd,
            source,
            roots,
            reject_hardlinks=reject_source_hardlinks,
        )
        destination_parent_fd, destination_name = _open_parent_anchored(destination)
    except BaseException:
        os.close(source_fd)
        raise
    try:
        filesystem = os.fstatvfs(destination_parent_fd)
    except BaseException:
        os.close(destination_parent_fd)
        os.close(source_fd)
        raise
    available = filesystem.f_bavail * filesystem.f_frsize
    if before.bytes + _MIN_FREE_BYTES_AFTER_COPY > available:
        os.close(destination_parent_fd)
        os.close(source_fd)
        raise ValueError("snapshot copy would exhaust filesystem free space")
    destination_identity: tuple[int, int] | None = None
    try:
        os.mkdir(destination_name, 0o700, dir_fd=destination_parent_fd)
        destination_metadata = _lstat_at(destination_parent_fd, destination_name)
        destination_identity = _identity(destination_metadata)
        destination_fd = _open_child_directory_at(
            destination_parent_fd,
            destination_name,
            destination_metadata,
            display=destination,
        )
    except BaseException:
        if destination_identity is not None:
            _remove_owned_entry_at(
                destination_parent_fd, destination_name, destination_identity
            )
        os.close(destination_parent_fd)
        os.close(source_fd)
        raise
    assert destination_identity is not None

    def copy_entry(
        source_parent_fd: int,
        destination_parent: int,
        name: str,
        relative: str,
    ) -> None:
        before_entry = _lstat_at(source_parent_fd, name)
        display = source / relative
        if stat.S_ISDIR(before_entry.st_mode) and not stat.S_ISLNK(
            before_entry.st_mode
        ):
            source_directory = _open_child_directory_at(
                source_parent_fd, name, before_entry, display=display
            )
            try:
                names = _directory_names_fd(source_directory)
                os.mkdir(name, 0o700, dir_fd=destination_parent)
                destination_entry_metadata = _lstat_at(destination_parent, name)
                destination_directory = _open_child_directory_at(
                    destination_parent,
                    name,
                    destination_entry_metadata,
                    display=destination / relative,
                )
                try:
                    for child_name in names:
                        copy_entry(
                            source_directory,
                            destination_directory,
                            child_name,
                            f"{relative}/{child_name}",
                        )
                finally:
                    os.close(destination_directory)
                if (
                    not _stat_unchanged(before_entry, os.fstat(source_directory))
                    or names != _directory_names_fd(source_directory)
                ):
                    raise ValueError(
                        f"snapshot directory changed while copied: {display}"
                    )
            finally:
                os.close(source_directory)
            if not _stat_unchanged(before_entry, _lstat_at(source_parent_fd, name)):
                raise ValueError(f"snapshot directory was replaced: {display}")
            return
        if stat.S_ISREG(before_entry.st_mode):
            if reject_source_hardlinks and before_entry.st_nlink != 1:
                raise ValueError(
                    f"snapshot input contains a hard-linked file: {display}"
                )
            payload = _read_regular_stable_at(
                source_parent_fd, name, before_entry, display
            )
            flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
            if hasattr(os, "O_NOFOLLOW"):
                flags |= os.O_NOFOLLOW
            descriptor = os.open(name, flags, 0o600, dir_fd=destination_parent)
            try:
                view = memoryview(payload)
                while view:
                    written = os.write(descriptor, view)
                    if written <= 0:
                        raise ValueError(
                            f"could not finish snapshot copy: {display}"
                        )
                    view = view[written:]
                os.fchmod(descriptor, 0o700 if before_entry.st_mode & 0o111 else 0o600)
                os.fsync(descriptor)
                copied = os.fstat(descriptor)
                if (
                    copied.st_nlink != 1
                    or copied.st_size != len(payload)
                    or (copied.st_dev, copied.st_ino)
                    == (before_entry.st_dev, before_entry.st_ino)
                ):
                    raise ValueError(
                        f"snapshot copy is not inode-independent: {destination / relative}"
                    )
            finally:
                os.close(descriptor)
            return
        if stat.S_ISLNK(before_entry.st_mode):
            target = _safe_symlink_target_at(
                source_fd,
                source_parent_fd,
                name,
                before_entry,
                relative,
                display,
            )
            os.symlink(target, name, dir_fd=destination_parent)
            return
        raise ValueError(f"snapshot input contains a special file: {display}")

    selected = tuple(sorted(roots)) if roots is not None else _directory_names_fd(source_fd)
    try:
        for relative in selected:
            _bounded_relative_path(relative)
            source_parent, source_name = _open_relative_parent_fd(source_fd, relative)
            destination_parent, destination_entry_name = _ensure_relative_parent_fd(
                destination_fd, relative
            )
            try:
                try:
                    _lstat_at(source_parent, source_name)
                except FileNotFoundError:
                    continue
                copy_entry(
                    source_parent,
                    destination_parent,
                    source_name,
                    relative,
                )
                if source_name != destination_entry_name:
                    raise AssertionError("snapshot copy entry names diverged")
            finally:
                os.close(source_parent)
                os.close(destination_parent)
        after = _bounded_tree_fingerprint_fd(
            source_fd,
            source,
            roots,
            reject_hardlinks=reject_source_hardlinks,
        )
        copied = _bounded_tree_fingerprint_fd(
            destination_fd,
            destination,
            roots,
            reject_hardlinks=True,
        )
        if before != after:
            raise ValueError("snapshot input changed while it was copied")
        if before != copied:
            raise ValueError("snapshot copy does not match its input")
        if not _path_still_names(source, source_identity):
            raise ValueError("snapshot input root changed while it was copied")
        if (
            _identity(_lstat_at(destination_parent_fd, destination_name))
            != destination_identity
        ):
            raise ValueError("snapshot destination was replaced while copied")
        return before
    except BaseException:
        _remove_owned_entry_at(
            destination_parent_fd, destination_name, destination_identity
        )
        raise
    finally:
        os.close(destination_fd)
        os.close(destination_parent_fd)
        os.close(source_fd)


def tree_fingerprint_json(value: TreeFingerprint) -> dict[str, Any]:
    return {
        "bytes": value.bytes,
        "files": value.files,
        "records": value.records,
        "sha256": value.sha256,
    }


def report_paths(out: Path) -> tuple[Path, Path, Path]:
    """Return the report and its two adjacent transcript paths."""

    return (
        out,
        out.with_suffix(out.suffix + ".jsonl"),
        out.with_suffix(out.suffix + ".stderr.log"),
    )


def private_state_path(out: Path) -> Path:
    """Return the invocation-private HOME/tmp root adjacent to one report."""

    return out.with_suffix(out.suffix + ".state")


def validate_private_roots(
    root: Path,
    target_dir: Path,
    out: Path,
    cargo_home: Path,
    rustup_home: Path,
) -> tuple[Path, Path]:
    """Validate explicit canonical cache/toolchain roots outside all outputs."""

    validated = []
    forbidden = (root, target_dir, private_state_path(out), *report_paths(out))
    for label, path in (("--cargo-home", cargo_home), ("--rustup-home", rustup_home)):
        if not path.is_absolute() or path != path.resolve():
            raise ValueError(f"{label} must be an absolute canonical path")
        if not path.is_dir() or path.is_symlink():
            raise ValueError(f"{label} must be a non-symlink directory")
        metadata = path.stat()
        if stat.S_IMODE(metadata.st_mode) & 0o077:
            raise ValueError(f"{label} must be caller-private (mode 0700)")
        if hasattr(os, "geteuid") and metadata.st_uid != os.geteuid():
            raise ValueError(f"{label} must be owned by the current user")
        if any(paths_overlap(path, other) for other in forbidden):
            raise ValueError(f"{label} must be external and disjoint")
        validated.append(path)
    if paths_overlap(*validated):
        raise ValueError("--cargo-home and --rustup-home must be disjoint")
    return validated[0], validated[1]


def resolve_tool(
    name: str,
    root: Path,
    search_path: str,
    environment: dict[str, str],
    forbidden_roots: Sequence[Path] = (),
) -> dict[str, str]:
    """Resolve and hash the actual executable used for one tool command."""

    found = shutil.which(name, path=search_path)
    if found is None:
        raise ValueError(f"required tool is not executable on PATH: {name}")
    discovered = Path(found).absolute()
    launcher = discovered.resolve(strict=True)
    launcher_metadata = launcher.stat()
    if (
        not stat.S_ISREG(launcher_metadata.st_mode)
        or not os.access(launcher, os.X_OK)
        or resolve_inside(launcher, root)
        or any(resolve_inside(launcher, path) for path in forbidden_roots)
    ):
        raise ValueError(f"{name} launcher must be an external regular executable")
    launcher_sha256 = sha256_bytes(
        _read_regular_stable(launcher, launcher_metadata)
    )
    executable = launcher
    if name in ("cargo", "rustc") and _is_rustup_proxy(
        discovered, launcher, search_path, launcher_sha256
    ):
        try:
            safe_cwd = Path(environment["HOME"]).resolve(strict=True)
        except (KeyError, OSError) as error:
            raise ValueError(
                "tool identity resolution requires a private HOME"
            ) from error
        if not safe_cwd.is_dir() or resolve_inside(safe_cwd, root):
            raise ValueError("tool identity HOME must be an external directory")
        selected = subprocess.check_output(
            [str(launcher), "which", name],
            cwd=safe_cwd,
            env=environment,
            text=True,
        ).strip()
        if not selected:
            raise ValueError(f"rustup returned an empty executable path for {name}")
        selected_path = Path(selected)
        if not selected_path.is_absolute():
            raise ValueError(f"rustup returned a non-absolute path for {name}")
        executable = selected_path.resolve(strict=True)
    metadata = executable.stat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or not os.access(executable, os.X_OK)
        or resolve_inside(executable, root)
        or any(resolve_inside(executable, path) for path in forbidden_roots)
    ):
        raise ValueError(f"{name} must resolve to an external regular executable")
    return {
        "discovered_path": str(discovered),
        "launcher_path": str(launcher),
        "launcher_sha256": launcher_sha256,
        "resolved_path": str(executable),
        "sha256": sha256_bytes(_read_regular_stable(executable, metadata)),
    }


def _copy_executable(
    source: Path,
    destination: Path,
    destination_parent_fd: int | None = None,
) -> tuple[str, os.stat_result]:
    """Copy one stable executable to a private, inode-independent path."""

    source_parent_fd, source_name = _open_parent_anchored(source)
    try:
        before = _lstat_at(source_parent_fd, source_name)
        if not stat.S_ISREG(before.st_mode) or not os.access(
            source_name,
            os.X_OK,
            dir_fd=source_parent_fd,
            follow_symlinks=False,
        ):
            raise ValueError(
                f"tool launcher is not an executable regular file: {source}"
            )
        payload = _read_regular_stable_at(
            source_parent_fd, source_name, before, source
        )
    finally:
        os.close(source_parent_fd)
    digest = sha256_bytes(payload)
    owns_parent_fd = destination_parent_fd is None
    if destination_parent_fd is None:
        destination_parent_fd, destination_name = _open_parent_anchored(destination)
    else:
        destination_name = destination.name
    assert destination_parent_fd is not None
    try:
        copied = _lstat_at(destination_parent_fd, destination_name)
    except FileNotFoundError:
        copied = None
    if copied is not None:
        if (
            not stat.S_ISREG(copied.st_mode)
            or not os.access(
                destination_name,
                os.X_OK,
                dir_fd=destination_parent_fd,
                follow_symlinks=False,
            )
            or sha256_bytes(
                _read_regular_stable_at(
                    destination_parent_fd,
                    destination_name,
                    copied,
                    destination,
                )
            )
            != digest
        ):
            if owns_parent_fd:
                os.close(destination_parent_fd)
            raise ValueError(f"private tool path conflicts: {destination}")
        if owns_parent_fd:
            os.close(destination_parent_fd)
        return digest, copied
    try:
        descriptor = os.open(
            destination_name,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o700,
            dir_fd=destination_parent_fd,
        )
    except BaseException:
        if owns_parent_fd:
            os.close(destination_parent_fd)
        raise
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise ValueError(f"could not finish copying tool: {source}")
            view = view[written:]
        os.fchmod(descriptor, 0o700)
        os.fsync(descriptor)
        copied = os.fstat(descriptor)
        if (
            copied.st_nlink != 1
            or (copied.st_dev, copied.st_ino) == (before.st_dev, before.st_ino)
        ):
            raise ValueError(
                f"private tool copy is not inode-independent: {destination}"
            )
    finally:
        os.close(descriptor)
        if owns_parent_fd:
            os.close(destination_parent_fd)
    return digest, copied


def _is_rustup_proxy(
    discovered: Path, launcher: Path, search_path: str, launcher_sha256: str
) -> bool:
    if launcher.name.startswith("rustup"):
        return True
    rustup_found = shutil.which("rustup", path=search_path)
    if rustup_found is None:
        return False
    rustup = Path(rustup_found).resolve(strict=True)
    metadata = rustup.stat()
    launcher_metadata = launcher.stat()
    if (metadata.st_dev, metadata.st_ino) == (
        launcher_metadata.st_dev,
        launcher_metadata.st_ino,
    ):
        return True
    return stat.S_ISREG(metadata.st_mode) and sha256_bytes(
        _read_regular_stable(rustup, metadata)
    ) == launcher_sha256


def _copy_private_tool(
    state: PrivateState, source: Path, name: str
) -> tuple[str, os.stat_result]:
    tools_metadata = _lstat_at(state.root_fd, "tools")
    tools_fd = _open_child_directory_at(
        state.root_fd, "tools", tools_metadata, display=state.tools
    )
    try:
        return _copy_executable(source, state.tools / name, tools_fd)
    finally:
        os.close(tools_fd)


def resolve_isolated_tool(
    name: str,
    root: Path,
    search_path: str,
    environment: dict[str, str],
    state: PrivateState,
    caller_rustup_home: Path,
    safe_cwd: Path,
    forbidden_roots: Sequence[Path] = (),
) -> dict[str, str]:
    """Resolve a tool, then invoke only an inode-independent private copy."""

    found = shutil.which(name, path=search_path)
    if found is None:
        raise ValueError(f"required tool is not executable on PATH: {name}")
    discovered = Path(found).absolute()
    launcher = discovered.resolve(strict=True)
    if (
        resolve_inside(launcher, root)
        or any(resolve_inside(launcher, path) for path in forbidden_roots)
    ):
        raise ValueError(f"{name} launcher must be external to profile inputs/outputs")
    launcher_sha256 = sha256_bytes(_read_regular_stable(launcher, launcher.lstat()))

    if name == "git":
        invoked = state.tools / "git"
        _copy_private_tool(state, launcher, "git")
        original_resolved = launcher
    elif name in ("cargo", "rustc"):
        try:
            relative = launcher.relative_to(caller_rustup_home)
        except ValueError:
            relative = None
        if relative is not None:
            invoked = (state.rustup_home / relative).resolve(strict=True)
            original_resolved = launcher
        elif _is_rustup_proxy(discovered, launcher, search_path, launcher_sha256):
            private_rustup = state.tools / "rustup"
            _copy_private_tool(state, launcher, "rustup")
            selected = subprocess.check_output(
                [str(private_rustup), "which", name],
                cwd=safe_cwd,
                env=environment,
                text=True,
            ).strip()
            if not selected:
                raise ValueError(f"rustup returned an empty executable path for {name}")
            selected_path = Path(selected)
            if not selected_path.is_absolute():
                raise ValueError(f"rustup returned a non-absolute path for {name}")
            invoked = selected_path.resolve(strict=True)
            relative = invoked.relative_to(state.rustup_home)
            original_resolved = (caller_rustup_home / relative).resolve(strict=True)
        else:
            raise ValueError(
                f"{name} must be selected by rustup or reside inside --rustup-home"
            )
        if not resolve_inside(invoked, state.rustup_home):
            raise ValueError(f"{name} escaped the private Rustup snapshot")
    else:
        raise ValueError(f"unsupported profile tool: {name}")

    metadata = invoked.stat()
    if not stat.S_ISREG(metadata.st_mode) or not os.access(invoked, os.X_OK):
        raise ValueError(f"private {name} is not an executable regular file")
    original_metadata = original_resolved.stat()
    if (
        not stat.S_ISREG(original_metadata.st_mode)
        or not os.access(original_resolved, os.X_OK)
    ):
        raise ValueError(f"resolved {name} is not an executable regular file")
    digest = sha256_bytes(_read_regular_stable(invoked, invoked.lstat()))
    if sha256_bytes(
        _read_regular_stable(original_resolved, original_metadata)
    ) != digest:
        raise ValueError(f"private {name} copy does not match its resolved input")
    return {
        "discovered_path": str(discovered),
        "launcher_path": str(launcher),
        "launcher_sha256": launcher_sha256,
        "resolved_path": str(original_resolved),
        "invoked_path": str(invoked),
        "sha256": digest,
    }


def verify_isolated_tool(
    name: str,
    identity: dict[str, str],
    search_path: str,
) -> None:
    """Fail if PATH selection, caller launcher, or private executable changed."""

    found = shutil.which(name, path=search_path)
    if found is None or str(Path(found).absolute()) != identity["discovered_path"]:
        raise ValueError(f"{name} PATH resolution changed during profiling")
    launcher = Path(found).resolve(strict=True)
    if (
        str(launcher) != identity["launcher_path"]
        or sha256_bytes(_read_regular_stable(launcher, launcher.lstat()))
        != identity["launcher_sha256"]
    ):
        raise ValueError(f"{name} launcher changed during profiling")
    resolved = Path(identity["resolved_path"])
    if sha256_bytes(
        _read_regular_stable(resolved, resolved.lstat())
    ) != identity["sha256"]:
        raise ValueError(f"resolved {name} executable changed during profiling")
    invoked = Path(identity.get("invoked_path", identity["resolved_path"]))
    if (
        sha256_bytes(_read_regular_stable(invoked, invoked.lstat()))
        != identity["sha256"]
    ):
        raise ValueError(f"private {name} executable changed during profiling")


def public_tool_identity(identity: dict[str, str]) -> dict[str, str]:
    """Remove invocation-private paths from stable report input identity."""

    return {key: value for key, value in identity.items() if key != "invoked_path"}


def tool_invocation_path(identity: dict[str, str]) -> str:
    return identity.get("invoked_path", identity["resolved_path"])


def expose_private_tools(
    state: PrivateState,
    tools: dict[str, dict[str, str]],
    environment: dict[str, str],
) -> None:
    """Prepend private Cargo/rustc/Git selections for nested child lookups."""

    tools_metadata = _lstat_at(state.root_fd, "tools")
    tools_fd = _open_child_directory_at(
        state.root_fd, "tools", tools_metadata, display=state.tools
    )
    try:
        for name, identity in tools.items():
            link = state.tools / name
            target = os.path.relpath(tool_invocation_path(identity), state.tools)
            try:
                existing = _lstat_at(tools_fd, name)
            except FileNotFoundError:
                os.symlink(target, name, dir_fd=tools_fd)
                continue
            if stat.S_ISLNK(existing.st_mode):
                if os.readlink(name, dir_fd=tools_fd) != target:
                    raise ValueError(f"private tool alias conflicts: {link}")
            elif Path(tool_invocation_path(identity)) != link:
                raise ValueError(f"private tool alias conflicts: {link}")
    finally:
        os.close(tools_fd)
    environment["PATH"] = str(state.tools) + os.pathsep + environment["PATH"]


def create_private_state(path: Path) -> PrivateState:
    """Create an absent invocation-private input and process-state root."""

    path = path.absolute()
    parent_fd, name = _open_parent_anchored(path)
    root_fd: int | None = None
    identity: tuple[int, int] | None = None
    try:
        os.mkdir(name, 0o700, dir_fd=parent_fd)
        metadata = _lstat_at(parent_fd, name)
        identity = _identity(metadata)
        root_fd = _open_child_directory_at(
            parent_fd, name, metadata, display=path
        )
        opened = os.fstat(root_fd)
        if (
            hasattr(os, "geteuid")
            and opened.st_uid != os.geteuid()
            or stat.S_IMODE(opened.st_mode) != 0o700
        ):
            raise ValueError("invocation-private state root is unsafe")
        for child_name in ("home", "tmp", "tools"):
            os.mkdir(child_name, 0o700, dir_fd=root_fd)
            child = _lstat_at(root_fd, child_name)
            if not stat.S_ISDIR(child.st_mode) or stat.S_ISLNK(child.st_mode):
                raise ValueError("invocation-private state child is unsafe")
        return PrivateState(
            root=path,
            identity=identity,
            parent_fd=parent_fd,
            root_fd=root_fd,
            name=name,
            home=path / "home",
            temporary=path / "tmp",
            tools=path / "tools",
            source=path / "source",
            cargo_home=path / "cargo-home",
            rustup_home=path / "rustup-home",
        )
    except BaseException:
        if root_fd is not None:
            os.close(root_fd)
        if identity is not None:
            _remove_owned_entry_at(parent_fd, name, identity)
        os.close(parent_fd)
        raise


def remove_private_state(state: PrivateState) -> None:
    """Remove state through its held descriptor and inode-bound parent entry."""

    if state.closed:
        return
    cleanup_error: Exception | None = None
    try:
        if _identity(os.fstat(state.root_fd)) != state.identity:
            raise ValueError("private state descriptor identity changed")
        os.fchmod(state.root_fd, 0o700)
        _remove_directory_contents_fd(state.root_fd)
        if not _remove_owned_entry_at(
            state.parent_fd, state.name, state.identity
        ):
            raise ValueError(
                "private state root was replaced; owned contents were cleaned "
                "without deleting the replacement"
            )
    except Exception as error:
        cleanup_error = error
    finally:
        os.close(state.root_fd)
        os.close(state.parent_fd)
        state.closed = True
    if cleanup_error is not None:
        raise cleanup_error


def base_environment(
    root: Path,
    cargo_home: Path,
    rustup_home: Path,
    state_dir: Path,
) -> dict[str, str]:
    """Return the closed environment used to resolve and run profile tools."""

    root = root.resolve()
    allowed = _HOST_ENV_KEYS + PROFILE_ENV_KEYS
    environment = {
        key: os.environ[key]
        for key in allowed
        if key in os.environ and os.environ[key]
    }
    environment.setdefault("PATH", os.defpath)
    for entry in environment["PATH"].split(os.pathsep):
        if not entry or not Path(entry).is_absolute():
            raise ValueError("PATH entries must be absolute and non-empty")
        if resolve_inside(Path(entry), root):
            raise ValueError("PATH must not select tools from the repository")
    environment.update(
        {
            "CARGO_HOME": str(cargo_home),
            "CARGO_NET_OFFLINE": "true",
            "CARGO_TERM_COLOR": "never",
            "GIT_ATTR_NOSYSTEM": "1",
            "GIT_CONFIG_COUNT": "5",
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_KEY_0": "core.fsmonitor",
            "GIT_CONFIG_KEY_1": "core.hooksPath",
            "GIT_CONFIG_KEY_2": "core.untrackedCache",
            "GIT_CONFIG_KEY_3": "core.excludesFile",
            "GIT_CONFIG_KEY_4": "core.pager",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_SYSTEM": os.devnull,
            "GIT_CONFIG_VALUE_0": "false",
            "GIT_CONFIG_VALUE_1": os.devnull,
            "GIT_CONFIG_VALUE_2": "false",
            "GIT_CONFIG_VALUE_3": os.devnull,
            "GIT_CONFIG_VALUE_4": "",
            "GIT_NO_LAZY_FETCH": "1",
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_PAGER": "",
            "GIT_TERMINAL_PROMPT": "0",
            "HOME": str(state_dir / "home"),
            "LANG": "C",
            "LC_ALL": "C",
            "RUSTUP_HOME": str(rustup_home),
            "TEMP": str(state_dir / "tmp"),
            "TMP": str(state_dir / "tmp"),
            "TMPDIR": str(state_dir / "tmp"),
        }
    )
    return environment


def validate_search_path_disjoint(
    search_path: str, forbidden_roots: Sequence[Path]
) -> None:
    """Keep caller input/cache roots out of the PATH disclosed to Cargo."""

    for entry in search_path.split(os.pathsep):
        directory = Path(entry).resolve()
        if any(paths_overlap(directory, root) for root in forbidden_roots):
            raise ValueError("PATH entries must be disjoint from caller input roots")


def minimal_environment(
    root: Path,
    source_root: Path,
    target_dir: Path,
    jobs: int | None,
    cargo_home: Path,
    rustup_home: Path,
    state_dir: Path,
    rustc: Path,
) -> dict[str, str]:
    """Return the closed, source-safe environment used for one Cargo profile."""

    environment = base_environment(root, cargo_home, rustup_home, state_dir)
    if jobs is not None:
        environment["CARGO_BUILD_JOBS"] = str(jobs)
    environment["CARGO_TARGET_DIR"] = str(target_dir)
    environment["GIT_CEILING_DIRECTORIES"] = str(source_root.parent)
    environment["RUSTC"] = str(rustc)
    return environment


def validate_paths(root: Path, target_dir: Path, out: Path, reuse_target: bool) -> None:
    """Validate that profiling outputs cannot perturb repository inputs."""
    unresolved_outputs = (
        *report_paths(out.absolute()),
        private_state_path(out.absolute()),
    )
    root = root.resolve()
    target_dir = target_dir.resolve()
    out = out.resolve()
    outputs = (*report_paths(out), private_state_path(out))
    if not (root / "Cargo.toml").is_file():
        raise ValueError(f"repository root has no Cargo.toml: {root}")
    if paths_overlap(target_dir, root):
        raise ValueError("--target-dir must be outside the repository")
    if target_dir == Path(target_dir.anchor):
        raise ValueError("--target-dir must not be a filesystem root")
    for output in outputs:
        if resolve_inside(output, root):
            raise ValueError(
                "--out must be outside the repository together with its logs"
            )
        if paths_overlap(output, target_dir):
            raise ValueError("--out and its logs must be outside --target-dir")
    for output in unresolved_outputs:
        if os.path.lexists(output):
            raise ValueError(f"profiling output already exists: {output}")
    if target_dir.exists() and not target_dir.is_dir():
        raise ValueError("--target-dir exists and is not a directory")
    if target_dir.exists() and not reuse_target:
        try:
            next(target_dir.iterdir())
        except StopIteration:
            pass
        else:
            raise ValueError(
                "--target-dir is non-empty; pass --reuse-target for a warm profile"
            )


def validate_writable_tree(path: Path, *, label: str) -> TreeFingerprint:
    """Fingerprint a writable output/input tree and reject write-through links."""

    try:
        return bounded_tree_fingerprint(path, reject_hardlinks=True)
    except ValueError as error:
        if "hard-linked file" in str(error):
            raise ValueError(f"{label} must not contain hard-linked files") from error
        raise


def _closed_git_command(git: str, root: Path) -> list[str]:
    """Return the fixed prefix for read-only, configuration-closed Git reads."""

    return [
        git,
        "--no-pager",
        "--no-optional-locks",
        "-c",
        "core.fsmonitor=false",
        "-c",
        f"core.hooksPath={os.devnull}",
        "-c",
        "core.untrackedCache=false",
        "-c",
        f"core.excludesFile={os.devnull}",
        "-C",
        str(root),
    ]


def validate_git_worktree(
    root: Path, environment: dict[str, str], git: str, safe_cwd: Path
) -> None:
    """Require Git's configured worktree to be exactly the caller source root."""

    top_level = command_output(
        [*_closed_git_command(git, root), "rev-parse", "--show-toplevel"],
        safe_cwd,
        environment,
    )
    if Path(top_level).resolve(strict=True) != root.resolve(strict=True):
        raise ValueError("Git worktree does not match the repository root")


def tracked_and_untracked_paths(
    root: Path,
    environment: dict[str, str],
    git: str,
    safe_cwd: Path,
) -> list[str]:
    """List tracked and non-ignored untracked repository paths."""
    raw = subprocess.check_output(
        [
            *_closed_git_command(git, root),
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        cwd=safe_cwd,
        env=environment,
    )
    paths = sorted(
        entry.decode("utf-8", "surrogateescape")
        for entry in raw.split(b"\0")
        if entry
    )
    for relative in paths:
        _bounded_relative_path(relative)
    return paths


def _source_fingerprint_fd(
    root_fd: int,
    root_display: Path,
    paths: Iterable[str],
    *,
    reject_hardlinks: bool = False,
) -> SourceFingerprint:
    digest = hashlib.sha256()
    file_count = 0
    byte_count = 0
    deleted_count = 0
    for relative in sorted(paths):
        _bounded_relative_path(relative)
        if file_count + deleted_count >= _MAX_TREE_RECORDS:
            raise ValueError("source snapshot contains too many paths")
        try:
            parent_fd, name = _open_relative_parent_fd(root_fd, relative)
        except (FileNotFoundError, NotADirectoryError):
            parent_fd = None
        if parent_fd is None:
            record = {
                "bytes": 0,
                "executable": False,
                "kind": "deleted",
                "path": relative.replace(os.sep, "/"),
                "sha256": None,
            }
            digest.update(canonical_json_bytes(record))
            digest.update(b"\n")
            deleted_count += 1
            continue
        try:
            try:
                metadata = _lstat_at(parent_fd, name)
            except FileNotFoundError:
                record = {
                    "bytes": 0,
                    "executable": False,
                    "kind": "deleted",
                    "path": relative.replace(os.sep, "/"),
                    "sha256": None,
                }
                digest.update(canonical_json_bytes(record))
                digest.update(b"\n")
                deleted_count += 1
                continue
            if stat.S_ISDIR(metadata.st_mode):
                raise ValueError(
                    "Git-selected source path is a directory "
                    f"(submodules unsupported): {relative}"
                )
            if stat.S_ISLNK(metadata.st_mode):
                payload = _read_symlink_stable_at(
                    parent_fd, name, metadata, root_display / relative
                ).encode("utf-8", "surrogateescape")
                kind = "symlink"
            elif stat.S_ISREG(metadata.st_mode):
                if reject_hardlinks and metadata.st_nlink != 1:
                    raise ValueError(
                        "repository source contains a hard-linked file: "
                        f"{relative}"
                    )
                payload = _read_regular_stable_at(
                    parent_fd, name, metadata, root_display / relative
                )
                kind = "file"
            else:
                raise ValueError(f"unsupported source path type: {relative}")
        finally:
            os.close(parent_fd)
        record = {
            "bytes": len(payload),
            "executable": bool(metadata.st_mode & stat.S_IXUSR),
            "kind": kind,
            "path": relative.replace(os.sep, "/"),
            "sha256": sha256_bytes(payload),
        }
        digest.update(canonical_json_bytes(record))
        digest.update(b"\n")
        file_count += 1
        byte_count += len(payload)
        if byte_count > _MAX_TREE_TOTAL_BYTES:
            raise ValueError("source snapshot exceeds its total byte limit")
    return SourceFingerprint(
        digest.hexdigest(),
        file_count,
        byte_count,
        deleted_count,
    )


def source_fingerprint(
    root: Path,
    paths: Iterable[str],
    *,
    expected_identity: tuple[int, int] | None = None,
    reject_hardlinks: bool = False,
) -> SourceFingerprint:
    """Hash source paths through one descriptor-anchored repository root."""

    root = root.absolute()
    descriptor = _open_directory_anchored(root)
    identity = _identity(os.fstat(descriptor))
    try:
        if expected_identity is not None and identity != expected_identity:
            raise ValueError(f"source root identity changed: {root}")
        result = _source_fingerprint_fd(
            descriptor, root, paths, reject_hardlinks=reject_hardlinks
        )
        if not _path_still_names(root, identity):
            raise ValueError(f"source root was replaced while read: {root}")
        return result
    finally:
        os.close(descriptor)


def capture_source_snapshot(
    root: Path,
    paths: Sequence[str],
    destination: Path,
    *,
    expected_root_identity: tuple[int, int] | None = None,
    reject_source_hardlinks: bool = False,
) -> SourceFingerprint:
    """Copy the exact dirty source input to a verified inode-independent tree."""

    root = root.absolute()
    destination = destination.absolute()
    if os.path.lexists(destination):
        raise ValueError(f"source snapshot destination already exists: {destination}")
    source_fd = _open_directory_anchored(root)
    source_identity = _identity(os.fstat(source_fd))
    if (
        expected_root_identity is not None
        and source_identity != expected_root_identity
    ):
        os.close(source_fd)
        raise ValueError(f"repository source root identity changed: {root}")
    destination_parent_fd, destination_name = _open_parent_anchored(destination)
    destination_fd: int | None = None
    destination_identity: tuple[int, int] | None = None
    try:
        before = _source_fingerprint_fd(
            source_fd,
            root,
            paths,
            reject_hardlinks=reject_source_hardlinks,
        )
        filesystem = os.fstatvfs(destination_parent_fd)
        available = filesystem.f_bavail * filesystem.f_frsize
        if before.bytes + _MIN_FREE_BYTES_AFTER_COPY > available:
            raise ValueError("source snapshot would exhaust filesystem free space")
        os.mkdir(destination_name, 0o700, dir_fd=destination_parent_fd)
        destination_metadata = _lstat_at(destination_parent_fd, destination_name)
        destination_identity = _identity(destination_metadata)
        destination_fd = _open_child_directory_at(
            destination_parent_fd,
            destination_name,
            destination_metadata,
            display=destination,
        )
        for relative in sorted(paths):
            _bounded_relative_path(relative)
            try:
                source_parent, source_name = _open_relative_parent_fd(
                    source_fd, relative
                )
            except (FileNotFoundError, NotADirectoryError):
                continue
            target_parent, target_name = _ensure_relative_parent_fd(
                destination_fd, relative
            )
            try:
                try:
                    metadata = _lstat_at(source_parent, source_name)
                except FileNotFoundError:
                    continue
                if stat.S_ISDIR(metadata.st_mode):
                    raise ValueError(
                        "Git-selected source directory cannot be snapshotted: "
                        f"{relative}"
                    )
                if stat.S_ISREG(metadata.st_mode):
                    if reject_source_hardlinks and metadata.st_nlink != 1:
                        raise ValueError(
                            "repository source contains a hard-linked file: "
                            f"{relative}"
                        )
                    payload = _read_regular_stable_at(
                        source_parent,
                        source_name,
                        metadata,
                        root / relative,
                    )
                    descriptor = os.open(
                        target_name,
                        os.O_WRONLY
                        | os.O_CREAT
                        | os.O_EXCL
                        | getattr(os, "O_CLOEXEC", 0)
                        | getattr(os, "O_NOFOLLOW", 0),
                        0o700 if metadata.st_mode & stat.S_IXUSR else 0o600,
                        dir_fd=target_parent,
                    )
                    try:
                        view = memoryview(payload)
                        while view:
                            written = os.write(descriptor, view)
                            if written <= 0:
                                raise ValueError(
                                    f"could not finish copying source: {relative}"
                                )
                            view = view[written:]
                        os.fchmod(
                            descriptor,
                            0o700 if metadata.st_mode & stat.S_IXUSR else 0o600,
                        )
                        os.fsync(descriptor)
                        copied = os.fstat(descriptor)
                        if (
                            copied.st_nlink != 1
                            or (copied.st_dev, copied.st_ino)
                            == (metadata.st_dev, metadata.st_ino)
                        ):
                            raise ValueError(
                                f"source copy is not inode-independent: {relative}"
                            )
                    finally:
                        os.close(descriptor)
                elif stat.S_ISLNK(metadata.st_mode):
                    os.symlink(
                        _safe_symlink_target_at(
                            source_fd,
                            source_parent,
                            source_name,
                            metadata,
                            relative,
                            root / relative,
                        ),
                        target_name,
                        dir_fd=target_parent,
                    )
                else:
                    raise ValueError(f"unsupported source path type: {relative}")
            finally:
                os.close(source_parent)
                os.close(target_parent)
        after = _source_fingerprint_fd(
            source_fd,
            root,
            paths,
            reject_hardlinks=reject_source_hardlinks,
        )
        copied_fingerprint = _source_fingerprint_fd(
            destination_fd, destination, paths, reject_hardlinks=True
        )
        if before != after:
            raise ValueError(
                "repository source changed while its snapshot was captured"
            )
        if before != copied_fingerprint:
            raise ValueError("source snapshot does not match the repository input")
        if not _path_still_names(root, source_identity):
            raise ValueError("repository source root was replaced during capture")
        if (
            destination_identity is None
            or _identity(_lstat_at(destination_parent_fd, destination_name))
            != destination_identity
        ):
            raise ValueError("source snapshot destination was replaced")
        return before
    except BaseException:
        if destination_identity is not None:
            _remove_owned_entry_at(
                destination_parent_fd, destination_name, destination_identity
            )
        raise
    finally:
        if destination_fd is not None:
            os.close(destination_fd)
        os.close(destination_parent_fd)
        os.close(source_fd)


def make_source_read_only(root: Path) -> None:
    """Make the held private source tree read-only without path traversal."""

    root = root.absolute()
    root_fd = _open_directory_anchored(root)
    root_identity = _identity(os.fstat(root_fd))

    def seal(directory_fd: int, display: Path) -> None:
        for name in _directory_names_fd(directory_fd):
            metadata = _lstat_at(directory_fd, name)
            child_display = display / name
            if stat.S_ISREG(metadata.st_mode):
                descriptor = os.open(
                    name,
                    os.O_RDONLY
                    | getattr(os, "O_CLOEXEC", 0)
                    | getattr(os, "O_NOFOLLOW", 0),
                    dir_fd=directory_fd,
                )
                try:
                    os.fchmod(
                        descriptor,
                        0o500 if metadata.st_mode & stat.S_IXUSR else 0o400,
                    )
                finally:
                    os.close(descriptor)
                after = _lstat_at(directory_fd, name)
                if not stat.S_ISREG(after.st_mode) or _identity(after) != _identity(
                    metadata
                ):
                    raise ValueError(f"source snapshot file changed: {child_display}")
            elif stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(
                metadata.st_mode
            ):
                child = _open_child_directory_at(
                    directory_fd, name, metadata, display=child_display
                )
                try:
                    seal(child, child_display)
                    os.fchmod(child, 0o500)
                finally:
                    os.close(child)
            elif stat.S_ISLNK(metadata.st_mode):
                continue
            else:
                raise ValueError(
                    f"source snapshot contains a special file: {child_display}"
                )

    try:
        seal(root_fd, root)
        os.fchmod(root_fd, 0o500)
        if not _path_still_names(root, root_identity):
            raise ValueError("source snapshot root was replaced while sealed")
    finally:
        os.close(root_fd)


def normalized_cargo_args(raw: Sequence[str], jobs: int) -> list[str]:
    """Build an offline, locked, JSON-emitting Cargo argument vector."""
    cargo_args = list(raw)
    if cargo_args and cargo_args[0] == "--":
        cargo_args.pop(0)
    if not cargo_args:
        cargo_args = ["build", "--workspace"]
    if cargo_args[0].startswith("-"):
        raise ValueError("the first Cargo argument must be a subcommand")
    if cargo_args[0] not in ("build", "check", "test"):
        raise ValueError("Cargo subcommand must be build, check, or test")
    separator = cargo_args.index("--") if "--" in cargo_args else len(cargo_args)
    cargo_controls = cargo_args[:separator]
    if "--locked" not in cargo_controls:
        cargo_args.insert(1, "--locked")
        separator += 1
        cargo_controls = cargo_args[:separator]
    if "--offline" not in cargo_controls:
        locked_index = cargo_controls.index("--locked")
        cargo_args.insert(locked_index + 1, "--offline")
        separator += 1
        cargo_controls = cargo_args[:separator]
    if cargo_args[0] == "test" and "--no-run" not in cargo_controls:
        cargo_args.insert(1, "--no-run")
        separator += 1
        cargo_controls = cargo_args[:separator]
    additions: list[str] = []
    if not any(
        item == "--message-format" or item.startswith("--message-format=")
        for item in cargo_controls
    ):
        additions.extend(["--message-format", "json-render-diagnostics"])
    if not any(
        item == "--timings" or item.startswith("--timings=")
        for item in cargo_controls
    ):
        additions.append("--timings")
    if not any(
        item in ("-j", "--jobs")
        or (item.startswith("-j") and item != "-j")
        or item.startswith("--jobs=")
        for item in cargo_controls
    ):
        additions.extend(["--jobs", str(jobs)])
    cargo_args[separator:separator] = additions
    return cargo_args


def validate_cargo_controls(
    cargo_args: Sequence[str],
    root: Path,
    caller_private_roots: Sequence[Path] = (),
) -> None:
    """Reject Cargo controls that can redirect writes or escape source closure."""

    separator = cargo_args.index("--") if "--" in cargo_args else len(cargo_args)
    controls = cargo_args[:separator]
    forbidden_strings = tuple(
        str(path.resolve()) for path in (root, *caller_private_roots)
    )

    def exposes_caller_root(candidate: str) -> bool:
        if any(forbidden in candidate for forbidden in forbidden_strings):
            return True
        path_candidate = Path(candidate)
        return path_candidate.is_absolute() and any(
            resolve_inside(path_candidate, Path(forbidden))
            for forbidden in forbidden_strings
        )

    def validate_target_triple(value: str) -> None:
        if (
            not value
            or Path(value).is_absolute()
            or value in (".", "..")
            or "/" in value
            or "\\" in value
        ):
            raise ValueError("--target must be a target triple, not a path")
    for option in ("--locked", "--offline"):
        if controls.count(option) != 1 or any(
            argument.startswith(option + "=") for argument in controls
        ):
            raise ValueError(f"Cargo controls require exactly one {option}")
    index = 0
    while index < len(controls):
        argument = controls[index]
        if argument in (
            "--artifact-dir",
            "--config",
            "--lockfile-path",
            "--target-dir",
        ) or argument.startswith(
            ("--artifact-dir=", "--config=", "--lockfile-path=", "--target-dir=")
        ):
            option = argument.split("=", 1)[0]
            raise ValueError(f"{option} is controlled by the profiler")
        if argument == "--target":
            if index + 1 == len(controls):
                raise ValueError("--target requires a value")
            validate_target_triple(controls[index + 1])
            index += 2
            continue
        elif argument.startswith("--target="):
            validate_target_triple(argument.split("=", 1)[1])
        if argument == "--manifest-path":
            index += 1
            if index == len(controls) or controls[index].startswith("-"):
                raise ValueError("--manifest-path requires a path value")
            value = controls[index]
        elif argument.startswith("--manifest-path="):
            value = argument.split("=", 1)[1]
            if not value:
                raise ValueError("--manifest-path requires a path value")
        else:
            candidates = (argument, argument.split("=", 1)[-1])
            if any(exposes_caller_root(candidate) for candidate in candidates):
                raise ValueError(
                    "Cargo arguments must not disclose caller source/cache/tool roots"
                )
            index += 1
            continue
        manifest = Path(value)
        if not manifest.is_absolute():
            manifest = root / manifest
        manifest = manifest.resolve()
        if not resolve_inside(manifest, root) or not manifest.is_file():
            raise ValueError(
                "--manifest-path must name a manifest inside the repository"
            )
        index += 1
    for argument in cargo_args[separator + 1 :]:
        if exposes_caller_root(argument):
            raise ValueError(
                "Cargo arguments must not disclose caller source/cache/tool roots"
            )


def cargo_execution_args(
    cargo_args: Sequence[str], caller_root: Path, source_root: Path
) -> list[str]:
    """Map every allowed manifest path into the private source snapshot."""

    execution = list(cargo_args)
    separator = execution.index("--") if "--" in execution else len(execution)
    index = 0
    while index < separator:
        argument = execution[index]
        if argument == "--manifest-path":
            value_index = index + 1
            manifest = Path(execution[value_index])
            if not manifest.is_absolute():
                manifest = caller_root / manifest
            relative = manifest.resolve(strict=True).relative_to(caller_root)
            execution[value_index] = str(source_root / relative)
            index += 2
            continue
        if argument.startswith("--manifest-path="):
            manifest = Path(argument.split("=", 1)[1])
            if not manifest.is_absolute():
                manifest = caller_root / manifest
            relative = manifest.resolve(strict=True).relative_to(caller_root)
            execution[index] = f"--manifest-path={source_root / relative}"
        index += 1
    return execution


def normalized_package_id(package_id: str) -> str:
    """Remove checkout-specific prefixes from path package identifiers."""
    if package_id.startswith("path+file://"):
        _, separator, fragment = package_id.rpartition("#")
        return f"workspace#{fragment}" if separator else "workspace"
    return package_id


def artifact_unit(message: dict[str, Any]) -> dict[str, Any] | None:
    """Project one Cargo compiler-artifact message into a stable unit identity."""
    if message.get("reason") != "compiler-artifact":
        return None
    target = message.get("target")
    profile = message.get("profile")
    package_id = message.get("package_id")
    if (
        not isinstance(target, dict)
        or not isinstance(profile, dict)
        or not isinstance(package_id, str)
    ):
        return None
    return {
        "crate_types": sorted(str(item) for item in target.get("crate_types", [])),
        "features": sorted(str(item) for item in message.get("features", [])),
        "kind": sorted(str(item) for item in target.get("kind", [])),
        "name": str(target.get("name", "")),
        "package_id": normalized_package_id(package_id),
        "profile": {
            "debug_assertions": bool(profile.get("debug_assertions", False)),
            "debuginfo": profile.get("debuginfo"),
            "opt_level": str(profile.get("opt_level", "")),
            "test": bool(profile.get("test", False)),
        },
    }


def parse_cargo_messages(lines: Iterable[str]) -> tuple[list[dict[str, Any]], int, int]:
    """Extract stable unit identities and fresh/compiled counts from Cargo JSON."""
    units: list[dict[str, Any]] = []
    fresh = 0
    compiled = 0
    for line in lines:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(message, dict):
            continue
        unit = artifact_unit(message)
        if unit is None:
            continue
        units.append(unit)
        if bool(message.get("fresh", False)):
            fresh += 1
        else:
            compiled += 1
    units.sort(key=canonical_json_bytes)
    return units, fresh, compiled


def timing_html(target_dir: Path) -> dict[str, Any] | None:
    """Describe Cargo's stable HTML timing artifact when present."""
    path = target_dir / "cargo-timings" / "cargo-timing.html"
    if not path.is_file():
        return None
    if not resolve_inside(path.resolve(strict=True), target_dir):
        raise ValueError("Cargo timing output escapes the target directory")
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError("Cargo timing output is not a regular file")
    payload = _read_regular_stable(path, metadata)
    return {
        "bytes": len(payload),
        "path": "cargo-timings/cargo-timing.html",
        "sha256": sha256_bytes(payload),
    }


def reserve_report_paths(out: Path) -> tuple[int, int, int]:
    """Exclusively reserve the report and transcripts before Cargo starts."""

    descriptors: list[int] = []
    created: list[tuple[str, tuple[int, int]]] = []
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    parent_fd = _open_directory_anchored(out.parent)
    try:
        for path in report_paths(out):
            if path.parent != out.parent:
                raise ValueError("profiling report paths must share one parent")
            descriptor = os.open(path.name, flags, 0o600, dir_fd=parent_fd)
            descriptors.append(descriptor)
            metadata = os.fstat(descriptor)
            created.append((path.name, _identity(metadata)))
    except BaseException:
        for descriptor in descriptors:
            os.close(descriptor)
        for name, identity in created:
            _remove_owned_entry_at(parent_fd, name, identity)
        raise
    finally:
        os.close(parent_fd)
    return descriptors[0], descriptors[1], descriptors[2]


def reserved_report_identities(
    descriptors: Sequence[int],
) -> tuple[tuple[int, int], ...]:
    """Capture the inode identities of freshly reserved report entries."""

    return tuple(
        (metadata.st_dev, metadata.st_ino)
        for metadata in (os.fstat(descriptor) for descriptor in descriptors)
    )


def remove_reserved_report_paths(
    out: Path, identities: Sequence[tuple[int, int]] | None = None
) -> None:
    """Remove only the fresh report entries reserved by this invocation."""

    if identities is None or len(identities) != len(report_paths(out)):
        raise ValueError("reserved profiling output identities are required")
    parent_fd = _open_directory_anchored(out.parent)
    try:
        for path, identity in zip(report_paths(out), identities):
            _remove_owned_entry_at(parent_fd, path.name, identity)
    finally:
        os.close(parent_fd)


def validate_reserved_report_paths(
    out: Path, identities: Sequence[tuple[int, int]]
) -> None:
    """Require every published path still to name its reserved inode."""

    if len(identities) != len(report_paths(out)):
        raise ValueError("reserved profiling output identity count is invalid")
    parent_fd = _open_directory_anchored(out.parent)
    try:
        for path, identity in zip(report_paths(out), identities):
            metadata = _lstat_at(parent_fd, path.name)
            if _identity(metadata) != identity:
                raise ValueError(f"reserved profiling output was replaced: {path}")
    finally:
        os.close(parent_fd)


def write_json_stream(output: Any, value: Any) -> None:
    """Write pretty, deterministic JSON with one trailing newline."""

    output.write(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    )


def selected_profile_environment(environment: dict[str, str]) -> dict[str, str]:
    """Return build-affecting environment inputs in stable key order."""
    return {
        key: environment[key]
        for key in PROFILE_ENV_KEYS
        if key in environment and environment[key]
    }


def closed_git_revision(
    root: Path,
    environment: dict[str, str],
    git: str,
    safe_cwd: Path,
) -> str:
    """Read HEAD with optional locks, hooks, and fsmonitor disabled."""

    return command_output(
        [*_closed_git_command(git, root), "rev-parse", "HEAD"],
        safe_cwd,
        environment,
    )


def capture_input_manifest(
    root: Path,
    cargo_args: Sequence[str],
    jobs: int,
    label: str,
    reuse_target: bool,
    environment: dict[str, str],
    cargo_tool: dict[str, str],
    rustc_tool: dict[str, str],
    git_tool: dict[str, str],
    *,
    safe_cwd: Path | None = None,
    source_paths: Sequence[str] | None = None,
    source: SourceFingerprint | None = None,
    git_revision: str | None = None,
    cargo_cache: TreeFingerprint | None = None,
    rustup_tree: TreeFingerprint | None = None,
    target_initial: TreeFingerprint | None = None,
    path_identity: str | None = None,
    cargo_version: str | None = None,
    rustc_version: str | None = None,
    cargo_lock_sha256: str | None = None,
) -> dict[str, Any]:
    """Capture every repository and toolchain input used by a profile."""
    cargo_lock = root / "Cargo.lock"
    if not cargo_lock.is_file():
        raise ValueError("Cargo.lock is missing")
    if safe_cwd is None:
        raise ValueError("input capture requires an external safe working directory")
    capture_cwd = safe_cwd
    paths = (
        list(source_paths)
        if source_paths is not None
        else tracked_and_untracked_paths(
            root, environment, tool_invocation_path(git_tool), capture_cwd
        )
    )
    captured_source = source or source_fingerprint(root, paths)
    revision = git_revision or closed_git_revision(
        root, environment, tool_invocation_path(git_tool), capture_cwd
    )
    return {
        "cargo_args": list(cargo_args),
        "cargo_lock_sha256": cargo_lock_sha256
        if cargo_lock_sha256 is not None
        else sha256_bytes(_read_regular_stable(cargo_lock, cargo_lock.lstat())),
        "git_revision": revision,
        "jobs": jobs,
        "label": label,
        "path": path_identity if path_identity is not None else environment["PATH"],
        "profile_mode": "warm" if reuse_target else "cold",
        "selected_env": selected_profile_environment(environment),
        "source": {
            "bytes": captured_source.bytes,
            "deleted": captured_source.deleted,
            "files": captured_source.files,
            "sha256": captured_source.sha256,
        },
        "cargo_cache": (
            tree_fingerprint_json(cargo_cache) if cargo_cache is not None else None
        ),
        "rustup_tree": (
            tree_fingerprint_json(rustup_tree) if rustup_tree is not None else None
        ),
        "target_initial": (
            tree_fingerprint_json(target_initial)
            if target_initial is not None
            else None
        ),
        "toolchain": {
            "cargo": {
                **public_tool_identity(cargo_tool),
                "version": cargo_version
                if cargo_version is not None
                else command_output(
                    [tool_invocation_path(cargo_tool), "-Vv"], capture_cwd, environment
                ),
            },
            "rustc": {
                **public_tool_identity(rustc_tool),
                "version": rustc_version
                if rustc_version is not None
                else command_output(
                    [tool_invocation_path(rustc_tool), "-Vv"], capture_cwd, environment
                ),
            },
            "git": public_tool_identity(git_tool),
        },
    }


def changed_input_fields(
    before: dict[str, Any], after: dict[str, Any]
) -> list[str]:
    """Return sorted manifest fields whose values changed during profiling."""
    return sorted(
        key
        for key in before.keys() | after.keys()
        if before.get(key) != after.get(key)
    )


def main(argv: Sequence[str] | None = None) -> int:
    """Run the requested Cargo profile and write its report."""
    args = parse_args(argv)
    root = args.root.resolve()
    state: PrivateState | None = None
    out: Path | None = None
    reserved_identities: tuple[tuple[int, int], ...] | None = None
    reserved_descriptors: tuple[int, int, int] | None = None
    try:
        validate_paths(root, args.target_dir, args.out, args.reuse_target)
        target_dir = args.target_dir.resolve()
        out = args.out.resolve()
        cargo_home, rustup_home = validate_private_roots(
            root,
            target_dir,
            out,
            args.cargo_home,
            args.rustup_home,
        )
        caller_root_identity = _directory_identity(root)
        caller_cargo_identity = _directory_identity(cargo_home)
        caller_rustup_identity = _directory_identity(rustup_home)
        cargo_args = normalized_cargo_args(args.cargo_args, args.jobs)
        validate_cargo_controls(
            cargo_args, root, (cargo_home, rustup_home)
        )
        target_descriptor = _open_directory_anchored(target_dir, create=True)
        os.close(target_descriptor)
        output_parent_descriptor = _open_directory_anchored(
            out.parent, create=True
        )
        os.close(output_parent_descriptor)
        state = create_private_state(private_state_path(out))
        cargo_cache_input = copy_bounded_tree(
            cargo_home,
            state.cargo_home,
            roots=("git", "registry"),
            expected_source_identity=caller_cargo_identity,
            reject_source_hardlinks=True,
        )
        rustup_input = copy_bounded_tree(
            rustup_home,
            state.rustup_home,
            expected_source_identity=caller_rustup_identity,
            reject_source_hardlinks=True,
        )
        discovery_environment = base_environment(
            root, state.cargo_home, state.rustup_home, state.root
        )
        search_path = discovery_environment["PATH"]
        validate_search_path_disjoint(
            search_path,
            (root, cargo_home, rustup_home, target_dir, state.root),
        )
        forbidden_tools = (target_dir, state.root)
        git_tool = resolve_isolated_tool(
            "git",
            root,
            search_path,
            discovery_environment,
            state,
            rustup_home,
            state.home,
            (*forbidden_tools, cargo_home, rustup_home),
        )
        validate_git_worktree(
            root,
            discovery_environment,
            tool_invocation_path(git_tool),
            state.home,
        )
        source_paths = tracked_and_untracked_paths(
            root,
            discovery_environment,
            tool_invocation_path(git_tool),
            state.home,
        )
        if not _path_still_names(root, caller_root_identity):
            raise ValueError("repository source root changed during Git inventory")
        captured_source = capture_source_snapshot(
            root,
            source_paths,
            state.source,
            expected_root_identity=caller_root_identity,
            reject_source_hardlinks=True,
        )
        git_revision = closed_git_revision(
            root,
            discovery_environment,
            tool_invocation_path(git_tool),
            state.home,
        )
        if not _path_still_names(root, caller_root_identity):
            raise ValueError("repository source root changed during Git revision read")
        cargo_tool = resolve_isolated_tool(
            "cargo",
            root,
            search_path,
            discovery_environment,
            state,
            rustup_home,
            state.source,
            forbidden_tools,
        )
        rustc_tool = resolve_isolated_tool(
            "rustc",
            root,
            search_path,
            discovery_environment,
            state,
            rustup_home,
            state.source,
            forbidden_tools,
        )
        cargo_version = command_output(
            [tool_invocation_path(cargo_tool), "-Vv"],
            state.home,
            discovery_environment,
        )
        rustc_version = command_output(
            [tool_invocation_path(rustc_tool), "-Vv"],
            state.home,
            discovery_environment,
        )
        environment = minimal_environment(
            root,
            state.source,
            target_dir,
            args.jobs,
            state.cargo_home,
            state.rustup_home,
            state.root,
            Path(tool_invocation_path(rustc_tool)),
        )
        environment["IROHA_GIT_COMMIT_HASH"] = git_revision
        environment["VERGEN_GIT_SHA"] = git_revision
        expose_private_tools(
            state,
            {"cargo": cargo_tool, "git": git_tool, "rustc": rustc_tool},
            environment,
        )
        target_initial = validate_writable_tree(
            target_dir, label="--target-dir"
        )
        private_cargo_input = validate_writable_tree(
            state.cargo_home, label="private Cargo cache"
        )
        private_rustup_input = validate_writable_tree(
            state.rustup_home, label="private Rustup tree"
        )
        source_execution_input = bounded_tree_fingerprint(
            state.source, reject_hardlinks=True
        )
        snapshot_lock = state.source / "Cargo.lock"
        snapshot_lock_sha256 = sha256_bytes(
            _read_regular_stable(snapshot_lock, snapshot_lock.lstat())
        )
        make_source_read_only(state.source)
        input_manifest = capture_input_manifest(
            root,
            cargo_args,
            args.jobs,
            args.label,
            args.reuse_target,
            environment,
            cargo_tool,
            rustc_tool,
            git_tool,
            safe_cwd=state.source,
            source_paths=source_paths,
            source=captured_source,
            git_revision=git_revision,
            cargo_cache=cargo_cache_input,
            rustup_tree=rustup_input,
            target_initial=target_initial,
            path_identity=search_path,
            cargo_version=cargo_version,
            rustc_version=rustc_version,
            cargo_lock_sha256=snapshot_lock_sha256,
        )
        input_manifest["execution_source"] = tree_fingerprint_json(
            source_execution_input
        )
        input_manifest["private_cargo_input"] = tree_fingerprint_json(
            private_cargo_input
        )
        input_manifest["private_rustup_input"] = tree_fingerprint_json(
            private_rustup_input
        )
        reserved_descriptors = reserve_report_paths(out)
        reserved_identities = reserved_report_identities(reserved_descriptors)
        execution_args = cargo_execution_args(cargo_args, root, state.source)
        command = [tool_invocation_path(cargo_tool), *execution_args]
        report_descriptor, message_descriptor, stderr_descriptor = reserved_descriptors
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        if reserved_descriptors is not None:
            if reserved_identities is None:
                try:
                    reserved_identities = reserved_report_identities(
                        reserved_descriptors
                    )
                except OSError:
                    pass
            for descriptor in reserved_descriptors:
                try:
                    os.close(descriptor)
                except OSError:
                    pass
            if out is not None and reserved_identities is not None:
                try:
                    remove_reserved_report_paths(out, reserved_identities)
                except (OSError, ValueError) as cleanup:
                    print(
                        "profile_cargo_build: report cleanup failed: "
                        f"{cleanup}",
                        file=sys.stderr,
                    )
        cleanup_error: Exception | None = None
        if state is not None:
            try:
                remove_private_state(state)
            except (OSError, ValueError) as cleanup:
                cleanup_error = cleanup
        print(f"profile_cargo_build: {error}", file=sys.stderr)
        if cleanup_error is not None:
            print(
                f"profile_cargo_build: private state cleanup failed: {cleanup_error}",
                file=sys.stderr,
            )
        return 2

    input_sha256 = sha256_bytes(canonical_json_bytes(input_manifest))

    assert state is not None and out is not None and reserved_identities is not None
    _, message_log, stderr_log = report_paths(out)
    print(
        "profile_cargo_build: "
        f"input={input_sha256} mode={input_manifest['profile_mode']} "
        f"command={' '.join(command)}",
        file=sys.stderr,
    )

    usage_before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started_ns = time.monotonic_ns()
    units: list[dict[str, Any]] = []
    fresh_units = 0
    compiled_units = 0
    messages = None
    errors = None
    try:
        messages = os.fdopen(message_descriptor, "w", encoding="utf-8")
        message_descriptor = -1
        errors = os.fdopen(stderr_descriptor, "w", encoding="utf-8")
        stderr_descriptor = -1
    except OSError as error:
        if messages is not None:
            messages.close()
        elif message_descriptor >= 0:
            os.close(message_descriptor)
        if errors is not None:
            errors.close()
        elif stderr_descriptor >= 0:
            os.close(stderr_descriptor)
        os.close(report_descriptor)
        try:
            remove_reserved_report_paths(out, reserved_identities)
        except (OSError, ValueError) as cleanup:
            print(
                f"profile_cargo_build: report cleanup failed: {cleanup}",
                file=sys.stderr,
            )
        try:
            remove_private_state(state)
        except (OSError, ValueError) as cleanup:
            print(
                f"profile_cargo_build: private state cleanup failed: {cleanup}",
                file=sys.stderr,
            )
        print(f"profile_cargo_build: {error}", file=sys.stderr)
        return 2
    assert messages is not None and errors is not None
    try:
        with messages, errors:
            process = subprocess.Popen(
                command,
                cwd=state.source,
                env=environment,
                stdout=subprocess.PIPE,
                stderr=errors,
                text=True,
                encoding="utf-8",
                errors="replace",
            )
            assert process.stdout is not None
            for line in process.stdout:
                messages.write(line)
                try:
                    message = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if not isinstance(message, dict):
                    continue
                unit = artifact_unit(message)
                if unit is None:
                    continue
                units.append(unit)
                if bool(message.get("fresh", False)):
                    fresh_units += 1
                else:
                    compiled_units += 1
            returncode = process.wait()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        os.close(report_descriptor)
        try:
            remove_reserved_report_paths(out, reserved_identities)
        except (OSError, ValueError) as cleanup:
            print(
                f"profile_cargo_build: report cleanup failed: {cleanup}",
                file=sys.stderr,
            )
        try:
            remove_private_state(state)
        except (OSError, ValueError) as cleanup:
            print(
                f"profile_cargo_build: private state cleanup failed: {cleanup}",
                file=sys.stderr,
            )
        print(f"profile_cargo_build: {error}", file=sys.stderr)
        return 2
    elapsed_ns = time.monotonic_ns() - started_ns
    usage_after = resource.getrusage(resource.RUSAGE_CHILDREN)

    post_input_manifest: dict[str, Any] | None = None
    input_capture_error: str | None = None
    try:
        verify_isolated_tool("cargo", cargo_tool, search_path)
        verify_isolated_tool("rustc", rustc_tool, search_path)
        verify_isolated_tool("git", git_tool, search_path)
        validate_git_worktree(
            root,
            environment,
            tool_invocation_path(git_tool),
            state.home,
        )
        post_source_paths = tracked_and_untracked_paths(
            root,
            environment,
            tool_invocation_path(git_tool),
            state.home,
        )
        post_source = source_fingerprint(
            root,
            post_source_paths,
            expected_identity=caller_root_identity,
            reject_hardlinks=True,
        )
        post_revision = closed_git_revision(
            root, environment, tool_invocation_path(git_tool), state.home
        )
        post_cargo_cache = bounded_tree_fingerprint(
            cargo_home,
            ("git", "registry"),
            expected_identity=caller_cargo_identity,
            reject_hardlinks=True,
        )
        post_rustup_tree = bounded_tree_fingerprint(
            rustup_home,
            expected_identity=caller_rustup_identity,
            reject_hardlinks=True,
        )
        post_cargo_version = command_output(
            [tool_invocation_path(cargo_tool), "-Vv"],
            state.home,
            discovery_environment,
        )
        post_rustc_version = command_output(
            [tool_invocation_path(rustc_tool), "-Vv"],
            state.home,
            discovery_environment,
        )
        post_input_manifest = capture_input_manifest(
            root,
            cargo_args,
            args.jobs,
            args.label,
            args.reuse_target,
            environment,
            cargo_tool,
            rustc_tool,
            git_tool,
            safe_cwd=state.source,
            source_paths=post_source_paths,
            source=post_source,
            git_revision=post_revision,
            cargo_cache=post_cargo_cache,
            rustup_tree=post_rustup_tree,
            target_initial=target_initial,
            path_identity=search_path,
            cargo_version=post_cargo_version,
            rustc_version=post_rustc_version,
        )
        post_input_manifest["execution_source"] = tree_fingerprint_json(
            bounded_tree_fingerprint(state.source, reject_hardlinks=True)
        )
        post_input_manifest["private_cargo_input"] = tree_fingerprint_json(
            private_cargo_input
        )
        post_input_manifest["private_rustup_input"] = tree_fingerprint_json(
            validate_writable_tree(
                state.rustup_home, label="private Rustup tree"
            )
        )
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        input_capture_error = f"{type(error).__name__}: {error}"

    if post_input_manifest is None:
        changed_fields = ["post_input_capture"]
        post_input_sha256 = None
    else:
        changed_fields = changed_input_fields(input_manifest, post_input_manifest)
        post_input_sha256 = sha256_bytes(canonical_json_bytes(post_input_manifest))
    input_stable = not changed_fields

    cleanup_error: str | None = None
    try:
        remove_private_state(state)
    except (OSError, ValueError) as error:
        cleanup_error = f"{type(error).__name__}: {error}"
        if "private_state_cleanup" not in changed_fields:
            changed_fields.append("private_state_cleanup")
            changed_fields.sort()
        input_stable = False

    units.sort(key=canonical_json_bytes)
    unit_inventory_sha256 = sha256_bytes(canonical_json_bytes(units))
    try:
        validate_reserved_report_paths(out, reserved_identities)
        report = {
            "schema_version": SCHEMA_VERSION,
            "valid": returncode == 0 and input_stable,
            "input": input_manifest,
            "input_sha256": input_sha256,
            "input_validation": {
                "changed_fields": changed_fields,
                "error": input_capture_error,
                "post_input": post_input_manifest,
                "post_input_sha256": post_input_sha256,
                "private_state_cleanup_error": cleanup_error,
                "stable": input_stable,
            },
            "result": {
                "compiled_units": compiled_units,
                "elapsed_ns": elapsed_ns,
                "fresh_units": fresh_units,
                "max_rss_raw": usage_after.ru_maxrss,
                "max_rss_unit": "bytes" if sys.platform == "darwin" else "KiB",
                "message_log": message_log.name,
                "platform": platform.platform(),
                "returncode": returncode,
                "stderr_log": stderr_log.name,
                "system_cpu_seconds": usage_after.ru_stime - usage_before.ru_stime,
                "timings_html": timing_html(target_dir),
                "unit_inventory": units,
                "unit_inventory_sha256": unit_inventory_sha256,
                "user_cpu_seconds": usage_after.ru_utime - usage_before.ru_utime,
            },
        }
        with os.fdopen(report_descriptor, "w", encoding="utf-8") as report_output:
            report_descriptor = -1
            write_json_stream(report_output, report)
        validate_reserved_report_paths(out, reserved_identities)
    except (OSError, ValueError) as error:
        if report_descriptor >= 0:
            os.close(report_descriptor)
        try:
            remove_reserved_report_paths(out, reserved_identities)
        except (OSError, ValueError) as cleanup:
            print(
                f"profile_cargo_build: report cleanup failed: {cleanup}",
                file=sys.stderr,
            )
        print(f"profile_cargo_build: {error}", file=sys.stderr)
        return 2
    if cleanup_error is not None:
        profiler_returncode = 2
    else:
        profiler_returncode = (
            INPUT_DRIFT_EXIT_CODE
            if returncode == 0 and not input_stable
            else returncode
        )
    print(
        "profile_cargo_build: "
        f"returncode={returncode} elapsed_ns={elapsed_ns} "
        f"compiled={compiled_units} fresh={fresh_units} "
        f"units={unit_inventory_sha256} input_stable={input_stable} report={out}",
        file=sys.stderr,
    )
    if not input_stable:
        print(
            "profile_cargo_build: report invalidated by input drift: "
            + ", ".join(changed_fields),
            file=sys.stderr,
        )
    return profiler_returncode


if __name__ == "__main__":
    raise SystemExit(main())
