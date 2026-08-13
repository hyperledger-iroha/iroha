#!/usr/bin/env python3
"""Create a bounded inode-independent Cargo cache snapshot and inventory."""

# RELEASE_CARGO_CACHE_COPY_HELPER_V1
from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import secrets
import shutil
import stat
import subprocess
import sys
import tarfile
import unicodedata
import zipfile


MAXIMUM_RECORDS = 250_000
MAXIMUM_FILE_BYTES = 4 * 1024 * 1024 * 1024
MAXIMUM_TOTAL_BYTES = 64 * 1024 * 1024 * 1024
MINIMUM_FREE_BYTES_AFTER_COPY = 1024 * 1024 * 1024
MAXIMUM_DEPTH = 128
MAXIMUM_PATH_BYTES = 4096
INPUT_FORMAT = "iroha-sumeragi-v2-cargo-cache-input"
FINAL_FORMAT = "iroha-sumeragi-v2-cargo-cache-final"
RETAINED_FORMAT = "iroha-sumeragi-v2-retained-release-evidence"
RETAINED_PRIVATE_PROVENANCE_FORMAT = (
    "iroha-sumeragi-v2-bootstrap-private-retained-provenance"
)
SDK_SOURCE_FORMAT = "iroha-sumeragi-v2-sdk-dependency-sources"
SDK_BUNDLE_FORMAT = "iroha-sumeragi-v2-sdk-dependency-bundle"
SDK_WORK_FORMAT = "iroha-sumeragi-v2-sdk-dependency-work-final"
SDK_BUNDLE_ARCHIVE_ID = "release-sdk-dependencies.bundle.v1"
SDK_SOURCE_INVENTORY_FORMAT = (
    "iroha-sumeragi-v2-sdk-dependency-source-inventory"
)
SDK_GRADLE_DISTRIBUTION_URL = (
    "https://services.gradle.org/distributions/gradle-9.3.0-bin.zip"
)
# Gradle PathAssembler uses the base-36 unsigned MD5 of the distribution URI.
# This is the exact key for SDK_GRADLE_DISTRIBUTION_URL; accepting an arbitrary
# cache directory would let a wrapper select bytes outside the authenticated
# distribution closure.
SDK_GRADLE_WRAPPER_CACHE_KEY = "79n14ral3mx1ozqr3csh2u872"
SDK_GRADLE_LAUNCHER_ARCHIVE_NAME = (
    "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
    f"{SDK_GRADLE_WRAPPER_CACHE_KEY}/gradle-9.3.0/bin/gradle"
)
SDK_OBJECT_ID_RE = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
VALIDATOR_FAILURE_FORMAT = "iroha-sumeragi-v2-receipt-validation-failure"
VALIDATOR_DIAGNOSTIC_BYTES = 64 * 1024
VALIDATOR_FAILURE_MARKER_BYTES = 64 * 1024
VALIDATOR_OPTION_ORDER = (
    "--candidate-identity",
    "--sealed-identity",
    "--release-root",
    "--bootstrap-completion",
    "--bootstrap-evidence-dir",
    "--bootstrap-identity",
    "--bootstrap-attestation",
    "--bootstrap-transcript",
    "--expected-bootstrap-completion-sha256",
    "--bootstrap-candidate-root",
    "--bootstrap-runner",
    "--signature-attestation",
    "--signature-transcript",
    "--signature-raw-commit",
    "--signature-cargo-lock",
    "--signature-allowed-signers",
    "--signature-revocation",
    "--signature-git",
    "--signature-ssh-keygen",
    "--expected-git-sha256",
    "--expected-ssh-keygen-sha256",
    "--expected-allowed-signers-sha256",
    "--expected-revocation-sha256",
    "--expected-signer-fingerprint",
    "--corridor-completion",
    "--formal-completion",
    "--seed-completion",
    "--chaos-completion",
    "--taira-completion",
    "--g4p-completion",
    "--g12-seed-completion",
    "--g12-fault-soak-completion",
    "--scaling-evidence-manifest",
    "--sdk-dependency-archive",
    "--sdk-dependency-input-inventory",
    "--sdk-dependency-final-work-inventory",
    "--runtime-tool-probe-manifest",
    "--runtime-tool-probe-result",
    "--expected-scaling-trial-harness-sha256",
    "--expected-scaling-configuration-sha256",
    "--expected-scaling-irohad-sha256",
    "--expected-scaling-iroha-cli-sha256",
    "--repository-root",
    "--output",
    "--verify-existing",
    "--validation-ack",
    "--source-manifest-sha256",
)
VALIDATOR_PATH_OPTIONS = frozenset(
    {
        "--candidate-identity",
        "--sealed-identity",
        "--release-root",
        "--bootstrap-completion",
        "--bootstrap-evidence-dir",
        "--bootstrap-identity",
        "--bootstrap-attestation",
        "--bootstrap-transcript",
        "--bootstrap-candidate-root",
        "--bootstrap-runner",
        "--signature-attestation",
        "--signature-transcript",
        "--signature-raw-commit",
        "--signature-cargo-lock",
        "--signature-allowed-signers",
        "--signature-revocation",
        "--signature-git",
        "--signature-ssh-keygen",
        "--corridor-completion",
        "--formal-completion",
        "--seed-completion",
        "--chaos-completion",
        "--taira-completion",
        "--g4p-completion",
        "--g12-seed-completion",
        "--g12-fault-soak-completion",
        "--scaling-evidence-manifest",
        "--sdk-dependency-archive",
        "--sdk-dependency-input-inventory",
        "--sdk-dependency-final-work-inventory",
        "--runtime-tool-probe-manifest",
        "--runtime-tool-probe-result",
        "--repository-root",
        "--output",
        "--validation-ack",
    }
)
IDENTITY_FIELDS = (
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
VALIDATION_ACK_COMPONENT_FILES = (
    "copy_sumeragi_v2_release_cargo_cache_validation_ack.py",
)
VALIDATION_ACK_COMPONENT_SHA256 = (
    "1a69a1cb8b1f0b0bb580762e8b569200ac9b9c85d463bf23913d3ed892d5004d"
)
VALIDATION_ACK_COMPONENT_MAXIMUM_BYTES = 512 * 1024


class CacheCopyError(RuntimeError):
    """The inherited Cargo cache is unsafe, unstable, or too large."""


def _unchanged(before: os.stat_result, after: os.stat_result) -> bool:
    return all(
        getattr(before, field) == getattr(after, field)
        for field in IDENTITY_FIELDS
    )


def _same_directory(before: os.stat_result, after: os.stat_result) -> bool:
    return (
        stat.S_ISDIR(before.st_mode)
        and stat.S_ISDIR(after.st_mode)
        and (before.st_dev, before.st_ino, before.st_mode, before.st_uid, before.st_gid)
        == (after.st_dev, after.st_ino, after.st_mode, after.st_uid, after.st_gid)
    )


def _same_directory_inode(before: os.stat_result, after: os.stat_result) -> bool:
    return (
        stat.S_ISDIR(before.st_mode)
        and stat.S_ISDIR(after.st_mode)
        and (before.st_dev, before.st_ino, before.st_uid, before.st_gid)
        == (after.st_dev, after.st_ino, after.st_uid, after.st_gid)
    )


def _contained(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def _overlap(left: Path, right: Path) -> bool:
    return left == right or _contained(left, right) or _contained(right, left)


def _bounded_relative(relative: str) -> None:
    path = Path(relative)
    if len(path.parts) > MAXIMUM_DEPTH or len(relative.encode("utf-8")) > MAXIMUM_PATH_BYTES:
        raise CacheCopyError("cache input path exceeds its structural limit")


def _normalized_absolute(path: Path, label: str) -> None:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise CacheCopyError(f"{label} must be absolute and normalized")


_DIRECTORY_FLAGS = (
    os.O_RDONLY
    | getattr(os, "O_DIRECTORY", 0)
    | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)


def _open_directory(path: Path, label: str) -> tuple[int, os.stat_result]:
    _normalized_absolute(path, label)
    descriptor = os.open(Path(path.anchor), _DIRECTORY_FLAGS)
    try:
        for part in path.parts[1:]:
            before = _entry_stat(descriptor, part, label)
            child = os.open(part, _DIRECTORY_FLAGS, dir_fd=descriptor)
            opened = os.fstat(child)
            if stat.S_ISLNK(before.st_mode) or not _unchanged(before, opened):
                os.close(child)
                raise CacheCopyError(f"{label} changed while opened")
            os.close(descriptor)
            descriptor = child
    except OSError as error:
        os.close(descriptor)
        raise CacheCopyError(f"could not safely open {label}: {error}") from error
    opened = os.fstat(descriptor)
    if not stat.S_ISDIR(opened.st_mode):
        os.close(descriptor)
        raise CacheCopyError(f"{label} changed while opened")
    return descriptor, opened


def _directory_entries(descriptor: int, label: str) -> tuple[str, ...]:
    names: list[str] = []
    try:
        with os.scandir(descriptor) as entries:
            for entry in entries:
                names.append(entry.name)
                if len(names) > MAXIMUM_RECORDS:
                    raise CacheCopyError(f"{label} contains too many entries")
    except OSError as error:
        raise CacheCopyError(f"could not enumerate {label}: {error}") from error
    return tuple(sorted(names))


def _entry_stat(parent_fd: int, name: str, label: str) -> os.stat_result:
    try:
        return os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except OSError as error:
        raise CacheCopyError(f"{label} is unavailable: {error}") from error


def _optional_entry_stat(parent_fd: int, name: str, label: str) -> os.stat_result | None:
    try:
        return os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return None
    except OSError as error:
        raise CacheCopyError(f"{label} is unavailable: {error}") from error


def _rename_with_flags_at(parent_fd: int, source: str, destination: str, flags: int) -> None:
    library = ctypes.CDLL(None, use_errno=True)
    if sys.platform == "darwin":
        rename = library.renameatx_np
    elif sys.platform.startswith("linux") and hasattr(library, "renameat2"):
        rename = library.renameat2
    else:
        raise OSError(errno.ENOTSUP, "atomic flagged rename is unavailable")
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(parent_fd, os.fsencode(source), parent_fd, os.fsencode(destination), flags) != 0:
        number = ctypes.get_errno()
        raise OSError(number, os.strerror(number), destination)


def _rename_noreplace_at(parent_fd: int, source: str, destination: str) -> None:
    _rename_with_flags_at(parent_fd, source, destination, 4 if sys.platform == "darwin" else 1)


def _rename_swap_at(parent_fd: int, left: str, right: str) -> None:
    _rename_with_flags_at(parent_fd, left, right, 2)


def _owned_remove_entry(parent_fd: int, name: str, identity: tuple[int, int], label: str) -> bool:
    """Atomically quarantine one exact identity; never unlink an active name."""

    current = _optional_entry_stat(parent_fd, name, label)
    if current is None or (current.st_dev, current.st_ino) != identity:
        return False
    quarantine = f".owned-quarantine.{secrets.token_hex(16)}"
    try:
        _rename_noreplace_at(parent_fd, name, quarantine)
    except FileNotFoundError:
        return False
    moved = _entry_stat(parent_fd, quarantine, label)
    moved_identity = (moved.st_dev, moved.st_ino)
    if moved_identity != identity:
        try:
            _rename_noreplace_at(parent_fd, quarantine, name)
        except OSError as error:
            raise CacheCopyError(f"{label} replacement retained as {quarantine}") from error
        raise CacheCopyError(f"{label} was replaced before quarantine")
    if _optional_entry_stat(parent_fd, name, f"{label} replacement") is not None:
        raise CacheCopyError(f"{label} replacement retained after quarantine")
    return True


def _revalidate_entry(
    parent_fd: int, name: str, expected: os.stat_result, label: str
) -> None:
    if not _unchanged(expected, _entry_stat(parent_fd, name, label)):
        raise CacheCopyError(f"{label} was replaced during traversal")


def _revalidate_path(path: Path, expected: os.stat_result, label: str) -> None:
    try:
        after = path.lstat()
    except OSError as error:
        raise CacheCopyError(f"{label} disappeared during traversal") from error
    if not _unchanged(expected, after):
        raise CacheCopyError(f"{label} was replaced during traversal")


def _revalidate_directory_path(path: Path, expected: os.stat_result, label: str) -> None:
    try:
        after = path.lstat()
    except OSError as error:
        raise CacheCopyError(f"{label} disappeared during traversal") from error
    if stat.S_ISLNK(after.st_mode) or not _same_directory(expected, after):
        raise CacheCopyError(f"{label} was replaced during traversal")


def _safe_target_parts(parent_parts: tuple[str, ...], target: str, label: str) -> tuple[str, ...]:
    rendered = PurePosixPath(target)
    if rendered.is_absolute():
        raise CacheCopyError(f"cache symlink has an absolute target: {label}")
    parts = list(parent_parts)
    for part in rendered.parts:
        if part in {"", "."}:
            continue
        if part == "..":
            if not parts:
                raise CacheCopyError(f"cache symlink escapes its cache root: {label}")
            parts.pop()
        else:
            parts.append(part)
    if not parts:
        raise CacheCopyError(f"cache symlink target is unavailable: {label}")
    return tuple(parts)


def _require_target(root_fd: int, parts: tuple[str, ...], label: str) -> None:
    descriptor = os.dup(root_fd)
    try:
        for index, part in enumerate(parts):
            metadata = _entry_stat(descriptor, part, label)
            if stat.S_ISLNK(metadata.st_mode):
                raise CacheCopyError(f"cache symlink target has a symlink component: {label}")
            if index + 1 == len(parts):
                if not (stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode)):
                    raise CacheCopyError(f"cache symlink target is special: {label}")
                return
            if not stat.S_ISDIR(metadata.st_mode):
                raise CacheCopyError(f"cache symlink target is unavailable: {label}")
            child = os.open(part, _DIRECTORY_FLAGS, dir_fd=descriptor)
            opened = os.fstat(child)
            if not _unchanged(metadata, opened):
                os.close(child)
                raise CacheCopyError(f"cache symlink target changed: {label}")
            os.close(descriptor)
            descriptor = child
    finally:
        os.close(descriptor)


def _validate_symlink(
    parent_fd: int,
    name: str,
    before: os.stat_result,
    root_fd: int,
    parent_parts: tuple[str, ...],
    label: str,
) -> str:
    try:
        target = os.readlink(name, dir_fd=parent_fd)
    except OSError as error:
        raise CacheCopyError(f"could not read cache symlink {label}: {error}") from error
    _require_target(root_fd, _safe_target_parts(parent_parts, target, label), label)
    after = _entry_stat(parent_fd, name, label)
    if (
        not stat.S_ISLNK(after.st_mode)
        or not _unchanged(before, after)
        or os.readlink(name, dir_fd=parent_fd) != target
    ):
        raise CacheCopyError(f"cache symlink changed while inspected: {label}")
    return target


def _preflight_directory(
    descriptor: int,
    root_fd: int,
    relative: str,
    relative_parts: tuple[str, ...],
    budget: dict[str, int],
) -> None:
    _bounded_relative(relative)
    before = os.fstat(descriptor)
    budget["records"] += 1
    if budget["records"] > MAXIMUM_RECORDS:
        raise CacheCopyError("cache input contains too many entries")
    names = _directory_entries(descriptor, f"cache directory {relative}")
    for name in names:
        child_label = f"cache entry {relative}/{name}"
        metadata = _entry_stat(descriptor, name, child_label)
        if stat.S_ISDIR(metadata.st_mode):
            child_fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=descriptor)
            try:
                if not _unchanged(metadata, os.fstat(child_fd)):
                    raise CacheCopyError(f"{child_label} changed while opened")
                _preflight_directory(child_fd, root_fd, f"{relative}/{name}", (*relative_parts, name), budget)
            finally:
                os.close(child_fd)
            _revalidate_entry(descriptor, name, metadata, child_label)
            continue
        budget["records"] += 1
        if budget["records"] > MAXIMUM_RECORDS:
            raise CacheCopyError("cache input contains too many entries")
        _bounded_relative(f"{relative}/{name}")
        if stat.S_ISREG(metadata.st_mode):
            if metadata.st_size > MAXIMUM_FILE_BYTES:
                raise CacheCopyError(
                    f"cache input file exceeds its size limit: {child_label}"
                )
            budget["bytes"] += metadata.st_size
            if budget["bytes"] > MAXIMUM_TOTAL_BYTES:
                raise CacheCopyError("cache input exceeds its total byte limit")
        elif stat.S_ISLNK(metadata.st_mode):
            _validate_symlink(descriptor, name, metadata, root_fd, relative_parts, child_label)
        else:
            raise CacheCopyError(f"cache entry is a forbidden special file: {child_label}")
    if _directory_entries(descriptor, f"cache directory {relative}") != names or not _unchanged(before, os.fstat(descriptor)):
        raise CacheCopyError(f"cache directory changed during preflight: {relative}")


def _copy_regular(
    source_parent_fd: int,
    destination_parent_fd: int,
    name: str,
    relative: str,
    before: os.stat_result,
    budget: dict[str, int],
    *,
    destination_name: str | None = None,
) -> dict[str, object]:
    if before.st_size > MAXIMUM_FILE_BYTES:
        raise CacheCopyError(f"cache input file exceeds its size limit: {relative}")
    budget["bytes"] += before.st_size
    if budget["bytes"] > MAXIMUM_TOTAL_BYTES:
        raise CacheCopyError("cache input exceeds its total byte limit")
    source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        source_flags |= os.O_NOFOLLOW
    try:
        source_fd = os.open(name, source_flags, dir_fd=source_parent_fd)
    except OSError as error:
        raise CacheCopyError(f"could not safely open cache file {relative}") from error
    destination_fd: int | None = None
    digest = hashlib.sha256()
    size = 0
    try:
        opened = os.fstat(source_fd)
        if not stat.S_ISREG(opened.st_mode) or not _unchanged(before, opened):
            raise CacheCopyError(f"cache file changed while opened: {relative}")
        destination_entry = name if destination_name is None else destination_name
        destination_fd = os.open(
            destination_entry,
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o600, dir_fd=destination_parent_fd,
        )
        while True:
            block = os.read(source_fd, 1024 * 1024)
            if not block:
                break
            digest.update(block)
            size += len(block)
            if size > MAXIMUM_FILE_BYTES or budget["bytes"] - before.st_size + size > MAXIMUM_TOTAL_BYTES:
                raise CacheCopyError(f"cache file grew beyond its size bound: {relative}")
            view = memoryview(block)
            while view:
                written = os.write(destination_fd, view)
                if written <= 0:
                    raise CacheCopyError(f"could not finish copying {relative}")
                view = view[written:]
        source_after = os.fstat(source_fd)
        if size != opened.st_size or not _unchanged(opened, source_after):
            raise CacheCopyError(f"cache file changed while copied: {relative}")
        _revalidate_entry(source_parent_fd, name, source_after, f"cache file {relative}")
        destination_mode = (
            0o700 if stat.S_IMODE(opened.st_mode) & 0o111 else 0o600
        )
        os.fchmod(destination_fd, destination_mode)
        os.fsync(destination_fd)
        copied = os.fstat(destination_fd)
        if (
            not stat.S_ISREG(copied.st_mode)
            or copied.st_nlink != 1
            or copied.st_size != size
            or (copied.st_dev, copied.st_ino) == (opened.st_dev, opened.st_ino)
        ):
            raise CacheCopyError(f"cache copy is not inode-independent: {relative}")
        _revalidate_entry(
            destination_parent_fd, destination_entry, copied,
            f"cache copy {relative}",
        )
    finally:
        if destination_fd is not None:
            os.close(destination_fd)
        os.close(source_fd)
    return {
        "path": relative,
        "kind": "file",
        "source_device": opened.st_dev,
        "source_inode": opened.st_ino,
        "source_mode": format(stat.S_IMODE(opened.st_mode), "04o"),
        "destination_device": copied.st_dev,
        "destination_inode": copied.st_ino,
        "destination_mode": format(stat.S_IMODE(copied.st_mode), "04o"),
        "size": size,
        "sha256": digest.hexdigest(),
    }


def _copy_symlink(
    source_parent_fd: int,
    destination_parent_fd: int,
    name: str,
    relative: str,
    before: os.stat_result,
    root_fd: int,
    parent_parts: tuple[str, ...],
) -> dict[str, object]:
    target = _validate_symlink(source_parent_fd, name, before, root_fd, parent_parts, relative)
    stage_name: str | None = None
    for _ in range(32):
        candidate = f".copy-stage.{secrets.token_hex(16)}"
        try:
            os.symlink(target, candidate, dir_fd=destination_parent_fd)
            stage_name = candidate; break
        except FileExistsError:
            continue
    if stage_name is None:
        raise CacheCopyError(f"could not allocate cache symlink stage: {relative}")
    staged = _entry_stat(destination_parent_fd, stage_name, f"cache symlink stage {relative}")
    try:
        _rename_noreplace_at(destination_parent_fd, stage_name, name)
    except BaseException:
        _owned_remove_entry(destination_parent_fd, stage_name, (staged.st_dev, staged.st_ino), f"partial cache symlink {relative}")
        raise
    copied = _entry_stat(destination_parent_fd, name, f"cache symlink copy {relative}")
    if not stat.S_ISLNK(copied.st_mode) or (copied.st_dev, copied.st_ino) != (staged.st_dev, staged.st_ino) or os.readlink(name, dir_fd=destination_parent_fd) != target:
        raise CacheCopyError(f"cache symlink copy is not exact: {relative}")
    _revalidate_entry(destination_parent_fd, name, copied, f"cache symlink copy {relative}")
    return {
        "path": relative,
        "kind": "symlink",
        "source_mode": format(stat.S_IMODE(before.st_mode), "04o"),
        "destination_mode": format(stat.S_IMODE(copied.st_mode), "04o"),
        "target": target,
    }


def _copy_directory(
    source_fd: int,
    destination_parent_fd: int,
    destination_name: str,
    relative: str,
    root_fd: int,
    relative_parts: tuple[str, ...],
    records: list[dict[str, object]],
    budget: dict[str, int],
) -> None:
    _bounded_relative(relative)
    before = os.fstat(source_fd)
    budget["records"] += 1
    if budget["records"] > MAXIMUM_RECORDS:
        raise CacheCopyError("cache input contains too many entries")
    names = _directory_entries(source_fd, f"cache directory {relative}")
    stage_name: str | None = None
    for _ in range(32):
        candidate = f".copy-stage.{secrets.token_hex(16)}"
        try:
            os.mkdir(candidate, mode=0o700, dir_fd=destination_parent_fd)
            stage_name = candidate; break
        except FileExistsError:
            continue
    if stage_name is None:
        raise CacheCopyError(f"could not allocate cache directory stage: {relative}")
    created = _entry_stat(destination_parent_fd, stage_name, f"new cache directory {relative}")
    destination_fd = os.open(stage_name, _DIRECTORY_FLAGS, dir_fd=destination_parent_fd)
    copied = os.fstat(destination_fd)
    if (
        stat.S_ISLNK(copied.st_mode)
        or not stat.S_ISDIR(copied.st_mode)
        or not _same_directory_inode(created, copied)
        or copied.st_uid != os.geteuid()
    ):
        os.close(destination_fd)
        _owned_remove_entry(destination_parent_fd, stage_name, (created.st_dev, created.st_ino), f"partial cache directory {relative}")
        raise CacheCopyError(f"private cache directory is unsafe: {relative}")
    os.fchmod(destination_fd, 0o700)
    copied = os.fstat(destination_fd)
    try:
        _rename_noreplace_at(destination_parent_fd, stage_name, destination_name)
    except BaseException:
        os.close(destination_fd)
        _owned_remove_entry(destination_parent_fd, stage_name, (copied.st_dev, copied.st_ino), f"partial cache directory {relative}")
        raise
    published = _entry_stat(destination_parent_fd, destination_name, f"published cache directory {relative}")
    if not _same_directory_inode(copied, published):
        os.close(destination_fd)
        _owned_remove_entry(destination_parent_fd, destination_name, (copied.st_dev, copied.st_ino), f"partial cache directory {relative}")
        raise CacheCopyError(f"private cache directory changed at publication: {relative}")
    root_record = {
        "path": relative,
        "kind": "directory",
        "source_device": before.st_dev,
        "source_inode": before.st_ino,
        "source_mode": format(stat.S_IMODE(before.st_mode), "04o"),
        "destination_device": copied.st_dev,
        "destination_inode": copied.st_ino,
        "destination_mode": "0700",
    }
    records.append(root_record)
    try:
        for name in names:
            entry_relative = f"{relative}/{name}"
            _bounded_relative(entry_relative)
            entry_before = _entry_stat(source_fd, name, f"cache entry {entry_relative}")
            if stat.S_ISDIR(entry_before.st_mode):
                child_fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=source_fd)
                try:
                    if not _unchanged(entry_before, os.fstat(child_fd)):
                        raise CacheCopyError(f"cache directory changed while opened: {entry_relative}")
                    _copy_directory(child_fd, destination_fd, name, entry_relative, root_fd, (*relative_parts, name), records, budget)
                finally:
                    os.close(child_fd)
                _revalidate_entry(source_fd, name, entry_before, f"cache directory {entry_relative}")
            elif stat.S_ISREG(entry_before.st_mode):
                budget["records"] += 1
                if budget["records"] > MAXIMUM_RECORDS:
                    raise CacheCopyError("cache input contains too many entries")
                records.append(_copy_regular(source_fd, destination_fd, name, entry_relative, entry_before, budget))
            elif stat.S_ISLNK(entry_before.st_mode):
                budget["records"] += 1
                if budget["records"] > MAXIMUM_RECORDS:
                    raise CacheCopyError("cache input contains too many entries")
                records.append(_copy_symlink(source_fd, destination_fd, name, entry_relative, entry_before, root_fd, relative_parts))
            else:
                raise CacheCopyError(f"cache entry is a forbidden special file: {entry_relative}")
        if _directory_entries(source_fd, f"cache directory {relative}") != names or not _unchanged(before, os.fstat(source_fd)):
            raise CacheCopyError(f"cache directory changed while copied: {relative}")
        destination_after = _entry_stat(destination_parent_fd, destination_name, f"cache directory copy {relative}")
        if not _same_directory_inode(copied, destination_after) or destination_after.st_uid != os.geteuid() or stat.S_IMODE(destination_after.st_mode) != 0o700:
            raise CacheCopyError(f"cache directory copy changed: {relative}")
    except BaseException:
        os.close(destination_fd)
        destination_fd = -1
        _owned_remove_entry(
            destination_parent_fd, destination_name,
            (copied.st_dev, copied.st_ino), f"partial cache directory {relative}",
        )
        if root_record in records:
            records.remove(root_record)
        raise
    finally:
        if destination_fd >= 0:
            os.close(destination_fd)


def _canonical_payload(document: dict[str, object]) -> bytes:
    payload = (
        json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()
    if len(payload) > 256 * 1024 * 1024:
        raise CacheCopyError("Cargo cache inventory exceeds its size limit")
    return payload


def _validator_invocation_value_sha256(kind: str, value: str | bool) -> str:
    payload = json.dumps(
        {"kind": kind, "value": value},
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def _validate_validator_invocation(
    value: object,
    *,
    expected_values: dict[str, tuple[str, str | bool]],
) -> None:
    """Independently recompute and authenticate a validator invocation digest."""

    if not isinstance(value, dict) or set(value) != {
        "profile",
        "operation",
        "python_flags",
        "validator",
        "ordered_options",
        "invocation_sha256",
    }:
        raise CacheCopyError("receipt validator invocation binding is malformed")
    options = value["ordered_options"]
    if (
        value["profile"] != "release"
        or value["operation"] != "verify-existing-and-ack"
        or value["python_flags"] != ["-I", "-S"]
        or value["validator"] != "protected:validate-receipt.py"
        or not isinstance(options, list)
        or len(options) != len(VALIDATOR_OPTION_ORDER)
        or not isinstance(value["invocation_sha256"], str)
        or re.fullmatch(r"[0-9a-f]{64}", value["invocation_sha256"]) is None
    ):
        raise CacheCopyError("receipt validator invocation contract is not exact")
    if set(expected_values) != set(VALIDATOR_OPTION_ORDER):
        raise CacheCopyError(
            "receipt validator invocation reconstruction is incomplete"
        )
    for expected_name, binding in zip(VALIDATOR_OPTION_ORDER, options):
        expected_kind = (
            "flag"
            if expected_name == "--verify-existing"
            else "path"
            if expected_name in VALIDATOR_PATH_OPTIONS
            else "text"
        )
        if (
            not isinstance(binding, dict)
            or set(binding)
            != {"name", "value_kind", "normalized_value_sha256"}
            or binding["name"] != expected_name
            or binding["value_kind"] != expected_kind
            or not isinstance(binding["normalized_value_sha256"], str)
            or re.fullmatch(r"[0-9a-f]{64}", binding["normalized_value_sha256"])
            is None
        ):
            raise CacheCopyError("receipt validator ordered option binding is not exact")
        known = expected_values.get(expected_name)
        if known is None:
            raise CacheCopyError(
                "receipt validator invocation reconstruction is incomplete"
            )
        kind, normalized = known
        if (
            (kind == "flag" and normalized is not True)
            or (kind in {"path", "text"} and not isinstance(normalized, str))
            or (
                kind == "path"
                and isinstance(normalized, str)
                and normalized != os.path.abspath(os.path.normpath(normalized))
            )
        ):
            raise CacheCopyError(
                "receipt validator reconstructed option value is not canonical"
            )
        if (
            kind != expected_kind
            or binding["normalized_value_sha256"]
            != _validator_invocation_value_sha256(kind, normalized)
        ):
            raise CacheCopyError(
                "receipt validator normalized option value is not exact"
            )
    invocation = {
        "profile": value["profile"],
        "operation": value["operation"],
        "python_flags": value["python_flags"],
        "validator": value["validator"],
        "ordered_options": options,
    }
    payload = json.dumps(
        invocation,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    if hashlib.sha256(payload).hexdigest() != value["invocation_sha256"]:
        raise CacheCopyError("receipt validator invocation digest changed")


def _publish_inventory(
    inventory_path: Path, payload: bytes
) -> tuple[os.stat_result, os.stat_result]:
    parent_fd, parent_identity = _open_directory(
        inventory_path.parent, "inventory parent directory"
    )
    if (
        parent_identity.st_uid != os.geteuid()
        or stat.S_IMODE(parent_identity.st_mode) != 0o700
    ):
        os.close(parent_fd)
        raise CacheCopyError("inventory parent must be owner-owned with mode 0700")
    descriptor: int | None = None
    temporary: str | None = None
    published: os.stat_result | None = None
    committed = False
    try:
        for _ in range(32):
            temporary = f".{inventory_path.name}.{secrets.token_hex(16)}"
            try:
                descriptor = os.open(
                    temporary,
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL
                    | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
                    0o600,
                    dir_fd=parent_fd,
                )
                break
            except FileExistsError:
                temporary = None
        if descriptor is None or temporary is None:
            raise CacheCopyError("could not allocate a private inventory temporary")
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise CacheCopyError("could not finish writing cache inventory")
            view = view[written:]
        os.fchmod(descriptor, 0o400)
        os.fsync(descriptor)
        published = os.fstat(descriptor)
        os.close(descriptor)
        descriptor = None
        _rename_noreplace_at(parent_fd, temporary, inventory_path.name)
        committed = True
        temporary = None
        final = _entry_stat(parent_fd, inventory_path.name, "published cache inventory")
        if (
            not stat.S_ISREG(final.st_mode)
            or final.st_uid != os.geteuid()
            or final.st_nlink != 1
            or stat.S_IMODE(final.st_mode) != 0o400
            or (final.st_dev, final.st_ino) != (published.st_dev, published.st_ino)
        ):
            raise CacheCopyError("published cache inventory metadata is unsafe")
        os.fsync(parent_fd)
        _revalidate_directory_path(inventory_path.parent, parent_identity, "inventory parent directory")
        return final, parent_identity
    except BaseException:
        if committed and published is not None:
            _owned_remove_entry(
                parent_fd, inventory_path.name,
                (published.st_dev, published.st_ino), "published cache inventory",
            )
        raise
    finally:
        if descriptor is not None:
            os.close(descriptor)
        if temporary is not None and published is not None:
            _owned_remove_entry(
                parent_fd, temporary, (published.st_dev, published.st_ino),
                "inventory publication temporary",
            )
        os.close(parent_fd)


def _remove_published(
    path: Path, identity: tuple[os.stat_result, os.stat_result] | None
) -> None:
    if identity is None:
        return
    file_identity, parent_identity = identity
    parent_fd, opened_parent = _open_directory(path.parent, "publication parent")
    try:
        if not _same_directory(parent_identity, opened_parent):
            return
        _owned_remove_entry(
            parent_fd, path.name, (file_identity.st_dev, file_identity.st_ino),
            "published file",
        )
    finally:
        os.close(parent_fd)


def _remove_tree_at(
    parent_fd: int, name: str, expected: os.stat_result, label: str
) -> None:
    _owned_remove_entry(parent_fd, name, (expected.st_dev, expected.st_ino), label)


def _remove_partial_roots(
    cargo_home: Path,
    cargo_home_identity: os.stat_result,
    created_roots: list[tuple[str, os.stat_result]],
) -> None:
    try:
        current_home = cargo_home.lstat()
    except FileNotFoundError:
        return
    if not _same_directory(cargo_home_identity, current_home):
        return
    home_fd, opened_home = _open_directory(cargo_home, "private Cargo home cleanup root")
    try:
        if not _same_directory(cargo_home_identity, opened_home):
            return
        for root_name, root_identity in reversed(created_roots):
            try:
                _remove_tree_at(home_fd, root_name, root_identity, root_name)
            except FileNotFoundError:
                continue
    finally:
        os.close(home_fd)


def _snapshot_regular(
    parent_fd: int,
    name: str,
    relative: str,
    before: os.stat_result,
    budget: dict[str, int],
) -> dict[str, object]:
    if (
        before.st_uid != os.geteuid()
        or before.st_nlink != 1
        or stat.S_IMODE(before.st_mode) & 0o022
        or before.st_size > MAXIMUM_FILE_BYTES
    ):
        raise CacheCopyError(f"private cache file metadata is unsafe: {relative}")
    budget["bytes"] += before.st_size
    if budget["bytes"] > MAXIMUM_TOTAL_BYTES:
        raise CacheCopyError("private cache exceeds its total byte limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, dir_fd=parent_fd)
    digest = hashlib.sha256()
    total = 0
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode) or not _unchanged(before, opened):
            raise CacheCopyError(f"private cache file changed while opened: {relative}")
        while True:
            block = os.read(descriptor, 1024 * 1024)
            if not block:
                break
            digest.update(block)
            total += len(block)
            if total > MAXIMUM_FILE_BYTES or budget["bytes"] - before.st_size + total > MAXIMUM_TOTAL_BYTES:
                raise CacheCopyError(f"private cache file exceeds its size limit: {relative}")
        after = os.fstat(descriptor)
        if total != opened.st_size or not _unchanged(opened, after):
            raise CacheCopyError(f"private cache file changed while read: {relative}")
        _revalidate_entry(parent_fd, name, after, f"private cache file {relative}")
    finally:
        os.close(descriptor)
    return {
        "path": relative,
        "kind": "file",
        "device": opened.st_dev,
        "inode": opened.st_ino,
        "mode": format(stat.S_IMODE(opened.st_mode), "04o"),
        "size": total,
        "sha256": digest.hexdigest(),
    }


def _snapshot_directory(
    root_fd: int,
    directory_fd: int,
    relative_directory: str | None,
    relative_parts: tuple[str, ...],
    records: list[dict[str, object]],
    budget: dict[str, int],
) -> None:
    before = os.fstat(directory_fd)
    if (
        not stat.S_ISDIR(before.st_mode)
        or before.st_uid != os.geteuid()
        or stat.S_IMODE(before.st_mode) & 0o022
    ):
        raise CacheCopyError(f"private cache directory metadata is unsafe: {relative_directory or '.'}")
    names = _directory_entries(directory_fd, f"private cache directory {relative_directory or '.'}")
    for name in names:
        relative = name if relative_directory is None else f"{relative_directory}/{name}"
        _bounded_relative(relative)
        budget["records"] += 1
        if budget["records"] > MAXIMUM_RECORDS:
            raise CacheCopyError("private cache contains too many entries")
        metadata = _entry_stat(directory_fd, name, f"private cache entry {relative}")
        mode = stat.S_IMODE(metadata.st_mode)
        if stat.S_ISDIR(metadata.st_mode):
            records.append(
                {
                    "path": relative,
                    "kind": "directory",
                    "device": metadata.st_dev,
                    "inode": metadata.st_ino,
                    "mode": format(mode, "04o"),
                }
            )
            child_fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=directory_fd)
            try:
                if not _unchanged(metadata, os.fstat(child_fd)):
                    raise CacheCopyError(f"private cache directory changed while opened: {relative}")
                _snapshot_directory(root_fd, child_fd, relative, (*relative_parts, name), records, budget)
            finally:
                os.close(child_fd)
            _revalidate_entry(directory_fd, name, metadata, f"private cache directory {relative}")
        elif stat.S_ISREG(metadata.st_mode):
            records.append(_snapshot_regular(directory_fd, name, relative, metadata, budget))
        elif stat.S_ISLNK(metadata.st_mode):
            if metadata.st_uid != os.geteuid():
                raise CacheCopyError(f"private cache symlink owner is unsafe: {relative}")
            target = _validate_symlink(directory_fd, name, metadata, root_fd, relative_parts, relative)
            records.append(
                {
                    "path": relative,
                    "kind": "symlink",
                    "mode": format(mode, "04o"),
                    "target": target,
                }
            )
        else:
            raise CacheCopyError(f"private cache contains a forbidden special file: {relative}")
    if _directory_entries(directory_fd, f"private cache directory {relative_directory or '.'}") != names or not _unchanged(before, os.fstat(directory_fd)):
        raise CacheCopyError(f"private cache directory changed while read: {relative_directory or '.'}")


def snapshot_cache(cargo_home: Path, inventory_path: Path) -> None:
    """Publish a canonical bounded inventory of every final cache entry."""

    _normalized_absolute(cargo_home, "private Cargo home")
    _normalized_absolute(inventory_path, "Cargo cache final inventory")
    cargo_fd, metadata = _open_directory(cargo_home, "private Cargo home")
    if (
        cargo_home.resolve(strict=True) != cargo_home
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or inventory_path.parent != cargo_home.parent
        or inventory_path.parent.resolve(strict=True) != inventory_path.parent
        or inventory_path.exists()
        or inventory_path.is_symlink()
    ):
        raise CacheCopyError("private Cargo home or final inventory path is unsafe")
    records: list[dict[str, object]] = []
    budget = {"records": 0, "bytes": 0}
    published: tuple[os.stat_result, os.stat_result] | None = None
    try:
        _snapshot_directory(cargo_fd, cargo_fd, None, (), records, budget)
        if any(record["path"] in {"config", "config.toml"} for record in records):
            raise CacheCopyError("private Cargo home contains external configuration")
        document = {
            "format": FINAL_FORMAT,
            "schema_version": 1,
            "cargo_home_path": str(cargo_home),
            "record_count": budget["records"],
            "file_bytes": budget["bytes"],
            "records": sorted(records, key=lambda record: str(record["path"])),
        }
        _revalidate_directory_path(cargo_home, metadata, "private Cargo home")
        published = _publish_inventory(inventory_path, _canonical_payload(document))
        _revalidate_directory_path(cargo_home, metadata, "private Cargo home")
    except BaseException:
        _remove_published(inventory_path, published)
        raise
    finally:
        os.close(cargo_fd)


def copy_cache(source_home: Path, cargo_home: Path, inventory_path: Path) -> None:
    """Copy registry/Git roots and publish their canonical input inventory."""

    _normalized_absolute(source_home, "inherited Cargo home")
    _normalized_absolute(cargo_home, "private Cargo home")
    _normalized_absolute(inventory_path, "Cargo cache input inventory")
    cargo_fd, metadata = _open_directory(cargo_home, "private Cargo home")
    if (
        cargo_home.resolve(strict=True) != cargo_home
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or _directory_entries(cargo_fd, "private Cargo home")
    ):
        raise CacheCopyError("private Cargo home must be empty and owner-only")
    if (
        inventory_path.parent != cargo_home.parent
        or inventory_path.parent.resolve(strict=True) != inventory_path.parent
        or inventory_path.exists()
        or inventory_path.is_symlink()
    ):
        raise CacheCopyError("cache inventory must be a new Cargo-home sibling")
    if _overlap(source_home, cargo_home) or _contained(inventory_path, source_home):
        raise CacheCopyError("source, private Cargo home, and inventory must be disjoint")
    roots: list[str] = []
    source_roots: list[tuple[str, int, os.stat_result]] = []
    source_home_fd: int | None = None
    source_metadata: os.stat_result | None = None
    source_parent_fd: int | None = None
    source_parent_metadata: os.stat_result | None = None
    if source_home.exists() or source_home.is_symlink():
        source_home_fd, source_metadata = _open_directory(source_home, "inherited Cargo home")
        if (
            stat.S_ISLNK(source_metadata.st_mode)
            or not stat.S_ISDIR(source_metadata.st_mode)
            or source_home.resolve(strict=True) != source_home
        ):
            raise CacheCopyError("inherited Cargo home must be a real canonical directory")
        for root_name in ("registry", "git"):
            try:
                root_metadata = os.stat(root_name, dir_fd=source_home_fd, follow_symlinks=False)
            except FileNotFoundError:
                continue
            if stat.S_ISLNK(root_metadata.st_mode) or not stat.S_ISDIR(
                root_metadata.st_mode
            ):
                raise CacheCopyError(
                    f"inherited Cargo cache root is not real: {root_name}"
                )
            root_fd = os.open(root_name, _DIRECTORY_FLAGS, dir_fd=source_home_fd)
            if not _unchanged(root_metadata, os.fstat(root_fd)):
                os.close(root_fd)
                raise CacheCopyError(f"inherited Cargo cache root changed: {root_name}")
            roots.append(root_name)
            source_roots.append((root_name, root_fd, root_metadata))
    else:
        source_parent_fd, source_parent_metadata = _open_directory(
            source_home.parent, "absent inherited Cargo-home parent",
        )
        if source_home.parent.resolve(strict=True) != source_home.parent or _optional_entry_stat(
            source_parent_fd, source_home.name, "absent inherited Cargo home",
        ) is not None:
            raise CacheCopyError("absent inherited Cargo home contract is unsafe")
    preflight = {"records": 0, "bytes": 0}
    for root_name, root_fd, _ in source_roots:
        _preflight_directory(root_fd, root_fd, root_name, (), preflight)
    filesystem = os.statvfs(cargo_home)
    available = filesystem.f_bavail * filesystem.f_frsize
    if preflight["bytes"] + MINIMUM_FREE_BYTES_AFTER_COPY > available:
        raise CacheCopyError("cache input would exhaust release filesystem space")
    records: list[dict[str, object]] = []
    copied = {"records": 0, "bytes": 0}
    created_roots: list[tuple[str, os.stat_result]] = []
    published: tuple[os.stat_result, os.stat_result] | None = None
    try:
        for root_name, root_fd, root_metadata in source_roots:
            _copy_directory(root_fd, cargo_fd, root_name, root_name, root_fd, (), records, copied)
            created_roots.append((root_name, _entry_stat(cargo_fd, root_name, root_name)))
            if source_home_fd is not None:
                _revalidate_entry(source_home_fd, root_name, root_metadata, root_name)
        if copied != preflight:
            raise CacheCopyError("cache input changed between preflight and copy")
        if source_home_fd is not None and source_metadata is not None:
            _revalidate_path(source_home, source_metadata, "inherited Cargo home")
        if source_parent_fd is not None and source_parent_metadata is not None:
            _revalidate_directory_path(source_home.parent, source_parent_metadata, "absent inherited Cargo-home parent")
            if _optional_entry_stat(source_parent_fd, source_home.name, "absent inherited Cargo home") is not None:
                raise CacheCopyError("inherited Cargo home appeared during cache copy")
        _revalidate_directory_path(cargo_home, metadata, "private Cargo home")
        document = {
            "format": INPUT_FORMAT,
            "schema_version": 1,
            "source_cargo_home_disclosure": "withheld",
            "source_read_semantics": "read-only; host filesystem may update access time",
            "cargo_home_path": str(cargo_home),
            "roots": roots,
            "input_record_count": copied["records"],
            "input_file_bytes": copied["bytes"],
            "records": sorted(records, key=lambda record: str(record["path"])),
        }
        published = _publish_inventory(inventory_path, _canonical_payload(document))
        for root_name, _, root_metadata in source_roots:
            assert source_home_fd is not None
            _revalidate_entry(source_home_fd, root_name, root_metadata, root_name)
        if source_home_fd is not None and source_metadata is not None:
            _revalidate_path(source_home, source_metadata, "inherited Cargo home")
        if source_parent_fd is not None and source_parent_metadata is not None:
            _revalidate_directory_path(source_home.parent, source_parent_metadata, "absent inherited Cargo-home parent")
            if _optional_entry_stat(source_parent_fd, source_home.name, "absent inherited Cargo home") is not None:
                raise CacheCopyError("inherited Cargo home appeared during cache publication")
        _revalidate_directory_path(cargo_home, metadata, "private Cargo home")
    except BaseException:
        _remove_published(inventory_path, published)
        _remove_partial_roots(cargo_home, metadata, created_roots)
        raise
    finally:
        for _, root_fd, _ in source_roots:
            os.close(root_fd)
        if source_home_fd is not None:
            os.close(source_home_fd)
        if source_parent_fd is not None:
            os.close(source_parent_fd)
        os.close(cargo_fd)


def verify_cache_sources(
    source_home: Path, cargo_home: Path, inventory_path: Path,
) -> None:
    """Revalidate every copied caller-cache input without disclosing its path."""

    _normalized_absolute(source_home, "inherited Cargo home")
    payload, metadata = _read_regular(inventory_path, "Cargo cache input inventory")
    try:
        document = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError("Cargo cache input inventory is malformed") from error
    keys = {
        "format", "schema_version", "source_cargo_home_disclosure",
        "source_read_semantics", "cargo_home_path", "roots",
        "input_record_count", "input_file_bytes", "records",
    }
    records = document.get("records") if isinstance(document, dict) else None
    roots = document.get("roots") if isinstance(document, dict) else None
    if (
        not isinstance(document, dict) or set(document) != keys
        or document.get("format") != INPUT_FORMAT
        or type(document.get("schema_version")) is not int
        or document["schema_version"] != 1
        or document.get("source_cargo_home_disclosure") != "withheld"
        or document.get("source_read_semantics")
        != "read-only; host filesystem may update access time"
        or not isinstance(document.get("cargo_home_path"), str)
        or not Path(document["cargo_home_path"]).is_absolute()
        or not isinstance(roots, list)
        or any(not isinstance(root, str) for root in roots)
        or len(roots) != len(set(roots))
        or any(root not in {"git", "registry"} for root in roots)
        or not isinstance(records, list)
        or stat.S_IMODE(metadata.st_mode) != 0o400
        or payload != _canonical_payload(document)
    ):
        raise CacheCopyError("Cargo cache source inventory contract is not exact")
    file_bytes = sum(
        record.get("size", 0) for record in records
        if isinstance(record, dict) and record.get("kind") == "file"
    )
    if (
        type(document.get("input_record_count")) is not int
        or document["input_record_count"] != len(records)
        or type(document.get("input_file_bytes")) is not int
        or document["input_file_bytes"] != file_bytes
    ):
        raise CacheCopyError("Cargo cache source inventory accounting is not exact")
    _normalized_absolute(cargo_home, "expected private Cargo home")
    if document["cargo_home_path"] != str(cargo_home):
        raise CacheCopyError("Cargo cache inventory names the wrong private home")
    if cargo_home.parent != inventory_path.parent or _overlap(source_home, cargo_home):
        raise CacheCopyError("Cargo cache source and destination paths are not exact")
    current_roots = []
    if source_home.exists() or source_home.is_symlink():
        source_fd, _ = _open_directory(source_home, "inherited Cargo home")
        try:
            for name in ("registry", "git"):
                observed = _optional_entry_stat(source_fd, name, f"inherited Cargo {name}")
                if observed is not None:
                    if not stat.S_ISDIR(observed.st_mode) or stat.S_ISLNK(observed.st_mode):
                        raise CacheCopyError(f"inherited Cargo cache root changed: {name}")
                    current_roots.append(name)
        finally:
            os.close(source_fd)
    if current_roots != roots:
        raise CacheCopyError("inherited Cargo cache roots changed after private copy")
    _verify_runtime_sources(
        {root: source_home / root for root in roots}, records,
    )
    cargo_fd, cargo_identity = _open_directory(cargo_home, "private Cargo home")
    current: list[dict[str, object]] = []
    budget = {"records": 0, "bytes": 0}
    try:
        _snapshot_directory(cargo_fd, cargo_fd, None, (), current, budget)
    finally:
        os.close(cargo_fd)
    _revalidate_directory_path(cargo_home, cargo_identity, "private Cargo home")
    if any(record.get("path") in {"config", "config.toml"} for record in current):
        raise CacheCopyError("private Cargo home contains external configuration")
    _bind_runtime_destinations(records, current, update=False)


def _hold_regular(path: Path, label: str) -> dict[str, object]:
    parent_fd, parent_identity = _open_directory(path.parent, f"{label} parent")
    try:
        before = _entry_stat(parent_fd, path.name, label)
        if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode) or before.st_uid != os.geteuid() or before.st_nlink != 1 or before.st_size > MAXIMUM_FILE_BYTES:
            raise CacheCopyError(f"retained {label} metadata is unsafe")
        descriptor = os.open(path.name, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0), dir_fd=parent_fd)
        opened = os.fstat(descriptor)
        if not _unchanged(before, opened):
            raise CacheCopyError(f"retained {label} changed while opened")
        data = bytearray()
        while block := os.read(descriptor, 1024 * 1024):
            data.extend(block)
            if len(data) > MAXIMUM_FILE_BYTES:
                raise CacheCopyError(f"retained {label} exceeds its bound")
        after = os.fstat(descriptor)
        if len(data) != opened.st_size or not _unchanged(opened, after):
            raise CacheCopyError(f"retained {label} changed while read")
        _revalidate_entry(parent_fd, path.name, after, label)
    except BaseException:
        os.close(parent_fd)
        if "descriptor" in locals():
            os.close(descriptor)
        raise
    held = {"path": path, "label": label, "parent_fd": parent_fd, "parent_identity": parent_identity, "descriptor": descriptor, "metadata": after, "data": bytes(data)}
    _revalidate_held_regular(held)
    return held


def _revalidate_held_regular(held: dict[str, object]) -> None:
    path, label = held["path"], held["label"]
    descriptor, parent_fd = held["descriptor"], held["parent_fd"]
    metadata, data = held["metadata"], held["data"]
    assert isinstance(path, Path) and isinstance(label, str) and isinstance(descriptor, int) and isinstance(parent_fd, int) and isinstance(metadata, os.stat_result) and isinstance(data, bytes)
    if not _unchanged(metadata, os.fstat(descriptor)) or not _unchanged(metadata, _entry_stat(parent_fd, path.name, label)):
        raise CacheCopyError(f"retained {label} changed while held")
    os.lseek(descriptor, 0, os.SEEK_SET)
    observed = bytearray()
    while block := os.read(descriptor, 1024 * 1024):
        observed.extend(block)
        if len(observed) > MAXIMUM_FILE_BYTES:
            raise CacheCopyError(f"retained {label} exceeds its bound")
    if bytes(observed) != data or not _unchanged(metadata, os.fstat(descriptor)):
        raise CacheCopyError(f"retained {label} bytes changed while held")
    parent_identity = held["parent_identity"]
    assert isinstance(parent_identity, os.stat_result)
    _revalidate_directory_path(path.parent, parent_identity, f"{label} parent")


def _close_held_regular(held: dict[str, object]) -> None:
    os.close(held["descriptor"]); os.close(held["parent_fd"])


def _read_regular(path: Path, label: str) -> tuple[bytes, os.stat_result]:
    held = _hold_regular(path, label)
    try:
        return held["data"], held["metadata"]
    finally:
        _close_held_regular(held)


def _digest_regular(path: Path, label: str) -> tuple[str, int, os.stat_result]:
    data, metadata = _read_regular(path, label)
    return hashlib.sha256(data).hexdigest(), len(data), metadata


def _retained_tree(root: Path, excluded: set[Path]) -> tuple[list[dict[str, object]], int]:
    excluded_names = {path.name for path in excluded if path.parent == root}
    records: list[dict[str, object]] = []
    budget = {"records": 0, "bytes": 0}
    root_fd, root_identity = _open_directory(root, "retained release root")

    def walk(directory_fd: int, relative: str) -> None:
        before = os.fstat(directory_fd)
        names = _directory_entries(directory_fd, f"retained {relative or '.'}")
        for name in names:
            if name.startswith((".owned-quarantine.", ".owned-quiescent.")):
                raise CacheCopyError("retained release root contains a prior cleanup quarantine")
            if not relative and name in excluded_names:
                continue
            child_relative = name if not relative else f"{relative}/{name}"
            _bounded_relative(child_relative)
            metadata = _entry_stat(directory_fd, name, f"retained {child_relative}")
            budget["records"] += 1
            if budget["records"] > MAXIMUM_RECORDS or metadata.st_uid != os.geteuid() or stat.S_ISLNK(metadata.st_mode):
                raise CacheCopyError(f"retained release evidence has an unsafe entry: {child_relative}")
            if stat.S_ISDIR(metadata.st_mode):
                records.append({"path": child_relative, "kind": "directory", "mode": format(stat.S_IMODE(metadata.st_mode), "04o")})
                child_fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=directory_fd)
                try:
                    if not _same_directory(metadata, os.fstat(child_fd)):
                        raise CacheCopyError(f"retained directory changed: {child_relative}")
                    walk(child_fd, child_relative)
                finally:
                    os.close(child_fd)
                _revalidate_entry(directory_fd, name, metadata, f"retained {child_relative}")
            elif stat.S_ISREG(metadata.st_mode):
                item = _snapshot_regular(directory_fd, name, child_relative, metadata, budget)
                records.append({key: item[key] for key in ("path", "kind", "mode", "size", "sha256")})
            else:
                raise CacheCopyError(f"retained release evidence has a special file: {child_relative}")
        if names != _directory_entries(directory_fd, f"retained {relative or '.'}") or not _same_directory(before, os.fstat(directory_fd)):
            raise CacheCopyError(f"retained directory changed while read: {relative or '.'}")

    try:
        walk(root_fd, "")
    finally:
        os.close(root_fd)
    _revalidate_directory_path(root, root_identity, "retained release root")
    return records, budget["bytes"]


def _owned_remove_tree(parent_fd: int, name: str, label: str) -> None:
    expected = _entry_stat(parent_fd, name, label)
    if not stat.S_ISDIR(expected.st_mode) or stat.S_ISLNK(expected.st_mode) or expected.st_uid != os.geteuid():
        raise CacheCopyError(f"refusing to prune unsafe retained entry: {label}")
    if not _owned_remove_entry(parent_fd, name, (expected.st_dev, expected.st_ino), label):
        raise CacheCopyError(f"retained prune root was replaced: {label}")


def _quiescent_remove_tree(root: Path, label: str) -> None:
    """Reclaim one quarantined tree after its owned child has naturally exited."""

    metadata = root.lstat()
    if root.resolve(strict=True) != root or stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != os.geteuid():
        raise CacheCopyError(f"quiescent {label} is unsafe")
    for directory, _, _ in os.walk(root, topdown=True, followlinks=False):
        current = Path(directory)
        observed = current.lstat()
        if (
            stat.S_ISLNK(observed.st_mode)
            or not stat.S_ISDIR(observed.st_mode)
            or observed.st_uid != os.geteuid()
        ):
            raise CacheCopyError(f"quiescent {label} directory is unsafe")
        if stat.S_IMODE(observed.st_mode) & 0o700 != 0o700:
            current.chmod(stat.S_IMODE(observed.st_mode) | 0o700)
    for directory, directories, files in os.walk(root, topdown=False, followlinks=False):
        current = Path(directory)
        for name in files:
            entry = current / name
            observed = entry.lstat()
            if observed.st_uid != os.geteuid() or stat.S_ISDIR(observed.st_mode):
                raise CacheCopyError(f"quiescent {label} entry is unsafe")
            entry.unlink()
        for name in directories:
            entry = current / name
            observed = entry.lstat()
            if observed.st_uid != os.geteuid():
                raise CacheCopyError(f"quiescent {label} directory is unsafe")
            if stat.S_ISLNK(observed.st_mode):
                entry.unlink(); continue
            if not stat.S_ISDIR(observed.st_mode):
                raise CacheCopyError(f"quiescent {label} directory is unsafe")
            entry.rmdir()
    root.rmdir()


def _quiescent_remove_named(
    parent: Path, name: str, label: str, *, require_directory: bool = False,
) -> bool:
    """Physically reclaim one owned entry after its child can no longer mutate it."""

    parent_fd, parent_identity = _open_directory(parent, f"quiescent {label} parent")
    try:
        entry = _optional_entry_stat(parent_fd, name, label)
        if entry is None:
            return False
        is_directory = stat.S_ISDIR(entry.st_mode) and not stat.S_ISLNK(entry.st_mode)
        is_regular = stat.S_ISREG(entry.st_mode) and not stat.S_ISLNK(entry.st_mode)
        if entry.st_uid != os.geteuid() or (require_directory and not is_directory) or (not require_directory and not (is_directory or is_regular)) or (is_regular and entry.st_nlink != 1):
            raise CacheCopyError(f"quiescent {label} is unsafe")
        quarantine = f".owned-quiescent.{secrets.token_hex(16)}"
        _rename_noreplace_at(parent_fd, name, quarantine)
        moved = _entry_stat(parent_fd, quarantine, f"quiescent {label}")
        if (moved.st_dev, moved.st_ino) != (entry.st_dev, entry.st_ino):
            _rename_noreplace_at(parent_fd, quarantine, name)
            raise CacheCopyError(f"quiescent {label} changed before cleanup")
    finally:
        os.close(parent_fd)
    quarantined = parent / quarantine
    if stat.S_ISDIR(moved.st_mode):
        _quiescent_remove_tree(quarantined, label)
    else:
        quarantined.unlink()
    _revalidate_directory_path(parent, parent_identity, f"quiescent {label} parent")
    return True


def cleanup_invocation(base: Path, root: Path, prefix: str) -> None:
    """Identity-delete one exact owner-private direct child of a trusted base."""

    _normalized_absolute(base, "cleanup base")
    _normalized_absolute(root, "cleanup root")
    if root.parent != base or not root.name.startswith(prefix):
        raise CacheCopyError("refusing to remove an unsafe private invocation")
    _quiescent_remove_named(base, root.name, "private invocation", require_directory=True)


def _stable_regular_digest(path: Path, label: str) -> tuple[str, int]:
    """Hash one stable owned regular file without retaining its payload."""

    parent_fd, parent_identity = _open_directory(path.parent, f"{label} parent")
    descriptor: int | None = None
    try:
        before = _entry_stat(parent_fd, path.name, label)
        if (
            not stat.S_ISREG(before.st_mode)
            or stat.S_ISLNK(before.st_mode)
            or before.st_uid != os.geteuid()
            or before.st_nlink != 1
            or before.st_size > MAXIMUM_FILE_BYTES
        ):
            raise CacheCopyError(f"{label} metadata is unsafe")
        descriptor = os.open(
            path.name,
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent_fd,
        )
        opened = os.fstat(descriptor)
        if not _unchanged(before, opened):
            raise CacheCopyError(f"{label} changed while opened")
        digest = hashlib.sha256()
        size = 0
        while block := os.read(descriptor, 1024 * 1024):
            size += len(block)
            if size > MAXIMUM_FILE_BYTES:
                raise CacheCopyError(f"{label} exceeds its size bound")
            digest.update(block)
        after = os.fstat(descriptor)
        if size != opened.st_size or not _unchanged(opened, after):
            raise CacheCopyError(f"{label} changed while hashed")
        _revalidate_entry(parent_fd, path.name, after, label)
        _revalidate_directory_path(path.parent, parent_identity, f"{label} parent")
        return digest.hexdigest(), size
    finally:
        if descriptor is not None:
            os.close(descriptor)
        os.close(parent_fd)


def _validator_diagnostic_prefix(path: Path, name: str) -> tuple[bytes, dict[str, object]]:
    """Read only the stable bounded prefix of one completed validator stream."""

    parent_fd, parent_identity = _open_directory(
        path.parent, f"receipt validator {name} parent"
    )
    descriptor: int | None = None
    try:
        before = _entry_stat(parent_fd, path.name, f"receipt validator {name}")
        if (
            not stat.S_ISREG(before.st_mode)
            or stat.S_ISLNK(before.st_mode)
            or before.st_uid != os.geteuid()
            or before.st_nlink != 1
            or stat.S_IMODE(before.st_mode) & 0o022
        ):
            raise CacheCopyError(f"receipt validator {name} metadata is unsafe")
        descriptor = os.open(
            path.name,
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent_fd,
        )
        opened = os.fstat(descriptor)
        if not _unchanged(before, opened):
            raise CacheCopyError(f"receipt validator {name} changed while opened")
        captured = bytearray()
        while len(captured) < min(opened.st_size, VALIDATOR_DIAGNOSTIC_BYTES):
            block = os.read(
                descriptor,
                min(1024 * 1024, VALIDATOR_DIAGNOSTIC_BYTES - len(captured)),
            )
            if not block:
                break
            captured.extend(block)
        after = os.fstat(descriptor)
        expected_size = min(opened.st_size, VALIDATOR_DIAGNOSTIC_BYTES)
        if len(captured) != expected_size or not _unchanged(opened, after):
            raise CacheCopyError(f"receipt validator {name} changed while captured")
        _revalidate_entry(parent_fd, path.name, after, f"receipt validator {name}")
        _revalidate_directory_path(
            path.parent, parent_identity, f"receipt validator {name} parent"
        )
        data = bytes(captured)
        return data, {
            "name": f"receipt-validator-failure.{name}",
            "sha256": hashlib.sha256(data).hexdigest(),
            "captured_size_bytes": len(data),
            "observed_size_bytes": opened.st_size,
            "truncated": opened.st_size > len(data),
            "mode": "0400",
        }
    finally:
        if descriptor is not None:
            os.close(descriptor)
        os.close(parent_fd)


def publish_validation_failure(
    invocation_root: Path,
    bootstrap_evidence: Path,
    cleanup_base: Path,
    cleanup_prefix: str,
    source_manifest_sha256: str,
    validator_exit_status: int,
) -> None:
    """Retain bounded validator diagnostics only after quiescent root cleanup."""

    for path, label in (
        (invocation_root, "release invocation root"),
        (bootstrap_evidence, "bootstrap evidence root"),
        (cleanup_base, "cleanup base"),
    ):
        _normalized_absolute(path, label)
    if (
        invocation_root.parent != cleanup_base
        or not invocation_root.name.startswith(cleanup_prefix)
        or _overlap(invocation_root, bootstrap_evidence)
        or not re.fullmatch(r"[0-9a-f]{64}", source_manifest_sha256)
        or type(validator_exit_status) is not int
        or not 1 <= validator_exit_status <= 255
    ):
        raise CacheCopyError("receipt validation failure inputs are not exact")
    bootstrap_fd, bootstrap_identity = _open_directory(
        bootstrap_evidence, "bootstrap evidence root"
    )
    try:
        if (
            bootstrap_identity.st_uid != os.geteuid()
            or stat.S_IMODE(bootstrap_identity.st_mode) != 0o700
        ):
            raise CacheCopyError("bootstrap evidence root is not owner-private")
        forbidden = {
            "BOOTSTRAP_RELEASE_COMPLETED.json", "RELEASE_COMPLETED.json",
            "receipt-validation-ack.json", "release-retained-inventory.json",
            "release-runner-private-provenance.json",
            "release-runner-result.json", "sealed-identity.json",
        }
        if any(
            _optional_entry_stat(bootstrap_fd, name, f"forbidden success evidence {name}")
            is not None
            for name in forbidden
        ):
            raise CacheCopyError("receipt validation failure follows published success evidence")
    finally:
        os.close(bootstrap_fd)

    identity_path = bootstrap_evidence / "candidate-identity.json"
    completion_path = bootstrap_evidence / "BOOTSTRAP_COMPLETED.json"
    validator_path = bootstrap_evidence / "validate-receipt.py"
    identity_payload, _ = _read_regular(identity_path, "candidate identity")
    try:
        identity = json.loads(identity_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError("candidate identity is malformed") from error
    identity_keys = {
        "schema_version", "head_commit", "head_tree", "index_tree",
        "workspace_source_manifest_sha256", "cargo_lock_sha256",
    }
    if (
        not isinstance(identity, dict)
        or set(identity) != identity_keys
        or type(identity.get("schema_version")) is not int
        or identity["schema_version"] != 1
        or any(
            not isinstance(identity.get(name), str)
            or re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", identity[name]) is None
            for name in ("head_commit", "head_tree", "index_tree")
        )
        or any(
            not isinstance(identity.get(name), str)
            or re.fullmatch(r"[0-9a-f]{64}", identity[name]) is None
            for name in ("workspace_source_manifest_sha256", "cargo_lock_sha256")
        )
        or identity_payload != _canonical_payload(identity)
    ):
        raise CacheCopyError("candidate identity contract is not exact")
    validator_sha256, _, _ = _digest_regular(
        validator_path, "archived receipt validator"
    )
    completion_sha256, _, _ = _digest_regular(
        completion_path, "bootstrap completion"
    )
    receipt_sha256, receipt_size = _stable_regular_digest(
        invocation_root / "output" / "release" / "RELEASE_COMPLETED.json",
        "unverified aggregate receipt",
    )
    stream_payloads: dict[str, bytes] = {}
    stream_records: dict[str, dict[str, object]] = {}
    for name in ("stdout", "stderr"):
        payload, record = _validator_diagnostic_prefix(
            invocation_root / f"receipt-validator.{name}", name
        )
        stream_payloads[name] = payload
        stream_records[name] = record

    publications: list[tuple[Path, tuple[os.stat_result, os.stat_result]]] = []
    try:
        cleanup_invocation(cleanup_base, invocation_root, cleanup_prefix)
        base_fd, _ = _open_directory(cleanup_base, "cleanup base")
        try:
            if _optional_entry_stat(
                base_fd, invocation_root.name, "cleaned release invocation"
            ) is not None:
                raise CacheCopyError("release invocation survived validator failure cleanup")
        finally:
            os.close(base_fd)
        _revalidate_directory_path(
            bootstrap_evidence, bootstrap_identity, "bootstrap evidence root"
        )
        marker = {
            "format": VALIDATOR_FAILURE_FORMAT,
            "schema_version": 2,
            "result": "release-failed",
            "stage": "protected-receipt-validation",
            "profile": "release",
            "bootstrap_completion_sha256": completion_sha256,
            "candidate_identity": {
                "sha256": hashlib.sha256(identity_payload).hexdigest(),
                "head_commit": identity["head_commit"],
                "head_tree": identity["head_tree"],
            },
            "sealed_source_manifest_sha256": source_manifest_sha256,
            "receipt": {
                "disclosure": "unverified-no-retention",
                "sha256": receipt_sha256,
                "size_bytes": receipt_size,
            },
            "validator": {
                "archive_name": "validate-receipt.py",
                "sha256": validator_sha256,
                "exit_status": validator_exit_status,
            },
            "argv": {
                "profile": "release",
                "python_flags": ["-I", "-S"],
                "validator": "protected:validate-receipt.py",
                "operation": "verify-existing-and-ack",
                "invocation_binding": "not-published-validation-failed",
            },
            "diagnostics": stream_records,
            "invocation_cleanup": "complete",
        }
        marker_payload = _canonical_payload(marker)
        if len(marker_payload) > VALIDATOR_FAILURE_MARKER_BYTES:
            raise CacheCopyError("receipt validation failure marker exceeds its bound")
        for name in ("stdout", "stderr"):
            path = bootstrap_evidence / str(stream_records[name]["name"])
            publications.append((path, _publish_inventory(path, stream_payloads[name])))
        marker_path = bootstrap_evidence / "RECEIPT_VALIDATION_FAILED.json"
        publications.append((marker_path, _publish_inventory(marker_path, marker_payload)))
        for path, _ in publications:
            payload, metadata = _read_regular(path, f"published {path.name}")
            if stat.S_IMODE(metadata.st_mode) != 0o400 or metadata.st_nlink != 1:
                raise CacheCopyError(f"published {path.name} metadata changed")
            if path == marker_path:
                if payload != marker_payload:
                    raise CacheCopyError("receipt validation failure marker changed")
            else:
                name = "stdout" if path.name.endswith(".stdout") else "stderr"
                if payload != stream_payloads[name]:
                    raise CacheCopyError(f"receipt validator {name} copy changed")
    except BaseException:
        for path, metadata in reversed(publications):
            _remove_published(path, metadata)
        raise




def _validation_ack(ack_held: dict[str, object], receipt_held: dict[str, object], source: Path, bootstrap_evidence: Path, source_manifest_sha256: str, candidate_root: Path, scaling_evidence_manifest: Path, expected_signer_fingerprint: str, expected_scaling_trial_harness_sha256: str, expected_scaling_configuration_sha256: str, expected_scaling_irohad_sha256: str, expected_scaling_iroha_cli_sha256: str) -> tuple[str, int]:
    component_name = VALIDATION_ACK_COMPONENT_FILES[0]
    sealed_component = source / "scripts" / component_name
    if sealed_component.exists() or sealed_component.is_symlink():
        component = sealed_component
    elif Path(__file__).name == "copy_sumeragi_v2_release_cargo_cache.py":
        component = Path(__file__).with_name(component_name)
    else:
        raise CacheCopyError("sealed validation acknowledgment component is missing")
    payload, metadata = _read_regular(
        component, "validation acknowledgment component"
    )
    if (
        len(payload) > VALIDATION_ACK_COMPONENT_MAXIMUM_BYTES
        or metadata.st_size != len(payload)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
    ):
        raise CacheCopyError("validation acknowledgment component metadata is not exact")
    if hashlib.sha256(payload).hexdigest() != VALIDATION_ACK_COMPONENT_SHA256:
        raise CacheCopyError("validation acknowledgment component digest mismatch")
    namespace = dict(globals())
    exec(compile(payload, str(component), "exec"), namespace)
    implementation = namespace.get("_validation_ack")
    if not callable(implementation) or implementation is _validation_ack:
        raise CacheCopyError("validation acknowledgment component entry point is invalid")
    return implementation(ack_held, receipt_held, source, bootstrap_evidence, source_manifest_sha256, candidate_root, scaling_evidence_manifest, expected_signer_fingerprint, expected_scaling_trial_harness_sha256, expected_scaling_configuration_sha256, expected_scaling_irohad_sha256, expected_scaling_iroha_cli_sha256)


def seal_release_result(
    invocation_root: Path,
    bootstrap_evidence: Path,
    source_manifest_sha256: str,
    candidate_root: Path,
    scaling_evidence_manifest: Path,
    expected_signer_fingerprint: str,
    expected_scaling_trial_harness_sha256: str,
    expected_scaling_configuration_sha256: str,
    expected_scaling_irohad_sha256: str,
    expected_scaling_iroha_cli_sha256: str,
) -> None:
    """Prune build runtime and publish a protected exact retained-evidence binding."""

    _normalized_absolute(invocation_root, "release invocation root")
    _normalized_absolute(bootstrap_evidence, "bootstrap evidence root")
    _normalized_absolute(candidate_root, "candidate root")
    _normalized_absolute(scaling_evidence_manifest, "scaling evidence manifest")
    invocation_fd, invocation_identity = _open_directory(invocation_root, "release invocation root")
    bootstrap_fd, bootstrap_identity = _open_directory(bootstrap_evidence, "bootstrap evidence root")
    os.close(invocation_fd)
    os.close(bootstrap_fd)
    if _overlap(invocation_root, bootstrap_evidence):
        raise CacheCopyError("release invocation and bootstrap evidence must be disjoint")
    if not re.fullmatch(r"[0-9a-f]{64}", source_manifest_sha256):
        raise CacheCopyError("retained source manifest digest is malformed")
    for metadata, label in (
        (invocation_identity, "invocation root"),
        (bootstrap_identity, "bootstrap evidence root"),
    ):
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) != 0o700
        ):
            raise CacheCopyError(f"retained {label} is not owner-private")

    source = invocation_root / "source"
    output = invocation_root / "output"
    target = invocation_root / "target"
    identity = invocation_root / "sealed-identity.json"
    receipt = output / "release" / "RELEASE_COMPLETED.json"
    ack = invocation_root / "receipt-validation-ack.json"
    inventory_path = invocation_root / "retained-evidence-inventory.json"
    protected_receipt = bootstrap_evidence / "RELEASE_COMPLETED.json"
    protected_identity = bootstrap_evidence / "sealed-identity.json"
    protected_inventory = bootstrap_evidence / "release-retained-inventory.json"
    protected_ack = bootstrap_evidence / "receipt-validation-ack.json"
    result_path = bootstrap_evidence / "release-runner-result.json"
    private_provenance_path = (
        bootstrap_evidence / "release-runner-private-provenance.json"
    )
    held_files: list[dict[str, object]] = []
    published: list[tuple[Path, tuple[os.stat_result, os.stat_result]]] = []
    try:
        ack_held = _hold_regular(ack, "receipt validation acknowledgment")
        held_files.append(ack_held)
        receipt_held = _hold_regular(receipt, "aggregate receipt")
        held_files.append(receipt_held)
        identity_held = _hold_regular(identity, "sealed identity")
        held_files.append(identity_held)
        ack_digest, ack_size = _validation_ack(
            ack_held,
            receipt_held,
            source,
            bootstrap_evidence,
            source_manifest_sha256,
            candidate_root,
            scaling_evidence_manifest,
            expected_signer_fingerprint,
            expected_scaling_trial_harness_sha256,
            expected_scaling_configuration_sha256,
            expected_scaling_irohad_sha256,
            expected_scaling_iroha_cli_sha256,
        )
        invocation_fd, invocation_identity = _open_directory(invocation_root, "release invocation root")
        output_fd, output_identity = _open_directory(output, "release output root")
        os.close(output_fd); os.close(invocation_fd)
        for parent, name in (
            (invocation_root, "runtime"),
            (invocation_root, "sdk-inputs"),
            (invocation_root, "sdk-work"),
            *((output, name) for name in ("home", "tmp", "cache", "cargo-home")),
            (invocation_root, "target"),
        ):
            _quiescent_remove_named(
                parent, name, f"retained disposable {name}", require_directory=True,
            )
        _revalidate_directory_path(invocation_root, invocation_identity, "release invocation root")
        _revalidate_directory_path(output, output_identity, "release output root")
        for parent, name in (
            *((output, name) for name in (
                "formal-completion-path", "seed-matrix-completion-path",
                "multilane-four-peer-completion-path", "nexus-cross-dataspace-completion-path",
                "nexus-cross-dataspace-soak-completion-path", "chaos-completion-path",
                "taira-completion-path", "release-child-result.json",
            )),
            (invocation_root, "receipt-validator.stdout"),
            (invocation_root, "receipt-validator.stderr"),
            (invocation_root, "runtime-tool-probe-manifest.json"),
            (invocation_root, "cancel-request.json"),
        ):
            _quiescent_remove_named(parent, name, f"retained control {name}")

        identity_bytes, receipt_bytes, ack_bytes = identity_held["data"], receipt_held["data"], ack_held["data"]
        assert isinstance(identity_bytes, bytes) and isinstance(receipt_bytes, bytes) and isinstance(ack_bytes, bytes)
        identity_digest, identity_size = hashlib.sha256(identity_bytes).hexdigest(), len(identity_bytes)
        receipt_digest, receipt_size = hashlib.sha256(receipt_bytes).hexdigest(), len(receipt_bytes)
        records, total_bytes = _retained_tree(invocation_root, {source, inventory_path})
        inventory = {
            "format": RETAINED_FORMAT,
            "schema_version": 2,
            "invocation_archive_id": "release-retained.invocation.v1",
            "source_archive_id": "release-retained.source.v1",
            "source_manifest_sha256": source_manifest_sha256,
            "record_count": len(records),
            "file_bytes": total_bytes,
            "records": records,
        }
        published.append((
            inventory_path,
            _publish_inventory(inventory_path, _canonical_payload(inventory)),
        ))
        inventory_held = _hold_regular(inventory_path, "retained evidence inventory")
        held_files.append(inventory_held)
        inventory_bytes = inventory_held["data"]
        assert isinstance(inventory_bytes, bytes)
        inventory_digest, inventory_size = hashlib.sha256(inventory_bytes).hexdigest(), len(inventory_bytes)
        for held in (identity_held, receipt_held, ack_held, inventory_held):
            _revalidate_held_regular(held)

        for path, data in (
            (protected_receipt, receipt_bytes),
            (protected_identity, identity_bytes),
            (protected_inventory, inventory_bytes),
            (protected_ack, ack_bytes),
        ):
            published.append((path, _publish_inventory(path, data)))
        private_provenance = {
            "format": RETAINED_PRIVATE_PROVENANCE_FORMAT,
            "schema_version": 1,
            "invocation_root": str(invocation_root),
            "source_root": str(source),
            "artifacts": {
                "sealed_identity": {
                    "path": str(identity),
                    "protected_path": str(protected_identity),
                },
                "receipt": {
                    "path": str(receipt),
                    "protected_path": str(protected_receipt),
                },
                "inventory": {
                    "path": str(inventory_path),
                    "protected_path": str(protected_inventory),
                },
                "receipt_validation": {
                    "path": str(ack),
                    "protected_path": str(protected_ack),
                },
            },
        }
        published.append((
            private_provenance_path,
            _publish_inventory(
                private_provenance_path, _canonical_payload(private_provenance)
            ),
        ))
        result = {
            "format": RETAINED_FORMAT,
            "schema_version": 2,
            "invocation_archive_id": "release-retained.invocation.v1",
            "source_archive_id": "release-retained.source.v1",
            "source_manifest_sha256": source_manifest_sha256,
            "sealed_identity": {
                "archive_id": "release-retained.identity.v1",
                "mode": "0400",
                "sha256": identity_digest,
                "size_bytes": identity_size,
            },
            "receipt": {
                "archive_id": "release-terminal.receipt.v1",
                "mode": "0400",
                "sha256": receipt_digest,
                "size_bytes": receipt_size,
            },
            "inventory": {
                "archive_id": "release-retained.inventory.v2",
                "mode": "0400",
                "sha256": inventory_digest,
                "size_bytes": inventory_size,
            },
            "receipt_validation": {
                "archive_id": "release-retained.receipt-validation-ack.v3",
                "mode": "0400",
                "sha256": ack_digest,
                "size_bytes": ack_size,
            },
        }
        published.append((result_path, _publish_inventory(result_path, _canonical_payload(result))))
        for held in (identity_held, receipt_held, ack_held, inventory_held):
            _revalidate_held_regular(held)
        if _retained_tree(invocation_root, {source, inventory_path}) != (records, total_bytes):
            raise CacheCopyError("retained release tree changed during protected publication")
    except BaseException:
        for path, metadata in reversed(published):
            _remove_published(path, metadata)
        raise
    finally:
        for held in reversed(held_files):
            _close_held_regular(held)


def _verify_runtime_sources(
    roots: dict[str, Path], records: list[dict[str, object]],
) -> None:
    for record in records:
        if not isinstance(record, dict):
            raise CacheCopyError("runtime source inventory record is not an object")
        kind = record.get("kind")
        keys = {
            "directory": {"path", "kind", "source_device", "source_inode", "source_mode", "destination_device", "destination_inode", "destination_mode"},
            "file": {"path", "kind", "source_device", "source_inode", "source_mode", "destination_device", "destination_inode", "destination_mode", "size", "sha256"},
            "symlink": {"path", "kind", "source_mode", "destination_mode", "target"},
        }.get(kind)
        path = record.get("path")
        if keys is None or set(record) != keys or not isinstance(path, str) or PurePosixPath(path).as_posix() != path or path.startswith("/") or ".." in PurePosixPath(path).parts:
            raise CacheCopyError("runtime source inventory record is not exact")
        for key in set(record) & {"source_device", "source_inode", "destination_device", "destination_inode", "size"}:
            if type(record[key]) is not int or record[key] < 0:
                raise CacheCopyError("runtime source inventory numeric field is not exact")
        for key in set(record) & {"source_mode", "destination_mode"}:
            if not isinstance(record[key], str) or re.fullmatch(r"[0-7]{4}", record[key]) is None:
                raise CacheCopyError("runtime source inventory mode is not exact")
        if kind == "file" and (not isinstance(record["sha256"], str) or re.fullmatch(r"[0-9a-f]{64}", record["sha256"]) is None):
            raise CacheCopyError("runtime source inventory digest is not exact")
        if kind == "symlink" and not isinstance(record["target"], str):
            raise CacheCopyError("runtime source inventory symlink is not exact")
    by_path = {str(record.get("path")): record for record in records}
    if len(by_path) != len(records):
        raise CacheCopyError("runtime source inventory has duplicate records")
    visited: set[str] = set()

    def walk(root_fd: int, prefix: str, relative: str = "") -> None:
        directory_path = prefix if not relative else f"{prefix}/{relative}"
        directory = by_path.get(directory_path)
        visited.add(directory_path)
        metadata = os.fstat(root_fd)
        if not isinstance(directory, dict) or directory.get("kind") != "directory" or directory.get("source_device") != metadata.st_dev or directory.get("source_inode") != metadata.st_ino or directory.get("source_mode") != format(stat.S_IMODE(metadata.st_mode), "04o"):
            raise CacheCopyError(f"runtime source directory changed: {directory_path}")
        names = _directory_entries(root_fd, f"runtime source {directory_path}")
        expected_children = {
            path.split("/")[len(directory_path.split("/"))]
            for path in by_path if path.startswith(f"{directory_path}/")
        }
        if set(names) != expected_children:
            raise CacheCopyError(f"runtime source inventory changed: {directory_path}")
        for name in names:
            item_path = f"{directory_path}/{name}"
            item = by_path.get(item_path)
            visited.add(item_path)
            observed = _entry_stat(root_fd, name, f"runtime source {item_path}")
            if not isinstance(item, dict) or item.get("source_mode") != format(stat.S_IMODE(observed.st_mode), "04o"):
                raise CacheCopyError(f"runtime source entry changed: {item_path}")
            if stat.S_ISDIR(observed.st_mode):
                child = os.open(name, _DIRECTORY_FLAGS, dir_fd=root_fd)
                try:
                    walk(child, prefix, name if not relative else f"{relative}/{name}")
                finally:
                    os.close(child)
            elif stat.S_ISREG(observed.st_mode):
                descriptor = os.open(name, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0), dir_fd=root_fd)
                try:
                    digest = hashlib.sha256()
                    total = 0
                    while block := os.read(descriptor, 1024 * 1024):
                        digest.update(block); total += len(block)
                        if total > MAXIMUM_FILE_BYTES:
                            raise CacheCopyError(f"runtime source file exceeds its bound: {item_path}")
                    opened = os.fstat(descriptor)
                finally:
                    os.close(descriptor)
                if item.get("kind") != "file" or item.get("source_device") != opened.st_dev or item.get("source_inode") != opened.st_ino or item.get("size") != total or item.get("sha256") != digest.hexdigest():
                    raise CacheCopyError(f"runtime source file changed: {item_path}")
            elif stat.S_ISLNK(observed.st_mode):
                if item.get("kind") != "symlink" or item.get("target") != os.readlink(name, dir_fd=root_fd):
                    raise CacheCopyError(f"runtime source symlink changed: {item_path}")
            else:
                raise CacheCopyError(f"runtime source contains special entry: {item_path}")

    for prefix, root in roots.items():
        if root.is_dir():
            descriptor, _ = _open_directory(root, f"runtime source {prefix}")
            try:
                walk(descriptor, prefix)
            finally:
                os.close(descriptor)
        else:
            record = by_path.get(prefix)
            visited.add(prefix)
            parent_fd, _ = _open_directory(root.parent, f"runtime source {prefix} parent")
            try:
                metadata = _entry_stat(parent_fd, root.name, f"runtime source {prefix}")
                descriptor = os.open(root.name, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0), dir_fd=parent_fd)
                try:
                    digest = hashlib.sha256()
                    total = 0
                    while block := os.read(descriptor, 1024 * 1024):
                        digest.update(block); total += len(block)
                    opened = os.fstat(descriptor)
                finally:
                    os.close(descriptor)
                if not isinstance(record, dict) or record.get("kind") != "file" or record.get("source_device") != opened.st_dev or record.get("source_inode") != opened.st_ino or record.get("source_mode") != format(stat.S_IMODE(opened.st_mode), "04o") or record.get("size") != total or record.get("sha256") != digest.hexdigest() or not _unchanged(metadata, opened):
                    raise CacheCopyError(f"runtime source file changed: {prefix}")
            finally:
                os.close(parent_fd)
    if visited != set(by_path):
        raise CacheCopyError("runtime source inventory has unbound records")


def _bind_runtime_destinations(
    inputs: list[dict[str, object]], outputs: list[dict[str, object]], *, update: bool,
) -> None:
    by_path = {record.get("path"): record for record in outputs if isinstance(record, dict)}
    if len(by_path) != len(outputs):
        raise CacheCopyError("private destination inventory has duplicate records")
    for record in inputs:
        path = record.get("path")
        output = by_path.get(path)
        if output is None and isinstance(path, str):
            output = by_path.get(f"bin/{path}")
        kind = record.get("kind")
        if not isinstance(output, dict) or output.get("kind") != kind:
            raise CacheCopyError("runtime source destination record is absent")
        if kind in {"directory", "file"}:
            expected = {
                "destination_device": output.get("device"),
                "destination_inode": output.get("inode"),
                "destination_mode": output.get("mode"),
            }
            if kind == "file" and (record.get("size"), record.get("sha256")) != (output.get("size"), output.get("sha256")):
                raise CacheCopyError("runtime source destination file bytes disagree")
        elif kind == "symlink":
            expected = {"destination_mode": output.get("mode")}
            if record.get("target") != output.get("target"):
                raise CacheCopyError("runtime source destination symlink disagrees")
        else:
            raise CacheCopyError("runtime source destination kind is unsupported")
        if update:
            record.update(expected)
        elif any(record.get(key) != value for key, value in expected.items()):
            raise CacheCopyError("runtime source destination provenance changed")


_RELEASE_SHELL_UTILITY_NAMES = (
    "awk", "basename", "cat", "chmod", "cmp", "cp", "cut", "diff",
    "dirname", "env", "find", "grep", "ln", "ls", "mkdir", "mkfifo",
    "mktemp", "mv", "openssl", "rm", "rmdir", "sed", "sh", "sleep",
    "tail", "tee", "tr", "uname", "wc", "xargs",
    "shasum" if sys.platform == "darwin" else "sha256sum",
)
_RELEASE_RUNTIME_NAMES = (
    "python3", "git", "ssh-keygen", "bash", "copy-release-runtime.py",
    "cargo", "rustc", "node",
    "swift", "tlapm", "java", "verus", "cargo-verus", "tla2tools.jar",
    "tlapm-stdlib", "git-upload-pack", "git-index-pack",
    *_RELEASE_SHELL_UTILITY_NAMES,
)
_PR_RUNTIME_NAMES = (
    "python3", "git", "bash", "cargo", "rustc",
    "git-upload-pack", "git-index-pack",
    *_RELEASE_SHELL_UTILITY_NAMES,
)


def _runtime_names(resolved: list[Path]) -> tuple[str, ...]:
    if len(resolved) == len(_RELEASE_RUNTIME_NAMES):
        return _RELEASE_RUNTIME_NAMES
    if len(resolved) == len(_PR_RUNTIME_NAMES):
        return _PR_RUNTIME_NAMES
    raise CacheCopyError("runtime source inputs are not exact")


def _framework_python_closure(
    resolved: list[Path],
) -> tuple[Path, str, str] | None:
    """Return the exact selected macOS framework-Python closure, if any."""

    if (
        sys.platform != "darwin"
        or Path(sys.executable).resolve(strict=True) != resolved[0]
    ):
        return None
    import sysconfig

    framework = sysconfig.get_config_var("PYTHONFRAMEWORK")
    if not isinstance(framework, str) or not framework:
        raise CacheCopyError("selected macOS Python is not a framework runtime")
    if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", framework) is None:
        raise CacheCopyError("selected Python framework name is unsafe")
    executable = resolved[0]
    version_root = executable.parent.parent
    if executable.parent.name != "bin" or executable.parent.parent != version_root:
        raise CacheCopyError("selected Python launcher is outside its framework version")
    prefixes = (
        sys.prefix,
        sys.exec_prefix,
        sys.base_prefix,
        sys.base_exec_prefix,
    )
    if any(
        not isinstance(prefix, str)
        or Path(prefix) != version_root
        or not Path(prefix).is_absolute()
        for prefix in prefixes
    ):
        raise CacheCopyError("selected Python prefixes do not name one framework version")

    stdlib_name = f"python{sys.version_info.major}.{sys.version_info.minor}"
    expected_path = (
        str(
            version_root
            / "lib"
            / f"python{sys.version_info.major}{sys.version_info.minor}.zip"
        ),
        str(version_root / "lib" / stdlib_name),
        str(version_root / "lib" / stdlib_name / "lib-dynload"),
    )
    if tuple(sys.path) != expected_path:
        raise CacheCopyError(
            "selected Python isolated path is not its exact framework stdlib"
        )

    root_fd, root_metadata = _open_directory(
        version_root, "selected Python framework version",
    )
    try:
        if (
            root_metadata.st_uid not in {0, os.geteuid()}
            or stat.S_IMODE(root_metadata.st_mode) & 0o022
        ):
            raise CacheCopyError(
                "selected Python framework version metadata is unsafe"
            )
        required = {
            framework: "file",
            "Resources": "directory",
            "lib": "directory",
            "bin": "directory",
        }
        for name, kind in required.items():
            metadata = _entry_stat(
                root_fd, name, f"selected Python framework {name}",
            )
            matches = (
                stat.S_ISREG(metadata.st_mode)
                if kind == "file"
                else stat.S_ISDIR(metadata.st_mode)
            )
            if stat.S_ISLNK(metadata.st_mode) or not matches:
                raise CacheCopyError(
                    f"selected Python framework {name} is not a no-follow {kind}"
                )
        bin_fd = os.open("bin", _DIRECTORY_FLAGS, dir_fd=root_fd)
        lib_fd = os.open("lib", _DIRECTORY_FLAGS, dir_fd=root_fd)
        resources_fd = os.open("Resources", _DIRECTORY_FLAGS, dir_fd=root_fd)
        try:
            launcher = _entry_stat(
                bin_fd, executable.name, "selected Python framework launcher",
            )
            source_launcher = executable.lstat()
            if (
                stat.S_ISLNK(launcher.st_mode)
                or not stat.S_ISREG(launcher.st_mode)
                or not _unchanged(launcher, source_launcher)
                or not launcher.st_mode & 0o111
                or launcher.st_uid not in {0, os.geteuid()}
                or stat.S_IMODE(launcher.st_mode) & 0o022
            ):
                raise CacheCopyError(
                    "selected Python framework launcher is unsafe"
                )
            stdlib = _entry_stat(
                lib_fd, stdlib_name, "selected Python framework stdlib",
            )
            if stat.S_ISLNK(stdlib.st_mode) or not stat.S_ISDIR(stdlib.st_mode):
                raise CacheCopyError(
                    "selected Python framework stdlib is unsafe"
                )
            stdlib_fd = os.open(stdlib_name, _DIRECTORY_FLAGS, dir_fd=lib_fd)
            try:
                dynload = _entry_stat(
                    stdlib_fd, "lib-dynload",
                    "selected Python framework extension directory",
                )
                if (
                    stat.S_ISLNK(dynload.st_mode)
                    or not stat.S_ISDIR(dynload.st_mode)
                ):
                    raise CacheCopyError(
                        "selected Python framework extension directory is unsafe"
                    )
            finally:
                os.close(stdlib_fd)
            descriptor = resources_fd
            opened: list[int] = []
            try:
                for part in ("Python.app", "Contents", "MacOS"):
                    metadata = _entry_stat(
                        descriptor, part,
                        "selected Python framework resource trampoline",
                    )
                    if (
                        stat.S_ISLNK(metadata.st_mode)
                        or not stat.S_ISDIR(metadata.st_mode)
                    ):
                        raise CacheCopyError(
                            "selected Python framework resource trampoline is unsafe"
                        )
                    child = os.open(part, _DIRECTORY_FLAGS, dir_fd=descriptor)
                    if not _unchanged(metadata, os.fstat(child)):
                        os.close(child)
                        raise CacheCopyError(
                            "selected Python framework resource trampoline changed"
                        )
                    opened.append(child)
                    descriptor = child
                trampoline = _entry_stat(
                    descriptor, "Python",
                    "selected Python framework resource executable",
                )
                if (
                    stat.S_ISLNK(trampoline.st_mode)
                    or not stat.S_ISREG(trampoline.st_mode)
                    or not trampoline.st_mode & 0o111
                ):
                    raise CacheCopyError(
                        "selected Python framework resource executable is unsafe"
                    )
            finally:
                for descriptor in reversed(opened):
                    os.close(descriptor)
        finally:
            os.close(bin_fd)
            os.close(lib_fd)
            os.close(resources_fd)
    finally:
        os.close(root_fd)
    return version_root, framework, stdlib_name


def _validate_framework_python_sources(
    version_root: Path, framework: str,
) -> None:
    """Validate every selected framework member without following a symlink."""

    allowed_roots = {framework, "Resources", "lib"}
    root_fd, root_metadata = _open_directory(
        version_root, "selected Python framework version",
    )

    def safe_metadata(metadata: os.stat_result, label: str) -> None:
        if (
            metadata.st_uid not in {0, os.geteuid()}
            or (
                not stat.S_ISLNK(metadata.st_mode)
                and stat.S_IMODE(metadata.st_mode) & 0o022
            )
        ):
            raise CacheCopyError(
                f"selected Python framework member is unsafe: {label}"
            )

    def walk(directory_fd: int, parts: tuple[str, ...]) -> None:
        before = os.fstat(directory_fd)
        safe_metadata(before, "/".join(parts))
        names = _directory_entries(
            directory_fd, f"selected Python framework {'/'.join(parts)}",
        )
        for name in names:
            child_parts = (*parts, name)
            label = "/".join(child_parts)
            _bounded_relative(label)
            metadata = _entry_stat(
                directory_fd, name, f"selected Python framework {label}",
            )
            safe_metadata(metadata, label)
            if stat.S_ISDIR(metadata.st_mode):
                child = os.open(name, _DIRECTORY_FLAGS, dir_fd=directory_fd)
                try:
                    if not _unchanged(metadata, os.fstat(child)):
                        raise CacheCopyError(
                            f"selected Python framework directory changed: {label}"
                        )
                    walk(child, child_parts)
                finally:
                    os.close(child)
                _revalidate_entry(
                    directory_fd, name, metadata,
                    f"selected Python framework directory {label}",
                )
            elif stat.S_ISREG(metadata.st_mode):
                continue
            elif stat.S_ISLNK(metadata.st_mode):
                target = _validate_symlink(
                    directory_fd, name, metadata, root_fd, parts, label,
                )
                target_parts = _safe_target_parts(parts, target, label)
                if target_parts[0] not in allowed_roots:
                    raise CacheCopyError(
                        "selected Python framework symlink leaves "
                        f"its archive closure: {label}"
                    )
            else:
                raise CacheCopyError(
                    f"selected Python framework contains a special member: {label}"
                )
        if (
            _directory_entries(
                directory_fd, f"selected Python framework {'/'.join(parts)}",
            )
            != names
            or not _unchanged(before, os.fstat(directory_fd))
        ):
            raise CacheCopyError(
                f"selected Python framework directory changed: {'/'.join(parts)}"
            )

    try:
        safe_metadata(root_metadata, ".")
        framework_metadata = _entry_stat(
            root_fd, framework, "selected Python framework library",
        )
        safe_metadata(framework_metadata, framework)
        if (
            stat.S_ISLNK(framework_metadata.st_mode)
            or not stat.S_ISREG(framework_metadata.st_mode)
        ):
            raise CacheCopyError("selected Python framework library is unsafe")
        for name in ("Resources", "lib"):
            metadata = _entry_stat(
                root_fd, name, f"selected Python framework {name}",
            )
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
                raise CacheCopyError(
                    f"selected Python framework {name} is unsafe"
                )
            descriptor = os.open(name, _DIRECTORY_FLAGS, dir_fd=root_fd)
            try:
                if not _unchanged(metadata, os.fstat(descriptor)):
                    raise CacheCopyError(
                        f"selected Python framework {name} changed while opened"
                    )
                walk(descriptor, (name,))
            finally:
                os.close(descriptor)
            _revalidate_entry(
                root_fd, name, metadata, f"selected Python framework {name}",
            )
    finally:
        os.close(root_fd)


def _validate_framework_python_runtime_records(
    records: list[dict[str, object]], framework: str, stdlib_name: str,
) -> None:
    """Require a complete, internally closed destination-member inventory."""

    by_path = {
        record.get("path"): record
        for record in records
        if isinstance(record, dict) and isinstance(record.get("path"), str)
    }
    if len(by_path) != len(records):
        raise CacheCopyError("private runtime framework inventory is not unique")
    required = {
        "bin/python3": "file",
        framework: "file",
        "Resources": "directory",
        "Resources/Python.app/Contents/MacOS/Python": "file",
        "lib": "directory",
        f"lib/{stdlib_name}": "directory",
        f"lib/{stdlib_name}/lib-dynload": "directory",
    }
    if any(
        not isinstance(by_path.get(path), dict)
        or by_path[path].get("kind") != kind
        for path, kind in required.items()
    ):
        raise CacheCopyError("private runtime framework closure is incomplete")
    allowed_roots = {framework, "Resources", "lib"}
    for path, record in by_path.items():
        if (
            not isinstance(path, str)
            or PurePosixPath(path).parts[0] not in allowed_roots
            or record.get("kind") != "symlink"
        ):
            continue
        target = record.get("target")
        if not isinstance(target, str):
            raise CacheCopyError("private runtime framework symlink is malformed")
        parent_parts = tuple(PurePosixPath(path).parts[:-1])
        target_parts = _safe_target_parts(parent_parts, target, path)
        if target_parts[0] not in allowed_roots:
            raise CacheCopyError(
                f"private runtime framework symlink leaves its closure: {path}"
            )
        for index in range(1, len(target_parts) + 1):
            target_path = "/".join(target_parts[:index])
            target_record = by_path.get(target_path)
            if not isinstance(target_record, dict):
                raise CacheCopyError(
                    f"private runtime framework symlink target is absent: {path}"
                )
            if (
                index < len(target_parts)
                and target_record.get("kind") != "directory"
            ):
                raise CacheCopyError(
                    f"private runtime framework symlink target is indirect: {path}"
                )
            if (
                index == len(target_parts)
                and target_record.get("kind") not in {"directory", "file"}
            ):
                raise CacheCopyError(
                    f"private runtime framework symlink target is unsafe: {path}"
                )


def _reject_framework_python_destination_overlap(
    version_root: Path, runtime_root: Path, inventory_path: Path,
) -> None:
    if (
        _overlap(version_root, runtime_root)
        or _contained(inventory_path, version_root)
    ):
        raise CacheCopyError(
            "Python framework source, private runtime, and inventory must be disjoint"
        )


_FRAMEWORK_PYTHON_PROBE = """\
import sys
expected_executable, expected_root, expected_zip, expected_stdlib, expected_dynload = sys.argv[1:]
if sys.executable != expected_executable:
    raise SystemExit(91)
if (sys.prefix, sys.exec_prefix, sys.base_prefix, sys.base_exec_prefix) != (expected_root,) * 4:
    raise SystemExit(92)
if sys.path != [expected_zip, expected_stdlib, expected_dynload]:
    raise SystemExit(93)
sys.stdout.buffer.write(sys.executable.encode('utf-8', 'surrogateescape') + b'\\n')
"""


def _probe_framework_python_runtime(
    runtime_root: Path, stdlib_name: str,
) -> bytes:
    """Run the archived interpreter and require its exact archive-local identity."""

    executable = runtime_root / "bin" / "python3"
    metadata = executable.lstat()
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o500
        or metadata.st_nlink != 1
    ):
        raise CacheCopyError(
            "archived framework Python executable metadata is unsafe"
        )
    expected_zip = runtime_root / "lib" / (
        f"python{sys.version_info.major}{sys.version_info.minor}.zip"
    )
    expected_stdlib = runtime_root / "lib" / stdlib_name
    expected_dynload = expected_stdlib / "lib-dynload"
    argv = [
        str(executable),
        "-I",
        "-S",
        "-c",
        _FRAMEWORK_PYTHON_PROBE,
        str(executable),
        str(runtime_root),
        str(expected_zip),
        str(expected_stdlib),
        str(expected_dynload),
    ]
    try:
        result = subprocess.run(
            argv,
            cwd=runtime_root,
            env={
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": str(runtime_root / "bin"),
            },
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise CacheCopyError(
            "archived framework Python probe could not run"
        ) from error
    expected_stdout = os.fsencode(str(executable)) + b"\n"
    if (
        result.returncode != 0
        or result.stdout != expected_stdout
        or result.stderr != b""
    ):
        raise CacheCopyError(
            "archived framework Python isolated probe did not report its executable"
        )
    return expected_stdout


def _runtime_source_roots(resolved: list[Path]) -> dict[str, Path]:
    names = _runtime_names(resolved)
    cargo_index = names.index("cargo")
    roots = {"rust-toolchain": resolved[cargo_index].parent.parent}
    if names == _RELEASE_RUNTIME_NAMES:
        roots.update({
            "swift-toolchain": resolved[names.index("swift")].parent.parent,
            "java-runtime": resolved[names.index("java")].parent.parent,
            "verus-distribution": resolved[names.index("verus")].parent,
        })
    for name, source in zip(names, resolved):
        if name not in {
            "cargo", "rustc", "swift", "java", "verus", "cargo-verus",
        }:
            roots[name] = source
    framework_python = _framework_python_closure(resolved)
    if framework_python is not None:
        version_root, framework, _ = framework_python
        roots.update({
            framework: version_root / framework,
            "Resources": version_root / "Resources",
            "lib": version_root / "lib",
        })
    return roots


def verify_runtime_sources(
    sources: list[Path], runtime_root: Path, inventory_path: Path,
) -> None:
    if len(sources) not in {len(_PR_RUNTIME_NAMES), len(_RELEASE_RUNTIME_NAMES)}:
        raise CacheCopyError("runtime source verification inputs are not exact")
    resolved = [source.resolve(strict=True) for source in sources]
    _runtime_names(resolved)
    framework_python = _framework_python_closure(resolved)
    if framework_python is not None:
        version_root, framework, stdlib_name = framework_python
        _reject_framework_python_destination_overlap(
            version_root, runtime_root, inventory_path,
        )
        _validate_framework_python_sources(version_root, framework)
    payload, inventory_metadata = _read_regular(inventory_path, "runtime inventory")
    try:
        inventory = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError("runtime inventory is malformed") from error
    keys = {"format", "schema_version", "runtime_root", "record_count", "file_bytes", "records", "source_disclosure", "input_record_count", "input_file_bytes", "input_records"}
    if not isinstance(inventory, dict) or set(inventory) != keys or inventory.get("format") != "iroha-sumeragi-v2-private-runtime" or type(inventory.get("schema_version")) is not int or inventory["schema_version"] != 1 or inventory.get("source_disclosure") != "withheld" or not isinstance(inventory.get("input_records"), list) or not isinstance(inventory.get("records"), list) or stat.S_IMODE(inventory_metadata.st_mode) != 0o400 or payload != _canonical_payload(inventory):
        raise CacheCopyError("runtime inventory contract is not exact")
    input_file_bytes = sum(record.get("size", 0) for record in inventory["input_records"] if isinstance(record, dict) and record.get("kind") == "file")
    if type(inventory.get("input_record_count")) is not int or inventory["input_record_count"] != len(inventory["input_records"]) or type(inventory.get("input_file_bytes")) is not int or inventory["input_file_bytes"] != input_file_bytes:
        raise CacheCopyError("runtime source inventory accounting is not exact")
    _verify_runtime_sources(_runtime_source_roots(resolved), inventory["input_records"])
    for record in inventory["records"]:
        if not isinstance(record, dict):
            raise CacheCopyError("private runtime record is not an object")
        kind = record.get("kind")
        keys = {
            "directory": {"path", "kind", "device", "inode", "mode"},
            "file": {"path", "kind", "device", "inode", "mode", "size", "sha256"},
            "symlink": {"path", "kind", "mode", "target"},
        }.get(kind)
        if keys is None or set(record) != keys or not isinstance(record.get("path"), str):
            raise CacheCopyError("private runtime record contract is not exact")
        for key in set(record) & {"device", "inode", "size"}:
            if type(record[key]) is not int or record[key] < 0:
                raise CacheCopyError("private runtime record numeric field is not exact")
        if not isinstance(record["mode"], str) or re.fullmatch(r"[0-7]{4}", record["mode"]) is None or (kind == "file" and (not isinstance(record["sha256"], str) or re.fullmatch(r"[0-9a-f]{64}", record["sha256"]) is None)) or (kind == "symlink" and not isinstance(record["target"], str)):
            raise CacheCopyError("private runtime record metadata is not exact")
    if framework_python is not None:
        _validate_framework_python_runtime_records(
            inventory["records"], framework, stdlib_name,
        )
    _bind_runtime_destinations(inventory["input_records"], inventory["records"], update=False)
    if not isinstance(inventory["runtime_root"], str):
        raise CacheCopyError("private runtime path is not exact")
    _normalized_absolute(runtime_root, "private runtime")
    if inventory["runtime_root"] != str(runtime_root):
        raise CacheCopyError("runtime inventory names the wrong private root")
    if runtime_root.parent != inventory_path.parent:
        raise CacheCopyError("private runtime and inventory are not siblings")
    if framework_python is not None:
        _probe_framework_python_runtime(runtime_root, stdlib_name)
    runtime_fd, runtime_identity = _open_directory(runtime_root, "private runtime")
    records: list[dict[str, object]] = []
    budget = {"records": 0, "bytes": 0}
    try:
        _snapshot_directory(runtime_fd, runtime_fd, None, (), records, budget)
    finally:
        os.close(runtime_fd)
    _revalidate_directory_path(runtime_root, runtime_identity, "private runtime")
    if (
        type(inventory.get("record_count")) is not int
        or inventory["record_count"] != len(records)
        or type(inventory.get("file_bytes")) is not int
        or inventory["file_bytes"] != budget["bytes"]
        or inventory.get("records") != sorted(records, key=lambda record: str(record["path"]))
    ):
        raise CacheCopyError("private runtime changed after publication")


def _seal_copied_tree(
    root: Path,
    label: str,
    *,
    owner_private_directory_roots: tuple[tuple[str, ...], ...] = (),
) -> None:
    root_fd, root_identity = _open_directory(root, label)

    def owner_private(parts: tuple[str, ...]) -> bool:
        return any(
            parts[:len(prefix)] == prefix
            for prefix in owner_private_directory_roots
        )

    def seal(directory_fd: int, relative: str, parts: tuple[str, ...]) -> None:
        for name in _directory_entries(directory_fd, relative):
            child_parts = (*parts, name)
            metadata = _entry_stat(directory_fd, name, f"{relative}/{name}")
            if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
                child = os.open(name, _DIRECTORY_FLAGS, dir_fd=directory_fd)
                try:
                    if not _same_directory(metadata, os.fstat(child)):
                        raise CacheCopyError(f"{label} directory changed: {relative}/{name}")
                    seal(child, f"{relative}/{name}", child_parts)
                finally:
                    os.close(child)
                sealed = _entry_stat(directory_fd, name, f"{relative}/{name}")
                expected_mode = 0o700 if owner_private(child_parts) else 0o500
                if not _same_directory_inode(metadata, sealed) or stat.S_IMODE(sealed.st_mode) != expected_mode:
                    raise CacheCopyError(f"{label} directory sealing changed identity: {relative}/{name}")
            elif stat.S_ISREG(metadata.st_mode):
                descriptor = os.open(name, os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0), dir_fd=directory_fd)
                try:
                    opened = os.fstat(descriptor)
                    if not _unchanged(metadata, opened):
                        raise CacheCopyError(f"{label} file changed: {relative}/{name}")
                    os.fchmod(descriptor, 0o500 if metadata.st_mode & 0o111 else 0o400)
                finally:
                    os.close(descriptor)
            elif not stat.S_ISLNK(metadata.st_mode):
                raise CacheCopyError(f"{label} contains a special entry: {relative}/{name}")
        os.fchmod(directory_fd, 0o700 if owner_private(parts) else 0o500)

    try:
        seal(root_fd, label, ())
    finally:
        os.close(root_fd)
    observed = root.lstat()
    if not _same_directory_inode(root_identity, observed) or stat.S_IMODE(observed.st_mode) != 0o500:
        raise CacheCopyError(f"{label} root changed while sealed")


def _verify_private_bundle(
    source_root: Path, bundle_root: Path, inventory_path: Path,
) -> dict[str, object]:
    _normalized_absolute(bundle_root, "private bundle root")
    payload, inventory_metadata = _read_regular(inventory_path, "private bundle inventory")
    try:
        inventory = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError("private bundle inventory is malformed") from error
    keys = {"format", "schema_version", "bundle_root", "record_count", "file_bytes", "records", "source_disclosure", "input_record_count", "input_file_bytes", "input_records"}
    if not isinstance(inventory, dict) or set(inventory) != keys or inventory.get("format") != "iroha-sumeragi-v2-private-bundle" or type(inventory.get("schema_version")) is not int or inventory["schema_version"] != 1 or inventory.get("source_disclosure") != "withheld" or not isinstance(inventory.get("bundle_root"), str) or not isinstance(inventory.get("records"), list) or not isinstance(inventory.get("input_records"), list) or stat.S_IMODE(inventory_metadata.st_mode) != 0o400 or payload != _canonical_payload(inventory):
        raise CacheCopyError("private bundle inventory contract is not exact")
    if inventory["bundle_root"] != str(bundle_root):
        raise CacheCopyError("private bundle inventory names the wrong private root")
    if bundle_root.parent != inventory_path.parent or source_root.name == "":
        raise CacheCopyError("private bundle paths are not exact siblings")
    input_bytes = sum(record.get("size", 0) for record in inventory["input_records"] if isinstance(record, dict) and record.get("kind") == "file")
    if type(inventory.get("input_record_count")) is not int or inventory["input_record_count"] != len(inventory["input_records"]) or type(inventory.get("input_file_bytes")) is not int or inventory["input_file_bytes"] != input_bytes:
        raise CacheCopyError("private bundle input accounting is not exact")
    _verify_runtime_sources({bundle_root.name: source_root.resolve(strict=True)}, inventory["input_records"])
    _bind_runtime_destinations(inventory["input_records"], inventory["records"], update=False)
    bundle_fd, bundle_identity = _open_directory(bundle_root, "private bundle")
    records: list[dict[str, object]] = [{"path": bundle_root.name, "kind": "directory", "mode": format(stat.S_IMODE(bundle_identity.st_mode), "04o"), "device": bundle_identity.st_dev, "inode": bundle_identity.st_ino}]
    budget = {"records": 1, "bytes": 0}
    try:
        _snapshot_directory(bundle_fd, bundle_fd, bundle_root.name, (bundle_root.name,), records, budget)
    finally:
        os.close(bundle_fd)
    _revalidate_directory_path(bundle_root, bundle_identity, "private bundle")
    if type(inventory.get("record_count")) is not int or inventory["record_count"] != len(records) or type(inventory.get("file_bytes")) is not int or inventory["file_bytes"] != budget["bytes"] or inventory["records"] != sorted(records, key=lambda record: str(record["path"])):
        raise CacheCopyError("private bundle changed after publication")
    return inventory


def copy_private_bundle(source_root: Path, bundle_root: Path, inventory_path: Path) -> None:
    """Copy, seal, and inventory one bounded path-withheld evidence bundle."""

    for path, label in ((source_root, "bundle source"), (bundle_root, "private bundle"), (inventory_path, "private bundle inventory")):
        _normalized_absolute(path, label)
    if bundle_root.parent != inventory_path.parent or _overlap(source_root, bundle_root) or _contained(inventory_path, source_root):
        raise CacheCopyError("bundle source, destination, and inventory must be disjoint")
    source_fd, source_identity = _open_directory(source_root, "bundle source")
    parent_fd, parent_identity = _open_directory(bundle_root.parent, "private bundle parent")
    records: list[dict[str, object]] = []
    budget = {"records": 0, "bytes": 0}
    published: tuple[os.stat_result, os.stat_result] | None = None
    created: os.stat_result | None = None
    try:
        if _optional_entry_stat(parent_fd, bundle_root.name, "private bundle") is not None or _optional_entry_stat(parent_fd, inventory_path.name, "private bundle inventory") is not None:
            raise CacheCopyError("private bundle outputs already exist")
        _copy_directory(source_fd, parent_fd, bundle_root.name, bundle_root.name, source_fd, (), records, budget)
        created = _entry_stat(parent_fd, bundle_root.name, "private bundle")
        _revalidate_directory_path(source_root, source_identity, "bundle source")
        _seal_copied_tree(bundle_root, "private bundle")
        bundle_fd, bundle_identity = _open_directory(bundle_root, "private bundle")
        copied_records: list[dict[str, object]] = [{"path": bundle_root.name, "kind": "directory", "mode": format(stat.S_IMODE(bundle_identity.st_mode), "04o"), "device": bundle_identity.st_dev, "inode": bundle_identity.st_ino}]
        copied_budget = {"records": 1, "bytes": 0}
        try:
            _snapshot_directory(bundle_fd, bundle_fd, bundle_root.name, (bundle_root.name,), copied_records, copied_budget)
        finally:
            os.close(bundle_fd)
        _bind_runtime_destinations(records, copied_records, update=True)
        document = {"format": "iroha-sumeragi-v2-private-bundle", "schema_version": 1, "bundle_root": str(bundle_root), "record_count": len(copied_records), "file_bytes": copied_budget["bytes"], "records": sorted(copied_records, key=lambda record: str(record["path"])), "source_disclosure": "withheld", "input_record_count": len(records), "input_file_bytes": budget["bytes"], "input_records": sorted(records, key=lambda record: str(record["path"]))}
        _verify_runtime_sources({bundle_root.name: source_root}, records)
        published = _publish_inventory(inventory_path, _canonical_payload(document))
        _verify_private_bundle(source_root, bundle_root, inventory_path)
        _revalidate_directory_path(bundle_root, bundle_identity, "private bundle")
        _revalidate_directory_path(bundle_root.parent, parent_identity, "private bundle parent")
    except BaseException:
        _remove_published(inventory_path, published)
        if created is not None:
            _owned_remove_entry(parent_fd, bundle_root.name, (created.st_dev, created.st_ino), "partial private bundle")
        raise
    finally:
        os.close(source_fd); os.close(parent_fd)


def _sdk_records_sha256(records: list[dict[str, object]]) -> str:
    """Digest one canonical path-free dependency member inventory."""

    payload = json.dumps(
        records, ensure_ascii=True, sort_keys=True, separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def _sdk_inventory_records_contract(
    value: object, label: str,
) -> tuple[list[dict[str, object]], int]:
    """Validate an exact complete source-tree member inventory."""

    if (
        not isinstance(value, list)
        or not value
        or len(value) > MAXIMUM_RECORDS
    ):
        raise CacheCopyError(f"{label} record count is not bounded")
    paths: list[str] = []
    by_path: dict[str, dict[str, object]] = {}
    file_bytes = 0
    for index, raw in enumerate(value):
        if not isinstance(raw, dict):
            raise CacheCopyError(f"{label} record {index} is not an object")
        kind = raw.get("kind")
        expected = {
            "directory": {"path", "kind", "mode"},
            "file": {"path", "kind", "mode", "size", "sha256"},
            "symlink": {"path", "kind", "mode", "target"},
        }.get(kind)
        relative = raw.get("path")
        mode = raw.get("mode")
        if (
            expected is None
            or set(raw) != expected
            or not isinstance(relative, str)
            or (relative != "." and (
                PurePosixPath(relative).is_absolute()
                or PurePosixPath(relative).as_posix() != relative
                or not PurePosixPath(relative).parts
                or any(
                    part in {"", ".", ".."}
                    for part in PurePosixPath(relative).parts
                )
            ))
            or not isinstance(mode, str)
            or re.fullmatch(r"[0-7]{4}", mode) is None
            or relative in by_path
        ):
            raise CacheCopyError(f"{label} record {index} is not canonical")
        if kind == "file":
            size = raw.get("size")
            digest = raw.get("sha256")
            if (
                type(size) is not int
                or not 0 <= size <= MAXIMUM_FILE_BYTES
                or not isinstance(digest, str)
                or re.fullmatch(r"[0-9a-f]{64}", digest) is None
            ):
                raise CacheCopyError(f"{label} file record is malformed")
            file_bytes += size
            if file_bytes > MAXIMUM_TOTAL_BYTES:
                raise CacheCopyError(f"{label} file bytes exceed their bound")
        elif kind == "symlink":
            target = raw.get("target")
            if (
                not isinstance(target, str)
                or "\0" in target
                or PurePosixPath(target).is_absolute()
                or PurePosixPath(target).as_posix() != target
            ):
                raise CacheCopyError(f"{label} symlink target is unsafe")
        paths.append(relative)
        by_path[relative] = raw
    root = value[0]
    if (
        root.get("path") != "."
        or root.get("kind") != "directory"
        or paths[1:] != sorted(paths[1:])
    ):
        raise CacheCopyError(f"{label} root or ordering is not exact")
    for relative, record in by_path.items():
        if relative == ".":
            continue
        pure = PurePosixPath(relative)
        parent = pure.parent.as_posix()
        parent_record = by_path.get(parent)
        if (
            not isinstance(parent_record, dict)
            or parent_record.get("kind") != "directory"
        ):
            raise CacheCopyError(f"{label} member lacks its exact parent")
        if record["kind"] == "symlink":
            parts = list(pure.parent.parts) if parent != "." else []
            for part in PurePosixPath(str(record["target"])).parts:
                if part in {"", "."}:
                    continue
                if part == "..":
                    if not parts:
                        raise CacheCopyError(f"{label} symlink escapes its root")
                    parts.pop()
                else:
                    parts.append(part)
            target = "/".join(parts) or "."
            if target not in by_path:
                raise CacheCopyError(f"{label} symlink target is not inventoried")
    return value, file_bytes


def _sdk_source_inventory_contract(
    value: object, label: str,
) -> tuple[list[dict[str, object]], int]:
    if not isinstance(value, dict) or set(value) != {
        "format", "schema_version", "record_count", "file_bytes",
        "records_sha256", "records",
    }:
        raise CacheCopyError(f"{label} schema is not exact")
    records, file_bytes = _sdk_inventory_records_contract(
        value["records"], label,
    )
    if (
        value["format"] != SDK_SOURCE_INVENTORY_FORMAT
        or type(value["schema_version"]) is not int
        or value["schema_version"] != 1
        or type(value["record_count"]) is not int
        or value["record_count"] != len(records)
        or type(value["file_bytes"]) is not int
        or value["file_bytes"] != file_bytes
        or not isinstance(value["records_sha256"], str)
        or value["records_sha256"] != _sdk_records_sha256(records)
    ):
        raise CacheCopyError(f"{label} accounting or digest is not exact")
    return records, file_bytes


def _sdk_source_manifest(
    path: Path, expected_sha256: str,
) -> tuple[dict[str, object], str]:
    """Read the protected, path-disclosing SDK source manifest."""

    _normalized_absolute(path, "SDK dependency source manifest")
    parent_fd, parent_metadata = _open_directory(
        path.parent, "SDK dependency source manifest parent",
    )
    os.close(parent_fd)
    if (
        parent_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(parent_metadata.st_mode) != 0o700
    ):
        raise CacheCopyError("SDK dependency source manifest is not bootstrap-private")
    payload, metadata = _read_regular(path, "SDK dependency source manifest")
    digest = hashlib.sha256(payload).hexdigest()
    if (
        re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is None
        or digest != expected_sha256
        or stat.S_IMODE(metadata.st_mode) != 0o400
    ):
        raise CacheCopyError("SDK dependency source manifest trust anchor changed")
    try:
        document = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError("SDK dependency source manifest is malformed") from error
    if (
        not isinstance(document, dict)
        or set(document) != {
            "format", "schema_version", "git", "node", "swiftpm", "gradle",
        }
        or document.get("format") != SDK_SOURCE_FORMAT
        or type(document.get("schema_version")) is not int
        or document["schema_version"] != 2
        or payload != _canonical_payload(document)
    ):
        raise CacheCopyError("SDK dependency source manifest contract is not exact")
    git = document["git"]
    node, swiftpm, gradle = document["node"], document["swiftpm"], document["gradle"]
    if (
        not isinstance(git, dict)
        or set(git) != {"executable", "sha256"}
        or not isinstance(node, dict)
        or set(node) != {
            "node_modules_root", "package_lock_sha256",
            "node_modules_inventory",
        }
        or not isinstance(swiftpm, dict)
        or set(swiftpm) != {
            "cache_root", "cache_inventory", "package_resolved_sha256",
            "resolved_revisions",
        }
        or not isinstance(gradle, dict)
        or set(gradle) != {
            "distribution_archive", "distribution_sha256", "gradle_user_home",
            "distribution_url", "gradle_user_home_inventory",
            "java_wrapper_properties_sha256", "kotlin_wrapper_properties_sha256",
            "version", "wrapper_cache_key",
        }
    ):
        raise CacheCopyError("SDK dependency source sections are not exact")
    for section, name in (
        (git, "executable"),
        (node, "node_modules_root"),
        (swiftpm, "cache_root"),
        (gradle, "distribution_archive"),
        (gradle, "gradle_user_home"),
    ):
        value = section.get(name)
        if not isinstance(value, str):
            raise CacheCopyError("SDK dependency source path is malformed")
        candidate = Path(value)
        _normalized_absolute(candidate, f"SDK dependency {name}")
        if candidate.resolve(strict=True) != candidate:
            raise CacheCopyError(f"SDK dependency {name} is not canonical")
    for section, name in (
        (git, "sha256"),
        (node, "package_lock_sha256"),
        (swiftpm, "package_resolved_sha256"),
        (gradle, "distribution_sha256"),
        (gradle, "java_wrapper_properties_sha256"),
        (gradle, "kotlin_wrapper_properties_sha256"),
    ):
        if not isinstance(section.get(name), str) or re.fullmatch(
            r"[0-9a-f]{64}", str(section[name]),
        ) is None:
            raise CacheCopyError(f"SDK dependency {name} is not lowercase SHA-256")
    git_digest, _, git_metadata = _digest_regular(
        Path(str(git["executable"])), "protected SDK Git executable",
    )
    if (
        git_digest != git["sha256"]
        or git_metadata.st_uid != os.geteuid()
        or git_metadata.st_nlink != 1
        or stat.S_IMODE(git_metadata.st_mode) & 0o111 == 0
    ):
        raise CacheCopyError("protected SDK Git executable changed")
    revisions = swiftpm.get("resolved_revisions")
    if not isinstance(revisions, list) or not revisions:
        raise CacheCopyError("SwiftPM resolved revisions are absent")
    seen_identities: set[str] = set()
    seen_checkouts: set[str] = set()
    for revision in revisions:
        if (
            not isinstance(revision, dict)
            or set(revision) != {"identity", "checkout", "revision", "tree"}
            or not isinstance(revision.get("identity"), str)
            or re.fullmatch(r"[a-z0-9][a-z0-9._-]*", revision["identity"]) is None
            or not isinstance(revision.get("checkout"), str)
            or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", revision["checkout"]) is None
            or not isinstance(revision.get("revision"), str)
            or SDK_OBJECT_ID_RE.fullmatch(revision["revision"]) is None
            or not isinstance(revision.get("tree"), str)
            or SDK_OBJECT_ID_RE.fullmatch(revision["tree"]) is None
            or revision["identity"] in seen_identities
            or revision["checkout"] in seen_checkouts
        ):
            raise CacheCopyError("SwiftPM resolved revision binding is malformed")
        seen_identities.add(revision["identity"])
        seen_checkouts.add(revision["checkout"])
    if revisions != sorted(revisions, key=lambda item: str(item["identity"])):
        raise CacheCopyError("SwiftPM resolved revisions are not canonically ordered")
    _sdk_source_inventory_contract(
        node["node_modules_inventory"], "Node node_modules source inventory",
    )
    _sdk_source_inventory_contract(
        swiftpm["cache_inventory"], "SwiftPM cache source inventory",
    )
    _sdk_source_inventory_contract(
        gradle["gradle_user_home_inventory"],
        "Gradle user-home source inventory",
    )
    if (
        gradle.get("version") != "9.3.0"
        or gradle.get("distribution_url") != SDK_GRADLE_DISTRIBUTION_URL
        or gradle.get("wrapper_cache_key") != SDK_GRADLE_WRAPPER_CACHE_KEY
    ):
        raise CacheCopyError("release Gradle dependency closure must be version 9.3.0")
    return document, digest


def _sdk_sources(
    manifest: dict[str, object], repository_root: Path,
) -> dict[str, Path]:
    node = manifest["node"]
    swiftpm = manifest["swiftpm"]
    gradle = manifest["gradle"]
    assert isinstance(node, dict) and isinstance(swiftpm, dict) and isinstance(gradle, dict)
    return {
        "node/node_modules": Path(str(node["node_modules_root"])),
        "node/package-lock.json": repository_root / "javascript/iroha_js/package-lock.json",
        "swiftpm/cache": Path(str(swiftpm["cache_root"])),
        "swiftpm/Package.resolved": repository_root / "IrohaSwift/Package.resolved",
        "gradle/gradle-user-home": Path(str(gradle["gradle_user_home"])),
        "gradle/gradle-9.3.0-bin.zip": Path(str(gradle["distribution_archive"])),
        "gradle/kotlin-gradle-wrapper.properties": repository_root / "kotlin/gradle/wrapper/gradle-wrapper.properties",
        "gradle/java-gradle-wrapper.properties": repository_root / "java/iroha_android/gradle/wrapper/gradle-wrapper.properties",
    }


def _sdk_validate_manifest_source_inventories(
    manifest: dict[str, object], sources: dict[str, Path],
) -> None:
    """Recompute every protected dependency tree against its member manifest."""

    node = manifest["node"]
    swiftpm = manifest["swiftpm"]
    gradle = manifest["gradle"]
    assert isinstance(node, dict)
    assert isinstance(swiftpm, dict)
    assert isinstance(gradle, dict)
    specifications = (
        (
            sources["node/node_modules"],
            node["node_modules_inventory"],
            "Node node_modules source inventory",
        ),
        (
            sources["swiftpm/cache"],
            swiftpm["cache_inventory"],
            "SwiftPM cache source inventory",
        ),
        (
            sources["gradle/gradle-user-home"],
            gradle["gradle_user_home_inventory"],
            "Gradle user-home source inventory",
        ),
    )
    for source, expected, label in specifications:
        expected_records, expected_bytes = _sdk_source_inventory_contract(
            expected, label,
        )
        observed_records, observed_bytes = _sdk_sanitized_snapshot(
            source, label,
        )
        if (
            observed_records != expected_records
            or observed_bytes != expected_bytes
            or _sdk_records_sha256(observed_records)
            != expected["records_sha256"]
        ):
            raise CacheCopyError(
                f"{label} differs from its protected complete member inventory"
            )
    _sdk_verify_swift_checkout_git(manifest, sources["swiftpm/cache"])


def _sdk_verify_swift_checkout_git(
    manifest: dict[str, object], swift_cache: Path,
) -> None:
    """Use the protected Git to bind each clean Swift checkout commit and tree."""

    git = manifest["git"]
    swiftpm = manifest["swiftpm"]
    assert isinstance(git, dict) and isinstance(swiftpm, dict)
    executable = Path(str(git["executable"]))
    environment = {
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": os.devnull,
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "",
        "TZ": "UTC",
    }
    for item in swiftpm["resolved_revisions"]:
        checkout = swift_cache / "checkouts" / str(item["checkout"])
        command = [
            str(executable),
            "-c", "core.hooksPath=/dev/null",
            "-c", "core.fsmonitor=false",
            "-C", str(checkout),
            "rev-parse", "HEAD^{commit}", "HEAD^{tree}",
        ]
        try:
            identity = subprocess.run(
                command,
                check=False,
                env=environment,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
            )
            status = subprocess.run(
                [
                    str(executable),
                    "-c", "core.hooksPath=/dev/null",
                    "-c", "core.fsmonitor=false",
                    "-C", str(checkout),
                    "status", "--porcelain=v1", "--untracked-files=all",
                ],
                check=False,
                env=environment,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise CacheCopyError("protected SDK Git verification failed") from error
        try:
            identity_lines = identity.stdout.decode("ascii", "strict").splitlines()
        except UnicodeDecodeError as error:
            raise CacheCopyError("protected SDK Git identity is malformed") from error
        if (
            identity.returncode != 0
            or identity.stderr
            or identity_lines != [item["revision"], item["tree"]]
            or status.returncode != 0
            or status.stdout
            or status.stderr
        ):
            raise CacheCopyError(
                "SwiftPM checkout is not the exact clean protected Git tree"
            )


def _sdk_source_state(
    sources: dict[str, Path],
) -> tuple[list[dict[str, object]], int]:
    records: list[dict[str, object]] = []
    budget = {"records": 0, "bytes": 0}
    for archive_name, source in sorted(sources.items()):
        metadata = source.lstat()
        if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
            descriptor, opened = _open_directory(source, f"SDK dependency {archive_name}")
            try:
                if (
                    opened.st_uid != os.geteuid()
                    or stat.S_IMODE(opened.st_mode) & 0o022
                ):
                    raise CacheCopyError(f"SDK dependency root is unsafe: {archive_name}")
                budget["records"] += 1
                records.append({
                    "path": archive_name, "kind": "directory",
                    "device": opened.st_dev, "inode": opened.st_ino,
                    "mode": format(stat.S_IMODE(opened.st_mode), "04o"),
                })
                nested: list[dict[str, object]] = []
                _snapshot_directory(descriptor, descriptor, None, (), nested, budget)
                for record in nested:
                    record["path"] = f"{archive_name}/{record['path']}"
                records.extend(nested)
            finally:
                os.close(descriptor)
            _revalidate_directory_path(source, opened, f"SDK dependency {archive_name}")
        else:
            parent_fd, _ = _open_directory(source.parent, f"SDK dependency {archive_name} parent")
            try:
                before = _entry_stat(parent_fd, source.name, f"SDK dependency {archive_name}")
                budget["records"] += 1
                records.append(_snapshot_regular(
                    parent_fd, source.name, archive_name, before, budget,
                ))
            finally:
                os.close(parent_fd)
    return sorted(records, key=lambda record: str(record["path"])), budget["bytes"]


def _sdk_source_state_sha256(records: list[dict[str, object]]) -> str:
    payload = json.dumps(records, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(payload).hexdigest()


def _sdk_sanitized_snapshot(
    root: Path, label: str,
) -> tuple[list[dict[str, object]], int]:
    descriptor, identity = _open_directory(root, label)
    records: list[dict[str, object]] = [{
        "path": ".", "kind": "directory",
        "mode": format(stat.S_IMODE(identity.st_mode), "04o"),
    }]
    raw: list[dict[str, object]] = []
    budget = {"records": 1, "bytes": 0}
    try:
        _snapshot_directory(descriptor, descriptor, None, (), raw, budget)
    finally:
        os.close(descriptor)
    _revalidate_directory_path(root, identity, label)
    fields = {
        "directory": ("path", "kind", "mode"),
        "file": ("path", "kind", "mode", "size", "sha256"),
        "symlink": ("path", "kind", "mode", "target"),
    }
    for record in raw:
        keys = fields.get(str(record.get("kind")))
        if keys is None:
            raise CacheCopyError(f"{label} inventory contains an unsupported entry")
        records.append({key: record[key] for key in keys})
    return sorted(records, key=lambda record: str(record["path"])), budget["bytes"]


def _sdk_new_directory(parent_fd: int, name: str, label: str) -> tuple[int, os.stat_result]:
    if _optional_entry_stat(parent_fd, name, label) is not None:
        raise CacheCopyError(f"{label} already exists")
    os.mkdir(name, 0o700, dir_fd=parent_fd)
    created = _entry_stat(parent_fd, name, label)
    descriptor = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent_fd)
    opened = os.fstat(descriptor)
    if (
        not _same_directory_inode(created, opened)
        or opened.st_uid != os.geteuid()
        or stat.S_IMODE(opened.st_mode) != 0o700
    ):
        os.close(descriptor)
        raise CacheCopyError(f"{label} is not owner-private")
    return descriptor, opened


def _sdk_copy_file(
    source: Path, destination_fd: int, destination_name: str, archive_name: str,
    records: list[dict[str, object]], budget: dict[str, int],
) -> None:
    source_parent, _ = _open_directory(source.parent, f"SDK dependency {archive_name} parent")
    try:
        before = _entry_stat(source_parent, source.name, f"SDK dependency {archive_name}")
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise CacheCopyError(f"SDK dependency file is not an unlinked regular file: {archive_name}")
        budget["records"] += 1
        if budget["records"] > MAXIMUM_RECORDS:
            raise CacheCopyError("SDK dependency closure contains too many entries")
        records.append(_copy_regular(
            source_parent, destination_fd, source.name, archive_name, before,
            budget, destination_name=destination_name,
        ))
    finally:
        os.close(source_parent)


def _sdk_copy_layout(
    sources: dict[str, Path], bundle_root: Path,
) -> os.stat_result:
    parent_fd, _ = _open_directory(bundle_root.parent, "SDK dependency bundle parent")
    root_fd: int | None = None
    created: os.stat_result | None = None
    complete = False
    try:
        root_fd, created = _sdk_new_directory(parent_fd, bundle_root.name, "SDK dependency bundle")
        children: dict[str, int] = {}
        try:
            for name in ("node", "swiftpm", "gradle"):
                child, _ = _sdk_new_directory(root_fd, name, f"SDK dependency {name} bundle")
                children[name] = child
            records: list[dict[str, object]] = []
            budget = {"records": 0, "bytes": 0}
            for archive_name, source in sorted(sources.items()):
                first, leaf = archive_name.split("/", 1)
                if "/" in leaf:
                    raise CacheCopyError("SDK dependency archive layout is not exact")
                if source.is_dir() and not source.is_symlink():
                    source_fd, source_identity = _open_directory(source, f"SDK dependency {archive_name}")
                    try:
                        _copy_directory(
                            source_fd, children[first], leaf, archive_name,
                            source_fd, (), records, budget,
                        )
                    finally:
                        os.close(source_fd)
                    _revalidate_directory_path(source, source_identity, f"SDK dependency {archive_name}")
                else:
                    _sdk_copy_file(
                        source, children[first], leaf, archive_name, records, budget,
                    )
        finally:
            for child in children.values():
                os.close(child)
        complete = True
        return created
    finally:
        if root_fd is not None:
            os.close(root_fd)
        os.close(parent_fd)
        if not complete and created is not None and bundle_root.exists():
            _quiescent_remove_tree(bundle_root, "partial SDK dependency bundle")


def _sdk_copy_work(bundle_root: Path, work_root: Path) -> os.stat_result:
    parent_fd, _ = _open_directory(work_root.parent, "SDK dependency work parent")
    root_fd: int | None = None
    created: os.stat_result | None = None
    complete = False
    bundle_fd, _ = _open_directory(bundle_root, "SDK dependency bundle")
    try:
        root_fd, created = _sdk_new_directory(parent_fd, work_root.name, "SDK dependency work root")
        records: list[dict[str, object]] = []
        budget = {"records": 0, "bytes": 0}
        for source_parts, destination_name in (
            (("swiftpm", "cache"), "swiftpm"),
            (("gradle", "gradle-user-home"), "gradle-home"),
        ):
            parent = os.dup(bundle_fd)
            try:
                for part in source_parts:
                    child = os.open(part, _DIRECTORY_FLAGS, dir_fd=parent)
                    os.close(parent)
                    parent = child
                _copy_directory(
                    parent, root_fd, destination_name, destination_name,
                    parent, (), records, budget,
                )
            finally:
                os.close(parent)
        complete = True
        return created
    finally:
        if root_fd is not None:
            os.close(root_fd)
        os.close(bundle_fd)
        os.close(parent_fd)
        if not complete and created is not None and work_root.exists():
            _quiescent_remove_tree(work_root, "partial SDK dependency work root")


def create_sdk_command_work(
    bundle_root: Path, command_work_root: Path,
) -> None:
    """Create one fresh disposable writable SDK work copy for one command."""

    _normalized_absolute(bundle_root, "SDK dependency immutable input root")
    _normalized_absolute(command_work_root, "SDK dependency command work root")
    if (
        command_work_root.parent != bundle_root.parent
        or command_work_root == bundle_root
        or _overlap(bundle_root, command_work_root)
    ):
        raise CacheCopyError("SDK command work root is not an exact disjoint sibling")
    _sdk_copy_work(bundle_root, command_work_root)


def cleanup_sdk_command_work(
    bundle_root: Path, command_work_root: Path,
) -> None:
    """Remove one exact fresh SDK command work copy after natural completion."""

    _normalized_absolute(bundle_root, "SDK dependency immutable input root")
    _normalized_absolute(command_work_root, "SDK dependency command work root")
    if (
        command_work_root.parent != bundle_root.parent
        or command_work_root == bundle_root
        or _overlap(bundle_root, command_work_root)
        or not command_work_root.name.startswith("sdk-command-work.")
    ):
        raise CacheCopyError("refusing to prune an unsafe SDK command work root")
    _quiescent_remove_named(
        command_work_root.parent,
        command_work_root.name,
        "SDK command work root",
        require_directory=True,
    )


def _sdk_reject_path_disclosure(
    bundle_root: Path, records: list[dict[str, object]], sources: dict[str, Path],
    forbidden_roots: tuple[Path, ...] = (),
) -> None:
    tokens = {
        os.fsencode(str(path))
        for source in (*sources.values(), *forbidden_roots)
        for path in (source, source.parent)
        if len(str(path)) > 1
    }
    longest = max((len(token) for token in tokens), default=1)
    for record in records:
        if record.get("kind") != "file" or record.get("path") == ".":
            continue
        path = bundle_root / str(record["path"])
        parent_fd, _ = _open_directory(path.parent, "SDK dependency disclosure scan parent")
        try:
            descriptor = os.open(
                path.name,
                os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
                dir_fd=parent_fd,
            )
            tail = b""
            try:
                while block := os.read(descriptor, 1024 * 1024):
                    sample = tail + block
                    if any(token in sample for token in tokens):
                        raise CacheCopyError(
                            "SDK dependency bundle contains an original absolute path"
                        )
                    tail = sample[-(longest - 1):] if longest > 1 else b""
            finally:
                os.close(descriptor)
        finally:
            os.close(parent_fd)


def _sdk_json(path: Path, label: str) -> tuple[dict[str, object], bytes]:
    payload, _ = _read_regular(path, label)
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError(f"{label} is malformed") from error
    if not isinstance(value, dict):
        raise CacheCopyError(f"{label} is not a JSON object")
    return value, payload


def _sdk_gradle_distribution(
    bundle_root: Path, expected_sha256: str,
) -> str:
    archive = bundle_root / "gradle/gradle-9.3.0-bin.zip"
    digest, _, _ = _digest_regular(archive, "Gradle 9.3.0 distribution archive")
    if digest != expected_sha256:
        raise CacheCopyError("Gradle 9.3.0 distribution archive digest changed")
    home = bundle_root / "gradle/gradle-user-home"
    if {path.name for path in home.iterdir()} != {"caches", "wrapper"}:
        raise CacheCopyError("Gradle offline home is not the exact curated cache closure")
    cache_names = {path.name for path in (home / "caches").iterdir()}
    if (
        not {"9.3.0", "modules-2"} <= cache_names
        or not cache_names <= {
            "9.3.0", "CACHEDIR.TAG", "jars-9", "journal-1", "modules-2",
        }
    ):
        raise CacheCopyError("Gradle offline home lacks versioned and module caches")
    wrapper_names = {path.name for path in (home / "wrapper").iterdir()}
    if wrapper_names != {"dists"}:
        raise CacheCopyError("Gradle wrapper home is not the exact distribution closure")
    dists_names = {path.name for path in (home / "wrapper/dists").iterdir()}
    if not {"gradle-9.3.0-bin"} <= dists_names or not dists_names <= {
        "CACHEDIR.TAG", "gradle-9.3.0-bin",
    }:
        raise CacheCopyError("Gradle wrapper home contains an unrelated distribution")
    if any(
        path.name.endswith((".lock", ".lck"))
        for path in home.rglob("*")
    ):
        raise CacheCopyError("Gradle offline archive contains a mutable lock file")
    distribution_root = home / "wrapper/dists/gradle-9.3.0-bin"
    cache_keys = [path for path in distribution_root.iterdir() if path.is_dir() and not path.is_symlink()]
    if (
        len(cache_keys) != 1
        or cache_keys[0].name != SDK_GRADLE_WRAPPER_CACHE_KEY
    ):
        raise CacheCopyError("Gradle 9.3.0 wrapper distribution cache is not exact")
    cache_entries = {path.name for path in cache_keys[0].iterdir()}
    if cache_entries != {"gradle-9.3.0", "gradle-9.3.0-bin.zip.ok"}:
        raise CacheCopyError("Gradle 9.3.0 wrapper cache contains stale or mutable entries")
    extracted = cache_keys[0] / "gradle-9.3.0"
    actual_records, _ = _sdk_sanitized_snapshot(extracted, "extracted Gradle 9.3.0")
    actual_files = {
        str(record["path"]): record
        for record in actual_records if record.get("kind") == "file"
    }
    actual_directories = {
        str(record["path"])
        for record in actual_records
        if record.get("kind") == "directory" and record.get("path") != "."
    }
    expected_files: set[str] = set()
    expected_directories: set[str] = set()
    expanded_bytes = 0
    try:
        with zipfile.ZipFile(archive) as distribution:
            infos = distribution.infolist()
            if len({info.filename for info in infos}) != len(infos):
                raise CacheCopyError("Gradle distribution ZIP has duplicate members")
            folded_names: set[str] = set()
            for info in infos:
                name = PurePosixPath(info.filename)
                canonical_name = name.as_posix() + ("/" if info.is_dir() else "")
                member_mode = (info.external_attr >> 16) & 0o177777
                member_type = stat.S_IFMT(member_mode)
                if (
                    info.flag_bits & 1
                    or info.filename != canonical_name
                    or unicodedata.normalize("NFC", info.filename) != info.filename
                    or name.is_absolute()
                    or ".." in name.parts
                    or "\\" in info.filename
                    or not name.parts
                    or name.parts[0] != "gradle-9.3.0"
                    or member_type == stat.S_IFLNK
                    or member_type not in {0, stat.S_IFREG, stat.S_IFDIR}
                ):
                    raise CacheCopyError("Gradle distribution ZIP member is unsafe")
                folded = canonical_name.rstrip("/").casefold()
                if folded in folded_names:
                    raise CacheCopyError(
                        "Gradle distribution ZIP has a platform-alias member"
                    )
                folded_names.add(folded)
                relative_path = PurePosixPath(*name.parts[1:])
                relative = relative_path.as_posix()
                if info.is_dir():
                    if relative and relative != ".":
                        expected_directories.add(relative)
                    continue
                if not relative:
                    raise CacheCopyError("Gradle distribution ZIP member is malformed")
                expected_directories.update(
                    parent.as_posix()
                    for parent in relative_path.parents
                    if parent.as_posix() != "."
                )
                expanded_bytes += info.file_size
                if info.file_size > MAXIMUM_FILE_BYTES or expanded_bytes > MAXIMUM_TOTAL_BYTES:
                    raise CacheCopyError("Gradle distribution ZIP exceeds release bounds")
                record = actual_files.get(relative)
                member_digest = hashlib.sha256()
                with distribution.open(info) as member:
                    while block := member.read(1024 * 1024):
                        member_digest.update(block)
                if (
                    not isinstance(record, dict)
                    or record.get("size") != info.file_size
                    or record.get("sha256") != member_digest.hexdigest()
                ):
                    raise CacheCopyError("extracted Gradle distribution disagrees with its ZIP")
                expected_files.add(relative)
    except zipfile.BadZipFile as error:
        raise CacheCopyError("Gradle 9.3.0 distribution is not a valid ZIP") from error
    if set(actual_files) != expected_files:
        raise CacheCopyError("extracted Gradle distribution has unbound files")
    if actual_directories != expected_directories:
        raise CacheCopyError("extracted Gradle distribution has unbound directories")
    launcher = actual_files.get("bin/gradle")
    if not isinstance(launcher, dict) or int(str(launcher["mode"]), 8) & 0o111 == 0:
        raise CacheCopyError("extracted Gradle launcher is not executable")
    return cache_keys[0].name


def _sdk_bindings(
    bundle_root: Path, manifest: dict[str, object],
) -> dict[str, object]:
    node = manifest["node"]
    swiftpm = manifest["swiftpm"]
    gradle = manifest["gradle"]
    assert isinstance(node, dict) and isinstance(swiftpm, dict) and isinstance(gradle, dict)
    package_lock, package_bytes = _sdk_json(
        bundle_root / "node/package-lock.json", "SDK package-lock.json",
    )
    hidden_lock, hidden_bytes = _sdk_json(
        bundle_root / "node/node_modules/.package-lock.json", "installed Node lockfile",
    )
    if (
        hashlib.sha256(package_bytes).hexdigest() != node["package_lock_sha256"]
        or package_lock.get("lockfileVersion") != 3
        or hidden_lock.get("lockfileVersion") != 3
        or (hidden_lock.get("name"), hidden_lock.get("version"))
        != (package_lock.get("name"), package_lock.get("version"))
        or not isinstance(package_lock.get("packages"), dict)
        or not isinstance(hidden_lock.get("packages"), dict)
        or not hidden_lock["packages"]
        or any(
            package_lock["packages"].get(name) != value
            for name, value in hidden_lock["packages"].items()
        )
    ):
        raise CacheCopyError("installed Node closure is not bound to package-lock.json")
    resolved, resolved_bytes = _sdk_json(
        bundle_root / "swiftpm/Package.resolved", "Swift Package.resolved",
    )
    resolved_pairs = sorted(
        (
            {"identity": pin.get("identity"), "revision": pin.get("state", {}).get("revision")}
            for pin in resolved.get("pins", [])
            if isinstance(pin, dict) and isinstance(pin.get("state"), dict)
        ),
        key=lambda item: str(item["identity"]),
    ) if isinstance(resolved.get("pins"), list) else []
    expected_revisions = [
        {"identity": item["identity"], "revision": item["revision"]}
        for item in swiftpm["resolved_revisions"]
    ]
    if (
        hashlib.sha256(resolved_bytes).hexdigest() != swiftpm["package_resolved_sha256"]
        or resolved.get("version") != 2
        or resolved_pairs != expected_revisions
    ):
        raise CacheCopyError("Swift Package.resolved revisions changed")
    checkout_root = bundle_root / "swiftpm/cache/checkouts"
    expected_checkouts = {str(item["checkout"]) for item in swiftpm["resolved_revisions"]}
    if {path.name for path in checkout_root.iterdir()} != expected_checkouts:
        raise CacheCopyError("SwiftPM checkout inventory changed")
    if {path.name for path in (bundle_root / "swiftpm/cache").iterdir()} != {"checkouts", "repositories"}:
        raise CacheCopyError("SwiftPM cache is not the exact path-free source closure")
    for item in swiftpm["resolved_revisions"]:
        head, _ = _read_regular(
            checkout_root / str(item["checkout"]) / ".git/HEAD",
            "SwiftPM detached checkout HEAD",
        )
        if head.decode("ascii", "strict").strip() != item["revision"]:
            raise CacheCopyError("SwiftPM checkout does not match Package.resolved")
    wrapper_digests: dict[str, str] = {}
    for kind in ("kotlin", "java"):
        wrapper = bundle_root / f"gradle/{kind}-gradle-wrapper.properties"
        payload, _ = _read_regular(wrapper, f"{kind} Gradle wrapper properties")
        digest = hashlib.sha256(payload).hexdigest()
        if digest != gradle[f"{kind}_wrapper_properties_sha256"]:
            raise CacheCopyError(f"{kind} Gradle wrapper properties changed")
        values = dict(
            line.split("=", 1) for line in payload.decode("utf-8").splitlines()
            if line and not line.startswith("#") and "=" in line
        )
        if values.get("distributionUrl") != SDK_GRADLE_DISTRIBUTION_URL.replace(
            ":", r"\:", 1,
        ):
            raise CacheCopyError(f"{kind} Gradle wrapper does not select 9.3.0")
        pinned = values.get("distributionSha256Sum")
        if pinned is not None and pinned != gradle["distribution_sha256"]:
            raise CacheCopyError(f"{kind} Gradle wrapper checksum disagrees with the protected manifest")
        wrapper_digests[kind] = digest
    gradle_cache_key = _sdk_gradle_distribution(
        bundle_root, str(gradle["distribution_sha256"]),
    )
    return {
        "node": {
            "node_modules_archive_name": "node/node_modules",
            "package_lock_archive_name": "node/package-lock.json",
            "package_lock_sha256": node["package_lock_sha256"],
            "installed_lock_sha256": hashlib.sha256(hidden_bytes).hexdigest(),
        },
        "swiftpm": {
            "cache_archive_name": "swiftpm/cache",
            "package_resolved_archive_name": "swiftpm/Package.resolved",
            "package_resolved_sha256": swiftpm["package_resolved_sha256"],
            "resolved_revisions": swiftpm["resolved_revisions"],
        },
        "gradle": {
            "distribution_archive_name": "gradle/gradle-9.3.0-bin.zip",
            "distribution_sha256": gradle["distribution_sha256"],
            "distribution_url": SDK_GRADLE_DISTRIBUTION_URL,
            "gradle_user_home_archive_name": "gradle/gradle-user-home",
            "wrapper_cache_key": gradle_cache_key,
            "launcher_archive_name": SDK_GRADLE_LAUNCHER_ARCHIVE_NAME,
            "version": "9.3.0",
            "wrapper_properties_sha256": wrapper_digests,
        },
    }


def _sdk_publish_tar(
    bundle_root: Path, archive_path: Path, records: list[dict[str, object]],
) -> tuple[dict[str, object], tuple[os.stat_result, os.stat_result]]:
    bundle_fd, bundle_identity = _open_directory(
        bundle_root, "SDK dependency archive source root",
    )
    parent_fd, parent_identity = _open_directory(archive_path.parent, "SDK archive parent")
    temporary = f".sdk-archive.{secrets.token_hex(16)}"
    descriptor: int | None = None
    published: os.stat_result | None = None
    try:
        if _optional_entry_stat(parent_fd, archive_path.name, "SDK dependency archive") is not None:
            raise CacheCopyError("SDK dependency archive already exists")
        descriptor = os.open(
            temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
            0o600, dir_fd=parent_fd,
        )
        with os.fdopen(os.dup(descriptor), "wb", closefd=True) as stream:
            with tarfile.open(fileobj=stream, mode="w", format=tarfile.PAX_FORMAT) as archive:
                for record in records:
                    relative = str(record["path"])
                    name = "sdk-inputs" if relative == "." else f"sdk-inputs/{relative}"
                    member = tarfile.TarInfo(name)
                    member.mode = int(str(record["mode"]), 8)
                    member.uid = member.gid = member.mtime = 0
                    member.uname = member.gname = ""
                    kind = record["kind"]
                    if kind == "directory":
                        member.type = tarfile.DIRTYPE
                        archive.addfile(member)
                    elif kind == "symlink":
                        member.type = tarfile.SYMTYPE
                        member.linkname = str(record["target"])
                        archive.addfile(member)
                    elif kind == "file":
                        member.size = int(record["size"])
                        parts = PurePosixPath(relative).parts
                        directory_fd = os.dup(bundle_fd)
                        try:
                            for part in parts[:-1]:
                                child = os.open(
                                    part, _DIRECTORY_FLAGS, dir_fd=directory_fd,
                                )
                                os.close(directory_fd)
                                directory_fd = child
                            source_fd = os.open(
                                parts[-1],
                                os.O_RDONLY
                                | getattr(os, "O_CLOEXEC", 0)
                                | getattr(os, "O_NOFOLLOW", 0),
                                dir_fd=directory_fd,
                            )
                            try:
                                opened = os.fstat(source_fd)
                                if (
                                    not stat.S_ISREG(opened.st_mode)
                                    or opened.st_nlink != 1
                                    or opened.st_size != member.size
                                    or stat.S_IMODE(opened.st_mode)
                                    != int(str(record["mode"]), 8)
                                ):
                                    raise CacheCopyError(
                                        "SDK dependency archive source changed"
                                    )
                                with os.fdopen(
                                    os.dup(source_fd), "rb", closefd=True,
                                ) as source_stream:
                                    archive.addfile(member, source_stream)
                            finally:
                                os.close(source_fd)
                        finally:
                            os.close(directory_fd)
                    else:
                        raise CacheCopyError("SDK dependency archive member kind is unsupported")
        os.fsync(descriptor)
        os.fchmod(descriptor, 0o400)
        final = os.fstat(descriptor)
        _rename_noreplace_at(parent_fd, temporary, archive_path.name)
        published = _entry_stat(parent_fd, archive_path.name, "SDK dependency archive")
        if (
            any(
                getattr(final, field) != getattr(published, field)
                for field in IDENTITY_FIELDS if field != "st_ctime_ns"
            )
            or published.st_nlink != 1
            or published.st_uid != os.geteuid()
            or stat.S_IMODE(published.st_mode) != 0o400
        ):
            raise CacheCopyError("SDK dependency archive publication is unsafe")
        os.fsync(parent_fd)
        digest, size, _ = _digest_regular(archive_path, "SDK dependency archive")
        _revalidate_directory_path(
            bundle_root, bundle_identity,
            "SDK dependency archive source root",
        )
        binding = {
            "archive_id": SDK_BUNDLE_ARCHIVE_ID,
            "archive_name": archive_path.name,
            "mode": "0400", "size_bytes": size, "sha256": digest,
        }
        return binding, (published, parent_identity)
    except BaseException:
        if published is not None:
            _owned_remove_entry(
                parent_fd, archive_path.name,
                (published.st_dev, published.st_ino), "partial SDK dependency archive",
            )
        raise
    finally:
        if descriptor is not None:
            os.close(descriptor)
        temporary_metadata = _optional_entry_stat(parent_fd, temporary, "SDK archive temporary")
        if temporary_metadata is not None:
            _owned_remove_entry(
                parent_fd, temporary, (temporary_metadata.st_dev, temporary_metadata.st_ino),
                "SDK archive temporary",
            )
        os.close(parent_fd)
        os.close(bundle_fd)


def _sdk_verify_tar(
    archive_path: Path, archive_binding: dict[str, object],
    records: list[dict[str, object]],
) -> None:
    digest, size, metadata = _digest_regular(archive_path, "SDK dependency archive")
    if archive_binding != {
        "archive_id": SDK_BUNDLE_ARCHIVE_ID,
        "archive_name": archive_path.name,
        "mode": "0400", "size_bytes": size, "sha256": digest,
    } or stat.S_IMODE(metadata.st_mode) != 0o400:
        raise CacheCopyError("SDK dependency archive binding changed")
    expected = {
        ("sdk-inputs" if record["path"] == "." else f"sdk-inputs/{record['path']}"): record
        for record in records
    }
    try:
        with tarfile.open(archive_path, mode="r:") as archive:
            members = archive.getmembers()
            if len(members) != len(expected) or {member.name for member in members} != set(expected):
                raise CacheCopyError("SDK dependency archive member inventory changed")
            for member in members:
                record = expected[member.name]
                kind = record["kind"]
                if (
                    member.uid != 0 or member.gid != 0 or member.mtime != 0
                    or member.mode != int(str(record["mode"]), 8)
                    or (kind == "directory" and not member.isdir())
                    or (kind == "symlink" and (not member.issym() or member.linkname != record["target"]))
                    or (kind == "file" and (not member.isfile() or member.size != record["size"]))
                ):
                    raise CacheCopyError("SDK dependency archive member metadata changed")
                if kind == "file":
                    stream = archive.extractfile(member)
                    if stream is None:
                        raise CacheCopyError("SDK dependency archive file is unavailable")
                    member_digest = hashlib.sha256()
                    while block := stream.read(1024 * 1024):
                        member_digest.update(block)
                    if member_digest.hexdigest() != record["sha256"]:
                        raise CacheCopyError("SDK dependency archive file bytes changed")
    except tarfile.TarError as error:
        raise CacheCopyError("SDK dependency archive is malformed") from error


def _sdk_inventory(
    inventory_path: Path,
) -> tuple[dict[str, object], bytes]:
    payload, metadata = _read_regular(inventory_path, "SDK dependency inventory")
    try:
        document = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError("SDK dependency inventory is malformed") from error
    keys = {
        "format", "schema_version", "archive_id", "source_disclosure",
        "source_manifest_sha256", "source_state_sha256", "bindings", "archive",
        "record_count", "file_bytes", "records", "work_initial_record_count",
        "work_initial_file_bytes", "work_initial_records",
    }
    if (
        not isinstance(document, dict) or set(document) != keys
        or document.get("format") != SDK_BUNDLE_FORMAT
        or document.get("schema_version") != 1
        or document.get("archive_id") != SDK_BUNDLE_ARCHIVE_ID
        or document.get("source_disclosure") != "withheld"
        or stat.S_IMODE(metadata.st_mode) != 0o400
        or payload != _canonical_payload(document)
        or not isinstance(document.get("records"), list)
        or not isinstance(document.get("work_initial_records"), list)
    ):
        raise CacheCopyError("SDK dependency inventory contract is not exact")
    for name in ("records", "work_initial_records"):
        for record in document[name]:
            kind = record.get("kind") if isinstance(record, dict) else None
            expected_keys = {
                "directory": {"path", "kind", "mode"},
                "file": {"path", "kind", "mode", "size", "sha256"},
                "symlink": {"path", "kind", "mode", "target"},
            }.get(kind)
            path = record.get("path") if isinstance(record, dict) else None
            if (
                expected_keys is None or set(record) != expected_keys
                or not isinstance(path, str)
                or (path != "." and (
                    PurePosixPath(path).as_posix() != path
                    or PurePosixPath(path).is_absolute()
                    or ".." in PurePosixPath(path).parts
                ))
                or not isinstance(record.get("mode"), str)
                or re.fullmatch(r"[0-7]{4}", str(record["mode"])) is None
            ):
                raise CacheCopyError("SDK dependency member record is malformed")
    return document, payload


def verify_sdk_dependencies(
    source_manifest: Path, expected_source_manifest_sha256: str,
    repository_root: Path, bundle_root: Path, work_root: Path,
    archive_path: Path, inventory_path: Path,
    final_work_inventory: Path | None = None,
    *, verify_initial_work: bool = False,
) -> None:
    """Reauthenticate external sources, retained archive, and private work roots."""

    manifest, manifest_sha256 = _sdk_source_manifest(
        source_manifest, expected_source_manifest_sha256,
    )
    parent_fd, parent_metadata = _open_directory(
        bundle_root.parent, "SDK dependency output parent",
    )
    os.close(parent_fd)
    if (
        parent_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(parent_metadata.st_mode) != 0o700
    ):
        raise CacheCopyError("SDK dependency output parent is not owner-private")
    inventory, inventory_payload = _sdk_inventory(inventory_path)
    if inventory["source_manifest_sha256"] != manifest_sha256:
        raise CacheCopyError("SDK dependency source-manifest binding changed")
    sources = _sdk_sources(manifest, repository_root)
    _sdk_validate_manifest_source_inventories(manifest, sources)
    source_records, _ = _sdk_source_state(sources)
    if inventory["source_state_sha256"] != _sdk_source_state_sha256(source_records):
        raise CacheCopyError("SDK dependency source closure changed after copy")
    records, file_bytes = _sdk_sanitized_snapshot(bundle_root, "SDK dependency bundle")
    if (
        not records or records[0] != {
            "path": ".", "kind": "directory", "mode": "0500",
        }
        or inventory["records"] != records
        or inventory["record_count"] != len(records)
        or inventory["file_bytes"] != file_bytes
        or inventory["bindings"] != _sdk_bindings(bundle_root, manifest)
    ):
        raise CacheCopyError("SDK dependency bundle changed after publication")
    _sdk_reject_path_disclosure(
        bundle_root, records, sources, (repository_root,),
    )
    if not isinstance(inventory.get("archive"), dict):
        raise CacheCopyError("SDK dependency archive binding is malformed")
    _sdk_verify_tar(archive_path, inventory["archive"], records)
    work_records, work_bytes = _sdk_sanitized_snapshot(work_root, "SDK dependency work root")
    if not work_records or work_records[0] != {
        "path": ".", "kind": "directory", "mode": "0700",
    }:
        raise CacheCopyError("SDK dependency work root is not owner-private and writable")
    if verify_initial_work and (
        inventory["work_initial_records"] != work_records
        or inventory["work_initial_record_count"] != len(work_records)
        or inventory["work_initial_file_bytes"] != work_bytes
    ):
        raise CacheCopyError("SDK dependency work root did not start from its archive")
    if final_work_inventory is not None:
        if any(
            path.name.startswith("sdk-command-work.")
            for path in bundle_root.parent.iterdir()
        ):
            raise CacheCopyError(
                "an SDK command work root survived natural completion"
            )
        if (
            inventory["work_initial_records"] != work_records
            or inventory["work_initial_record_count"] != len(work_records)
            or inventory["work_initial_file_bytes"] != work_bytes
        ):
            raise CacheCopyError(
                "SDK dependency work template changed after child execution"
            )
        document = {
            "format": SDK_WORK_FORMAT, "schema_version": 1,
            "archive_id": "release-sdk-dependencies.work-final.v1",
            "sdk_dependency_inventory_sha256": hashlib.sha256(inventory_payload).hexdigest(),
            "record_count": len(work_records), "file_bytes": work_bytes,
            "records": work_records,
        }
        _publish_inventory(final_work_inventory, _canonical_payload(document))


def copy_sdk_dependencies(
    source_manifest: Path, expected_source_manifest_sha256: str,
    repository_root: Path, bundle_root: Path, work_root: Path,
    archive_path: Path, inventory_path: Path,
) -> None:
    """Create one inode-independent, path-withheld offline SDK dependency archive."""

    for path, label in (
        (repository_root, "SDK candidate root"), (bundle_root, "SDK bundle root"),
        (work_root, "SDK work root"), (archive_path, "SDK archive"),
        (inventory_path, "SDK inventory"),
    ):
        _normalized_absolute(path, label)
    if (
        len({bundle_root.parent, work_root.parent, archive_path.parent, inventory_path.parent}) != 1
        or len({bundle_root, work_root, archive_path, inventory_path}) != 4
    ):
        raise CacheCopyError("SDK dependency outputs are not exact siblings")
    parent_fd, parent_metadata = _open_directory(
        bundle_root.parent, "SDK dependency output parent",
    )
    os.close(parent_fd)
    if (
        parent_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(parent_metadata.st_mode) != 0o700
    ):
        raise CacheCopyError("SDK dependency output parent is not owner-private")
    manifest, manifest_sha256 = _sdk_source_manifest(
        source_manifest, expected_source_manifest_sha256,
    )
    sources = _sdk_sources(manifest, repository_root)
    _sdk_validate_manifest_source_inventories(manifest, sources)
    if any(_overlap(source, bundle_root) or _overlap(source, work_root) for source in sources.values()):
        raise CacheCopyError("SDK dependency source and private output roots overlap")
    before_records, _ = _sdk_source_state(sources)
    created_bundle: os.stat_result | None = None
    created_work: os.stat_result | None = None
    archive_publication: tuple[os.stat_result, os.stat_result] | None = None
    inventory_publication: tuple[os.stat_result, os.stat_result] | None = None
    try:
        created_bundle = _sdk_copy_layout(sources, bundle_root)
        after_records, _ = _sdk_source_state(sources)
        if after_records != before_records:
            raise CacheCopyError("SDK dependency sources changed during private copy")
        unsealed_records, _ = _sdk_sanitized_snapshot(bundle_root, "SDK dependency bundle")
        _sdk_reject_path_disclosure(
            bundle_root, unsealed_records, sources, (repository_root,),
        )
        bindings = _sdk_bindings(bundle_root, manifest)
        _seal_copied_tree(bundle_root, "SDK dependency bundle")
        records, file_bytes = _sdk_sanitized_snapshot(bundle_root, "SDK dependency bundle")
        created_work = _sdk_copy_work(bundle_root, work_root)
        work_records, work_bytes = _sdk_sanitized_snapshot(work_root, "SDK dependency work root")
        archive_binding, archive_publication = _sdk_publish_tar(
            bundle_root, archive_path, records,
        )
        document = {
            "format": SDK_BUNDLE_FORMAT, "schema_version": 1,
            "archive_id": SDK_BUNDLE_ARCHIVE_ID, "source_disclosure": "withheld",
            "source_manifest_sha256": manifest_sha256,
            "source_state_sha256": _sdk_source_state_sha256(before_records),
            "bindings": bindings, "archive": archive_binding,
            "record_count": len(records), "file_bytes": file_bytes, "records": records,
            "work_initial_record_count": len(work_records),
            "work_initial_file_bytes": work_bytes,
            "work_initial_records": work_records,
        }
        rendered = _canonical_payload(document)
        if any(os.fsencode(str(path)) in rendered for path in sources.values()):
            raise CacheCopyError("SDK dependency inventory disclosed an original path")
        inventory_publication = _publish_inventory(inventory_path, rendered)
        verify_sdk_dependencies(
            source_manifest, expected_source_manifest_sha256, repository_root,
            bundle_root, work_root, archive_path, inventory_path,
            verify_initial_work=True,
        )
    except BaseException:
        _remove_published(inventory_path, inventory_publication)
        _remove_published(archive_path, archive_publication)
        for root, created, label in (
            (work_root, created_work, "partial SDK dependency work root"),
            (bundle_root, created_bundle, "partial SDK dependency bundle"),
        ):
            if created is not None and (root.exists() or root.is_symlink()):
                _quiescent_remove_tree(root, label)
        raise


def _populate_runtime(
    runtime_root: Path, sources: list[Path], inventory_path: Path,
) -> tuple[os.stat_result, os.stat_result]:
    """Create an inode-independent child runtime, including one complete Rust sysroot."""

    resolved = [source.resolve(strict=True) for source in sources]
    names = _runtime_names(resolved)
    framework_python = _framework_python_closure(resolved)
    if framework_python is not None:
        version_root, framework, stdlib_name = framework_python
        _reject_framework_python_destination_overlap(
            version_root, runtime_root, inventory_path,
        )
        _validate_framework_python_sources(version_root, framework)
    cargo_index = names.index("cargo")
    rustc_index = names.index("rustc")
    cargo, rustc = resolved[cargo_index], resolved[rustc_index]
    cargo_toolchain = cargo.parent.parent
    if (
        cargo.name != "cargo" or rustc.name != "rustc"
        or cargo.parent.name != "bin" or rustc.parent != cargo.parent
    ):
        raise CacheCopyError("Cargo and rustc do not share one selected Rust toolchain")
    bin_root = runtime_root / "bin"
    toolchain = runtime_root / "rust-toolchain"
    input_records: list[dict[str, object]] = []
    input_budget = {"records": 0, "bytes": 0}
    def copy_stable_tree(
        source: Path,
        destination: Path,
        label: str,
        *,
        symlink_root: Path | None = None,
        symlink_parts: tuple[str, ...] = (),
    ) -> None:
        source_fd, source_identity = _open_directory(source, label)
        destination_parent_fd, destination_parent_identity = _open_directory(
            destination.parent, f"private {label} parent",
        )
        symlink_root_fd: int | None = None
        try:
            if symlink_root is None:
                symlink_root_fd = os.dup(source_fd)
            else:
                symlink_root_fd, _ = _open_directory(
                    symlink_root, f"{label} symlink root",
                )
            _copy_directory(
                source_fd, destination_parent_fd, destination.name,
                destination.name, symlink_root_fd, symlink_parts,
                input_records, input_budget,
            )
            _revalidate_directory_path(source, source_identity, label)
            _revalidate_directory_path(
                destination.parent, destination_parent_identity,
                f"private {label} parent",
            )
        finally:
            if symlink_root_fd is not None:
                os.close(symlink_root_fd)
            os.close(source_fd)
            os.close(destination_parent_fd)

    def copy_stable_file(source: Path, destination: Path, label: str) -> None:
        source_parent_fd, source_parent_identity = _open_directory(source.parent, f"{label} parent")
        destination_parent_fd, destination_parent_identity = _open_directory(destination.parent, f"private {label} parent")
        try:
            before = _entry_stat(source_parent_fd, source.name, label)
            if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode):
                raise CacheCopyError(f"runtime source is not a regular file: {label}")
            input_budget["records"] += 1
            input_records.append(_copy_regular(
                source_parent_fd, destination_parent_fd, source.name,
                destination.name, before, input_budget,
                destination_name=destination.name,
            ))
            _revalidate_entry(source_parent_fd, source.name, before, label)
            _revalidate_directory_path(source.parent, source_parent_identity, f"{label} parent")
            _revalidate_directory_path(destination.parent, destination_parent_identity, f"private {label} parent")
        finally:
            os.close(source_parent_fd)
            os.close(destination_parent_fd)

    copy_stable_tree(cargo_toolchain, toolchain, "selected Rust toolchain")
    source_roots = _runtime_source_roots(resolved)
    if names == _RELEASE_RUNTIME_NAMES:
        swift_root = source_roots["swift-toolchain"]
        java_root = source_roots["java-runtime"]
        verus_root = source_roots["verus-distribution"]
        if (
            resolved[names.index("swift")] != swift_root / "bin" / "swift"
            or resolved[names.index("java")] != java_root / "bin" / "java"
            or resolved[names.index("verus")] != verus_root / "verus"
            or resolved[names.index("cargo-verus")]
            != verus_root / "cargo-verus"
        ):
            raise CacheCopyError("release runtime executables do not match their copied closures")
        copy_stable_tree(swift_root, runtime_root / "swift-toolchain", "Swift toolchain")
        copy_stable_tree(java_root, runtime_root / "java-runtime", "Java runtime")
        copy_stable_tree(verus_root, runtime_root / "verus-distribution", "Verus distribution")
    for name, source in zip(names, resolved):
        if name in {"cargo", "rustc"}:
            destination = toolchain / "bin" / name
        elif name == "swift":
            destination = runtime_root / "swift-toolchain" / "bin" / name
        elif name == "java":
            destination = runtime_root / "java-runtime" / "bin" / name
        elif name in {"verus", "cargo-verus"}:
            destination = runtime_root / "verus-distribution" / name
        else:
            destination = (
                runtime_root / name
                if name in {
                    "copy-release-runtime.py", "tla2tools.jar", "tlapm-stdlib",
                }
                else bin_root / name
            )
            if source.is_dir():
                copy_stable_tree(source, destination, f"runtime source {name}")
            else:
                copy_stable_file(source, destination, f"runtime source {name}")
        if name in {"cargo", "rustc", "swift", "java", "verus", "cargo-verus"}:
            link = bin_root / name
            root_name = {
                "cargo": "rust-toolchain/bin", "rustc": "rust-toolchain/bin",
                "swift": "swift-toolchain/bin", "java": "java-runtime/bin",
                "verus": "verus-distribution", "cargo-verus": "verus-distribution",
            }[name]
            os.symlink(f"../{root_name}/{name}", link)
    if framework_python is not None:
        copy_stable_file(
            version_root / framework,
            runtime_root / framework,
            "Python framework library",
        )
        for name in ("Resources", "lib"):
            copy_stable_tree(
                version_root / name,
                runtime_root / name,
                f"Python framework {name}",
                symlink_root=version_root,
                symlink_parts=(name,),
            )
        _validate_framework_python_sources(version_root, framework)
    _seal_copied_tree(
        runtime_root,
        "runtime",
        owner_private_directory_roots=(
            (("Resources", "Python.app"),)
            if framework_python is not None
            else ()
        ),
    )
    if framework_python is not None:
        _probe_framework_python_runtime(runtime_root, stdlib_name)
    runtime_fd, runtime_identity = _open_directory(runtime_root, "private runtime")
    records: list[dict[str, object]] = []
    runtime_budget = {"records": 0, "bytes": 0}
    try:
        _snapshot_directory(runtime_fd, runtime_fd, None, (), records, runtime_budget)
    finally:
        os.close(runtime_fd)
    _revalidate_directory_path(runtime_root, runtime_identity, "private runtime")
    if framework_python is not None:
        _validate_framework_python_runtime_records(
            records, framework, stdlib_name,
        )
    _bind_runtime_destinations(input_records, records, update=True)
    document = {
        "format": "iroha-sumeragi-v2-private-runtime",
        "schema_version": 1,
        "runtime_root": str(runtime_root),
        "record_count": len(records), "file_bytes": runtime_budget["bytes"],
        "records": sorted(records, key=lambda record: str(record["path"])),
        "source_disclosure": "withheld",
        "input_record_count": len(input_records),
        "input_file_bytes": input_budget["bytes"],
        "input_records": sorted(input_records, key=lambda record: str(record["path"])),
    }
    _verify_runtime_sources(source_roots, input_records)
    return _publish_inventory(inventory_path, _canonical_payload(document))


def copy_runtime(runtime_root: Path, sources: list[Path], inventory_path: Path) -> None:
    """Create one owned runtime root without path-based check/create races."""

    _normalized_absolute(runtime_root, "private child runtime")
    _normalized_absolute(inventory_path, "runtime inventory")
    if len(sources) not in {len(_PR_RUNTIME_NAMES), len(_RELEASE_RUNTIME_NAMES)} or inventory_path.parent != runtime_root.parent:
        raise CacheCopyError("private child runtime inputs are not exact")
    parent_fd, parent_identity = _open_directory(runtime_root.parent, "private runtime parent")
    created = False
    published: tuple[os.stat_result, os.stat_result] | None = None
    try:
        try:
            _entry_stat(parent_fd, runtime_root.name, "private child runtime")
        except CacheCopyError as error:
            if not isinstance(error.__cause__, FileNotFoundError):
                raise
        else:
            raise CacheCopyError("private child runtime already exists")
        os.mkdir(runtime_root.name, mode=0o700, dir_fd=parent_fd)
        created = True
        runtime_fd = os.open(runtime_root.name, _DIRECTORY_FLAGS, dir_fd=parent_fd)
        try:
            opened = os.fstat(runtime_fd)
            if opened.st_uid != os.geteuid() or stat.S_IMODE(opened.st_mode) != 0o700:
                raise CacheCopyError("private child runtime root is unsafe")
            os.mkdir("bin", mode=0o700, dir_fd=runtime_fd)
        finally:
            os.close(runtime_fd)
        published = _populate_runtime(runtime_root, sources, inventory_path)
        _revalidate_directory_path(runtime_root.parent, parent_identity, "private runtime parent")
        verify_runtime_sources(sources, runtime_root, inventory_path)
    except BaseException:
        _remove_published(inventory_path, published)
        if created:
            _owned_remove_tree(parent_fd, runtime_root.name, "partial private runtime")
        raise
    finally:
        os.close(parent_fd)


def _framework_python_runtime_inventory(
    runtime_root: Path,
    records: list[dict[str, object]],
    budget: dict[str, int],
    input_records: list[dict[str, object]],
    input_budget: dict[str, int],
) -> dict[str, object]:
    return {
        "format": "iroha-sumeragi-v2-private-framework-python-runtime",
        "schema_version": 1,
        "runtime_root": str(runtime_root),
        "record_count": len(records),
        "file_bytes": budget["bytes"],
        "records": sorted(records, key=lambda record: str(record["path"])),
        "source_disclosure": "withheld",
        "input_record_count": len(input_records),
        "input_file_bytes": input_budget["bytes"],
        "input_records": sorted(
            input_records, key=lambda record: str(record["path"])
        ),
    }


def verify_framework_python_runtime(
    runtime_root: Path, inventory_path: Path,
) -> dict[str, object]:
    """Authenticate one protected framework-Python archive and its sources."""

    source_python = Path(sys.executable).resolve(strict=True)
    closure = _framework_python_closure([source_python])
    if closure is None:
        raise CacheCopyError("selected Python has no macOS framework closure")
    version_root, framework, stdlib_name = closure
    _reject_framework_python_destination_overlap(
        version_root, runtime_root, inventory_path,
    )
    _validate_framework_python_sources(version_root, framework)
    payload, inventory_metadata = _read_regular(
        inventory_path, "framework Python runtime inventory",
    )
    try:
        inventory = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CacheCopyError(
            "framework Python runtime inventory is malformed"
        ) from error
    keys = {
        "format",
        "schema_version",
        "runtime_root",
        "record_count",
        "file_bytes",
        "records",
        "source_disclosure",
        "input_record_count",
        "input_file_bytes",
        "input_records",
    }
    if (
        not isinstance(inventory, dict)
        or set(inventory) != keys
        or inventory.get("format")
        != "iroha-sumeragi-v2-private-framework-python-runtime"
        or type(inventory.get("schema_version")) is not int
        or inventory["schema_version"] != 1
        or inventory.get("runtime_root") != str(runtime_root)
        or inventory.get("source_disclosure") != "withheld"
        or not isinstance(inventory.get("records"), list)
        or not isinstance(inventory.get("input_records"), list)
        or stat.S_IMODE(inventory_metadata.st_mode) != 0o400
        or payload != _canonical_payload(inventory)
    ):
        raise CacheCopyError(
            "framework Python runtime inventory contract is not exact"
        )
    records = inventory["records"]
    input_records = inventory["input_records"]
    input_bytes = sum(
        record.get("size", 0)
        for record in input_records
        if isinstance(record, dict) and record.get("kind") == "file"
    )
    if (
        type(inventory.get("input_record_count")) is not int
        or inventory["input_record_count"] != len(input_records)
        or type(inventory.get("input_file_bytes")) is not int
        or inventory["input_file_bytes"] != input_bytes
    ):
        raise CacheCopyError(
            "framework Python source inventory accounting is not exact"
        )
    source_roots = {
        "python3": source_python,
        framework: version_root / framework,
        "Resources": version_root / "Resources",
        "lib": version_root / "lib",
    }
    _verify_runtime_sources(source_roots, input_records)
    _bind_runtime_destinations(input_records, records, update=False)
    _validate_framework_python_runtime_records(records, framework, stdlib_name)
    _probe_framework_python_runtime(runtime_root, stdlib_name)
    runtime_fd, runtime_identity = _open_directory(
        runtime_root, "protected framework Python runtime",
    )
    observed: list[dict[str, object]] = []
    budget = {"records": 0, "bytes": 0}
    try:
        _snapshot_directory(
            runtime_fd, runtime_fd, None, (), observed, budget,
        )
    finally:
        os.close(runtime_fd)
    _revalidate_directory_path(
        runtime_root, runtime_identity, "protected framework Python runtime",
    )
    if (
        type(inventory.get("record_count")) is not int
        or inventory["record_count"] != len(observed)
        or type(inventory.get("file_bytes")) is not int
        or inventory["file_bytes"] != budget["bytes"]
        or records
        != sorted(observed, key=lambda record: str(record["path"]))
    ):
        raise CacheCopyError(
            "protected framework Python runtime changed after publication"
        )
    return inventory


def copy_framework_python_runtime(
    runtime_root: Path, inventory_path: Path,
) -> None:
    """Copy and seal the selected macOS framework-Python runtime closure."""

    _normalized_absolute(runtime_root, "protected framework Python runtime")
    _normalized_absolute(
        inventory_path, "framework Python runtime inventory",
    )
    if runtime_root.parent != inventory_path.parent:
        raise CacheCopyError(
            "framework Python runtime and inventory must be siblings"
        )
    source_python = Path(sys.executable).resolve(strict=True)
    closure = _framework_python_closure([source_python])
    if closure is None:
        raise CacheCopyError("selected Python has no macOS framework closure")
    version_root, framework, stdlib_name = closure
    _reject_framework_python_destination_overlap(
        version_root, runtime_root, inventory_path,
    )
    _validate_framework_python_sources(version_root, framework)
    parent_fd, parent_identity = _open_directory(
        runtime_root.parent, "framework Python archive parent",
    )
    created = False
    published: tuple[os.stat_result, os.stat_result] | None = None
    input_records: list[dict[str, object]] = []
    input_budget = {"records": 0, "bytes": 0}

    def copy_file(source: Path, destination: Path, label: str) -> None:
        source_parent_fd, source_parent = _open_directory(
            source.parent, f"{label} parent",
        )
        destination_parent_fd, destination_parent = _open_directory(
            destination.parent, f"archived {label} parent",
        )
        try:
            before = _entry_stat(source_parent_fd, source.name, label)
            if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
                raise CacheCopyError(f"{label} is not a regular source file")
            input_budget["records"] += 1
            if input_budget["records"] > MAXIMUM_RECORDS:
                raise CacheCopyError(
                    "framework Python source contains too many entries"
                )
            input_records.append(
                _copy_regular(
                    source_parent_fd,
                    destination_parent_fd,
                    source.name,
                    destination.name,
                    before,
                    input_budget,
                    destination_name=destination.name,
                )
            )
            _revalidate_entry(source_parent_fd, source.name, before, label)
            _revalidate_directory_path(
                source.parent, source_parent, f"{label} parent",
            )
            _revalidate_directory_path(
                destination.parent,
                destination_parent,
                f"archived {label} parent",
            )
        finally:
            os.close(source_parent_fd)
            os.close(destination_parent_fd)

    def copy_tree(source: Path, destination: Path, name: str) -> None:
        source_fd, source_identity = _open_directory(
            source, f"Python framework {name}",
        )
        destination_parent_fd, destination_parent = _open_directory(
            destination.parent, f"archived Python framework {name} parent",
        )
        root_fd, _ = _open_directory(
            version_root, "Python framework symlink root",
        )
        try:
            _copy_directory(
                source_fd,
                destination_parent_fd,
                destination.name,
                destination.name,
                root_fd,
                (name,),
                input_records,
                input_budget,
            )
            _revalidate_directory_path(
                source, source_identity, f"Python framework {name}",
            )
            _revalidate_directory_path(
                destination.parent,
                destination_parent,
                f"archived Python framework {name} parent",
            )
        finally:
            os.close(source_fd)
            os.close(destination_parent_fd)
            os.close(root_fd)

    try:
        if (
            _optional_entry_stat(
                parent_fd, runtime_root.name,
                "protected framework Python runtime",
            )
            is not None
            or _optional_entry_stat(
                parent_fd, inventory_path.name,
                "framework Python runtime inventory",
            )
            is not None
        ):
            raise CacheCopyError(
                "framework Python archive outputs already exist"
            )
        os.mkdir(runtime_root.name, mode=0o700, dir_fd=parent_fd)
        created = True
        runtime_fd = os.open(
            runtime_root.name, _DIRECTORY_FLAGS, dir_fd=parent_fd,
        )
        try:
            opened = os.fstat(runtime_fd)
            if (
                opened.st_uid != os.geteuid()
                or stat.S_IMODE(opened.st_mode) != 0o700
            ):
                raise CacheCopyError(
                    "protected framework Python runtime root is unsafe"
                )
            os.mkdir("bin", mode=0o700, dir_fd=runtime_fd)
        finally:
            os.close(runtime_fd)
        copy_file(
            source_python, runtime_root / "bin" / "python3",
            "Python framework launcher",
        )
        copy_file(
            version_root / framework, runtime_root / framework,
            "Python framework library",
        )
        for name in ("Resources", "lib"):
            copy_tree(version_root / name, runtime_root / name, name)
        _validate_framework_python_sources(version_root, framework)
        _seal_copied_tree(
            runtime_root,
            "protected framework Python runtime",
            owner_private_directory_roots=(("Resources", "Python.app"),),
        )
        _probe_framework_python_runtime(runtime_root, stdlib_name)
        runtime_fd, runtime_identity = _open_directory(
            runtime_root, "protected framework Python runtime",
        )
        records: list[dict[str, object]] = []
        budget = {"records": 0, "bytes": 0}
        try:
            _snapshot_directory(
                runtime_fd, runtime_fd, None, (), records, budget,
            )
        finally:
            os.close(runtime_fd)
        _revalidate_directory_path(
            runtime_root, runtime_identity,
            "protected framework Python runtime",
        )
        _validate_framework_python_runtime_records(
            records, framework, stdlib_name,
        )
        _bind_runtime_destinations(input_records, records, update=True)
        document = _framework_python_runtime_inventory(
            runtime_root, records, budget, input_records, input_budget,
        )
        source_roots = {
            "python3": source_python,
            framework: version_root / framework,
            "Resources": version_root / "Resources",
            "lib": version_root / "lib",
        }
        _verify_runtime_sources(source_roots, input_records)
        published = _publish_inventory(
            inventory_path, _canonical_payload(document),
        )
        verify_framework_python_runtime(runtime_root, inventory_path)
        _revalidate_directory_path(
            runtime_root.parent,
            parent_identity,
            "framework Python archive parent",
        )
    except BaseException:
        _remove_published(inventory_path, published)
        if created:
            _owned_remove_tree(
                parent_fd,
                runtime_root.name,
                "partial framework Python runtime",
            )
        raise
    finally:
        os.close(parent_fd)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-cargo-home", type=Path)
    parser.add_argument("--cargo-home", type=Path)
    parser.add_argument("--inventory", type=Path)
    parser.add_argument("--final", action="store_true")
    parser.add_argument("--publish-validation-failure", action="store_true")
    parser.add_argument("--seal-release-result", action="store_true")
    parser.add_argument("--copy-runtime", action="store_true")
    parser.add_argument("--copy-framework-python", action="store_true")
    parser.add_argument("--verify-framework-python", action="store_true")
    parser.add_argument("--verify-runtime-sources", action="store_true")
    parser.add_argument("--verify-cache-sources", action="store_true")
    parser.add_argument("--copy-private-bundle", action="store_true")
    parser.add_argument("--verify-private-bundle", action="store_true")
    parser.add_argument("--copy-sdk-dependencies", action="store_true")
    parser.add_argument("--verify-sdk-dependencies", action="store_true")
    parser.add_argument("--create-sdk-command-work", action="store_true")
    parser.add_argument("--cleanup-sdk-command-work", action="store_true")
    parser.add_argument("--cleanup-invocation", action="store_true")
    parser.add_argument("--runtime-root", type=Path)
    parser.add_argument("--runtime-inventory", type=Path)
    parser.add_argument("--runtime-source", type=Path, action="append", default=[])
    parser.add_argument("--bundle-source", type=Path)
    parser.add_argument("--bundle-root", type=Path)
    parser.add_argument("--sdk-dependency-bundle-manifest", type=Path)
    parser.add_argument("--expected-sdk-dependency-bundle-manifest-sha256")
    parser.add_argument("--repository-root", type=Path)
    parser.add_argument("--sdk-input-root", type=Path)
    parser.add_argument("--sdk-work-root", type=Path)
    parser.add_argument("--sdk-archive", type=Path)
    parser.add_argument("--sdk-dependency-inventory", type=Path)
    parser.add_argument("--sdk-work-final-inventory", type=Path)
    parser.add_argument("--invocation-root", type=Path)
    parser.add_argument("--bootstrap-evidence", type=Path)
    parser.add_argument("--source-manifest-sha256")
    parser.add_argument("--candidate-root", type=Path)
    parser.add_argument("--scaling-evidence-manifest", type=Path)
    parser.add_argument("--expected-signer-fingerprint")
    parser.add_argument("--expected-scaling-trial-harness-sha256")
    parser.add_argument("--expected-scaling-configuration-sha256")
    parser.add_argument("--expected-scaling-irohad-sha256")
    parser.add_argument("--expected-scaling-iroha-cli-sha256")
    parser.add_argument("--validator-exit-status", type=int)
    parser.add_argument("--cleanup-base", type=Path)
    parser.add_argument("--cleanup-prefix")
    args = parser.parse_args()
    try:
        if args.create_sdk_command_work or args.cleanup_sdk_command_work:
            if (
                args.create_sdk_command_work == args.cleanup_sdk_command_work
                or args.sdk_input_root is None
                or args.sdk_work_root is None
                or any((
                    args.final, args.publish_validation_failure,
                    args.seal_release_result, args.copy_runtime,
                    args.copy_framework_python, args.verify_framework_python,
                    args.verify_runtime_sources, args.verify_cache_sources,
                    args.copy_private_bundle, args.verify_private_bundle,
                    args.copy_sdk_dependencies, args.verify_sdk_dependencies,
                    args.cleanup_invocation,
                ))
                or args.runtime_source
                or any(value is not None for value in (
                    args.source_cargo_home, args.cargo_home, args.inventory,
                    args.runtime_root, args.runtime_inventory,
                    args.bundle_source, args.bundle_root,
                    args.sdk_dependency_bundle_manifest,
                    args.expected_sdk_dependency_bundle_manifest_sha256,
                    args.repository_root, args.sdk_archive,
                    args.sdk_dependency_inventory,
                    args.sdk_work_final_inventory, args.invocation_root,
                    args.bootstrap_evidence, args.source_manifest_sha256,
                    args.candidate_root, args.scaling_evidence_manifest,
                    args.expected_signer_fingerprint,
                    args.expected_scaling_trial_harness_sha256,
                    args.expected_scaling_configuration_sha256,
                    args.expected_scaling_irohad_sha256,
                    args.expected_scaling_iroha_cli_sha256,
                    args.validator_exit_status, args.cleanup_base,
                    args.cleanup_prefix,
                ))
            ):
                raise CacheCopyError("SDK command work inputs are not exact")
            if args.create_sdk_command_work:
                create_sdk_command_work(args.sdk_input_root, args.sdk_work_root)
            else:
                cleanup_sdk_command_work(args.sdk_input_root, args.sdk_work_root)
            return 0
        if args.copy_sdk_dependencies or args.verify_sdk_dependencies:
            if (
                args.copy_sdk_dependencies == args.verify_sdk_dependencies
                or any(value is None for value in (
                    args.sdk_dependency_bundle_manifest,
                    args.expected_sdk_dependency_bundle_manifest_sha256,
                    args.repository_root, args.sdk_input_root, args.sdk_work_root,
                    args.sdk_archive, args.sdk_dependency_inventory,
                ))
                or (args.copy_sdk_dependencies and args.sdk_work_final_inventory is not None)
                or (args.verify_sdk_dependencies and args.sdk_work_final_inventory is None)
                or any((
                    args.final, args.publish_validation_failure,
                    args.seal_release_result, args.copy_runtime,
                    args.copy_framework_python, args.verify_framework_python,
                    args.verify_runtime_sources, args.verify_cache_sources,
                    args.copy_private_bundle, args.verify_private_bundle,
                    args.create_sdk_command_work,
                    args.cleanup_sdk_command_work,
                    args.cleanup_invocation,
                ))
                or any(value is not None for value in (
                    args.source_cargo_home, args.cargo_home, args.inventory,
                    args.runtime_root, args.runtime_inventory,
                    args.bundle_source, args.bundle_root,
                    args.invocation_root, args.bootstrap_evidence,
                    args.source_manifest_sha256, args.candidate_root,
                    args.scaling_evidence_manifest,
                    args.expected_signer_fingerprint,
                    args.expected_scaling_trial_harness_sha256,
                    args.expected_scaling_configuration_sha256,
                    args.expected_scaling_irohad_sha256,
                    args.expected_scaling_iroha_cli_sha256,
                    args.validator_exit_status, args.cleanup_base,
                    args.cleanup_prefix,
                ))
                or args.runtime_source
            ):
                raise CacheCopyError("SDK dependency bundle inputs are not exact")
            sdk_arguments = (
                args.sdk_dependency_bundle_manifest,
                args.expected_sdk_dependency_bundle_manifest_sha256,
                args.repository_root, args.sdk_input_root, args.sdk_work_root,
                args.sdk_archive, args.sdk_dependency_inventory,
            )
            if args.copy_sdk_dependencies:
                copy_sdk_dependencies(*sdk_arguments)
            else:
                verify_sdk_dependencies(
                    *sdk_arguments,
                    final_work_inventory=args.sdk_work_final_inventory,
                )
            return 0
        if args.copy_framework_python or args.verify_framework_python:
            if (
                args.copy_framework_python == args.verify_framework_python
                or args.runtime_root is None
                or args.runtime_inventory is None
                or args.runtime_source
                or args.source_cargo_home is not None
                or args.cargo_home is not None
                or args.inventory is not None
                or args.final
                or args.publish_validation_failure
                or args.seal_release_result
                or args.copy_runtime
                or args.verify_runtime_sources
                or args.verify_cache_sources
                or args.copy_private_bundle
                or args.verify_private_bundle
                or args.copy_sdk_dependencies
                or args.verify_sdk_dependencies
                or args.create_sdk_command_work
                or args.cleanup_sdk_command_work
                or args.cleanup_invocation
            ):
                raise CacheCopyError(
                    "framework Python runtime inputs are not exact"
                )
            if args.copy_framework_python:
                copy_framework_python_runtime(
                    args.runtime_root, args.runtime_inventory,
                )
            else:
                verify_framework_python_runtime(
                    args.runtime_root, args.runtime_inventory,
                )
            return 0
        if args.verify_cache_sources:
            if args.source_cargo_home is None or args.cargo_home is None or args.inventory is None:
                raise CacheCopyError("caller cache verification lacks required inputs")
            verify_cache_sources(args.source_cargo_home, args.cargo_home, args.inventory)
        elif args.verify_private_bundle:
            if args.bundle_source is None or args.bundle_root is None or args.inventory is None:
                raise CacheCopyError("private bundle verification lacks required inputs")
            _verify_private_bundle(args.bundle_source, args.bundle_root, args.inventory)
        elif args.copy_private_bundle:
            if args.bundle_source is None or args.bundle_root is None or args.inventory is None:
                raise CacheCopyError("private bundle copy lacks required inputs")
            copy_private_bundle(args.bundle_source, args.bundle_root, args.inventory)
        elif args.verify_runtime_sources:
            if args.runtime_root is None or args.runtime_inventory is None:
                raise CacheCopyError("runtime source verification lacks its inventory")
            verify_runtime_sources(args.runtime_source, args.runtime_root, args.runtime_inventory)
        elif args.cleanup_invocation:
            if args.cleanup_base is None or args.invocation_root is None or args.cleanup_prefix is None:
                raise CacheCopyError("private invocation cleanup lacks required inputs")
            cleanup_invocation(args.cleanup_base, args.invocation_root, args.cleanup_prefix)
        elif args.publish_validation_failure:
            if any(value is None for value in (
                args.invocation_root, args.bootstrap_evidence, args.cleanup_base,
                args.cleanup_prefix, args.source_manifest_sha256,
                args.validator_exit_status,
            )):
                raise CacheCopyError("receipt validation failure lacks required inputs")
            publish_validation_failure(
                args.invocation_root, args.bootstrap_evidence, args.cleanup_base,
                args.cleanup_prefix, args.source_manifest_sha256,
                args.validator_exit_status,
            )
        elif args.copy_runtime:
            if args.runtime_root is None or args.runtime_inventory is None:
                raise CacheCopyError("private child runtime lacks its root")
            copy_runtime(args.runtime_root, args.runtime_source, args.runtime_inventory)
        elif args.seal_release_result:
            if any(value is None for value in (
                args.invocation_root,
                args.bootstrap_evidence,
                args.source_manifest_sha256,
                args.candidate_root,
                args.scaling_evidence_manifest,
                args.expected_signer_fingerprint,
                args.expected_scaling_trial_harness_sha256,
                args.expected_scaling_configuration_sha256,
                args.expected_scaling_irohad_sha256,
                args.expected_scaling_iroha_cli_sha256,
            )):
                raise CacheCopyError("retained release publication lacks required inputs")
            seal_release_result(
                args.invocation_root,
                args.bootstrap_evidence,
                args.source_manifest_sha256,
                args.candidate_root,
                args.scaling_evidence_manifest,
                args.expected_signer_fingerprint,
                args.expected_scaling_trial_harness_sha256,
                args.expected_scaling_configuration_sha256,
                args.expected_scaling_irohad_sha256,
                args.expected_scaling_iroha_cli_sha256,
            )
        elif args.final:
            if args.cargo_home is None or args.inventory is None:
                raise CacheCopyError("final cache snapshot lacks required paths")
            if args.source_cargo_home is not None:
                raise CacheCopyError("final cache snapshot does not accept a source home")
            snapshot_cache(args.cargo_home, args.inventory)
        else:
            if args.source_cargo_home is None or args.cargo_home is None or args.inventory is None:
                raise CacheCopyError("cache copy requires a source home")
            copy_cache(args.source_cargo_home, args.cargo_home, args.inventory)
    except (CacheCopyError, OSError) as error:
        print(f"release Cargo cache isolation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
