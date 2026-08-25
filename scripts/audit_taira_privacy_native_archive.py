#!/usr/bin/env python3
"""Fail closed on unsafe or stage-divergent Taira privacy evidence archives.

The capture workflow invokes this after creating its deterministic ``tar.gz``.
Only regular files and directories from the canonical staged evidence root are
admitted.  The archive must contain that exact inventory, every regular member
must equal the stable post-creation stage snapshot, and the staged
``provenance/SHA256SUMS`` must cover every other staged regular file exactly.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import re
import stat
import sys
import tarfile
from typing import BinaryIO, Iterator


CHECKSUM_RELATIVE_PATH = "provenance/SHA256SUMS"
MAX_ARCHIVE_BYTES = 16 * 1024 * 1024 * 1024
MAX_MEMBER_BYTES = 8 * 1024 * 1024 * 1024
MAX_TOTAL_FILE_BYTES = 16 * 1024 * 1024 * 1024
MAX_MEMBER_COUNT = 100_000
MAX_PATH_BYTES = 4096
MAX_CHECKSUM_MANIFEST_BYTES = 16 * 1024 * 1024
COPY_CHUNK_BYTES = 1024 * 1024
SHA256_LINE = re.compile(rb"([0-9a-f]{64})  ([^\r\n]+)")
STABLE_FILE_FIELDS = (
    "st_dev",
    "st_ino",
    "st_mode",
    "st_nlink",
    "st_size",
    "st_mtime_ns",
    "st_ctime_ns",
)
STABLE_DIRECTORY_FIELDS = (
    "st_dev",
    "st_ino",
    "st_mode",
    "st_mtime_ns",
    "st_ctime_ns",
)


class ArchiveAuditError(RuntimeError):
    """The archive cannot be proven safe and identical to its staged source."""


@dataclass(frozen=True)
class StageEntry:
    """One stable staged directory or regular file."""

    kind: str
    mode: int
    size: int
    digest: str | None
    metadata: os.stat_result


def _canonical_directory(raw: str, label: str) -> Path:
    path = Path(raw)
    if not path.is_absolute():
        raise ArchiveAuditError(f"{label} must be an absolute path")
    canonical = path.resolve(strict=True)
    metadata = path.lstat()
    if canonical != path or not stat.S_ISDIR(metadata.st_mode):
        raise ArchiveAuditError(f"{label} must be one canonical physical directory")
    return canonical


def _canonical_archive(raw: str, staged_root: Path) -> tuple[Path, os.stat_result]:
    path = Path(raw)
    if not path.is_absolute():
        raise ArchiveAuditError("archive must be an absolute path")
    canonical = path.resolve(strict=True)
    metadata = path.lstat()
    if canonical != path or not stat.S_ISREG(metadata.st_mode):
        raise ArchiveAuditError("archive must be one canonical physical regular file")
    if metadata.st_nlink != 1 or not 0 < metadata.st_size <= MAX_ARCHIVE_BYTES:
        raise ArchiveAuditError("archive must be non-empty, bounded, and singly linked")
    try:
        canonical.relative_to(staged_root)
    except ValueError:
        return canonical, metadata
    raise ArchiveAuditError("archive must be outside the staged evidence root")


def _metadata_matches(
    expected: os.stat_result,
    observed: os.stat_result,
    fields: tuple[str, ...],
) -> bool:
    return all(getattr(expected, field) == getattr(observed, field) for field in fields)


@contextmanager
def _stable_regular_reader(
    path: Path,
    expected: os.stat_result,
    label: str,
) -> Iterator[BinaryIO]:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    stream = os.fdopen(descriptor, "rb")
    try:
        opened = os.fstat(stream.fileno())
        if not _metadata_matches(expected, opened, STABLE_FILE_FIELDS):
            raise ArchiveAuditError(f"{label} changed before it was opened")
        yield stream
        after = os.fstat(stream.fileno())
        if not _metadata_matches(expected, after, STABLE_FILE_FIELDS):
            raise ArchiveAuditError(f"{label} changed while it was read")
    finally:
        stream.close()
    final = path.lstat()
    if not _metadata_matches(expected, final, STABLE_FILE_FIELDS):
        raise ArchiveAuditError(f"{label} pathname changed while it was read")


def _digest_stream(stream: BinaryIO, maximum_bytes: int, label: str) -> tuple[str, int]:
    digest = hashlib.sha256()
    length = 0
    while chunk := stream.read(COPY_CHUNK_BYTES):
        length += len(chunk)
        if length > maximum_bytes:
            raise ArchiveAuditError(f"{label} exceeds its byte bound")
        digest.update(chunk)
    return digest.hexdigest(), length


def _relative_path(value: str, label: str) -> str:
    if not value or value.startswith("/") or value.endswith("/"):
        raise ArchiveAuditError(f"{label} is not a canonical relative path")
    if "\\" in value or any(
        ord(character) < 0x20 or ord(character) == 0x7F for character in value
    ):
        raise ArchiveAuditError(f"{label} contains a forbidden character")
    try:
        encoded = value.encode("utf-8", "strict")
    except UnicodeEncodeError as error:
        raise ArchiveAuditError(f"{label} is not canonical UTF-8") from error
    if len(encoded) > MAX_PATH_BYTES:
        raise ArchiveAuditError(f"{label} exceeds its byte bound")
    components = value.split("/")
    if any(component in ("", ".", "..") for component in components):
        raise ArchiveAuditError(f"{label} contains an unsafe path component")
    return value


def _archive_member_path(raw: str) -> str:
    if raw == ".":
        return "."
    if not raw.startswith("./"):
        raise ArchiveAuditError(f"archive member is not dot-anchored: {raw!r}")
    return _relative_path(raw[2:], "archive member")


def _snapshot_stage(root: Path) -> dict[str, StageEntry]:
    entries: dict[str, StageEntry] = {}
    total_file_bytes = 0

    def reject_walk_error(error: OSError) -> None:
        raise ArchiveAuditError(f"cannot enumerate staged evidence: {error}") from error

    for current_raw, directory_names, file_names in os.walk(
        root, followlinks=False, onerror=reject_walk_error
    ):
        current = Path(current_raw)
        relative_current = current.relative_to(root).as_posix()
        key = "." if relative_current == "." else _relative_path(
            relative_current, "staged directory"
        )
        metadata = current.lstat()
        if not stat.S_ISDIR(metadata.st_mode):
            raise ArchiveAuditError(f"staged path is not a physical directory: {key}")
        entries[key] = StageEntry(
            "directory", stat.S_IMODE(metadata.st_mode), 0, None, metadata
        )
        if len(entries) > MAX_MEMBER_COUNT:
            raise ArchiveAuditError("staged evidence contains too many members")

        directory_names.sort()
        file_names.sort()
        for name in directory_names:
            child = current / name
            child_metadata = child.lstat()
            child_relative = _relative_path(
                child.relative_to(root).as_posix(), "staged directory"
            )
            if not stat.S_ISDIR(child_metadata.st_mode):
                raise ArchiveAuditError(
                    f"staged directory is a link or special file: {child_relative}"
                )
        for name in file_names:
            path = current / name
            relative = _relative_path(path.relative_to(root).as_posix(), "staged file")
            before = path.lstat()
            if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
                raise ArchiveAuditError(
                    f"staged file is not a singly linked regular file: {relative}"
                )
            if before.st_size > MAX_MEMBER_BYTES:
                raise ArchiveAuditError(f"staged file exceeds its byte bound: {relative}")
            with _stable_regular_reader(path, before, f"staged file {relative}") as stream:
                digest, length = _digest_stream(stream, MAX_MEMBER_BYTES, relative)
            if length != before.st_size:
                raise ArchiveAuditError(f"staged file length changed: {relative}")
            total_file_bytes += length
            if total_file_bytes > MAX_TOTAL_FILE_BYTES:
                raise ArchiveAuditError("staged regular files exceed their aggregate bound")
            entries[relative] = StageEntry(
                "file", stat.S_IMODE(before.st_mode), length, digest, before
            )
            if len(entries) > MAX_MEMBER_COUNT:
                raise ArchiveAuditError("staged evidence contains too many members")

    if not entries or all(entry.kind != "file" for entry in entries.values()):
        raise ArchiveAuditError("staged evidence contains no regular files")
    for relative, entry in entries.items():
        path = root if relative == "." else root / relative
        observed = path.lstat()
        fields = STABLE_FILE_FIELDS if entry.kind == "file" else STABLE_DIRECTORY_FIELDS
        if not _metadata_matches(entry.metadata, observed, fields):
            raise ArchiveAuditError(f"staged evidence changed during snapshot: {relative}")
    return entries


def _read_checksum_manifest(
    root: Path, entries: dict[str, StageEntry]
) -> dict[str, str]:
    manifest_entry = entries.get(CHECKSUM_RELATIVE_PATH)
    if manifest_entry is None or manifest_entry.kind != "file":
        raise ArchiveAuditError(f"staged evidence is missing {CHECKSUM_RELATIVE_PATH}")
    if manifest_entry.size > MAX_CHECKSUM_MANIFEST_BYTES:
        raise ArchiveAuditError("staged checksum manifest exceeds its byte bound")
    path = root / CHECKSUM_RELATIVE_PATH
    with _stable_regular_reader(
        path, manifest_entry.metadata, "staged checksum manifest"
    ) as stream:
        encoded = stream.read(MAX_CHECKSUM_MANIFEST_BYTES + 1)
    if len(encoded) > MAX_CHECKSUM_MANIFEST_BYTES or not encoded.endswith(b"\n"):
        raise ArchiveAuditError("staged checksum manifest is oversized or unterminated")

    checksums: dict[str, str] = {}
    ordered_paths: list[str] = []
    for line in encoded.splitlines():
        match = SHA256_LINE.fullmatch(line)
        if match is None:
            raise ArchiveAuditError("staged checksum manifest has a malformed line")
        try:
            relative = _relative_path(match.group(2).decode("ascii"), "checksum path")
        except UnicodeDecodeError as error:
            raise ArchiveAuditError("checksum path is not canonical ASCII") from error
        if relative in checksums:
            raise ArchiveAuditError(f"staged checksum manifest repeats {relative}")
        checksums[relative] = match.group(1).decode("ascii")
        ordered_paths.append(relative)
    if ordered_paths != sorted(ordered_paths):
        raise ArchiveAuditError("staged checksum manifest paths are not sorted")

    expected_paths = {
        relative
        for relative, entry in entries.items()
        if entry.kind == "file" and relative != CHECKSUM_RELATIVE_PATH
    }
    if set(checksums) != expected_paths:
        raise ArchiveAuditError("staged checksum manifest does not cover the exact file set")
    for relative, expected_digest in checksums.items():
        if entries[relative].digest != expected_digest:
            raise ArchiveAuditError(f"staged checksum mismatch for {relative}")
    return checksums


def _audit_archive(
    archive: Path,
    archive_metadata: os.stat_result,
    stage: dict[str, StageEntry],
) -> None:
    observed_paths: set[str] = set()
    total_file_bytes = 0
    with _stable_regular_reader(archive, archive_metadata, "native evidence archive") as source:
        with tarfile.open(fileobj=source, mode="r:gz") as bundle:
            for member in bundle:
                if len(observed_paths) >= MAX_MEMBER_COUNT:
                    raise ArchiveAuditError("archive contains too many members")
                relative = _archive_member_path(member.name)
                if relative in observed_paths:
                    raise ArchiveAuditError(f"archive repeats member {relative}")
                observed_paths.add(relative)
                if member.issym() or member.islnk():
                    raise ArchiveAuditError(f"archive contains a forbidden link: {relative}")
                if not member.isdir() and not member.isreg():
                    raise ArchiveAuditError(
                        f"archive contains a forbidden special file: {relative}"
                    )
                expected = stage.get(relative)
                if expected is None:
                    raise ArchiveAuditError(f"archive contains unexpected member {relative}")
                if member.uid != 0 or member.gid != 0 or member.mtime != 0:
                    raise ArchiveAuditError(
                        f"archive member metadata is not canonical: {relative}"
                    )
                if stat.S_IMODE(member.mode) != expected.mode:
                    raise ArchiveAuditError(f"archive mode differs from stage: {relative}")

                if member.isdir():
                    if expected.kind != "directory" or member.size != 0:
                        raise ArchiveAuditError(
                            f"archive directory differs from stage: {relative}"
                        )
                    continue
                if expected.kind != "file" or member.size != expected.size:
                    raise ArchiveAuditError(
                        f"archive file length differs from stage: {relative}"
                    )
                if member.size > MAX_MEMBER_BYTES:
                    raise ArchiveAuditError(f"archive member exceeds its byte bound: {relative}")
                extracted = bundle.extractfile(member)
                if extracted is None:
                    raise ArchiveAuditError(f"cannot read archive member {relative}")
                digest, length = _digest_stream(extracted, MAX_MEMBER_BYTES, relative)
                if length != member.size or digest != expected.digest:
                    raise ArchiveAuditError(f"archive content differs from stage: {relative}")
                total_file_bytes += length
                if total_file_bytes > MAX_TOTAL_FILE_BYTES:
                    raise ArchiveAuditError("archive regular files exceed their aggregate bound")

    expected_paths = set(stage)
    if observed_paths != expected_paths:
        missing = sorted(expected_paths - observed_paths)
        raise ArchiveAuditError(
            "archive does not contain the exact staged inventory; missing "
            + ", ".join(missing[:8])
        )


def audit(archive_raw: str, staged_root_raw: str) -> None:
    """Validate one archive against one stable post-creation evidence stage."""

    staged_root = _canonical_directory(staged_root_raw, "staged root")
    archive, archive_metadata = _canonical_archive(archive_raw, staged_root)
    stage = _snapshot_stage(staged_root)
    _read_checksum_manifest(staged_root, stage)
    _audit_archive(archive, archive_metadata, stage)


def _parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", required=True, help="absolute tar.gz path")
    parser.add_argument(
        "--staged-root", required=True, help="absolute staged evidence directory"
    )
    return parser.parse_args()


def main() -> int:
    """Run the strict archive audit."""

    arguments = _parse_arguments()
    try:
        audit(arguments.archive, arguments.staged_root)
    except (ArchiveAuditError, OSError, tarfile.TarError) as error:
        print(f"Taira native privacy archive audit failed: {error}", file=sys.stderr)
        return 1
    print("Taira native privacy archive matches its safe staged checksum closure")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
