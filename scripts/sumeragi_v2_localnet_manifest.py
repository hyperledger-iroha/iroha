#!/usr/bin/env python3
"""Create canonical content manifests for retained Sumeragi localnets."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path, PurePosixPath
import stat
import sys
from typing import BinaryIO


_MANIFEST_HEADER = b"path\tsize_bytes\tsha256\n"
_READ_CHUNK_BYTES = 1024 * 1024


class LocalnetManifestError(RuntimeError):
    """The retained localnet cannot be represented by a safe manifest."""


def _stable_identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _canonical_component(name: str) -> bytes:
    if name in {"", ".", ".."} or any(character in name for character in "\t\r\n"):
        raise LocalnetManifestError("retained localnet contains an unsafe path component")
    try:
        encoded = name.encode("utf-8")
    except UnicodeEncodeError as error:
        raise LocalnetManifestError(
            "retained localnet path is not canonical UTF-8"
        ) from error
    if b"/" in encoded or b"\x00" in encoded:
        raise LocalnetManifestError("retained localnet contains an unsafe path component")
    return encoded


def _hash_open_file(stream: BinaryIO) -> str:
    digest = hashlib.sha256()
    while chunk := stream.read(_READ_CHUNK_BYTES):
        digest.update(chunk)
    return digest.hexdigest()


def canonical_localnet_manifest(root: Path) -> bytes:
    """Return the canonical regular-file manifest for ``root``.

    Directory traversal is descriptor-relative and never follows symlinks.
    Every retained entry must be a directory or regular file, and the complete
    directory tree must remain unchanged while it is read.
    """

    root = Path(root)
    if not root.is_absolute() or Path(os.path.abspath(root)) != root:
        raise LocalnetManifestError(
            "retained localnet root must be absolute and normalized"
        )
    try:
        resolved_root = root.resolve(strict=True)
        root_metadata = root.lstat()
    except (OSError, RuntimeError) as error:
        raise LocalnetManifestError(
            f"retained localnet root is unavailable: {root}"
        ) from error
    if (
        resolved_root != root
        or stat.S_ISLNK(root_metadata.st_mode)
        or not stat.S_ISDIR(root_metadata.st_mode)
    ):
        raise LocalnetManifestError(
            "retained localnet root must be a resolved real directory"
        )
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    directory_flag = getattr(os, "O_DIRECTORY", 0)
    if nofollow == 0 or directory_flag == 0:
        raise LocalnetManifestError(
            "retained localnet manifests require O_NOFOLLOW and O_DIRECTORY"
        )
    directory_flags = os.O_RDONLY | os.O_CLOEXEC | nofollow | directory_flag
    file_flags = os.O_RDONLY | os.O_CLOEXEC | nofollow
    try:
        root_fd = os.open(root, directory_flags)
    except OSError as error:
        raise LocalnetManifestError("retained localnet root could not be opened safely") from error

    records: list[tuple[bytes, int, str]] = []

    def walk(directory_fd: int, relative_parts: tuple[str, ...]) -> None:
        before = os.fstat(directory_fd)
        if not stat.S_ISDIR(before.st_mode):
            raise LocalnetManifestError("retained localnet directory changed type")
        try:
            with os.scandir(directory_fd) as iterator:
                entries = list(iterator)
        except OSError as error:
            raise LocalnetManifestError("retained localnet directory cannot be scanned") from error
        try:
            entries.sort(key=lambda entry: _canonical_component(entry.name))
        except LocalnetManifestError:
            raise

        for entry in entries:
            name_bytes = _canonical_component(entry.name)
            try:
                metadata = os.stat(entry.name, dir_fd=directory_fd, follow_symlinks=False)
            except OSError as error:
                raise LocalnetManifestError(
                    "retained localnet entry changed during traversal"
                ) from error
            if stat.S_ISLNK(metadata.st_mode):
                raise LocalnetManifestError("retained localnet contains a symlink")
            if stat.S_ISDIR(metadata.st_mode):
                try:
                    child_fd = os.open(entry.name, directory_flags, dir_fd=directory_fd)
                except OSError as error:
                    raise LocalnetManifestError(
                        "retained localnet directory could not be opened safely"
                    ) from error
                try:
                    opened = os.fstat(child_fd)
                    if (
                        opened.st_dev != metadata.st_dev
                        or opened.st_ino != metadata.st_ino
                        or not stat.S_ISDIR(opened.st_mode)
                    ):
                        raise LocalnetManifestError(
                            "retained localnet directory changed during traversal"
                        )
                    walk(child_fd, (*relative_parts, entry.name))
                finally:
                    os.close(child_fd)
                continue
            if not stat.S_ISREG(metadata.st_mode):
                raise LocalnetManifestError(
                    "retained localnet contains a non-regular special file"
                )
            try:
                file_fd = os.open(entry.name, file_flags, dir_fd=directory_fd)
            except OSError as error:
                raise LocalnetManifestError(
                    "retained localnet file could not be opened safely"
                ) from error
            try:
                opened = os.fstat(file_fd)
                if (
                    opened.st_dev != metadata.st_dev
                    or opened.st_ino != metadata.st_ino
                    or not stat.S_ISREG(opened.st_mode)
                ):
                    raise LocalnetManifestError(
                        "retained localnet file changed during traversal"
                    )
                with os.fdopen(os.dup(file_fd), "rb", closefd=True) as stream:
                    digest = _hash_open_file(stream)
                after = os.fstat(file_fd)
                if _stable_identity(opened) != _stable_identity(after):
                    raise LocalnetManifestError(
                        "retained localnet file changed while it was hashed"
                    )
            finally:
                os.close(file_fd)
            relative = PurePosixPath(*relative_parts, entry.name).as_posix()
            relative_bytes = relative.encode("utf-8")
            if relative_bytes.startswith(b"/") or ".." in PurePosixPath(relative).parts:
                raise LocalnetManifestError("retained localnet path escaped its root")
            records.append((relative_bytes, after.st_size, digest))

        after = os.fstat(directory_fd)
        if _stable_identity(before) != _stable_identity(after):
            raise LocalnetManifestError(
                "retained localnet directory changed during traversal"
            )

    try:
        opened_root = os.fstat(root_fd)
        if (
            opened_root.st_dev != root_metadata.st_dev
            or opened_root.st_ino != root_metadata.st_ino
            or not stat.S_ISDIR(opened_root.st_mode)
        ):
            raise LocalnetManifestError("retained localnet root changed during traversal")
        walk(root_fd, ())
        try:
            final_root = root.lstat()
            final_resolved_root = root.resolve(strict=True)
        except (OSError, RuntimeError) as error:
            raise LocalnetManifestError(
                "retained localnet root changed during traversal"
            ) from error
        if (
            final_resolved_root != root
            or _stable_identity(root_metadata) != _stable_identity(final_root)
            or (final_root.st_dev, final_root.st_ino)
            != (opened_root.st_dev, opened_root.st_ino)
        ):
            raise LocalnetManifestError(
                "retained localnet root changed during traversal"
            )
    finally:
        os.close(root_fd)
    if not records:
        raise LocalnetManifestError("retained localnet contains no regular files")
    records.sort(key=lambda record: record[0])
    return _MANIFEST_HEADER + b"".join(
        path + b"\t" + str(size).encode("ascii") + b"\t" + digest.encode("ascii") + b"\n"
        for path, size, digest in records
    )


def write_manifest(root: Path, output: Path) -> None:
    """Exclusively create ``output`` with the canonical manifest for ``root``."""

    output = Path(output)
    if not output.is_absolute() or Path(os.path.abspath(output)) != output:
        raise LocalnetManifestError(
            "manifest output path must be absolute and normalized"
        )
    parent = output.parent
    try:
        resolved_parent = parent.resolve(strict=True)
        parent_metadata = parent.lstat()
    except (OSError, RuntimeError) as error:
        raise LocalnetManifestError("manifest output directory is unavailable") from error
    if (
        resolved_parent != parent
        or stat.S_ISLNK(parent_metadata.st_mode)
        or not stat.S_ISDIR(parent_metadata.st_mode)
    ):
        raise LocalnetManifestError("manifest output parent must be a real directory")
    data = canonical_localnet_manifest(root)
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
    flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        output_fd = os.open(output, flags, 0o600)
    except OSError as error:
        raise LocalnetManifestError("manifest output already exists or is unsafe") from error
    try:
        with os.fdopen(output_fd, "wb", closefd=False) as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
    except BaseException:
        try:
            output.unlink()
        except OSError:
            pass
        raise
    finally:
        os.close(output_fd)


def main(argv: list[str] | None = None) -> int:
    """Write one retained-localnet manifest from command-line arguments."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        write_manifest(args.root, args.output)
    except LocalnetManifestError as error:
        print(f"localnet manifest rejected: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
