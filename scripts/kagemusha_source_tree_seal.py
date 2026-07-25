#!/usr/bin/env python3
"""Capture and verify the exact reviewed Kagemusha source closure.

Candidate generation is allowed from a dirty checkout only when the complete
tracked diff, untracked regular files, ignored root ``Cargo.lock``, and full
source-tree identity match one canonical descriptor whose raw SHA-256 is pinned
independently.  ``descriptor`` emits the observation that must be reviewed and
pinned.  ``identity`` and ``fingerprint`` never accept an unpinned observation.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import pathlib
import re
import stat
import subprocess
import sys
from dataclasses import dataclass
from typing import Any


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
REQUIRED_IGNORED_BUILD_INPUT = b"Cargo.lock"
GIT = pathlib.Path("/usr/bin/git")
GIT_ARGUMENT_PREFIX = (
    "-c",
    "core.attributesFile=/dev/null",
    "-c",
    "core.excludesFile=/dev/null",
    "-c",
    "core.fsmonitor=false",
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


def _git_environment() -> dict[str, str]:
    return {
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


def _git(root: pathlib.Path, *arguments: str) -> bytes:
    if not GIT.is_file() or GIT.is_symlink():
        raise SourceSealError("pinned /usr/bin/git is unavailable")
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
        or (not allow_cargo_lock and path == REQUIRED_IGNORED_BUILD_INPUT)
    ):
        raise SourceSealError("Git returned an unsafe source path")


def _index_entries(root: pathlib.Path) -> list[IndexEntry]:
    records = _git(root, "ls-files", "--stage", "-z", "--").split(b"\0")
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
    untracked_before = _untracked_paths(root)
    ignored_before = _ignored_paths(root)
    if ignored_before != [REQUIRED_IGNORED_BUILD_INPUT]:
        raise SourceSealError(
            "ignored source set must contain exactly the separately bound root Cargo.lock"
        )
    entries = _index_entries(root)
    source_hasher = hashlib.sha256(SOURCE_TREE_DOMAIN)
    root_bytes = os.fsencode(root)

    for entry in entries:
        absolute = os.path.join(root_bytes, entry.path)
        _field(source_hasher, b"tracked-source-v1")
        _field(source_hasher, entry.path)
        try:
            metadata = os.lstat(absolute)
        except FileNotFoundError:
            _field(source_hasher, b"absent")
            continue
        if stat.S_ISREG(metadata.st_mode):
            current_mode = b"100755" if metadata.st_mode & 0o111 else b"100644"
            _field(source_hasher, current_mode)
            _hash_regular_file(absolute, source_hasher)
        elif stat.S_ISLNK(metadata.st_mode):
            _field(source_hasher, b"120000")
            _field(source_hasher, _stable_symlink_bytes(absolute))
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

    cargo_lock_path = os.path.join(root_bytes, REQUIRED_IGNORED_BUILD_INPUT)
    cargo_metadata = os.lstat(cargo_lock_path)
    if cargo_metadata.st_mode & 0o111:
        raise SourceSealError("ignored root Cargo.lock must not be executable")
    _field(source_hasher, b"required-ignored-build-input-v1")
    _field(source_hasher, REQUIRED_IGNORED_BUILD_INPUT)
    _field(source_hasher, b"100644")
    cargo_lock_size, cargo_lock_sha256, _ = _hash_regular_file(
        cargo_lock_path,
        source_hasher,
        maximum_bytes=MAX_CARGO_LOCK_BYTES,
        require_nonempty=True,
    )

    head_after = _head(root)
    diff_after = _git(root, *TRACKED_DIFF_ARGUMENTS)
    untracked_after = _untracked_paths(root)
    ignored_after = _ignored_paths(root)
    cargo_recheck_size, cargo_recheck_sha256, _ = _hash_regular_file(
        cargo_lock_path,
        hashlib.sha256(),
        maximum_bytes=MAX_CARGO_LOCK_BYTES,
        require_nonempty=True,
    )
    if (
        head_after != head_before
        or diff_after != diff_before
        or untracked_after != untracked_before
        or ignored_after != ignored_before
        or cargo_recheck_size != cargo_lock_size
        or cargo_recheck_sha256 != cargo_lock_sha256
    ):
        raise SourceSealError("Kagemusha source HEAD or closure changed while sealing")

    tracked_binary_diff_sha256 = hashlib.sha256(diff_before).hexdigest()
    untracked_manifest_sha256 = hashlib.sha256(
        _untracked_manifest_bytes(untracked_manifest)
    ).hexdigest()
    combined = hashlib.sha256()
    combined.update(SOURCE_DIFF_DOMAIN)
    combined.update(TRACKED_DIFF_DOMAIN)
    combined.update(bytes.fromhex(tracked_binary_diff_sha256))
    combined.update(UNTRACKED_MANIFEST_DOMAIN)
    combined.update(bytes.fromhex(untracked_manifest_sha256))
    source_repo_dirty = (
        tracked_binary_diff_sha256 != EMPTY_SHA256 or bool(untracked_manifest)
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
    derived_dirty = (
        value["tracked_binary_diff_sha256"] != EMPTY_SHA256 or file_count != 0
    )
    if value["source_repo_dirty"] is not derived_dirty:
        raise SourceSealError("source_repo_dirty does not equal the derived closure state")
    if not derived_dirty:
        raise SourceSealError("reviewed Kagemusha source closure must be nonempty")
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
