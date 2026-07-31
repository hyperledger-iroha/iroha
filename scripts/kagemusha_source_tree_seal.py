#!/usr/bin/env python3
"""Capture and verify the exact reviewed Kagemusha source closure.

Candidate generation is allowed only when the complete tracked diff, untracked
regular files, separately bound root ``Cargo.lock``, and full source-tree
identity match one canonical descriptor whose raw SHA-256 is pinned
independently.  The lockfile may be a tracked ``100644`` entry or the sole
ignored path; either representation is hashed exactly once.  ``descriptor``
emits the observation that must be reviewed and pinned.  ``identity`` and
``fingerprint`` never accept an unpinned observation.

For a legacy commit whose root lockfile is ignored, start from a fresh
worktree, place the independently reviewed exact ``Cargo.lock`` at its root,
and keep ``CARGO_TARGET_DIR`` outside the worktree.  Every other ignored path
and every tracked assume-unchanged or skip-worktree index entry is rejected.

The macOS UDRO context provided here is a developer verification aid, not the
production boundary against a concurrently hostile process with the same UID.
Production candidate generation consumes a separately provisioned root-owned,
non-writable, content-addressed source-and-tool closure.
"""

from __future__ import annotations

import argparse
import base64
from contextlib import contextmanager
import hashlib
import json
import os
import pathlib
import plistlib
import re
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from typing import Any, Iterator, Sequence


SOURCE_TREE_DOMAIN = b"iroha.kagemusha.full-source-tree-sha256.v3\0"
SOURCE_DIFF_DOMAIN = b"iroha-source-diff-v1\0"
TRACKED_DIFF_DOMAIN = b"tracked-binary-diff-sha256\0"
UNTRACKED_MANIFEST_DOMAIN = b"untracked-path-blob-manifest-sha256\0"
REVIEWED_SOURCE_CLOSURE_SCHEMA = "iroha.reviewed-source-closure.v1"
SOURCE_IDENTITY_SCHEMA = "iroha.kagemusha.reviewed_source_tree_identity.v1"
ALLOWED_INDEX_MODES = {b"100644", b"100755", b"120000"}
ALLOWED_UNTRACKED_MODES = {"100644", "100755"}
EMPTY_SHA256 = hashlib.sha256(b"").hexdigest()
MAX_DESCRIPTOR_BYTES = 16 * 1024 * 1024
MAX_CARGO_LOCK_BYTES = 16 * 1024 * 1024
MAX_UNTRACKED_FILE_BYTES = 16 * 1024 * 1024
MAX_UNTRACKED_FILES = 100_000
REQUIRED_CARGO_LOCK_PATH = b"Cargo.lock"
GIT = pathlib.Path(
    os.environ.get("KAGEMUSHA_SOURCE_SEAL_GIT_EXECUTABLE", "/usr/bin/git")
)
GIT_EXEC_PATH: pathlib.Path | None = (
    pathlib.Path(value)
    if (value := os.environ.get("KAGEMUSHA_SOURCE_SEAL_GIT_EXEC_PATH"))
    else None
)
MACOS_SANDBOX_EXEC = pathlib.Path("/usr/bin/sandbox-exec")
MACOS_HDIUTIL = pathlib.Path("/usr/bin/hdiutil")
LINUX_BWRAP = pathlib.Path("/usr/bin/bwrap")
MACOS_SOURCE_WRITE_DENIAL_PROFILE = """\
(version 1)
(allow default)
(deny file-write* (literal (param "SOURCE_ROOT")))
(deny file-write* (subpath (param "SOURCE_ROOT")))
(deny file-write* (literal (param "SOURCE_DESCRIPTOR")))
"""
GIT_ARGUMENT_PREFIX = (
    "-c",
    "core.attributesFile=/dev/null",
    "-c",
    "core.autocrlf=false",
    "-c",
    "core.excludesFile=/dev/null",
    "-c",
    "core.fileMode=true",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.safecrlf=false",
    "-c",
    "core.untrackedCache=false",
)
TRACKED_DIFF_ARGUMENTS = (
    "--no-pager",
    "diff",
    "--binary",
    "--full-index",
    "--no-renames",
    "--diff-algorithm=myers",
    "--no-ext-diff",
    "--no-textconv",
    "--ignore-submodules=none",
    "HEAD",
    "--",
    ".",
)
STAGED_DIFF_ARGUMENTS = (
    "--no-pager",
    "diff",
    "--binary",
    "--full-index",
    "--no-renames",
    "--diff-algorithm=myers",
    "--no-ext-diff",
    "--no-textconv",
    "--ignore-submodules=none",
    "--cached",
    "HEAD",
    "--",
    ".",
)
UNSTAGED_DIFF_ARGUMENTS = (
    "--no-pager",
    "diff",
    "--binary",
    "--full-index",
    "--no-renames",
    "--diff-algorithm=myers",
    "--no-ext-diff",
    "--no-textconv",
    "--ignore-submodules=none",
    "--",
    ".",
)
TRACKED_DIFF_PATH_ARGUMENTS = (
    "--no-pager",
    "diff",
    "--name-only",
    "-z",
    "--no-renames",
    "--diff-algorithm=myers",
    "--no-ext-diff",
    "--no-textconv",
    "--ignore-submodules=none",
    "HEAD",
    "--",
    ".",
)
CONTENT_CONVERSION_ATTRIBUTES = (
    b"filter",
    b"ident",
    b"text",
    b"eol",
    b"crlf",
    b"working-tree-encoding",
)
DESCRIPTOR_KEYS = {
    "schema",
    "base_commit",
    "source_commit",
    "source_repo_dirty",
    "source_tree_sha256",
    "tracked_binary_diff_sha256",
    "untracked_file_count",
    "untracked_path_mode_blob_oid_manifest",
    "untracked_path_mode_blob_oid_manifest_sha256",
    "ignored_cargo_lock_size_bytes",
    "ignored_cargo_lock_sha256",
    "combined_source_fingerprint_sha256",
}
MANIFEST_ENTRY_KEYS = {
    "blob_sha256",
    "git_blob_oid",
    "git_mode",
    "path",
    "path_bytes_base64",
}


class SourceSealError(RuntimeError):
    """The checkout cannot produce the exact independently pinned source seal."""


@dataclass(frozen=True)
class IndexEntry:
    mode: bytes
    object_id: bytes
    path: bytes


@dataclass(frozen=True)
class SourceIdentity:
    source_commit: str
    source_tree_sha256: str
    source_repo_dirty: bool
    reviewed_source_closure: dict[str, Any]
    reviewed_source_closure_descriptor_sha256: str


@dataclass(frozen=True)
class SourceMaterialization:
    root: pathlib.Path
    reviewed_source_closure: pathlib.Path
    identity: SourceIdentity


def _git_environment() -> dict[str, str]:
    environment = {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_LITERAL_PATHSPECS": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_PAGER": "cat",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PAGER": "cat",
        "PATH": "/usr/bin:/bin",
        "TZ": "UTC",
    }
    if GIT_EXEC_PATH is not None:
        environment["GIT_EXEC_PATH"] = os.fspath(GIT_EXEC_PATH)
    return environment


def configure_production_git(
    executable: pathlib.Path,
    exec_path: pathlib.Path,
) -> None:
    """Select the root-published Git and exec-helper directory."""

    global GIT, GIT_EXEC_PATH
    try:
        executable_metadata = executable.lstat()
        exec_path_metadata = exec_path.lstat()
    except OSError as exc:
        raise SourceSealError("production Git closure is unavailable") from exc
    if (
        executable.resolve(strict=True) != executable
        or exec_path.resolve(strict=True) != exec_path
        or not stat.S_ISREG(executable_metadata.st_mode)
        or executable_metadata.st_uid != 0
        or executable_metadata.st_nlink != 1
        or executable_metadata.st_mode & 0o222 != 0
        or executable_metadata.st_mode & 0o111 == 0
        or not stat.S_ISDIR(exec_path_metadata.st_mode)
        or exec_path_metadata.st_uid != 0
        or exec_path_metadata.st_mode & 0o222 != 0
    ):
        raise SourceSealError("production Git closure has unsafe metadata")
    GIT = executable
    GIT_EXEC_PATH = exec_path


def _git(root: pathlib.Path, *arguments: str) -> bytes:
    if not GIT.is_file() or GIT.is_symlink():
        raise SourceSealError("pinned Git is unavailable")
    try:
        return subprocess.run(
            [
                os.fspath(GIT),
                *GIT_ARGUMENT_PREFIX,
                "-C",
                os.fspath(root),
                *arguments,
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=_git_environment(),
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        raise SourceSealError(f"pinned Git failed: {' '.join(arguments)}") from exc


def _git_with_input(root: pathlib.Path, payload: bytes, *arguments: str) -> bytes:
    if not GIT.is_file() or GIT.is_symlink():
        raise SourceSealError("pinned Git is unavailable")
    try:
        return subprocess.run(
            [
                os.fspath(GIT),
                *GIT_ARGUMENT_PREFIX,
                "-C",
                os.fspath(root),
                *arguments,
            ],
            input=payload,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=_git_environment(),
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        raise SourceSealError(f"pinned Git failed: {' '.join(arguments)}") from exc


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
    if (
        len(value) != 40
        or value == b"0" * 40
        or any(byte not in b"0123456789abcdef" for byte in value)
    ):
        raise SourceSealError("Git HEAD is not one nonzero canonical SHA-1 commit id")
    return value


def status(root: pathlib.Path) -> bytes:
    return _git(
        root,
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    )


def _safe_relative_path(path: bytes, *, allow_cargo_lock: bool = False) -> None:
    if (
        not path
        or path.startswith(b"/")
        or path.endswith(b"/")
        or b"\0" in path
        or any(component in (b"", b".", b"..") for component in path.split(b"/"))
        or path.split(b"/", 1)[0] == b".git"
        or (not allow_cargo_lock and path == REQUIRED_CARGO_LOCK_PATH)
    ):
        raise SourceSealError("Git returned an unsafe source path")


def _index_entries(root: pathlib.Path) -> list[IndexEntry]:
    # `--stage -v` returns each stage entry and its index-status tag in one
    # pinned Git snapshot. Lowercase tags expose assume-unchanged entries;
    # `S`/`s` exposes skip-worktree entries even when diff/status hides their
    # working-tree bytes.
    records = _git(root, "ls-files", "--stage", "-v", "-z", "--").split(b"\0")
    entries: list[IndexEntry] = []
    seen: set[bytes] = set()
    for record in records:
        if not record:
            continue
        try:
            tagged_metadata, path = record.split(b"\t", 1)
            tag, metadata = tagged_metadata.split(b" ", 1)
            mode, object_id, stage = metadata.split(b" ", 2)
        except ValueError as exc:
            raise SourceSealError("Git returned a malformed index record") from exc
        if len(tag) != 1:
            raise SourceSealError("Git returned a malformed source index tag")
        forbidden_flags: list[str] = []
        if b"a" <= tag <= b"z":
            forbidden_flags.append("assume-unchanged")
        if tag.upper() == b"S":
            forbidden_flags.append("skip-worktree")
        if forbidden_flags:
            raise SourceSealError(
                "tracked source index flags are forbidden "
                f"({', '.join(forbidden_flags)}): {os.fsdecode(path)}"
            )
        if tag != b"H":
            raise SourceSealError(
                f"unsupported Git source index tag {os.fsdecode(tag)!r}"
            )
        if mode not in ALLOWED_INDEX_MODES:
            raise SourceSealError(
                f"unsupported Git index mode {os.fsdecode(mode)!r} for {os.fsdecode(path)!r}"
            )
        if len(object_id) != 40 or any(
            byte not in b"0123456789abcdef" for byte in object_id
        ):
            raise SourceSealError("Git returned a non-canonical index object id")
        if stage != b"0":
            raise SourceSealError("the source index contains an unresolved merge stage")
        _safe_relative_path(path, allow_cargo_lock=True)
        if path in seen:
            raise SourceSealError("Git returned a duplicate source path")
        seen.add(path)
        entries.append(IndexEntry(mode=mode, object_id=object_id, path=path))
    if not entries:
        raise SourceSealError("the source index is empty")
    entries.sort(key=lambda entry: entry.path)
    return entries


def _head_entries(root: pathlib.Path) -> list[IndexEntry]:
    records = _git(
        root,
        "ls-tree",
        "-r",
        "-z",
        "--full-tree",
        "HEAD",
        "--",
    ).split(b"\0")
    entries: list[IndexEntry] = []
    seen: set[bytes] = set()
    for record in records:
        if not record:
            continue
        try:
            metadata, path = record.split(b"\t", 1)
            mode, object_type, object_id = metadata.split(b" ", 2)
        except ValueError as exc:
            raise SourceSealError("Git returned a malformed HEAD tree record") from exc
        if mode not in ALLOWED_INDEX_MODES or object_type != b"blob":
            raise SourceSealError(
                f"unsupported Git HEAD mode/type for {os.fsdecode(path)!r}"
            )
        if len(object_id) != 40 or any(
            byte not in b"0123456789abcdef" for byte in object_id
        ):
            raise SourceSealError("Git returned a non-canonical HEAD object id")
        _safe_relative_path(path, allow_cargo_lock=True)
        if path in seen:
            raise SourceSealError("Git returned a duplicate HEAD source path")
        seen.add(path)
        entries.append(IndexEntry(mode=mode, object_id=object_id, path=path))
    if not entries:
        raise SourceSealError("the source HEAD tree is empty")
    entries.sort(key=lambda entry: entry.path)
    return entries


def _tracked_diff_paths(root: pathlib.Path) -> list[bytes]:
    records = _git(root, *TRACKED_DIFF_PATH_ARGUMENTS).split(b"\0")
    paths = [path for path in records if path]
    for path in paths:
        _safe_relative_path(path, allow_cargo_lock=True)
    if paths != sorted(set(paths)):
        raise SourceSealError(
            "Git tracked diff paths are not unique and raw-byte sorted"
        )
    return paths


def _reject_content_conversion_attributes(
    root: pathlib.Path,
    paths: list[bytes],
) -> None:
    payload = b"\0".join(paths) + b"\0"
    output = _git_with_input(
        root,
        payload,
        "check-attr",
        "-z",
        "--stdin",
        *(attribute.decode("ascii") for attribute in CONTENT_CONVERSION_ATTRIBUTES),
    )
    records = output.split(b"\0")
    if records and records[-1] == b"":
        records.pop()
    expected_records = len(paths) * len(CONTENT_CONVERSION_ATTRIBUTES) * 3
    if len(records) != expected_records:
        raise SourceSealError("Git returned a malformed content-attribute inventory")
    offset = 0
    for expected_path in paths:
        for expected_attribute in CONTENT_CONVERSION_ATTRIBUTES:
            path, attribute, value = records[offset : offset + 3]
            offset += 3
            if path != expected_path or attribute != expected_attribute:
                raise SourceSealError(
                    "Git content-attribute inventory is not exact and ordered"
                )
            if value not in (b"unspecified", b"unset"):
                raise SourceSealError(
                    f"tracked source has a forbidden content-conversion attribute "
                    f"{os.fsdecode(attribute)}={os.fsdecode(value)}: "
                    f"{os.fsdecode(path)}"
                )


def _field(hasher: "hashlib._Hash", value: bytes) -> None:
    hasher.update(len(value).to_bytes(8, "big"))
    hasher.update(value)


def _stable_identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _hash_regular_file(
    path: bytes,
    source_hasher: "hashlib._Hash",
    *,
    maximum_bytes: int | None = None,
    require_nonempty: bool = False,
) -> tuple[int, str, str]:
    before = os.lstat(path)
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise SourceSealError(
            f"source must be a singly linked regular file: {os.fsdecode(path)}"
        )
    if (
        (require_nonempty and before.st_size <= 0)
        or (maximum_bytes is not None and before.st_size > maximum_bytes)
    ):
        raise SourceSealError(f"source file has an invalid size: {os.fsdecode(path)}")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened_before = os.fstat(descriptor)
        source_hasher.update(opened_before.st_size.to_bytes(8, "big"))
        sha256 = hashlib.sha256()
        blob_oid = hashlib.sha1(
            b"blob " + str(opened_before.st_size).encode("ascii") + b"\0",
            usedforsecurity=False,
        )
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            source_hasher.update(chunk)
            sha256.update(chunk)
            blob_oid.update(chunk)
        opened_after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = os.lstat(path)
    if not (
        _stable_identity(before)
        == _stable_identity(opened_before)
        == _stable_identity(opened_after)
        == _stable_identity(after)
    ):
        raise SourceSealError(f"source changed while read: {os.fsdecode(path)}")
    if total != opened_after.st_size:
        raise SourceSealError(f"source was truncated while read: {os.fsdecode(path)}")
    return total, sha256.hexdigest(), blob_oid.hexdigest()


def _stable_symlink_bytes(path: bytes) -> bytes:
    before = os.lstat(path)
    if not stat.S_ISLNK(before.st_mode) or before.st_nlink != 1:
        raise SourceSealError(f"tracked symlink must be singly linked: {os.fsdecode(path)}")
    payload = os.readlink(path)
    after = os.lstat(path)
    if not isinstance(payload, bytes):
        payload = os.fsencode(payload)
    if _stable_identity(before) != _stable_identity(after):
        raise SourceSealError(f"tracked symlink changed while read: {os.fsdecode(path)}")
    return payload


def _git_blob_oid(payload: bytes) -> bytes:
    digest = hashlib.sha1(
        b"blob " + str(len(payload)).encode("ascii") + b"\0",
        usedforsecurity=False,
    )
    digest.update(payload)
    return digest.hexdigest().encode("ascii")


def _normalize_symlink_target(link_path: bytes, payload: bytes) -> list[bytes]:
    if not payload or payload.startswith(b"/") or b"\0" in payload:
        raise SourceSealError(
            f"tracked symlink target is absolute or empty: {os.fsdecode(link_path)}"
        )
    components = link_path.split(b"/")[:-1]
    for component in payload.split(b"/"):
        if component in (b"", b"."):
            continue
        if component == b"..":
            if not components:
                raise SourceSealError(
                    f"tracked symlink target escapes the repository: "
                    f"{os.fsdecode(link_path)}"
                )
            components.pop()
            continue
        components.append(component)
    if not components:
        raise SourceSealError(
            f"tracked symlink target resolves to the repository root: "
            f"{os.fsdecode(link_path)}"
        )
    return components


def _validate_tracked_symlink_closure(
    root: pathlib.Path,
    entries: list[IndexEntry],
    symlink_payloads: dict[bytes, bytes],
    untracked_paths: list[bytes],
) -> None:
    if not symlink_payloads:
        return
    closure_members = {entry.path for entry in entries}
    closure_members.update(untracked_paths)
    closure_members.add(REQUIRED_CARGO_LOCK_PATH)
    closure_directories: set[bytes] = set()
    for member in closure_members:
        components = member.split(b"/")
        for length in range(1, len(components)):
            closure_directories.add(b"/".join(components[:length]))

    root_bytes = os.fsencode(root)
    for original_path, original_payload in symlink_payloads.items():
        components = _normalize_symlink_target(original_path, original_payload)
        visited = {original_path}
        while True:
            replacement_found = False
            for length in range(1, len(components) + 1):
                prefix = b"/".join(components[:length])
                payload = symlink_payloads.get(prefix)
                if payload is None:
                    continue
                if prefix in visited:
                    raise SourceSealError(
                        f"tracked symlink chain contains a cycle: "
                        f"{os.fsdecode(original_path)}"
                    )
                visited.add(prefix)
                replacement = _normalize_symlink_target(prefix, payload)
                components = replacement + components[length:]
                replacement_found = True
                break
            if not replacement_found:
                break
        final_path = b"/".join(components)
        _safe_relative_path(final_path, allow_cargo_lock=True)
        final_absolute = os.path.join(root_bytes, final_path)
        if (
            os.path.lexists(final_absolute)
            and final_path not in closure_members
            and final_path not in closure_directories
        ):
            raise SourceSealError(
                f"tracked symlink resolves to an existing path outside the "
                f"reviewed closure: {os.fsdecode(original_path)}"
            )


def _untracked_paths(root: pathlib.Path) -> list[bytes]:
    records = _git(
        root,
        "ls-files",
        "--others",
        "--exclude-standard",
        "-z",
        "--",
    ).split(b"\0")
    paths = [path for path in records if path]
    if len(paths) > MAX_UNTRACKED_FILES:
        raise SourceSealError("untracked source inventory exceeds its file-count bound")
    for path in paths:
        _safe_relative_path(path)
    if paths != sorted(set(paths)):
        raise SourceSealError("untracked source paths are not unique and raw-byte sorted")
    return paths


def _ignored_paths(root: pathlib.Path) -> list[bytes]:
    records = _git(
        root,
        "ls-files",
        "--others",
        "--ignored",
        "--exclude-standard",
        "-z",
        "--",
    ).split(b"\0")
    paths = sorted({path for path in records if path})
    for path in paths:
        _safe_relative_path(path, allow_cargo_lock=True)
    return paths


def _canonical_json_bytes(value: Any) -> bytes:
    try:
        return (
            json.dumps(
                value,
                allow_nan=False,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeError) as exc:
        raise SourceSealError("reviewed source closure is not canonical JSON") from exc


def _untracked_manifest_bytes(entries: list[dict[str, Any]]) -> bytes:
    return b"".join(_canonical_json_bytes(entry) for entry in entries)


def _capture_observed_descriptor(root: pathlib.Path) -> dict[str, Any]:
    root = _repository_root(root)
    head_before = _head(root)
    diff_before = _git(root, *TRACKED_DIFF_ARGUMENTS)
    diff_paths_before = _tracked_diff_paths(root)
    untracked_before = _untracked_paths(root)
    ignored_before = _ignored_paths(root)
    entries = _index_entries(root)
    head_entries = _head_entries(root)
    _reject_content_conversion_attributes(
        root,
        [entry.path for entry in entries],
    )
    cargo_lock_entries = [
        entry for entry in entries if entry.path == REQUIRED_CARGO_LOCK_PATH
    ]
    if len(cargo_lock_entries) > 1 or (
        cargo_lock_entries and cargo_lock_entries[0].mode != b"100644"
    ):
        raise SourceSealError(
            "tracked root Cargo.lock must be exactly one 100644 index entry"
        )
    expected_ignored = [] if cargo_lock_entries else [REQUIRED_CARGO_LOCK_PATH]
    if ignored_before != expected_ignored:
        raise SourceSealError(
            "root Cargo.lock must be either one tracked 100644 entry or the "
            "sole ignored path"
        )
    source_hasher = hashlib.sha256(SOURCE_TREE_DOMAIN)
    root_bytes = os.fsencode(root)
    actual_tracked_state: dict[bytes, tuple[bytes, bytes] | None] = {}
    symlink_payloads: dict[bytes, bytes] = {}

    for entry in entries:
        if entry.path == REQUIRED_CARGO_LOCK_PATH:
            continue
        absolute = os.path.join(root_bytes, entry.path)
        _field(source_hasher, b"tracked-source-v1")
        _field(source_hasher, entry.path)
        try:
            metadata = os.lstat(absolute)
        except FileNotFoundError:
            _field(source_hasher, b"absent")
            actual_tracked_state[entry.path] = None
            continue
        if stat.S_ISREG(metadata.st_mode):
            current_mode = b"100755" if metadata.st_mode & 0o111 else b"100644"
            _field(source_hasher, current_mode)
            _, _, git_blob_oid = _hash_regular_file(absolute, source_hasher)
            actual_tracked_state[entry.path] = (
                current_mode,
                git_blob_oid.encode("ascii"),
            )
        elif stat.S_ISLNK(metadata.st_mode):
            payload = _stable_symlink_bytes(absolute)
            _field(source_hasher, b"120000")
            _field(source_hasher, payload)
            actual_tracked_state[entry.path] = (b"120000", _git_blob_oid(payload))
            symlink_payloads[entry.path] = payload
        else:
            raise SourceSealError(
                f"tracked source has an unsafe file type: {os.fsdecode(entry.path)}"
            )

    untracked_manifest: list[dict[str, Any]] = []
    for path in untracked_before:
        absolute = os.path.join(root_bytes, path)
        metadata = os.lstat(absolute)
        if not stat.S_ISREG(metadata.st_mode):
            raise SourceSealError(
                f"untracked source must be a regular file: {os.fsdecode(path)}"
            )
        git_mode = "100755" if metadata.st_mode & 0o111 else "100644"
        _field(source_hasher, b"untracked-source-v1")
        _field(source_hasher, path)
        _field(source_hasher, git_mode.encode("ascii"))
        _, blob_sha256, git_blob_oid = _hash_regular_file(
            absolute,
            source_hasher,
            maximum_bytes=MAX_UNTRACKED_FILE_BYTES,
            require_nonempty=True,
        )
        untracked_manifest.append(
            {
                "blob_sha256": blob_sha256,
                "git_blob_oid": git_blob_oid,
                "git_mode": git_mode,
                "path": os.fsdecode(path),
                "path_bytes_base64": base64.b64encode(path).decode("ascii"),
            }
        )

    _validate_tracked_symlink_closure(
        root,
        entries,
        symlink_payloads,
        untracked_before,
    )

    cargo_lock_path = os.path.join(root_bytes, REQUIRED_CARGO_LOCK_PATH)
    try:
        cargo_metadata = os.lstat(cargo_lock_path)
    except FileNotFoundError as exc:
        raise SourceSealError("root Cargo.lock is missing") from exc
    if not stat.S_ISREG(cargo_metadata.st_mode) or cargo_metadata.st_nlink != 1:
        raise SourceSealError(
            "root Cargo.lock must be a singly linked regular file"
        )
    if cargo_metadata.st_mode & 0o111:
        raise SourceSealError("root Cargo.lock must not be executable")
    # V1's serialized field names and digest preimage label say "ignored".
    # Preserve those bytes for descriptor compatibility while admitting the
    # tracked representation and hashing either representation only here.
    _field(source_hasher, b"required-ignored-build-input-v1")
    _field(source_hasher, REQUIRED_CARGO_LOCK_PATH)
    _field(source_hasher, b"100644")
    cargo_lock_size, cargo_lock_sha256, cargo_lock_blob_oid = _hash_regular_file(
        cargo_lock_path,
        source_hasher,
        maximum_bytes=MAX_CARGO_LOCK_BYTES,
        require_nonempty=True,
    )
    if cargo_lock_entries:
        actual_tracked_state[REQUIRED_CARGO_LOCK_PATH] = (
            b"100644",
            cargo_lock_blob_oid.encode("ascii"),
        )

    head_after = _head(root)
    diff_after = _git(root, *TRACKED_DIFF_ARGUMENTS)
    diff_paths_after = _tracked_diff_paths(root)
    untracked_after = _untracked_paths(root)
    ignored_after = _ignored_paths(root)
    entries_after = _index_entries(root)
    cargo_recheck_size, cargo_recheck_sha256, _ = _hash_regular_file(
        cargo_lock_path,
        hashlib.sha256(),
        maximum_bytes=MAX_CARGO_LOCK_BYTES,
        require_nonempty=True,
    )
    if (
        head_after != head_before
        or diff_after != diff_before
        or diff_paths_after != diff_paths_before
        or untracked_after != untracked_before
        or ignored_after != ignored_before
        or entries_after != entries
        or cargo_recheck_size != cargo_lock_size
        or cargo_recheck_sha256 != cargo_lock_sha256
    ):
        raise SourceSealError("Kagemusha source HEAD or closure changed while sealing")

    tracked_binary_diff_sha256 = hashlib.sha256(diff_before).hexdigest()
    head_state = {
        entry.path: (entry.mode, entry.object_id) for entry in head_entries
    }
    index_state = {
        entry.path: (entry.mode, entry.object_id) for entry in entries
    }
    for path, indexed in index_state.items():
        if indexed not in (head_state.get(path), actual_tracked_state.get(path)):
            raise SourceSealError(
                "Git index contains an unbound intermediary blob or mode: "
                f"{os.fsdecode(path)}"
            )
    for path in head_state:
        if actual_tracked_state.get(path) is None and path in index_state:
            raise SourceSealError(
                "tracked worktree deletions must be staged for deterministic "
                f"materialization: {os.fsdecode(path)}"
            )
    raw_changed_paths = {
        path
        for path in set(actual_tracked_state) | set(head_state)
        if actual_tracked_state.get(path) != head_state.get(path)
    }
    if raw_changed_paths != set(diff_paths_before):
        raise SourceSealError(
            "Git tracked diff paths disagree with raw HEAD bytes or modes"
        )
    raw_tracked_dirty = bool(raw_changed_paths)
    git_tracked_dirty = tracked_binary_diff_sha256 != EMPTY_SHA256
    if raw_tracked_dirty != git_tracked_dirty:
        raise SourceSealError(
            "Git tracked diff state disagrees with raw HEAD bytes or modes"
        )
    untracked_manifest_sha256 = hashlib.sha256(
        _untracked_manifest_bytes(untracked_manifest)
    ).hexdigest()
    combined = hashlib.sha256()
    combined.update(SOURCE_DIFF_DOMAIN)
    combined.update(TRACKED_DIFF_DOMAIN)
    combined.update(bytes.fromhex(tracked_binary_diff_sha256))
    combined.update(UNTRACKED_MANIFEST_DOMAIN)
    combined.update(bytes.fromhex(untracked_manifest_sha256))
    # A legacy ignored Cargo.lock is separately bound, but it is still a build
    # input absent from HEAD.  Report that checkout as dirty even when ordinary
    # Git status is empty so no consumer can mistake different build bytes for
    # an exact clean commit.
    source_repo_dirty = (
        raw_tracked_dirty
        or bool(untracked_manifest)
        or not cargo_lock_entries
    )
    descriptor = {
        "base_commit": head_before.decode("ascii"),
        "combined_source_fingerprint_sha256": combined.hexdigest(),
        "ignored_cargo_lock_sha256": cargo_lock_sha256,
        "ignored_cargo_lock_size_bytes": cargo_lock_size,
        "schema": REVIEWED_SOURCE_CLOSURE_SCHEMA,
        "source_commit": head_before.decode("ascii"),
        "source_repo_dirty": source_repo_dirty,
        "source_tree_sha256": source_hasher.hexdigest(),
        "tracked_binary_diff_sha256": tracked_binary_diff_sha256,
        "untracked_file_count": len(untracked_manifest),
        "untracked_path_mode_blob_oid_manifest": untracked_manifest,
        "untracked_path_mode_blob_oid_manifest_sha256": untracked_manifest_sha256,
    }
    if len(_canonical_json_bytes(descriptor)) > MAX_DESCRIPTOR_BYTES:
        raise SourceSealError("reviewed source closure descriptor exceeds its size bound")
    return descriptor


def _reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise SourceSealError(f"duplicate JSON member: {key}")
        value[key] = item
    return value


def _reject_constant(value: str) -> None:
    raise SourceSealError(f"non-finite JSON number is forbidden: {value}")


def _require_digest(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or re.fullmatch(r"[0-9a-f]{64}", value) is None
        or value == "0" * 64
    ):
        raise SourceSealError(f"{label} must be one nonzero lowercase SHA-256")
    return value


def _require_commit(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or re.fullmatch(r"[0-9a-f]{40}", value) is None
        or value == "0" * 40
    ):
        raise SourceSealError(f"{label} must be one nonzero lowercase SHA-1 commit")
    return value


def _decode_manifest_path(entry: dict[str, Any], label: str) -> bytes:
    display_path = entry["path"]
    encoded_path = entry["path_bytes_base64"]
    if (
        not isinstance(display_path, str)
        or not display_path
        or not isinstance(encoded_path, str)
        or not encoded_path
    ):
        raise SourceSealError(f"{label} path fields must be nonempty strings")
    try:
        path_bytes = base64.b64decode(encoded_path, validate=True)
    except (ValueError, base64.binascii.Error) as exc:
        raise SourceSealError(f"{label} path bytes are not canonical Base64") from exc
    _safe_relative_path(path_bytes)
    if (
        base64.b64encode(path_bytes).decode("ascii") != encoded_path
        or os.fsdecode(path_bytes) != display_path
    ):
        raise SourceSealError(f"{label} path display/base64 binding is not exact")
    return path_bytes


def _validate_descriptor(value: Any, required_commit: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != DESCRIPTOR_KEYS:
        raise SourceSealError("reviewed source closure keys are not exact")
    if value["schema"] != REVIEWED_SOURCE_CLOSURE_SCHEMA:
        raise SourceSealError("reviewed source closure schema is not exact")
    required_commit = _require_commit(required_commit, "required source commit")
    base_commit = _require_commit(value["base_commit"], "base_commit")
    source_commit = _require_commit(value["source_commit"], "source_commit")
    if base_commit != required_commit or source_commit != required_commit:
        raise SourceSealError(
            "reviewed source closure does not use the exact pinned signed base commit"
        )
    for field in (
        "source_tree_sha256",
        "tracked_binary_diff_sha256",
        "untracked_path_mode_blob_oid_manifest_sha256",
        "ignored_cargo_lock_sha256",
        "combined_source_fingerprint_sha256",
    ):
        _require_digest(value[field], field)
    file_count = value["untracked_file_count"]
    cargo_lock_size = value["ignored_cargo_lock_size_bytes"]
    if (
        type(file_count) is not int
        or file_count < 0
        or file_count > MAX_UNTRACKED_FILES
    ):
        raise SourceSealError("untracked_file_count is not one bounded JSON integer")
    if (
        type(cargo_lock_size) is not int
        or cargo_lock_size <= 0
        or cargo_lock_size > MAX_CARGO_LOCK_BYTES
    ):
        raise SourceSealError(
            "ignored_cargo_lock_size_bytes is not one bounded positive JSON integer"
        )
    raw_manifest = value["untracked_path_mode_blob_oid_manifest"]
    if not isinstance(raw_manifest, list) or len(raw_manifest) != file_count:
        raise SourceSealError("untracked manifest count is not exact")
    paths: list[bytes] = []
    for index, raw_entry in enumerate(raw_manifest):
        label = f"untracked_path_mode_blob_oid_manifest[{index}]"
        if not isinstance(raw_entry, dict) or set(raw_entry) != MANIFEST_ENTRY_KEYS:
            raise SourceSealError(f"{label} keys are not exact")
        _require_digest(raw_entry["blob_sha256"], f"{label}.blob_sha256")
        if (
            not isinstance(raw_entry["git_blob_oid"], str)
            or re.fullmatch(r"[0-9a-f]{40}", raw_entry["git_blob_oid"]) is None
        ):
            raise SourceSealError(f"{label}.git_blob_oid is not lowercase SHA-1")
        if raw_entry["git_mode"] not in ALLOWED_UNTRACKED_MODES:
            raise SourceSealError(f"{label}.git_mode is not canonical")
        paths.append(_decode_manifest_path(raw_entry, label))
    if paths != sorted(set(paths)):
        raise SourceSealError(
            "untracked manifest paths are not unique and raw-byte sorted"
        )
    manifest_sha256 = hashlib.sha256(
        _untracked_manifest_bytes(raw_manifest)
    ).hexdigest()
    if manifest_sha256 != value["untracked_path_mode_blob_oid_manifest_sha256"]:
        raise SourceSealError("untracked manifest SHA-256 is not self-consistent")
    combined = hashlib.sha256()
    combined.update(SOURCE_DIFF_DOMAIN)
    combined.update(TRACKED_DIFF_DOMAIN)
    combined.update(bytes.fromhex(value["tracked_binary_diff_sha256"]))
    combined.update(UNTRACKED_MANIFEST_DOMAIN)
    combined.update(bytes.fromhex(manifest_sha256))
    if combined.hexdigest() != value["combined_source_fingerprint_sha256"]:
        raise SourceSealError("combined source fingerprint is not self-consistent")
    generically_derived_dirty = (
        value["tracked_binary_diff_sha256"] != EMPTY_SHA256 or file_count != 0
    )
    if not isinstance(value["source_repo_dirty"], bool) or (
        generically_derived_dirty and value["source_repo_dirty"] is not True
    ):
        raise SourceSealError(
            "source_repo_dirty does not conservatively cover the derived closure state"
        )
    return value


def _read_descriptor_payload(path: str) -> bytes:
    selected = pathlib.Path(path)
    if not selected.is_absolute() or os.path.normpath(path) != path:
        raise SourceSealError("reviewed source closure path must be absolute and normalized")
    resolved = selected.resolve(strict=True)
    if resolved != selected:
        raise SourceSealError("reviewed source closure path must not traverse symlinks")
    before = selected.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > MAX_DESCRIPTOR_BYTES
    ):
        raise SourceSealError("reviewed source closure must be one bounded regular file")
    payload = selected.read_bytes()
    after = selected.lstat()
    if _stable_identity(before) != _stable_identity(after):
        raise SourceSealError("reviewed source closure changed while read")
    if not payload or len(payload) > MAX_DESCRIPTOR_BYTES:
        raise SourceSealError("reviewed source closure payload has an invalid size")
    return payload


def _require_private_destination(
    source_root: pathlib.Path,
    destination: pathlib.Path,
) -> pathlib.Path:
    if (
        not destination.is_absolute()
        or os.path.normpath(os.fspath(destination)) != os.fspath(destination)
    ):
        raise SourceSealError("private materialization path must be absolute and normalized")
    try:
        destination.relative_to(source_root)
    except ValueError:
        pass
    else:
        raise SourceSealError("private materialization must be outside the source repository")
    parent = destination.parent
    try:
        parent_metadata = parent.lstat()
        parent_resolved = parent.resolve(strict=True)
    except OSError as exc:
        raise SourceSealError("private materialization parent is unavailable") from exc
    if (
        parent_resolved != parent
        or not stat.S_ISDIR(parent_metadata.st_mode)
        or parent_metadata.st_uid != os.geteuid()
        or parent_metadata.st_mode & 0o077 != 0
    ):
        raise SourceSealError(
            "private materialization parent must be canonical and owner-private"
        )
    if destination.exists() or destination.is_symlink():
        raise SourceSealError("private materialization path must not already exist")
    return destination


def _require_source_parents_are_directories(root: bytes, path: bytes) -> None:
    current = root
    for component in path.split(b"/")[:-1]:
        current = os.path.join(current, component)
        metadata = os.lstat(current)
        if not stat.S_ISDIR(metadata.st_mode):
            raise SourceSealError(
                f"source parent is not a real directory: {os.fsdecode(path)}"
            )


def _ensure_private_destination_parent(root: bytes, path: bytes) -> bytes:
    current = root
    for component in path.split(b"/")[:-1]:
        current = os.path.join(current, component)
        try:
            metadata = os.lstat(current)
        except FileNotFoundError:
            os.mkdir(current, 0o700)
            metadata = os.lstat(current)
        if not stat.S_ISDIR(metadata.st_mode):
            raise SourceSealError(
                f"private materialization parent is unsafe: {os.fsdecode(path)}"
            )
    return os.path.dirname(os.path.join(root, path))


def _copy_reviewed_regular_file(
    source_root: pathlib.Path,
    destination_root: pathlib.Path,
    path: bytes,
    *,
    expected_size: int,
    expected_sha256: str,
    expected_mode: bytes,
) -> None:
    _safe_relative_path(path, allow_cargo_lock=True)
    source_root_bytes = os.fsencode(source_root)
    destination_root_bytes = os.fsencode(destination_root)
    _require_source_parents_are_directories(source_root_bytes, path)
    source = os.path.join(source_root_bytes, path)
    destination = os.path.join(destination_root_bytes, path)
    destination_parent = _ensure_private_destination_parent(
        destination_root_bytes,
        path,
    )
    before = os.lstat(source)
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size != expected_size
    ):
        raise SourceSealError(
            f"reviewed source file changed before materialization: {os.fsdecode(path)}"
        )
    observed_mode = b"100755" if before.st_mode & 0o111 else b"100644"
    if observed_mode != expected_mode:
        raise SourceSealError(
            f"reviewed source mode changed before materialization: {os.fsdecode(path)}"
        )
    source_flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    source_descriptor = os.open(source, source_flags)
    temporary_descriptor, temporary_path = tempfile.mkstemp(
        prefix=b".kagemusha-materialize-",
        dir=destination_parent,
    )
    try:
        opened_before = os.fstat(source_descriptor)
        digest = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(source_descriptor, 1024 * 1024)
            if not chunk:
                break
            chunk_written = 0
            while chunk_written < len(chunk):
                chunk_written += os.write(
                    temporary_descriptor,
                    chunk[chunk_written:],
                )
            digest.update(chunk)
            total += len(chunk)
        opened_after = os.fstat(source_descriptor)
        os.fchmod(temporary_descriptor, 0o755 if expected_mode == b"100755" else 0o644)
        os.fsync(temporary_descriptor)
    finally:
        os.close(source_descriptor)
        os.close(temporary_descriptor)
    after = os.lstat(source)
    if not (
        _stable_identity(before)
        == _stable_identity(opened_before)
        == _stable_identity(opened_after)
        == _stable_identity(after)
    ):
        os.unlink(temporary_path)
        raise SourceSealError(
            f"reviewed source changed while materialized: {os.fsdecode(path)}"
        )
    if total != expected_size or digest.hexdigest() != expected_sha256:
        os.unlink(temporary_path)
        raise SourceSealError(
            f"reviewed source differs from its closure: {os.fsdecode(path)}"
        )
    try:
        destination_metadata = os.lstat(destination)
    except FileNotFoundError:
        destination_metadata = None
    if destination_metadata is not None and stat.S_ISDIR(destination_metadata.st_mode):
        os.unlink(temporary_path)
        raise SourceSealError(
            f"private materialization destination is a directory: {os.fsdecode(path)}"
        )
    os.replace(temporary_path, destination)
    materialized = os.lstat(destination)
    if (
        not stat.S_ISREG(materialized.st_mode)
        or materialized.st_nlink != 1
        or materialized.st_size != expected_size
    ):
        raise SourceSealError(
            f"private materialization is not exact: {os.fsdecode(path)}"
        )


def _write_private_descriptor(destination: pathlib.Path, payload: bytes) -> None:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(os.fsencode(destination), flags, 0o600)
    try:
        written = 0
        while written < len(payload):
            written += os.write(descriptor, payload[written:])
        os.fsync(descriptor)
        os.fchmod(descriptor, 0o400)
    finally:
        os.close(descriptor)


def _canonical_boundary_path(
    path: pathlib.Path,
    *,
    label: str,
    require_directory: bool,
) -> pathlib.Path:
    if (
        not path.is_absolute()
        or os.path.normpath(os.fspath(path)) != os.fspath(path)
    ):
        raise SourceSealError(f"{label} must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as exc:
        raise SourceSealError(f"{label} is unavailable") from exc
    expected_type = (
        stat.S_ISDIR(metadata.st_mode)
        if require_directory
        else stat.S_ISREG(metadata.st_mode)
    )
    if (
        resolved != path
        or not expected_type
        or metadata.st_uid not in (0, os.geteuid())
        or metadata.st_mode & 0o022 != 0
    ):
        raise SourceSealError(
            f"{label} must be canonical, root/build-owner controlled, and not a symlink"
        )
    return resolved


def _admitted_root_executable(path: pathlib.Path, *, label: str) -> str:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise SourceSealError(f"required {label} is unavailable: {path}") from exc
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_mode & 0o022 != 0
        or metadata.st_mode & 0o111 == 0
    ):
        raise SourceSealError(f"{label} has unsafe metadata: {path}")
    return os.fspath(path)


def write_denied_source_command(
    root: pathlib.Path,
    reviewed_source_closure: pathlib.Path,
    command: Sequence[str],
    *,
    platform_name: str | None = None,
) -> list[str]:
    """Wrap a command in an OS boundary that denies writes to reviewed source."""

    source_root = _canonical_boundary_path(
        root,
        label="private reviewed source root",
        require_directory=True,
    )
    descriptor = _canonical_boundary_path(
        reviewed_source_closure,
        label="private reviewed source closure descriptor",
        require_directory=False,
    )
    if descriptor == source_root or source_root in descriptor.parents:
        raise SourceSealError(
            "private reviewed source descriptor must be outside the source root"
        )
    if (
        not command
        or any(not isinstance(argument, str) or "\0" in argument for argument in command)
    ):
        raise SourceSealError("source write-denial command is malformed")

    selected_platform = sys.platform if platform_name is None else platform_name
    if selected_platform == "darwin":
        boundary = _admitted_root_executable(
            MACOS_SANDBOX_EXEC,
            label="source write-denial boundary",
        )
        return [
            boundary,
            "-D",
            f"SOURCE_ROOT={source_root}",
            "-D",
            f"SOURCE_DESCRIPTOR={descriptor}",
            "-p",
            MACOS_SOURCE_WRITE_DENIAL_PROFILE,
            *command,
        ]
    if selected_platform.startswith("linux"):
        boundary = _admitted_root_executable(
            LINUX_BWRAP,
            label="source write-denial boundary",
        )
        return [
            boundary,
            "--die-with-parent",
            "--new-session",
            "--unshare-pid",
            "--bind",
            "/",
            "/",
            "--ro-bind",
            os.fspath(source_root),
            os.fspath(source_root),
            "--ro-bind",
            os.fspath(descriptor),
            os.fspath(descriptor),
            "--proc",
            "/proc",
            "--chdir",
            os.fspath(source_root),
            "--",
            *command,
        ]
    raise SourceSealError(
        f"no source write-denial boundary is supported on {selected_platform}"
    )


def materialize_reviewed_closure(
    root: pathlib.Path,
    destination: pathlib.Path,
    descriptor_destination: pathlib.Path,
    reviewed_source_closure: str,
    reviewed_source_closure_sha256: str,
    *,
    expected_identity: SourceIdentity | None = None,
) -> SourceMaterialization:
    """Create and verify one private exact copy of the pinned closure.

    Callers must execute source-consuming tools through
    :func:`write_denied_source_command`; owner mode bits are not an
    immutability boundary.
    """

    root = _repository_root(root)
    destination = _require_private_destination(root, destination)
    descriptor_destination = _require_private_destination(
        root,
        descriptor_destination,
    )
    if destination.parent != descriptor_destination.parent:
        raise SourceSealError(
            "private source and descriptor must share one owner-private parent"
        )
    first = compute_identity(
        root,
        reviewed_source_closure,
        reviewed_source_closure_sha256,
    )
    if expected_identity is not None and first != expected_identity:
        raise SourceSealError("reviewed source changed before private materialization")
    descriptor_payload = _read_descriptor_payload(reviewed_source_closure)
    if hashlib.sha256(descriptor_payload).hexdigest() != reviewed_source_closure_sha256:
        raise SourceSealError("reviewed source closure descriptor differs from its pin")

    final_patch = _git(root, *TRACKED_DIFF_ARGUMENTS)
    if (
        compute_identity(
            root,
            reviewed_source_closure,
            reviewed_source_closure_sha256,
        )
        != first
    ):
        raise SourceSealError("reviewed source changed while materialization began")

    _git(
        root,
        "init",
        "--quiet",
        "--template=/dev/null",
        os.fspath(destination),
    )
    destination.chmod(0o700)
    _git(
        destination,
        "fetch",
        "--quiet",
        "--depth=1",
        "--no-tags",
        os.fspath(root),
        first.source_commit,
    )
    _git(destination, "checkout", "--quiet", "--detach", "FETCH_HEAD")
    if final_patch:
        _git_with_input(
            destination,
            final_patch,
            "apply",
            "--binary",
            "--index",
            "--whitespace=nowarn",
        )

    for index, entry in enumerate(
        first.reviewed_source_closure[
            "untracked_path_mode_blob_oid_manifest"
        ]
    ):
        path = _decode_manifest_path(
            entry,
            f"untracked_path_mode_blob_oid_manifest[{index}]",
        )
        _copy_reviewed_regular_file(
            root,
            destination,
            path,
            expected_size=os.lstat(os.path.join(os.fsencode(root), path)).st_size,
            expected_sha256=entry["blob_sha256"],
            expected_mode=entry["git_mode"].encode("ascii"),
        )
    _copy_reviewed_regular_file(
        root,
        destination,
        REQUIRED_CARGO_LOCK_PATH,
        expected_size=first.reviewed_source_closure[
            "ignored_cargo_lock_size_bytes"
        ],
        expected_sha256=first.reviewed_source_closure["ignored_cargo_lock_sha256"],
        expected_mode=b"100644",
    )
    _write_private_descriptor(descriptor_destination, descriptor_payload)

    materialized_identity = compute_identity(
        destination,
        os.fspath(descriptor_destination),
        reviewed_source_closure_sha256,
    )
    if materialized_identity != first:
        raise SourceSealError(
            "private materialization differs from the pinned reviewed closure"
        )
    final_identity = compute_identity(
        destination,
        os.fspath(descriptor_destination),
        reviewed_source_closure_sha256,
    )
    if final_identity != first:
        raise SourceSealError("private materialization changed after verification")
    return SourceMaterialization(
        root=destination,
        reviewed_source_closure=descriptor_destination,
        identity=final_identity,
    )


def _hdiutil(*arguments: str, plist: bool = False) -> bytes:
    executable = _admitted_root_executable(
        MACOS_HDIUTIL,
        label="read-only source image tool",
    )
    environment = {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "TMPDIR": "/private/tmp",
        "TZ": "UTC",
    }
    command = [executable, *arguments]
    if plist:
        command.append("-plist")
    try:
        return subprocess.run(
            command,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        raise SourceSealError(
            f"read-only source image operation failed: {' '.join(arguments)}"
        ) from exc


def _attached_image_device(payload: bytes, mount_point: pathlib.Path) -> str:
    try:
        value = plistlib.loads(payload)
        entities = value["system-entities"]
    except (KeyError, TypeError, ValueError, plistlib.InvalidFileException) as exc:
        raise SourceSealError("read-only source image returned malformed metadata") from exc
    if not isinstance(entities, list):
        raise SourceSealError("read-only source image entity inventory is malformed")
    matches = [
        entity
        for entity in entities
        if isinstance(entity, dict)
        and entity.get("mount-point") == os.fspath(mount_point)
        and isinstance(entity.get("dev-entry"), str)
    ]
    if len(matches) != 1:
        raise SourceSealError("read-only source image mount identity is ambiguous")
    device = matches[0]["dev-entry"]
    if re.fullmatch(r"/dev/disk[0-9]+(?:s[0-9]+)?", device) is None:
        raise SourceSealError("read-only source image device is malformed")
    return device


def _validate_attached_image_device(device: str) -> None:
    try:
        metadata = pathlib.Path(device).lstat()
    except OSError as exc:
        raise SourceSealError("read-only source image device is unavailable") from exc
    if (
        not stat.S_ISBLK(metadata.st_mode)
        or metadata.st_uid not in (0, os.geteuid())
        or metadata.st_mode & 0o222 != 0
    ):
        raise SourceSealError("read-only source image device has unsafe metadata")


@contextmanager
def sealed_reviewed_closure(
    root: pathlib.Path,
    workspace: pathlib.Path,
    reviewed_source_closure: str,
    reviewed_source_closure_sha256: str,
    *,
    expected_identity: SourceIdentity,
    platform_name: str | None = None,
) -> Iterator[SourceMaterialization]:
    """Yield an exact source copy from an unlinked read-only filesystem image.

    The process-local command sandbox is defense in depth.  This context is
    the immutability boundary: on macOS it mounts a read-only UDRO image and
    unlinks the writable backing pathname before verifying or yielding it.
    Other platforms fail closed until an equivalent sealed filesystem backend
    is implemented.
    """

    selected_platform = sys.platform if platform_name is None else platform_name
    if selected_platform != "darwin":
        raise SourceSealError(
            "sealed production source materialization currently requires "
            "the macOS read-only disk-image boundary"
        )
    root = _repository_root(root)
    workspace = _require_private_destination(root, workspace)
    workspace.mkdir(mode=0o700)
    workspace_metadata = workspace.lstat()
    if (
        not stat.S_ISDIR(workspace_metadata.st_mode)
        or workspace_metadata.st_uid != os.geteuid()
        or workspace_metadata.st_mode & 0o077 != 0
    ):
        raise SourceSealError("sealed source workspace is not owner-private")

    staging = workspace / "staging"
    staging.mkdir(mode=0o700)
    staged = materialize_reviewed_closure(
        root,
        staging / "reviewed-source",
        staging / "reviewed-source-closure.json",
        reviewed_source_closure,
        reviewed_source_closure_sha256,
        expected_identity=expected_identity,
    )
    if staged.identity != expected_identity:
        raise SourceSealError("staged source image input is not the reviewed identity")

    image = workspace / "reviewed-source.dmg"
    mount_point = workspace / "mount"
    mount_point.mkdir(mode=0o700)
    _hdiutil(
        "create",
        "-quiet",
        "-srcfolder",
        os.fspath(staging),
        "-format",
        "UDRO",
        "-fs",
        "Case-sensitive APFS",
        "-volname",
        "KagemushaReviewedSource",
        os.fspath(image),
    )
    image_metadata = image.lstat()
    if (
        not stat.S_ISREG(image_metadata.st_mode)
        or image_metadata.st_uid != os.geteuid()
        or image_metadata.st_nlink != 1
        or image_metadata.st_size <= 0
        or image_metadata.st_mode & 0o022 != 0
    ):
        raise SourceSealError("sealed source image has unsafe metadata")

    device: str | None = None
    attached = False
    try:
        attach_payload = _hdiutil(
            "attach",
            "-readonly",
            "-nobrowse",
            "-noautoopen",
            "-noautofsck",
            "-owners",
            "on",
            "-mountpoint",
            os.fspath(mount_point),
            os.fspath(image),
            plist=True,
        )
        attached = True
        device = _attached_image_device(attach_payload, mount_point)
        _validate_attached_image_device(device)
        image.unlink()
        if image.exists() or image.is_symlink():
            raise SourceSealError("sealed source image backing path remained reachable")
        mounted_root = (mount_point / "reviewed-source").resolve(strict=True)
        mounted_descriptor = (
            mount_point / "reviewed-source-closure.json"
        ).resolve(strict=True)
        if (
            mounted_root.parent != mount_point
            or mounted_descriptor.parent != mount_point
            or mounted_root.stat().st_dev != mounted_descriptor.stat().st_dev
            or os.statvfs(mounted_root).f_flag & os.ST_RDONLY == 0
        ):
            raise SourceSealError(
                "sealed source image is not one exact read-only filesystem"
            )
        mounted_identity = compute_identity(
            mounted_root,
            os.fspath(mounted_descriptor),
            reviewed_source_closure_sha256,
        )
        if mounted_identity != expected_identity:
            raise SourceSealError(
                "sealed read-only source differs from the reviewed identity"
            )
        materialization = SourceMaterialization(
            root=mounted_root,
            reviewed_source_closure=mounted_descriptor,
            identity=mounted_identity,
        )
        yield materialization
        final_identity = compute_identity(
            mounted_root,
            os.fspath(mounted_descriptor),
            reviewed_source_closure_sha256,
        )
        if final_identity != mounted_identity:
            raise SourceSealError(
                "sealed read-only source identity changed while in use"
            )
    finally:
        if device is not None:
            _hdiutil("detach", "-quiet", device)
        elif attached:
            _hdiutil("detach", "-quiet", os.fspath(mount_point))


def _load_descriptor(
    path: str,
    expected_sha256: str,
    *,
    required_commit: str,
) -> tuple[dict[str, Any], str]:
    expected_sha256 = _require_digest(
        expected_sha256, "reviewed source closure descriptor SHA-256"
    )
    payload = _read_descriptor_payload(path)
    observed_sha256 = hashlib.sha256(payload).hexdigest()
    if observed_sha256 != expected_sha256:
        raise SourceSealError("reviewed source closure descriptor digest differs from its pin")
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_reject_duplicates,
            parse_constant=_reject_constant,
        )
    except (json.JSONDecodeError, UnicodeError, SourceSealError) as exc:
        raise SourceSealError("reviewed source closure is not strict JSON") from exc
    if _canonical_json_bytes(value) != payload:
        raise SourceSealError("reviewed source closure bytes are not canonical")
    return _validate_descriptor(value, required_commit), observed_sha256


def compute_observed_descriptor(root: pathlib.Path) -> dict[str, Any]:
    """Capture the canonical descriptor that must be reviewed independently."""

    return _capture_observed_descriptor(root)


def compute_identity(
    root: pathlib.Path,
    reviewed_source_closure: str,
    reviewed_source_closure_sha256: str,
) -> SourceIdentity:
    root = _repository_root(root)
    required_commit = _head(root).decode("ascii")
    descriptor, descriptor_sha256 = _load_descriptor(
        reviewed_source_closure,
        reviewed_source_closure_sha256,
        required_commit=required_commit,
    )
    observed = _capture_observed_descriptor(root)
    if observed != descriptor:
        raise SourceSealError(
            "current source closure differs from the independently pinned descriptor"
        )
    return SourceIdentity(
        source_commit=descriptor["source_commit"],
        source_tree_sha256=descriptor["source_tree_sha256"],
        source_repo_dirty=descriptor["source_repo_dirty"],
        reviewed_source_closure=descriptor,
        reviewed_source_closure_descriptor_sha256=descriptor_sha256,
    )


def compute_fingerprint(
    root: pathlib.Path,
    reviewed_source_closure: str,
    reviewed_source_closure_sha256: str,
) -> str:
    return compute_identity(
        root,
        reviewed_source_closure,
        reviewed_source_closure_sha256,
    ).source_tree_sha256


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "mode",
        choices=("descriptor", "fingerprint", "identity", "status", "paths"),
    )
    parser.add_argument("--root", type=pathlib.Path, required=True)
    parser.add_argument("--reviewed-source-closure")
    parser.add_argument("--reviewed-source-closure-sha256")
    return parser.parse_args()


def _require_review_pin(args: argparse.Namespace) -> tuple[str, str]:
    if not args.reviewed_source_closure or not args.reviewed_source_closure_sha256:
        raise SourceSealError(
            "identity/fingerprint require --reviewed-source-closure and "
            "--reviewed-source-closure-sha256"
        )
    return args.reviewed_source_closure, args.reviewed_source_closure_sha256


def main() -> int:
    args = parse_args()
    root = _repository_root(args.root)
    if args.mode == "descriptor":
        if args.reviewed_source_closure or args.reviewed_source_closure_sha256:
            raise SourceSealError("descriptor observation does not accept a review pin")
        sys.stdout.buffer.write(_canonical_json_bytes(compute_observed_descriptor(root)))
    elif args.mode == "fingerprint":
        path, sha256 = _require_review_pin(args)
        print(compute_fingerprint(root, path, sha256))
    elif args.mode == "identity":
        path, sha256 = _require_review_pin(args)
        identity = compute_identity(root, path, sha256)
        sys.stdout.buffer.write(
            _canonical_json_bytes(
                {
                    "reviewed_source_closure": identity.reviewed_source_closure,
                    "reviewed_source_closure_descriptor_sha256": (
                        identity.reviewed_source_closure_descriptor_sha256
                    ),
                    "schema": SOURCE_IDENTITY_SCHEMA,
                    "source_commit": identity.source_commit,
                    "source_repo_dirty": identity.source_repo_dirty,
                    "source_tree_sha256": identity.source_tree_sha256,
                }
            )
        )
    elif args.mode == "status":
        if args.reviewed_source_closure or args.reviewed_source_closure_sha256:
            raise SourceSealError("status does not accept a review pin")
        value = status(root)
        if value:
            sys.stdout.buffer.write(value)
    else:
        if args.reviewed_source_closure or args.reviewed_source_closure_sha256:
            raise SourceSealError("paths does not accept a review pin")
        for entry in _index_entries(root):
            sys.stdout.buffer.write(entry.path + b"\n")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, SourceSealError) as exc:
        print(f"Kagemusha source-tree seal failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
