#!/usr/bin/env python3
"""Compute the canonical Kagemusha full-source-tree SHA-256.

The seal covers every path in the clean Git index, including its executable
mode and exact regular-file bytes or symlink target bytes. It additionally
binds the ignored root ``Cargo.lock`` consumed by ``cargo build --locked``;
other ignored build outputs remain outside the source tree. A fingerprint is
returned only while the checkout HEAD and full porcelain status remain
unchanged and clean.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import stat
import subprocess
import sys
from dataclasses import dataclass


DOMAIN = b"iroha.kagemusha.full-source-tree-sha256.v2\0"
ALLOWED_MODES = {b"100644", b"100755", b"120000"}
REQUIRED_IGNORED_BUILD_INPUTS = (b"Cargo.lock",)


class SourceSealError(RuntimeError):
    """The checkout cannot produce an unambiguous full-source-tree seal."""


@dataclass(frozen=True)
class IndexEntry:
    mode: bytes
    object_id: bytes
    path: bytes


@dataclass(frozen=True)
class SourceIdentity:
    source_commit: str
    source_tree_sha256: str


def _git(root: pathlib.Path, *arguments: str) -> bytes:
    try:
        return subprocess.run(
            ["git", "-C", os.fspath(root), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        raise SourceSealError(f"git {' '.join(arguments)} failed") from exc


def _repository_root(root: pathlib.Path) -> pathlib.Path:
    requested = root.resolve(strict=True)
    discovered = pathlib.Path(
        os.fsdecode(_git(requested, "rev-parse", "--show-toplevel")).strip()
    ).resolve(strict=True)
    if discovered != requested:
        raise SourceSealError(
            f"--root must be the exact repository root ({discovered})"
        )
    return requested


def _head(root: pathlib.Path) -> bytes:
    value = _git(root, "rev-parse", "--verify", "HEAD^{commit}").strip()
    if len(value) != 40 or any(byte not in b"0123456789abcdef" for byte in value):
        raise SourceSealError("Git HEAD is not one canonical SHA-1 commit id")
    return value


def status(root: pathlib.Path) -> bytes:
    return _git(
        root,
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    )


def _index_entries(root: pathlib.Path) -> list[IndexEntry]:
    records = _git(root, "ls-files", "--stage", "-z").split(b"\0")
    entries: list[IndexEntry] = []
    seen: set[bytes] = set()
    for record in records:
        if not record:
            continue
        try:
            metadata, path = record.split(b"\t", 1)
            mode, object_id, stage = metadata.split(b" ", 2)
        except ValueError as exc:
            raise SourceSealError("Git returned a malformed index record") from exc
        if mode not in ALLOWED_MODES:
            raise SourceSealError(
                f"unsupported Git index mode {os.fsdecode(mode)!r} for {os.fsdecode(path)!r}"
            )
        if len(object_id) != 40 or any(
            byte not in b"0123456789abcdef" for byte in object_id
        ):
            raise SourceSealError("Git returned a non-canonical index object id")
        if stage != b"0":
            raise SourceSealError("the source index contains an unresolved merge stage")
        if (
            not path
            or path.startswith(b"/")
            or path.endswith(b"/")
            or b"\0" in path
            or any(component in (b"", b".", b"..") for component in path.split(b"/"))
            or path in seen
        ):
            raise SourceSealError("Git returned an unsafe or duplicate source path")
        seen.add(path)
        entries.append(IndexEntry(mode=mode, object_id=object_id, path=path))
    if not entries:
        raise SourceSealError("the source index is empty")
    if entries != sorted(entries, key=lambda entry: entry.path):
        entries.sort(key=lambda entry: entry.path)
    return entries


def _field(hasher: "hashlib._Hash", value: bytes) -> None:
    hasher.update(len(value).to_bytes(8, "big"))
    hasher.update(value)


def _regular_hash(
    path: bytes,
    expected_executable: bool,
    entry: IndexEntry,
    source_hasher: "hashlib._Hash",
) -> None:
    before = os.lstat(path)
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise SourceSealError(
            f"tracked source must be a singly linked regular file: {os.fsdecode(path)}"
        )
    if bool(before.st_mode & 0o111) != expected_executable:
        raise SourceSealError(
            f"tracked source executable mode differs from the index: {os.fsdecode(path)}"
        )
    flags = os.O_RDONLY
    flags |= getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened_before = os.fstat(descriptor)
        blob_hasher = hashlib.sha1(
            b"blob " + str(opened_before.st_size).encode("ascii") + b"\0",
            usedforsecurity=False,
        )
        source_hasher.update(opened_before.st_size.to_bytes(8, "big"))
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            source_hasher.update(chunk)
            blob_hasher.update(chunk)
        opened_after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = os.lstat(path)
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    if (
        identity(before) != identity(opened_before)
        or identity(opened_before) != identity(opened_after)
        or identity(opened_after) != identity(after)
    ):
        raise SourceSealError(f"tracked source changed while read: {os.fsdecode(path)}")
    if total != opened_after.st_size:
        raise SourceSealError(f"tracked source was truncated while read: {os.fsdecode(path)}")
    if blob_hasher.hexdigest().encode("ascii") != entry.object_id:
        raise SourceSealError(
            f"tracked source bytes differ from the clean Git index: {os.fsdecode(path)}"
        )


def _symlink_bytes(path: bytes, entry: IndexEntry) -> bytes:
    before = os.lstat(path)
    if not stat.S_ISLNK(before.st_mode) or before.st_nlink != 1:
        raise SourceSealError(
            f"tracked symlink must be singly linked: {os.fsdecode(path)}"
        )
    payload = os.readlink(path)
    after = os.lstat(path)
    if not isinstance(payload, bytes):
        payload = os.fsencode(payload)
    if (
        before.st_dev,
        before.st_ino,
        before.st_mode,
        before.st_nlink,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    ) != (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_nlink,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ):
        raise SourceSealError(f"tracked symlink changed while read: {os.fsdecode(path)}")
    blob_hasher = hashlib.sha1(
        b"blob " + str(len(payload)).encode("ascii") + b"\0",
        usedforsecurity=False,
    )
    blob_hasher.update(payload)
    if blob_hasher.hexdigest().encode("ascii") != entry.object_id:
        raise SourceSealError(
            f"tracked symlink differs from the clean Git index: {os.fsdecode(path)}"
        )
    return payload


def _required_ignored_regular_hash(
    path: bytes,
    display_path: bytes,
    source_hasher: "hashlib._Hash",
) -> bytes:
    """Seal one mandatory ignored Cargo build input with stable file identity."""

    before = os.lstat(path)
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_mode & 0o111 != 0
    ):
        raise SourceSealError(
            "required ignored build input must be a singly linked, "
            f"non-executable regular file: {os.fsdecode(display_path)}"
        )
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened_before = os.fstat(descriptor)
        encoded_size = opened_before.st_size.to_bytes(8, "big")
        source_hasher.update(encoded_size)
        input_hasher = hashlib.sha256(encoded_size)
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            source_hasher.update(chunk)
            input_hasher.update(chunk)
        opened_after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = os.lstat(path)
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_nlink,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    if not (
        identity(before)
        == identity(opened_before)
        == identity(opened_after)
        == identity(after)
    ):
        raise SourceSealError(
            f"required ignored build input changed while read: {os.fsdecode(display_path)}"
        )
    if total != opened_after.st_size:
        raise SourceSealError(
            f"required ignored build input was truncated: {os.fsdecode(display_path)}"
        )
    return input_hasher.digest()


def compute_identity(root: pathlib.Path) -> SourceIdentity:
    root = _repository_root(root)
    head_before = _head(root)
    if status(root):
        raise SourceSealError(
            "Kagemusha source tree must be clean, including untracked files"
        )
    entries = _index_entries(root)
    hasher = hashlib.sha256(DOMAIN)
    root_bytes = os.fsencode(root)
    for entry in entries:
        absolute = os.path.join(root_bytes, entry.path)
        _field(hasher, entry.path)
        _field(hasher, entry.mode)
        if entry.mode == b"120000":
            payload = _symlink_bytes(absolute, entry)
            _field(hasher, payload)
        else:
            _regular_hash(absolute, entry.mode == b"100755", entry, hasher)
    ignored_input_digests: dict[bytes, bytes] = {}
    for relative in REQUIRED_IGNORED_BUILD_INPUTS:
        _field(hasher, b"required-ignored-build-input-v1")
        _field(hasher, relative)
        _field(hasher, b"100644")
        ignored_input_digests[relative] = _required_ignored_regular_hash(
            os.path.join(root_bytes, relative), relative, hasher
        )
    if _head(root) != head_before or status(root):
        raise SourceSealError("Kagemusha source HEAD or tree changed while sealing")
    for relative, expected_digest in ignored_input_digests.items():
        actual_digest = _required_ignored_regular_hash(
            os.path.join(root_bytes, relative),
            relative,
            hashlib.sha256(b"ignored-input-recheck"),
        )
        if actual_digest != expected_digest:
            raise SourceSealError(
                f"required ignored build input changed while sealing: {os.fsdecode(relative)}"
            )
    if _head(root) != head_before or status(root):
        raise SourceSealError("Kagemusha source HEAD or tree changed while sealing")
    return SourceIdentity(
        source_commit=head_before.decode("ascii"),
        source_tree_sha256=hasher.hexdigest(),
    )


def compute_fingerprint(root: pathlib.Path) -> str:
    return compute_identity(root).source_tree_sha256


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("fingerprint", "identity", "status", "paths"))
    parser.add_argument("--root", type=pathlib.Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = _repository_root(args.root)
    if args.mode == "fingerprint":
        print(compute_fingerprint(root))
    elif args.mode == "identity":
        identity = compute_identity(root)
        sys.stdout.write(
            json.dumps(
                {
                    "schema": "iroha.kagemusha.full_source_tree_identity.v1",
                    "source_commit": identity.source_commit,
                    "source_repo_dirty": False,
                    "source_tree_sha256": identity.source_tree_sha256,
                },
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        )
    elif args.mode == "status":
        value = status(root)
        if value:
            sys.stdout.buffer.write(value)
    else:
        for entry in _index_entries(root):
            sys.stdout.buffer.write(entry.path + b"\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, SourceSealError) as exc:
        print(f"Kagemusha source-tree seal failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
