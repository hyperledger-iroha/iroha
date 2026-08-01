#!/usr/bin/env python3
"""Create a deterministic, fail-closed content seal for one Rust sysroot."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
from typing import Any


SCHEMA = "iroha.taira.rust-toolchain-tree.v1"
DOMAIN = b"iroha.taira.rust-toolchain-tree.v1\0"
MAX_ENTRIES = 100_000
MAX_FILE_BYTES = 2 * 1024 * 1024 * 1024
MAX_TOTAL_BYTES = 16 * 1024 * 1024 * 1024
MAX_PATH_BYTES = 4096
MAX_MANIFEST_BYTES = 64 * 1024 * 1024
READ_BYTES = 1024 * 1024
STABLE_FIELDS = (
    "st_dev",
    "st_ino",
    "st_mode",
    "st_nlink",
    "st_size",
    "st_mtime_ns",
    "st_ctime_ns",
)


class TreeHashError(RuntimeError):
    """The toolchain tree cannot be sealed without ambiguity."""


def _canonical_directory(raw: str) -> Path:
    root = Path(raw)
    if not root.is_absolute():
        raise TreeHashError("Rust sysroot must be an absolute path")
    try:
        canonical = root.resolve(strict=True)
        metadata = root.lstat()
    except OSError as error:
        raise TreeHashError(f"Rust sysroot is unavailable: {error}") from error
    if canonical != root or not stat.S_ISDIR(metadata.st_mode):
        raise TreeHashError("Rust sysroot must be one canonical physical directory")
    return canonical


def _canonical_output(raw: str, root: Path) -> Path:
    output = Path(raw)
    if not output.is_absolute():
        raise TreeHashError("toolchain manifest output must be an absolute path")
    try:
        parent = output.parent.resolve(strict=True)
    except OSError as error:
        raise TreeHashError(f"toolchain manifest parent is unavailable: {error}") from error
    if parent != output.parent:
        raise TreeHashError("toolchain manifest parent must be canonical and physical")
    try:
        output.relative_to(root)
    except ValueError:
        pass
    else:
        raise TreeHashError("toolchain manifest must be outside the Rust sysroot")
    if os.path.lexists(output):
        raise TreeHashError("toolchain manifest output already exists")
    return output


def _relative_name(path: Path, root: Path) -> str:
    relative = path.relative_to(root).as_posix()
    if relative == "." or relative.startswith("/"):
        raise TreeHashError("toolchain entry path is not canonical and relative")
    try:
        encoded = relative.encode("utf-8", "strict")
    except UnicodeEncodeError as error:
        raise TreeHashError("toolchain entry path is not canonical UTF-8") from error
    if (
        not encoded
        or len(encoded) > MAX_PATH_BYTES
        or any(byte < 0x20 or byte == 0x7F for byte in encoded)
        or any(component in ("", ".", "..") for component in relative.split("/"))
    ):
        raise TreeHashError(f"toolchain entry path is unsafe: {relative!r}")
    return relative


def _same_metadata(left: os.stat_result, right: os.stat_result) -> bool:
    return all(getattr(left, field) == getattr(right, field) for field in STABLE_FIELDS)


def _file_digest(path: Path, before: os.stat_result, relative: str) -> str:
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size < 0
        or before.st_size > MAX_FILE_BYTES
    ):
        raise TreeHashError(
            f"toolchain file is not bounded, regular, and singly linked: {relative}"
        )
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if not _same_metadata(before, opened):
            raise TreeHashError(f"toolchain file changed before opening: {relative}")
        digest = hashlib.sha256()
        remaining = before.st_size
        while remaining:
            chunk = os.read(descriptor, min(READ_BYTES, remaining))
            if not chunk:
                raise TreeHashError(f"toolchain file ended early: {relative}")
            digest.update(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            raise TreeHashError(f"toolchain file grew while hashing: {relative}")
        if not _same_metadata(before, os.fstat(descriptor)):
            raise TreeHashError(f"toolchain file changed while hashing: {relative}")
        return digest.hexdigest()
    finally:
        os.close(descriptor)


def _symlink_target(path: Path, root: Path, before: os.stat_result, relative: str) -> str:
    if before.st_nlink != 1:
        raise TreeHashError(f"toolchain symlink is multiply linked: {relative}")
    target = os.readlink(path)
    try:
        encoded = target.encode("utf-8", "strict")
    except UnicodeEncodeError as error:
        raise TreeHashError(f"toolchain symlink target is not UTF-8: {relative}") from error
    if (
        not encoded
        or len(encoded) > MAX_PATH_BYTES
        or encoded.startswith(b"/")
        or any(byte < 0x20 or byte == 0x7F for byte in encoded)
    ):
        raise TreeHashError(f"toolchain symlink target is unsafe: {relative}")
    try:
        resolved = path.resolve(strict=True)
        resolved.relative_to(root)
    except (OSError, ValueError) as error:
        raise TreeHashError(
            f"toolchain symlink escapes or is dangling: {relative}"
        ) from error
    if not _same_metadata(before, path.lstat()) or os.readlink(path) != target:
        raise TreeHashError(f"toolchain symlink changed while reading: {relative}")
    return target


def _entry_key(path: Path, root: Path) -> bytes:
    return _relative_name(path, root).encode("utf-8")


def _snapshot(root: Path) -> tuple[list[dict[str, Any]], int, list[tuple[Path, os.stat_result]]]:
    entries: list[dict[str, Any]] = []
    snapshots: list[tuple[Path, os.stat_result]] = [(root, root.lstat())]
    pending = [root]
    total_bytes = 0
    while pending:
        directory = pending.pop()
        before_directory = directory.lstat()
        if not stat.S_ISDIR(before_directory.st_mode):
            raise TreeHashError("toolchain directory changed during enumeration")
        try:
            children = sorted(
                (Path(entry.path) for entry in os.scandir(directory)),
                key=lambda child: _entry_key(child, root),
            )
        except OSError as error:
            raise TreeHashError(f"cannot enumerate Rust sysroot: {error}") from error
        if not _same_metadata(before_directory, directory.lstat()):
            raise TreeHashError("toolchain directory changed during enumeration")
        child_directories: list[Path] = []
        for path in children:
            relative = _relative_name(path, root)
            before = path.lstat()
            snapshots.append((path, before))
            mode = f"{stat.S_IMODE(before.st_mode):04o}"
            if stat.S_ISDIR(before.st_mode):
                entry: dict[str, Any] = {
                    "kind": "directory",
                    "mode": mode,
                    "path": relative,
                }
                child_directories.append(path)
            elif stat.S_ISREG(before.st_mode):
                digest = _file_digest(path, before, relative)
                total_bytes += before.st_size
                if total_bytes > MAX_TOTAL_BYTES:
                    raise TreeHashError("Rust sysroot exceeds its aggregate byte bound")
                entry = {
                    "kind": "file",
                    "mode": mode,
                    "path": relative,
                    "sha256": digest,
                    "size_bytes": before.st_size,
                }
            elif stat.S_ISLNK(before.st_mode):
                entry = {
                    "kind": "symlink",
                    "mode": mode,
                    "path": relative,
                    "target": _symlink_target(path, root, before, relative),
                }
            else:
                raise TreeHashError(f"toolchain contains a special file: {relative}")
            entries.append(entry)
            if len(entries) > MAX_ENTRIES:
                raise TreeHashError("Rust sysroot contains too many entries")
        pending.extend(reversed(child_directories))
    entries.sort(key=lambda entry: entry["path"].encode("utf-8"))
    return entries, total_bytes, snapshots


def _revalidate(snapshots: list[tuple[Path, os.stat_result]]) -> None:
    for path, expected in snapshots:
        try:
            observed = path.lstat()
        except OSError as error:
            raise TreeHashError(f"toolchain entry disappeared during sealing: {path}") from error
        if not _same_metadata(expected, observed):
            raise TreeHashError(f"toolchain entry changed during sealing: {path}")


def _canonical_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def _tree_digest(entries: list[dict[str, Any]]) -> str:
    identity = _canonical_json({"entries": entries, "schema": SCHEMA})
    digest = hashlib.sha256()
    digest.update(DOMAIN)
    digest.update(len(identity).to_bytes(8, "big"))
    digest.update(identity)
    return digest.hexdigest()


def _write_create_new(path: Path, encoded: bytes) -> None:
    if len(encoded) > MAX_MANIFEST_BYTES:
        raise TreeHashError("toolchain manifest exceeds its fixed byte bound")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(encoded)
            stream.flush()
            os.fchmod(stream.fileno(), 0o600)
            os.fsync(stream.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise
    directory = os.open(path.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def seal(root: Path, output: Path) -> dict[str, Any]:
    """Hash one stable sysroot and create its canonical entry manifest."""

    entries, total_bytes, snapshots = _snapshot(root)
    if not entries:
        raise TreeHashError("Rust sysroot is empty")
    _revalidate(snapshots)
    tree_sha256 = _tree_digest(entries)
    manifest = {
        "entry_count": len(entries),
        "root_path": str(root),
        "schema": SCHEMA,
        "total_file_bytes": total_bytes,
        "tree_sha256": tree_sha256,
        "tree_identity": {"entries": entries, "schema": SCHEMA},
    }
    encoded = json.dumps(manifest, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    _write_create_new(output, encoded)
    try:
        _revalidate(snapshots)
    except BaseException:
        output.unlink(missing_ok=True)
        directory = os.open(
            output.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0)
        )
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
        raise
    return manifest


def _parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Seal the exact path, type, mode, link, and byte closure of a Rust sysroot."
    )
    parser.add_argument("--sysroot", required=True)
    parser.add_argument("--manifest-out", required=True)
    return parser.parse_args()


def main() -> int:
    arguments = _parse_arguments()
    try:
        root = _canonical_directory(arguments.sysroot)
        output = _canonical_output(arguments.manifest_out, root)
        manifest = seal(root, output)
    except (OSError, TreeHashError) as error:
        print(f"Taira Rust toolchain sealing failed: {error}", file=sys.stderr)
        return 1
    print(manifest["tree_sha256"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
