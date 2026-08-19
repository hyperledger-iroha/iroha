#!/usr/bin/env python3
"""Shared fail-closed contracts for deterministic release artifacts."""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import re
import stat
import sys
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Iterable, Mapping, NoReturn


RELEASE_MANIFEST_SCHEMA = "iroha.release_manifest"
RELEASE_MANIFEST_SCHEMA_VERSION = 1
# gzip and several release-channel formats carry an unsigned 32-bit timestamp.
# The shared policy uses the narrowest supported representation so one accepted
# epoch cannot fail later in a mandatory builder.
MAX_SOURCE_DATE_EPOCH = 4_294_967_295
MAX_RELEASE_MANIFEST_SIZE = 16 * 1024 * 1024
MAX_RELATIVE_PATH_BYTES = 512
ALLOWED_PROFILES = frozenset({"iroha2", "iroha3", "shared"})
ALLOWED_KIND_FORMATS: Mapping[str, frozenset[str]] = {
    "bundle": frozenset({"tar.zst"}),
    "checksum": frozenset({"sha256"}),
    "builder-manifest": frozenset({"json"}),
    "image": frozenset({"docker-archive", "oci-archive"}),
    "profile-matrix": frozenset({"json"}),
    "changelog": frozenset({"markdown"}),
    "sbom": frozenset({"json", "spdx-json", "cyclonedx-json"}),
    "provenance": frozenset({"json", "intoto-jsonl", "sigstore"}),
    "release-evidence": frozenset(
        {"binary", "csv", "json", "markdown", "tar.zst", "text", "yaml", "yml"}
    ),
    "sdk-package": frozenset(
        {"aar", "binary", "jar", "json", "module", "pom", "sha256", "zip"}
    ),
    "reference-validator": frozenset(
        {"binary", "header", "json", "sha256", "tar.gz"}
    ),
}
_HEX_SHA256_RE = re.compile(r"[0-9a-f]{64}")
_SAFE_TOKEN_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._+-]{0,127}")
_FORMAT_SUFFIXES: Mapping[str, tuple[str, ...]] = {
    "aar": (".aar",),
    "cyclonedx-json": (".json",),
    "docker-archive": (".tar",),
    "header": (".h",),
    "intoto-jsonl": (".jsonl",),
    "jar": (".jar",),
    "json": (".json",),
    "markdown": (".md",),
    "module": (".module",),
    "oci-archive": (".tar",),
    "pom": (".pom",),
    "sha256": (".sha256",),
    "sigstore": (".sigstore",),
    "spdx-json": (".json",),
    "tar.gz": (".tar.gz",),
    "tar.zst": (".tar.zst",),
    "text": (".txt",),
    "yaml": (".yaml",),
    "yml": (".yml",),
    "zip": (".zip",),
}


class ReleaseArtifactError(RuntimeError):
    """Raised when release data violates the canonical artifact contract."""


@dataclass(frozen=True)
class StableFile:
    """Identity and digest captured from one stable regular-file descriptor."""

    sha256: str
    size: int
    mode: int
    device: int
    inode: int
    mtime_ns: int
    ctime_ns: int
    link_count: int


def _fail(message: str) -> NoReturn:
    raise ReleaseArtifactError(message)


def _contains_control(value: str) -> bool:
    return any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)


def parse_source_date_epoch(raw: str) -> int:
    """Parse the one accepted canonical SOURCE_DATE_EPOCH representation."""

    if not isinstance(raw, str) or re.fullmatch(r"0|[1-9][0-9]*", raw) is None:
        _fail("SOURCE_DATE_EPOCH must be canonical nonnegative decimal")
    epoch = int(raw)
    if epoch > MAX_SOURCE_DATE_EPOCH:
        _fail(
            "SOURCE_DATE_EPOCH exceeds the supported UTC range "
            f"(maximum {MAX_SOURCE_DATE_EPOCH})"
        )
    try:
        dt.datetime.fromtimestamp(epoch, tz=dt.timezone.utc)
    except (OverflowError, OSError, ValueError) as exc:
        raise ReleaseArtifactError(
            "SOURCE_DATE_EPOCH is outside the platform UTC range"
        ) from exc
    return epoch


def format_source_date_epoch(epoch: int) -> str:
    """Render an already validated epoch as canonical UTC RFC3339 seconds."""

    validated = parse_source_date_epoch(str(epoch))
    return dt.datetime.fromtimestamp(
        validated, tz=dt.timezone.utc
    ).strftime("%Y-%m-%dT%H:%M:%SZ")


def canonical_relative_path(value: str) -> str:
    """Validate and return one canonical portable release-relative path."""

    if not isinstance(value, str) or not value:
        _fail("release artifact path must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_RELATIVE_PATH_BYTES:
        _fail("release artifact path exceeds the 512-byte limit")
    if _contains_control(value):
        _fail("release artifact path must not contain control characters")
    if "\\" in value:
        _fail("release artifact path must not contain backslashes")
    if ":" in value:
        _fail("release artifact path must not contain colon characters")
    if value.startswith("/") or value.endswith("/") or "//" in value:
        _fail("release artifact path must be canonical and relative")
    pure = PurePosixPath(value)
    if pure.is_absolute() or any(part in {"", ".", ".."} for part in pure.parts):
        _fail("release artifact path must not contain dot or parent segments")
    if pure.as_posix() != value:
        _fail("release artifact path is not in canonical POSIX form")
    return value


def validate_artifact_descriptor(
    descriptor: Mapping[str, object],
    *,
    require_digest: bool,
) -> dict[str, object]:
    """Validate the exact aggregate-manifest artifact row schema."""

    required = {"profile", "target", "kind", "format", "path"}
    if require_digest:
        required |= {"sha256", "size"}
    if set(descriptor) != required:
        _fail(
            "release artifact row fields must be exactly "
            + ", ".join(sorted(required))
        )
    profile = descriptor["profile"]
    target = descriptor["target"]
    kind = descriptor["kind"]
    fmt = descriptor["format"]
    path = descriptor["path"]
    if not isinstance(profile, str) or profile not in ALLOWED_PROFILES:
        _fail(f"unsupported release artifact profile: {profile!r}")
    if not isinstance(target, str) or _SAFE_TOKEN_RE.fullmatch(target) is None:
        _fail("release artifact target must be a bounded safe target token")
    if not isinstance(kind, str) or kind not in ALLOWED_KIND_FORMATS:
        _fail(f"unsupported release artifact kind: {kind!r}")
    if not isinstance(fmt, str) or fmt not in ALLOWED_KIND_FORMATS[kind]:
        _fail(f"unsupported format {fmt!r} for release artifact kind {kind!r}")
    if not isinstance(path, str):
        _fail("release artifact path must be a string")
    normalized_path = canonical_relative_path(path)
    suffixes = _FORMAT_SUFFIXES.get(fmt)
    if suffixes is not None and not normalized_path.endswith(suffixes):
        _fail(
            f"release artifact path {normalized_path!r} does not match format {fmt!r}"
        )
    result: dict[str, object] = {
        "profile": profile,
        "target": target,
        "kind": kind,
        "format": fmt,
        "path": normalized_path,
    }
    if require_digest:
        digest = descriptor["sha256"]
        size = descriptor["size"]
        if not isinstance(digest, str) or _HEX_SHA256_RE.fullmatch(digest) is None:
            _fail("release artifact SHA256 must be exactly 64 lowercase hex characters")
        if isinstance(size, bool) or not isinstance(size, int) or size <= 0:
            _fail("release artifact size must be a positive integer")
        result["sha256"] = digest
        result["size"] = size
    return result


def parse_artifact_spec(raw: str) -> dict[str, object]:
    """Parse `profile:target:kind:format:path` without ambiguity."""

    if not isinstance(raw, str):
        _fail("release artifact specification must be a string")
    parts = raw.split(":", 4)
    if len(parts) != 5 or any(not part for part in parts):
        _fail(
            "release artifact specification must be "
            "profile:target:kind:format:relative-path"
        )
    return validate_artifact_descriptor(
        {
            "profile": parts[0],
            "target": parts[1],
            "kind": parts[2],
            "format": parts[3],
            "path": parts[4],
        },
        require_digest=False,
    )


def _directory_open_flags() -> int:
    return (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def _open_absolute_directory(path: Path, label: str) -> tuple[int, Path, os.stat_result]:
    """Open every absolute path component relative to a pinned parent fd."""

    absolute = Path(os.path.abspath(path))
    components = absolute.parts[1:]
    current_fd = -1
    try:
        current_fd = os.open("/", _directory_open_flags())
        for component in components:
            before = os.stat(component, dir_fd=current_fd, follow_symlinks=False)
            next_fd = os.open(
                component,
                _directory_open_flags(),
                dir_fd=current_fd,
            )
            opened = os.fstat(next_fd)
            if not stat.S_ISDIR(opened.st_mode):
                os.close(next_fd)
                _fail(f"{label} component is not a directory: {component}")
            if (before.st_dev, before.st_ino) != (opened.st_dev, opened.st_ino):
                os.close(next_fd)
                _fail(f"{label} changed while its path was opened")
            os.close(current_fd)
            current_fd = next_fd
        identity = os.fstat(current_fd)
        return current_fd, absolute, identity
    except ReleaseArtifactError:
        if current_fd >= 0:
            os.close(current_fd)
        raise
    except OSError as exc:
        if current_fd >= 0:
            os.close(current_fd)
        raise ReleaseArtifactError(f"failed to open {label} {absolute}: {exc}") from exc


def _absolute_without_symlink_components(path: Path, label: str) -> Path:
    directory_fd, absolute, _ = _open_absolute_directory(path, label)
    os.close(directory_fd)
    return absolute


def create_fresh_directory(path: Path, *, mode: int = 0o755) -> Path:
    """Create one fresh directory tree without following existing links."""

    if mode not in {0o700, 0o755}:
        _fail("release directory mode must be exactly 0700 or 0755")
    absolute = Path(os.path.abspath(path))
    components = absolute.parts[1:]
    if not components:
        _fail("release directory path must not be the filesystem root")
    current_fd = os.open("/", _directory_open_flags())
    try:
        for index, component in enumerate(components):
            final = index == len(components) - 1
            try:
                before = os.stat(
                    component,
                    dir_fd=current_fd,
                    follow_symlinks=False,
                )
            except FileNotFoundError:
                before = None
            created = before is None
            if before is not None:
                if final:
                    _fail(f"fresh release directory already exists: {absolute}")
                if not stat.S_ISDIR(before.st_mode):
                    _fail(
                        f"release directory component is not a directory: "
                        f"{component}"
                    )
                if before.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
                    _fail(
                        f"release directory component is group- or "
                        f"world-writable: {component}"
                    )
            else:
                os.mkdir(component, mode=mode, dir_fd=current_fd)
                os.fsync(current_fd)
                before = os.stat(
                    component,
                    dir_fd=current_fd,
                    follow_symlinks=False,
                )
            next_fd = os.open(
                component,
                _directory_open_flags(),
                dir_fd=current_fd,
            )
            opened = os.fstat(next_fd)
            if (
                not stat.S_ISDIR(opened.st_mode)
                or (before.st_dev, before.st_ino)
                != (opened.st_dev, opened.st_ino)
            ):
                os.close(next_fd)
                _fail(
                    f"release directory component changed while it was opened: "
                    f"{component}"
                )
            if created:
                os.fchmod(next_fd, mode)
                created_info = os.fstat(next_fd)
                if stat.S_IMODE(created_info.st_mode) != mode:
                    os.close(next_fd)
                    _fail(f"fresh release directory mode is not {mode:04o}")
                os.fsync(next_fd)
            os.close(current_fd)
            current_fd = next_fd
        return absolute
    except ReleaseArtifactError:
        raise
    except OSError as exc:
        raise ReleaseArtifactError(
            f"failed to create fresh release directory {absolute}: {exc}"
        ) from exc
    finally:
        if current_fd >= 0:
            os.close(current_fd)


def _open_anchored_regular(root: Path, relative_path: str) -> tuple[int, os.stat_result, int]:
    root_flags = _directory_open_flags()
    file_flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        # A path can be exchanged for a FIFO or device after the no-follow
        # stat but before open(2).  Nonblocking open lets the descriptor-side
        # regular-file check reject that race instead of hanging the release
        # authority on an attacker-controlled FIFO.
        | getattr(os, "O_NONBLOCK", 0)
    )
    current_fd = -1
    try:
        current_fd, _, _ = _open_absolute_directory(
            root, "release artifact root"
        )
        parts = PurePosixPath(canonical_relative_path(relative_path)).parts
        for component in parts[:-1]:
            before = os.stat(component, dir_fd=current_fd, follow_symlinks=False)
            next_fd = os.open(component, root_flags, dir_fd=current_fd)
            opened = os.fstat(next_fd)
            if (
                not stat.S_ISDIR(opened.st_mode)
                or (before.st_dev, before.st_ino)
                != (opened.st_dev, opened.st_ino)
            ):
                os.close(next_fd)
                _fail(
                    f"release artifact parent changed while opening "
                    f"{relative_path!r}"
                )
            os.close(current_fd)
            current_fd = next_fd
        before_path = os.stat(parts[-1], dir_fd=current_fd, follow_symlinks=False)
        if not stat.S_ISREG(before_path.st_mode):
            _fail(f"release artifact {relative_path!r} must be a regular file")
        file_fd = os.open(parts[-1], file_flags, dir_fd=current_fd)
    except ReleaseArtifactError:
        if current_fd >= 0:
            os.close(current_fd)
        raise
    except OSError as exc:
        if current_fd >= 0:
            os.close(current_fd)
        raise ReleaseArtifactError(
            f"failed to open release artifact {relative_path!r}: {exc}"
        ) from exc
    return file_fd, before_path, current_fd


def stable_read_relative(
    root: Path,
    relative_path: str,
    *,
    max_size: int | None = None,
    return_payload: bool,
) -> tuple[StableFile, bytes | None]:
    """Hash/read one direct regular file and reject identity changes."""

    normalized = canonical_relative_path(relative_path)
    file_fd = -1
    parent_fd = -1
    try:
        file_fd, before_path, parent_fd = _open_anchored_regular(root, normalized)
        before = os.fstat(file_fd)
        if not stat.S_ISREG(before.st_mode):
            _fail(f"release artifact {normalized!r} must be a regular file")
        if (before.st_dev, before.st_ino) != (before_path.st_dev, before_path.st_ino):
            _fail(f"release artifact {normalized!r} changed before it was pinned")
        if before.st_nlink != 1:
            _fail(f"release artifact {normalized!r} must have exactly one hard link")
        if before.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
            _fail(
                f"release artifact {normalized!r} must not be group- or world-writable"
            )
        if before.st_size <= 0:
            _fail(f"release artifact {normalized!r} must not be empty")
        if max_size is not None and before.st_size > max_size:
            _fail(
                f"release artifact {normalized!r} exceeds the {max_size}-byte limit"
            )
        digest = hashlib.sha256()
        payload = bytearray() if return_payload else None
        total = 0
        while True:
            chunk = os.read(file_fd, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            digest.update(chunk)
            if payload is not None:
                payload.extend(chunk)
        after = os.fstat(file_fd)
        after_path = os.stat(
            PurePosixPath(normalized).parts[-1],
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
        stable_fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
            "st_mode",
            "st_nlink",
        )
        if total != before.st_size or any(
            getattr(before, field) != getattr(after, field)
            or getattr(before, field) != getattr(after_path, field)
            for field in stable_fields
        ):
            _fail(f"release artifact {normalized!r} changed while it was read")
        reopened_fd = -1
        reopened_parent_fd = -1
        try:
            reopened_fd, reopened_path, reopened_parent_fd = _open_anchored_regular(
                root, normalized
            )
            reopened = os.fstat(reopened_fd)
            if any(
                getattr(before, field) != getattr(reopened, field)
                or getattr(before, field) != getattr(reopened_path, field)
                for field in stable_fields
            ):
                _fail(
                    f"release artifact {normalized!r} path was replaced while it "
                    "was read"
                )
        finally:
            if reopened_fd >= 0:
                os.close(reopened_fd)
            if reopened_parent_fd >= 0:
                os.close(reopened_parent_fd)
        info = StableFile(
            sha256=digest.hexdigest(),
            size=total,
            mode=stat.S_IMODE(before.st_mode),
            device=before.st_dev,
            inode=before.st_ino,
            mtime_ns=before.st_mtime_ns,
            ctime_ns=before.st_ctime_ns,
            link_count=before.st_nlink,
        )
        return info, bytes(payload) if payload is not None else None
    except OSError as exc:
        raise ReleaseArtifactError(
            f"failed to read release artifact {normalized!r}: {exc}"
        ) from exc
    finally:
        if file_fd >= 0:
            os.close(file_fd)
        if parent_fd >= 0:
            os.close(parent_fd)


def stable_hash_relative(
    root: Path,
    relative_path: str,
    *,
    max_size: int | None = None,
) -> StableFile:
    info, _ = stable_read_relative(
        root,
        relative_path,
        max_size=max_size,
        return_payload=False,
    )
    return info


@contextmanager
def stable_open_relative(
    root: Path,
    relative_path: str,
    *,
    expected: StableFile,
):
    """Yield a pinned read descriptor matching an earlier stable capture."""

    normalized = canonical_relative_path(relative_path)
    file_fd = -1
    parent_fd = -1
    reopened_fd = -1
    reopened_parent_fd = -1
    stable_fields = (
        "st_dev",
        "st_ino",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
        "st_mode",
        "st_nlink",
    )
    try:
        file_fd, before_path, parent_fd = _open_anchored_regular(root, normalized)
        before = os.fstat(file_fd)
        if not stat.S_ISREG(before.st_mode):
            _fail(f"release artifact {normalized!r} must be a regular file")
        if (before.st_dev, before.st_ino) != (before_path.st_dev, before_path.st_ino):
            _fail(f"release artifact {normalized!r} changed before it was pinned")
        if before.st_nlink != 1:
            _fail(f"release artifact {normalized!r} must have exactly one hard link")
        if before.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
            _fail(
                f"release artifact {normalized!r} must not be group- or world-writable"
            )
        if (
            before.st_size != expected.size
            or stat.S_IMODE(before.st_mode) != expected.mode
            or before.st_dev != expected.device
            or before.st_ino != expected.inode
            or before.st_mtime_ns != expected.mtime_ns
            or before.st_ctime_ns != expected.ctime_ns
            or before.st_nlink != expected.link_count
        ):
            _fail(
                f"release artifact {normalized!r} no longer matches its "
                "stable capture"
            )
        yield file_fd
        after = os.fstat(file_fd)
        named = os.stat(
            PurePosixPath(normalized).parts[-1],
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
        if any(
            getattr(before, field) != getattr(after, field)
            or getattr(before, field) != getattr(named, field)
            for field in stable_fields
        ):
            _fail(f"release artifact {normalized!r} changed while it was streamed")
        reopened_fd, reopened_path, reopened_parent_fd = _open_anchored_regular(
            root, normalized
        )
        reopened = os.fstat(reopened_fd)
        if any(
            getattr(before, field) != getattr(reopened, field)
            or getattr(before, field) != getattr(reopened_path, field)
            for field in stable_fields
        ):
            _fail(
                f"release artifact {normalized!r} path was replaced while it "
                "was streamed"
            )
    except OSError as exc:
        raise ReleaseArtifactError(
            f"failed to stream release artifact {normalized!r}: {exc}"
        ) from exc
    finally:
        if reopened_fd >= 0:
            os.close(reopened_fd)
        if reopened_parent_fd >= 0:
            os.close(reopened_parent_fd)
        if file_fd >= 0:
            os.close(file_fd)
        if parent_fd >= 0:
            os.close(parent_fd)


def stable_read_path(
    path: Path,
    *,
    max_size: int | None = None,
) -> tuple[StableFile, bytes]:
    absolute = Path(os.path.abspath(path))
    info, payload = stable_read_relative(
        absolute.parent,
        absolute.name,
        max_size=max_size,
        return_payload=True,
    )
    assert payload is not None
    return info, payload


def stable_hash_path(
    path: Path,
    *,
    max_size: int | None = None,
) -> StableFile:
    """Hash one absolute or working-directory-relative stable file."""

    absolute = Path(os.path.abspath(path))
    return stable_hash_relative(
        absolute.parent,
        absolute.name,
        max_size=max_size,
    )


def scan_inventory_paths(
    root: Path,
    *,
    ignored: Iterable[str] = (),
) -> list[str]:
    """Return a sorted closed inventory, rejecting links and special files."""

    ignored_set = {canonical_relative_path(value) for value in ignored}
    paths: list[str] = []

    def visit(directory_fd: int, prefix: PurePosixPath) -> None:
        try:
            names = sorted(os.listdir(directory_fd))
        except OSError as exc:
            raise ReleaseArtifactError(
                f"failed to scan release artifact directory {prefix}: {exc}"
            ) from exc
        for name in names:
            relative = (prefix / name).as_posix()
            canonical_relative_path(relative)
            try:
                info = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
            except OSError as exc:
                raise ReleaseArtifactError(
                    f"failed to inspect release inventory entry {relative!r}: {exc}"
                ) from exc
            if stat.S_ISLNK(info.st_mode):
                _fail(f"release inventory entry {relative!r} must not be a symlink")
            if stat.S_ISDIR(info.st_mode):
                if info.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
                    _fail(
                        f"release inventory directory {relative!r} must not be "
                        "group- or world-writable"
                    )
                child_fd = -1
                try:
                    child_fd = os.open(
                        name,
                        _directory_open_flags(),
                        dir_fd=directory_fd,
                    )
                    opened = os.fstat(child_fd)
                    if (info.st_dev, info.st_ino) != (
                        opened.st_dev,
                        opened.st_ino,
                    ):
                        _fail(
                            f"release inventory directory {relative!r} changed "
                            "while it was opened"
                        )
                    visit(child_fd, prefix / name)
                    after = os.stat(
                        name,
                        dir_fd=directory_fd,
                        follow_symlinks=False,
                    )
                    if (info.st_dev, info.st_ino) != (
                        after.st_dev,
                        after.st_ino,
                    ):
                        _fail(
                            f"release inventory directory {relative!r} was "
                            "replaced while it was scanned"
                        )
                finally:
                    if child_fd >= 0:
                        os.close(child_fd)
            elif stat.S_ISREG(info.st_mode):
                if relative not in ignored_set:
                    paths.append(relative)
            else:
                _fail(
                    f"release inventory entry {relative!r} must be a regular "
                    "file or directory"
                )

    root_fd = -1
    reopened_fd = -1
    try:
        root_fd, _, root_identity = _open_absolute_directory(
            root, "release artifact root"
        )
        if root_identity.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
            _fail(
                "release artifact root must not be group- or world-writable"
            )
        visit(root_fd, PurePosixPath())
        reopened_fd, _, reopened_identity = _open_absolute_directory(
            root, "release artifact root"
        )
        if (root_identity.st_dev, root_identity.st_ino) != (
            reopened_identity.st_dev,
            reopened_identity.st_ino,
        ):
            _fail("release artifact root was replaced while it was scanned")
    finally:
        if root_fd >= 0:
            os.close(root_fd)
        if reopened_fd >= 0:
            os.close(reopened_fd)
    return paths


def parse_sha256sums(root: Path, relative_path: str = "SHA256SUMS") -> dict[str, str]:
    """Parse exact canonical sha256sum rows with no omissions or duplicates."""

    _, payload = stable_read_relative(
        root,
        relative_path,
        max_size=MAX_RELEASE_MANIFEST_SIZE,
        return_payload=True,
    )
    assert payload is not None
    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError as exc:
        raise ReleaseArtifactError("SHA256SUMS must be ASCII") from exc
    if not text.endswith("\n"):
        _fail("SHA256SUMS must end with one newline")
    lines = text[:-1].split("\n")
    if not lines or any(not line for line in lines):
        _fail("SHA256SUMS must contain only non-empty canonical rows")
    result: dict[str, str] = {}
    ordered_paths: list[str] = []
    for line in lines:
        match = re.fullmatch(r"([0-9a-f]{64})  ([^\r\n]+)", line)
        if match is None:
            _fail(f"malformed canonical SHA256SUMS row: {line!r}")
        digest, raw_path = match.groups()
        path = canonical_relative_path(raw_path)
        if path == relative_path:
            _fail("SHA256SUMS must not contain a self-referential row")
        if path in result:
            _fail(f"duplicate SHA256SUMS path: {path}")
        result[path] = digest
        ordered_paths.append(path)
    if ordered_paths != sorted(ordered_paths):
        _fail("SHA256SUMS rows must be sorted by canonical path")
    return result


def _reject_duplicate_json_keys(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def load_json_object(payload: bytes, label: str) -> dict[str, object]:
    try:
        value = json.loads(payload, object_pairs_hook=_reject_duplicate_json_keys)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReleaseArtifactError(f"{label} is not canonical UTF-8 JSON: {exc}") from exc
    if not isinstance(value, dict):
        _fail(f"{label} root must be an object")
    return value


def validate_release_manifest(manifest: Mapping[str, object]) -> dict[str, object]:
    """Validate and normalize the exact canonical release-manifest schema."""

    required = {
        "schema",
        "schema_version",
        "version",
        "commit",
        "source_date_epoch",
        "built_at",
        "os",
        "arch",
        "artifacts",
    }
    if set(manifest) != required:
        _fail(
            "release manifest fields must be exactly "
            + ", ".join(sorted(required))
        )
    if manifest["schema"] != RELEASE_MANIFEST_SCHEMA:
        _fail("release manifest schema identifier is unsupported")
    if manifest["schema_version"] != RELEASE_MANIFEST_SCHEMA_VERSION:
        _fail("release manifest schema version is unsupported")
    for name in ("version", "os", "arch"):
        value = manifest[name]
        if not isinstance(value, str) or _SAFE_TOKEN_RE.fullmatch(value) is None:
            _fail(f"release manifest {name} must be a bounded safe token")
    commit = manifest["commit"]
    if not isinstance(commit, str) or re.fullmatch(
        r"(?:[0-9a-f]{40}|[0-9a-f]{64})", commit
    ) is None:
        _fail("release manifest commit must be a full 40- or 64-hex identifier")
    epoch = manifest["source_date_epoch"]
    if isinstance(epoch, bool) or not isinstance(epoch, int):
        _fail("release manifest source_date_epoch must be an integer")
    epoch = parse_source_date_epoch(str(epoch))
    if manifest["built_at"] != format_source_date_epoch(epoch):
        _fail("release manifest built_at does not match source_date_epoch")
    artifacts = manifest["artifacts"]
    if not isinstance(artifacts, list) or not artifacts:
        _fail("release manifest artifacts must be a non-empty array")
    normalized_artifacts: list[dict[str, object]] = []
    seen_paths: set[str] = set()
    for raw in artifacts:
        if not isinstance(raw, dict):
            _fail("release manifest artifact rows must be objects")
        row = validate_artifact_descriptor(raw, require_digest=True)
        path = str(row["path"])
        if path in seen_paths:
            _fail(f"duplicate release manifest artifact path: {path}")
        seen_paths.add(path)
        normalized_artifacts.append(row)
    if normalized_artifacts != sorted(
        normalized_artifacts,
        key=lambda row: (
            str(row["path"]),
            str(row["profile"]),
            str(row["target"]),
            str(row["kind"]),
            str(row["format"]),
        ),
    ):
        _fail("release manifest artifact rows must be canonically sorted")
    return {
        "schema": RELEASE_MANIFEST_SCHEMA,
        "schema_version": RELEASE_MANIFEST_SCHEMA_VERSION,
        "version": manifest["version"],
        "commit": commit,
        "source_date_epoch": epoch,
        "built_at": manifest["built_at"],
        "os": manifest["os"],
        "arch": manifest["arch"],
        "artifacts": normalized_artifacts,
    }


def canonical_json_bytes(value: object) -> bytes:
    return (
        json.dumps(
            value,
            indent=2,
            sort_keys=True,
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")


def verify_private_python_source_closure(
    root: Path,
    manifest: Mapping[str, object],
    expected_sha256: str,
    *,
    owner_uid: int,
    entrypoint: str | None = None,
    require_isolated_runtime: bool = False,
) -> None:
    """Verify one exact owner-private Python source tree and its loaded origins.

    This contract is intentionally stricter than an ordinary release inventory:
    the root and every directory are mode 0700, every source file is mode 0600
    with one link, and no unlisted path (including bytecode caches) is admitted.
    When used by an entry point, Python must have been launched with ``-I -B -S``
    and every loaded closure module must originate in the verified tree.
    """

    if (
        not isinstance(expected_sha256, str)
        or _HEX_SHA256_RE.fullmatch(expected_sha256) is None
        or set(manifest) != {"schema", "files"}
        or not isinstance(manifest.get("schema"), str)
        or not isinstance(manifest.get("files"), list)
    ):
        _fail("private Python source closure manifest is invalid")
    rows = manifest["files"]
    assert isinstance(rows, list)
    normalized_rows: list[dict[str, object]] = []
    for row in rows:
        if not isinstance(row, Mapping) or set(row) != {"path", "sha256", "size"}:
            _fail("private Python source closure row is invalid")
        path = row.get("path")
        digest = row.get("sha256")
        size = row.get("size")
        if (
            not isinstance(path, str)
            or canonical_relative_path(path) != path
            or not isinstance(digest, str)
            or _HEX_SHA256_RE.fullmatch(digest) is None
            or isinstance(size, bool)
            or not isinstance(size, int)
            or size <= 0
        ):
            _fail("private Python source closure row is invalid")
        normalized_rows.append({"path": path, "sha256": digest, "size": size})
    if [row["path"] for row in normalized_rows] != sorted(
        {str(row["path"]) for row in normalized_rows}
    ):
        _fail("private Python source closure rows are not uniquely sorted")
    if hashlib.sha256(canonical_json_bytes(dict(manifest))).hexdigest() != expected_sha256:
        _fail("private Python source closure digest differs from the manifest")

    absolute_root = Path(os.path.abspath(root))
    try:
        root_info = absolute_root.lstat()
    except OSError as exc:
        raise ReleaseArtifactError(
            f"failed to inspect private Python source closure root: {exc}"
        ) from exc
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or root_info.st_uid != owner_uid
        or stat.S_IMODE(root_info.st_mode) != 0o700
    ):
        _fail("private Python source closure root must be owner UID mode 0700")

    expected_files = {str(row["path"]) for row in normalized_rows}
    expected_directories: set[str] = set()
    for relative in expected_files:
        parent = PurePosixPath(relative).parent
        while parent != PurePosixPath("."):
            expected_directories.add(parent.as_posix())
            parent = parent.parent
    actual_files: set[str] = set()
    actual_directories: set[str] = set()
    for current, directories, files in os.walk(absolute_root, followlinks=False):
        current_path = Path(current)
        current_relative = current_path.relative_to(absolute_root)
        for name in directories:
            path = current_path / name
            relative = (current_relative / name).as_posix()
            info = path.lstat()
            if (
                stat.S_ISLNK(info.st_mode)
                or not stat.S_ISDIR(info.st_mode)
                or info.st_uid != owner_uid
                or stat.S_IMODE(info.st_mode) != 0o700
            ):
                _fail(f"private Python source closure directory is unsafe: {relative}")
            actual_directories.add(relative)
        for name in files:
            path = current_path / name
            relative = (current_relative / name).as_posix()
            info = path.lstat()
            if (
                stat.S_ISLNK(info.st_mode)
                or not stat.S_ISREG(info.st_mode)
                or info.st_uid != owner_uid
                or info.st_nlink != 1
                or stat.S_IMODE(info.st_mode) != 0o600
            ):
                _fail(f"private Python source closure file is unsafe: {relative}")
            actual_files.add(relative)
    if actual_directories != expected_directories or actual_files != expected_files:
        _fail("private Python source closure inventory is not exact")
    for row in normalized_rows:
        info = stable_hash_relative(
            absolute_root,
            str(row["path"]),
            max_size=MAX_RELEASE_MANIFEST_SIZE * 32,
        )
        if info.sha256 != row["sha256"] or info.size != row["size"]:
            _fail(f"private Python source closure file changed: {row['path']}")

    if not require_isolated_runtime:
        return
    if not (sys.flags.isolated and sys.flags.dont_write_bytecode and sys.flags.no_site):
        _fail("private Python source closure requires python3 -I -B -S")
    if entrypoint is None or entrypoint not in expected_files:
        _fail("private Python source closure entrypoint is not bound")
    if Path(sys.argv[0]).resolve() != (absolute_root / entrypoint).resolve():
        _fail("executing entrypoint is outside the private source closure")
    script_rows = {
        PurePosixPath(relative).stem: relative
        for relative in expected_files
        if relative.startswith("scripts/") and relative.endswith(".py")
    }
    for name, loaded in tuple(sys.modules.items()):
        origin = getattr(loaded, "__file__", None)
        stem = name.rsplit(".", 1)[-1]
        expected_relative = script_rows.get(stem)
        if expected_relative is not None:
            if not isinstance(origin, str) or Path(origin).resolve() != (
                absolute_root / expected_relative
            ).resolve():
                _fail(f"loaded closure module has an unbound origin: {name}")
        if not isinstance(origin, str):
            continue
        resolved = Path(origin).resolve()
        try:
            relative = resolved.relative_to(absolute_root).as_posix()
        except ValueError:
            continue
        if name != "__main__" and relative not in expected_files:
            _fail(f"loaded module is absent from the exact source closure: {name}")


def load_canonical_release_manifest(payload: bytes) -> dict[str, object]:
    """Load the manifest only when its bytes are the canonical JSON rendering."""

    if len(payload) > MAX_RELEASE_MANIFEST_SIZE:
        _fail(
            f"release manifest exceeds the {MAX_RELEASE_MANIFEST_SIZE}-byte limit"
        )
    manifest = validate_release_manifest(
        load_json_object(payload, "release manifest")
    )
    if canonical_json_bytes(manifest) != payload:
        _fail("release manifest JSON is not in canonical deterministic form")
    return manifest


def exclusive_write_bytes(path: Path, payload: bytes, *, mode: int = 0o644) -> None:
    """Create one direct regular file without following or replacing links."""

    with exclusive_output_fd(path, mode=mode) as fd:
        view = memoryview(payload)
        while view:
            written = os.write(fd, view)
            if written <= 0:
                raise ReleaseArtifactError("short release output write")
            view = view[written:]


@contextmanager
def exclusive_output_fd(path: Path, *, mode: int = 0o644):
    """Yield a descriptor for one exclusive descriptor-anchored output."""

    if mode not in {0o600, 0o644, 0o755}:
        _fail("release output mode must be exactly 0600, 0644, or 0755")
    absolute = Path(os.path.abspath(path))
    parent_fd = -1
    reopened_parent_fd = -1
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    fd = -1
    created = False
    committed = False
    output_identity: tuple[int, int] | None = None
    try:
        parent_fd, _, parent_identity = _open_absolute_directory(
            absolute.parent, "output directory"
        )
        fd = os.open(absolute.name, flags, mode, dir_fd=parent_fd)
        created = True
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            _fail(f"release output {absolute} must be a regular file")
        if opened.st_nlink != 1:
            _fail(f"release output {absolute} must have exactly one hard link")
        output_identity = (opened.st_dev, opened.st_ino)
        os.fchmod(fd, mode)
        yield fd
        os.fsync(fd)
        after = os.fstat(fd)
        named = os.stat(
            absolute.name,
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
        if (
            not stat.S_ISREG(after.st_mode)
            or after.st_nlink != 1
            or output_identity != (after.st_dev, after.st_ino)
            or output_identity != (named.st_dev, named.st_ino)
            or stat.S_IMODE(after.st_mode) != mode
            or stat.S_IMODE(named.st_mode) != mode
        ):
            _fail(f"release output {absolute} changed while it was written")
        reopened_parent_fd, _, reopened_parent_identity = _open_absolute_directory(
            absolute.parent, "output directory"
        )
        if (parent_identity.st_dev, parent_identity.st_ino) != (
            reopened_parent_identity.st_dev,
            reopened_parent_identity.st_ino,
        ):
            _fail(f"release output directory {absolute.parent} was replaced")
        os.fsync(parent_fd)
        final_after = os.fstat(fd)
        final_named = os.stat(
            absolute.name,
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
        if (
            not stat.S_ISREG(final_after.st_mode)
            or final_after.st_nlink != 1
            or output_identity != (final_after.st_dev, final_after.st_ino)
            or output_identity != (final_named.st_dev, final_named.st_ino)
            or stat.S_IMODE(final_after.st_mode) != mode
            or stat.S_IMODE(final_named.st_mode) != mode
        ):
            _fail(f"release output {absolute} changed before it was committed")
        committed = True
    except ReleaseArtifactError:
        raise
    except OSError as exc:
        raise ReleaseArtifactError(
            f"failed to create release output {absolute}: {exc}"
        ) from exc
    finally:
        if created and not committed and fd >= 0:
            # Scrub the pinned inode before unlinking our name. If an attacker
            # raced in an additional hard link or renamed the file, the bytes
            # must not survive through that alternate name.
            try:
                os.ftruncate(fd, 0)
                os.fsync(fd)
            except OSError:
                pass
            if parent_fd >= 0 and output_identity is not None:
                try:
                    named = os.stat(
                        absolute.name,
                        dir_fd=parent_fd,
                        follow_symlinks=False,
                    )
                    if output_identity == (named.st_dev, named.st_ino):
                        os.unlink(absolute.name, dir_fd=parent_fd)
                        os.fsync(parent_fd)
                except OSError:
                    pass
        if fd >= 0:
            os.close(fd)
        if reopened_parent_fd >= 0:
            os.close(reopened_parent_fd)
        if parent_fd >= 0:
            os.close(parent_fd)


def format_artifact_spec(descriptor: Mapping[str, object]) -> str:
    row = validate_artifact_descriptor(descriptor, require_digest=False)
    return ":".join(
        str(row[field])
        for field in ("profile", "target", "kind", "format", "path")
    )
