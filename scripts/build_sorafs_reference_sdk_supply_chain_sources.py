#!/usr/bin/env python3
"""Assemble and verify canonical SF-11 supply-chain source indexes.

The release workflow already owns the immutable release archives, package
replay summaries, SPDX documents, SARIF reports, GitHub attestations, cosign
bundles, and authenticated release manifest. This helper binds those files into
the four schema-closed source indexes consumed by the SF-11 canary builder.
Each ``github-attestations/<target>.json`` file is a stable per-target copy of
the sign job's one aggregate, multi-subject attestation. The five files are not
claims that native runners independently emitted five attestations.

Ed25519-signed release rehearsal and provenance verification receipts remain
external runtime evidence. This helper copies and verifies them; it never
creates success receipts or receives signing material. The external root must
contain exactly:

``release-rehearsal/<target>.json`` and
``provenance-verification/<target>.json``

for each canonical native target. Receipts sign the exact domain-separated
bytes returned by their corresponding ``*_receipt_signing_bytes`` helper in
``sorafs_reference_sdk_supply_chain.py``. Provenance receipts also bind the
opened ``SHA256SUMS`` manifest and its cosign bundle.

Assembly is intentionally one-shot inside the protected workflow's disposable
fresh download root. On any failure this helper closes its descriptors but
deletes no generated name: portable POSIX APIs cannot condition unlink/rmdir on
an inode identity, so the caller must discard the entire failed workspace.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import secrets
import stat
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sccp_release_common import verify_ed25519  # noqa: E402
from sorafs_evidence_validation import (  # noqa: E402
    require_rollout_deployment_id,
    require_rollout_environment,
)
from sorafs_path_identity import resolve_path_identity  # noqa: E402
from sorafs_reference_sdk_supply_chain import (  # noqa: E402
    DEFAULT_SOURCE_ARTIFACT_PATHS,
    PROVENANCE_BUNDLE_SCHEMA,
    RELEASE_REHEARSAL_SCHEMA,
    REQUIRED_RELEASE_TARGETS,
    SBOM_INDEX_SCHEMA,
    VULNERABILITY_REPORT_SCHEMA,
    SupplyChainSourceResult,
    validate_supply_chain_sources,
)


SUMMARY_SCHEMA = "sorafs.reference_sdk.supply_chain_source_build.v1"
EVIDENCE_SUBDIRECTORY = "reference-sdk-evidence"
RELEASE_RECEIPT_SUBDIRECTORY = "release-rehearsal"
PROVENANCE_RECEIPT_SUBDIRECTORY = "provenance-verification"
RELEASE_CANDIDATE_SCHEMA = "sorafs.cli.candidate-manifest.v1"
MAX_JSON_BYTES = 16 * 1024 * 1024
MAX_RECEIPT_BYTES = 2 * 1024 * 1024
MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_TIMESTAMP = (1 << 63) - 1
HEX64_PATTERN = re.compile(r"^[0-9a-f]{64}\Z")
VERSION_PATTERN = re.compile(r"^[0-9A-Za-z][0-9A-Za-z.+-]{0,127}\Z")
_DESCRIPTOR_RELATIVE_ACCESS_AVAILABLE = (
    all(
        function in os.supports_dir_fd
        for function in (os.open, os.stat, os.mkdir)
    )
    and os.stat in os.supports_follow_symlinks
    and os.listdir in os.supports_fd
    and hasattr(os, "O_DIRECTORY")
    and hasattr(os, "O_NOFOLLOW")
)
RELEASE_SUMMARY_FIELDS = frozenset(
    {
        "schema",
        "status",
        "version",
        "target",
        "archive",
        "archive_sha256",
        "manifest",
        "manifest_sha256",
        "payload_file_count",
        "clean_smoke_binary_count",
    }
)


class SourceBuildError(ValueError):
    """Raised when workflow-owned or external source evidence is invalid."""


class _DuplicateJsonKey(ValueError):
    """Raised internally when strict JSON input repeats a key."""


@dataclass(frozen=True)
class CandidateEvidence:
    """Exact workflow-owned files for one native release target.

    ``attestation_bundle`` retains its schema-generic name, but points to the
    target-addressed stable copy of the aggregate multi-subject sign-job
    attestation.
    """

    target: str
    archive: str
    archive_sha256: str
    source_sbom: str
    source_report: str
    platform_sbom: str
    platform_report: str
    attestation_bundle: str
    cosign_bundle: str
    sha256sums: str
    sha256sums_cosign_bundle: str


@dataclass(frozen=True)
class _DirectoryHandle:
    """One held directory descriptor and its immutable opened identity."""

    fd: int
    identity: tuple[int, ...]
    label: str
    # ``path`` is the canonical path used for callers that must reopen the
    # tree. ``lexical_path`` retains the caller's spelling so a permitted
    # ancestor alias cannot be retargeted unnoticed after this handle opens.
    path: Path | None = None
    lexical_path: Path | None = None


@dataclass(frozen=True)
class _CreatedFile:
    """One exclusively created file retained for final publication checks."""

    directory: _DirectoryHandle
    name: str
    identity: tuple[int, ...]
    size: int
    sha256: str
    label: str


@dataclass(frozen=True)
class _CreatedDirectory:
    """One created directory retained for final publication checks."""

    parent: _DirectoryHandle
    name: str
    directory: _DirectoryHandle
    label: str


def _fail(message: str) -> None:
    raise SourceBuildError(message)


def _object_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    payload: dict[str, Any] = {}
    for key, value in pairs:
        if key in payload:
            raise _DuplicateJsonKey
        payload[key] = value
    return payload


def _reject_json_constant(_value: str) -> None:
    raise ValueError


def _finite_json_float(value: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ValueError
    return parsed


def _stat_identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_uid,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _directory_identity(metadata: os.stat_result) -> tuple[int, ...]:
    """Return directory identity fields that do not change as entries change."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        stat.S_IFMT(metadata.st_mode),
    )


def _absolute_lexical(path: Path) -> Path:
    return path if path.is_absolute() else Path.cwd() / path


def _directory_open_flags() -> int:
    return (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def _require_descriptor_relative_access() -> None:
    if not _DESCRIPTOR_RELATIVE_ACCESS_AVAILABLE:
        _fail("source assembly requires descriptor-relative file access")


def _open_root_directory(path: Path, *, label: str) -> _DirectoryHandle:
    _require_descriptor_relative_access()
    absolute = _absolute_lexical(path)
    descriptor = -1
    identity_errors: list[str] = []
    resolved = resolve_path_identity(
        absolute,
        identity_errors,
        label=label,
        failure_template="{label} `{path}` cannot be resolved safely: {error}",
    )
    if resolved is None:
        _fail(f"{label} must be an existing directory")
    try:
        lexical_leaf = absolute.lstat()
        metadata = resolved.lstat()
    except (OSError, RuntimeError) as error:
        raise SourceBuildError(f"{label} must be an existing directory") from error
    if stat.S_ISLNK(lexical_leaf.st_mode):
        _fail(f"{label} must not be a symlink")
    if (
        not stat.S_ISDIR(lexical_leaf.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or _stat_identity(lexical_leaf) != _stat_identity(metadata)
    ):
        _fail(f"{label} must be an existing directory")
    try:
        descriptor = os.open(resolved, _directory_open_flags())
        opened = os.fstat(descriptor)
    except OSError as error:
        if descriptor >= 0:
            os.close(descriptor)
        raise SourceBuildError(f"{label} could not be opened safely") from error
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        raise
    if (
        not stat.S_ISDIR(opened.st_mode)
        or _stat_identity(metadata) != _stat_identity(opened)
    ):
        os.close(descriptor)
        _fail(f"{label} changed while it was opened")
    return _DirectoryHandle(
        descriptor,
        _directory_identity(opened),
        label,
        resolved,
        absolute,
    )


def _close_directory(handle: _DirectoryHandle | None) -> None:
    if handle is None or handle.fd < 0:
        return
    try:
        os.close(handle.fd)
    except OSError:
        pass


def _verify_directory_descriptor(handle: _DirectoryHandle) -> None:
    try:
        opened = os.fstat(handle.fd)
    except OSError as error:
        raise SourceBuildError(f"{handle.label} descriptor is no longer valid") from error
    if (
        not stat.S_ISDIR(opened.st_mode)
        or _directory_identity(opened) != handle.identity
    ):
        _fail(f"{handle.label} changed during source assembly")


def _fsync_directory(handle: _DirectoryHandle) -> None:
    """Persist directory-entry updates through one retained descriptor."""

    _verify_directory_descriptor(handle)
    try:
        os.fsync(handle.fd)
    except OSError as error:
        raise SourceBuildError(
            f"{handle.label} could not be synchronized durably"
        ) from error


def _verify_root_binding(handle: _DirectoryHandle) -> None:
    _verify_directory_descriptor(handle)
    if handle.path is None and handle.lexical_path is None:
        return
    if handle.path is None or handle.lexical_path is None:
        _fail(f"{handle.label} path binding is incomplete")
    for current_path in (handle.path, handle.lexical_path):
        try:
            current = current_path.lstat()
        except OSError as error:
            raise SourceBuildError(
                f"{handle.label} path changed during source assembly"
            ) from error
        if (
            not stat.S_ISDIR(current.st_mode)
            or stat.S_ISLNK(current.st_mode)
            or _directory_identity(current) != handle.identity
        ):
            _fail(f"{handle.label} path changed during source assembly")


def _single_component(value: str, *, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or value in {".", ".."}
        or "/" in value
        or "\\" in value
        or "\x00" in value
    ):
        _fail(f"{label} must be one canonical path component")
    return value


def _relative_components(value: str, *, label: str) -> tuple[str, ...]:
    if (
        not isinstance(value, str)
        or not value
        or "\\" in value
        or "\x00" in value
        or value.startswith("/")
        or "//" in value
    ):
        _fail(f"{label} must be a canonical relative path")
    path = PurePosixPath(value)
    if (
        not path.parts
        or str(path) != value
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        _fail(f"{label} must be a canonical relative path")
    return path.parts


def _open_child_directory(
    parent: _DirectoryHandle,
    name: str,
    *,
    label: str,
) -> _DirectoryHandle:
    _verify_directory_descriptor(parent)
    component = _single_component(name, label=label)
    descriptor = -1
    try:
        before = os.stat(
            component,
            dir_fd=parent.fd,
            follow_symlinks=False,
        )
    except OSError as error:
        raise SourceBuildError(f"{label} must be an existing directory") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISDIR(before.st_mode):
        _fail(f"{label} must be a non-symlink directory")
    try:
        descriptor = os.open(
            component,
            _directory_open_flags(),
            dir_fd=parent.fd,
        )
        opened = os.fstat(descriptor)
    except OSError as error:
        if descriptor >= 0:
            os.close(descriptor)
        raise SourceBuildError(f"{label} could not be opened safely") from error
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        raise
    if (
        not stat.S_ISDIR(opened.st_mode)
        or _stat_identity(opened) != _stat_identity(before)
    ):
        os.close(descriptor)
        _fail(f"{label} changed while it was opened")
    return _DirectoryHandle(
        descriptor,
        _directory_identity(opened),
        label,
    )


def _create_child_directory(
    parent: _DirectoryHandle,
    name: str,
    *,
    label: str,
) -> _DirectoryHandle:
    _verify_directory_descriptor(parent)
    component = _single_component(name, label=label)
    try:
        os.mkdir(component, 0o700, dir_fd=parent.fd)
    except OSError as error:
        raise SourceBuildError(f"{label} already exists or cannot be created") from error
    try:
        directory = _open_child_directory(parent, component, label=label)
        try:
            _fsync_directory(directory)
            _fsync_directory(parent)
        except BaseException:
            _close_directory(directory)
            raise
        return directory
    except BaseException:
        # The entry may have been substituted between mkdir and open, or its
        # durability may be unknown. A later name-based delete cannot prove
        # ownership atomically, so failure deliberately leaves it fail-closed.
        raise


def _open_regular(
    directory: _DirectoryHandle,
    relative_path: str,
    *,
    label: str,
) -> tuple[int, os.stat_result]:
    components = _relative_components(relative_path, label=label)
    parent_fd = -1
    descriptor = -1
    try:
        parent_fd = os.dup(directory.fd)
        for component in components[:-1]:
            before_directory = os.stat(
                component,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
            if (
                stat.S_ISLNK(before_directory.st_mode)
                or not stat.S_ISDIR(before_directory.st_mode)
            ):
                _fail(f"{label} path must not contain symlinks")
            child_fd = -1
            try:
                child_fd = os.open(
                    component,
                    _directory_open_flags(),
                    dir_fd=parent_fd,
                )
                opened_directory = os.fstat(child_fd)
                if (
                    not stat.S_ISDIR(opened_directory.st_mode)
                    or _stat_identity(opened_directory)
                    != _stat_identity(before_directory)
                ):
                    _fail(f"{label} parent directory changed while it was opened")
            except BaseException:
                if child_fd >= 0:
                    os.close(child_fd)
                raise
            previous_fd = parent_fd
            parent_fd = child_fd
            child_fd = -1
            os.close(previous_fd)

        leaf = components[-1]
        before = os.stat(
            leaf,
            dir_fd=parent_fd,
            follow_symlinks=False,
        )
        if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
            _fail(f"{label} must be a non-symlink regular file")
        if before.st_nlink != 1:
            _fail(f"{label} must have exactly one hard link")
        descriptor = os.open(
            leaf,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent_fd,
        )
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or _stat_identity(opened) != _stat_identity(before)
        ):
            _fail(f"{label} changed while it was opened")
        return descriptor, opened
    except SourceBuildError:
        if descriptor >= 0:
            os.close(descriptor)
        raise
    except FileNotFoundError as error:
        if descriptor >= 0:
            os.close(descriptor)
        raise SourceBuildError(
            f"{label} must be an existing regular file"
        ) from error
    except OSError as error:
        if descriptor >= 0:
            os.close(descriptor)
        raise SourceBuildError(f"{label} could not be opened safely") from error
    except BaseException:
        if descriptor >= 0:
            os.close(descriptor)
        raise
    finally:
        if parent_fd >= 0:
            os.close(parent_fd)


def _read_regular_bytes(
    directory: _DirectoryHandle,
    relative_path: str,
    *,
    label: str,
    max_bytes: int,
) -> bytes:
    descriptor, opened = _open_regular(
        directory,
        relative_path,
        label=label,
    )
    try:
        chunks: list[bytes] = []
        observed = 0
        while True:
            chunk = os.read(descriptor, min(1024 * 1024, max_bytes + 1 - observed))
            if not chunk:
                break
            chunks.append(chunk)
            observed += len(chunk)
            if observed > max_bytes:
                _fail(f"{label} exceeds its byte limit")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if observed != opened.st_size or _stat_identity(after) != _stat_identity(opened):
        _fail(f"{label} changed while it was read")
    return b"".join(chunks)


def _hash_regular_file(
    directory: _DirectoryHandle,
    relative_path: str,
    *,
    label: str,
    max_bytes: int,
) -> tuple[str, int]:
    descriptor, opened = _open_regular(
        directory,
        relative_path,
        label=label,
    )
    digest = hashlib.sha256()
    observed = 0
    try:
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            observed += len(chunk)
            if observed > max_bytes:
                _fail(f"{label} exceeds its byte limit")
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if observed != opened.st_size or _stat_identity(after) != _stat_identity(opened):
        _fail(f"{label} changed while it was hashed")
    return digest.hexdigest(), observed


def _load_json(
    directory: _DirectoryHandle,
    relative_path: str,
    *,
    label: str,
    max_bytes: int,
) -> tuple[Any, bytes]:
    raw = _read_regular_bytes(
        directory,
        relative_path,
        label=label,
        max_bytes=max_bytes,
    )
    try:
        payload = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_object_pairs,
            parse_constant=_reject_json_constant,
            parse_float=_finite_json_float,
        )
    except (
        UnicodeDecodeError,
        json.JSONDecodeError,
        _DuplicateJsonKey,
        RecursionError,
        ValueError,
    ) as error:
        raise SourceBuildError(
            f"{label} must be strict UTF-8 JSON without duplicate keys"
        ) from error
    return payload, raw


def _canonical_json_bytes(payload: Any) -> bytes:
    try:
        return (
            json.dumps(
                payload,
                allow_nan=False,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise SourceBuildError("generated source index is not canonical JSON") from error


def _verify_child_binding(
    parent: _DirectoryHandle,
    name: str,
    child: _DirectoryHandle,
    *,
    label: str,
) -> None:
    """Require one child name to remain bound to its retained descriptor."""

    _verify_directory_descriptor(parent)
    _verify_directory_descriptor(child)
    component = _single_component(name, label=label)
    try:
        current = os.stat(
            component,
            dir_fd=parent.fd,
            follow_symlinks=False,
        )
    except OSError as error:
        raise SourceBuildError(f"{label} changed during source assembly") from error
    if (
        stat.S_ISLNK(current.st_mode)
        or not stat.S_ISDIR(current.st_mode)
        or _directory_identity(current) != child.identity
    ):
        _fail(f"{label} changed during source assembly")


def _write_exclusive(
    directory: _DirectoryHandle,
    name: str,
    payload: bytes,
    *,
    label: str,
) -> _CreatedFile:
    _verify_directory_descriptor(directory)
    component = _single_component(name, label=label)
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = -1
    try:
        descriptor = os.open(
            component,
            flags,
            0o600,
            dir_fd=directory.fd,
        )
    except OSError as error:
        raise SourceBuildError(f"{label} already exists or cannot be created") from error
    created: _CreatedFile | None = None
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise OSError("short write")
            view = view[written:]
        os.fsync(descriptor)
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_nlink != 1
            or opened.st_size != len(payload)
        ):
            _fail(f"{label} changed while it was written")
        created = _CreatedFile(
            directory,
            component,
            _stat_identity(opened),
            len(payload),
            hashlib.sha256(payload).hexdigest(),
            label,
        )
        current = os.stat(
            component,
            dir_fd=directory.fd,
            follow_symlinks=False,
        )
        if _stat_identity(current) != created.identity:
            _fail(f"{label} changed while it was written")
        _fsync_directory(directory)
        os.close(descriptor)
        descriptor = -1
        return created
    except BaseException as error:
        if descriptor >= 0:
            try:
                os.close(descriptor)
            except OSError as close_error:
                raise SourceBuildError(
                    f"{label} failed and its descriptor could not be closed"
                ) from close_error
        raise


def _reverify_created_file(created: _CreatedFile) -> None:
    """Re-open one output and require its original inode and exact bytes."""

    descriptor, opened = _open_regular(
        created.directory,
        created.name,
        label=created.label,
    )
    digest = hashlib.sha256()
    observed = 0
    try:
        if _stat_identity(opened) != created.identity:
            _fail(f"{created.label} changed after it was created")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            observed += len(chunk)
            if observed > created.size:
                _fail(f"{created.label} changed after it was created")
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        observed != created.size
        or _stat_identity(after) != created.identity
        or not secrets.compare_digest(digest.hexdigest(), created.sha256)
    ):
        _fail(f"{created.label} changed after it was created")
    try:
        current = os.stat(
            created.name,
            dir_fd=created.directory.fd,
            follow_symlinks=False,
        )
    except OSError as error:
        raise SourceBuildError(
            f"{created.label} changed after it was created"
        ) from error
    if _stat_identity(current) != created.identity:
        _fail(f"{created.label} changed after it was created")


def _verify_directory_inventory(
    directory: _DirectoryHandle,
    expected: set[str],
    *,
    label: str,
) -> None:
    """Require an opened generated directory to retain its exact entries."""

    _verify_directory_descriptor(directory)
    try:
        observed = set(os.listdir(directory.fd))
    except OSError as error:
        raise SourceBuildError(f"{label} cannot be enumerated safely") from error
    if observed != expected:
        _fail(f"{label} changed after it was created")


def _verify_created_directory_bindings(
    created_directories: list[_CreatedDirectory],
) -> None:
    """Require every generated child name to retain its opened directory."""

    for created in created_directories:
        _verify_child_binding(
            created.parent,
            created.name,
            created.directory,
            label=created.label,
        )


def _canonical_text(value: Any, *, label: str) -> str:
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or any(ord(character) < 32 or ord(character) == 127 for character in value)
    ):
        _fail(f"{label} must be a non-empty canonical string")
    return value


def _canonical_hex64(value: Any, *, label: str) -> str:
    if (
        not isinstance(value, str)
        or HEX64_PATTERN.fullmatch(value) is None
        or not any(character != "0" for character in value)
    ):
        _fail(f"{label} must be non-zero lowercase SHA-256")
    return value


def _positive_timestamp(value: Any, *, label: str) -> int:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value <= 0
        or value > MAX_TIMESTAMP
    ):
        _fail(f"{label} must be a positive bounded timestamp")
    return value


def _decode_public_key(value: str) -> bytes:
    canonical = _canonical_hex64(
        value,
        label="provenance verification public key",
    )
    public_key = bytes.fromhex(canonical)
    if len(public_key) != 32:
        _fail("provenance verification public key must be 32 raw bytes")
    return public_key


def _validate_rollout_context(deployment_id: str, environment: str) -> None:
    payload = {
        "deployment_id": deployment_id,
        "environment": environment,
    }
    errors: list[str] = []
    require_rollout_deployment_id(payload, errors)
    require_rollout_environment(payload, errors)
    if errors:
        _fail("release deployment context is not canonical")


def _file_reference(
    directory: _DirectoryHandle,
    relative_path: str,
    *,
    label: str,
    artifact_path: str | None = None,
) -> dict[str, str]:
    canonical_relative = "/".join(
        _relative_components(relative_path, label=label)
    )
    canonical_artifact = (
        canonical_relative
        if artifact_path is None
        else "/".join(_relative_components(artifact_path, label=label))
    )
    digest, _size = _hash_regular_file(
        directory,
        canonical_relative,
        label=label,
        max_bytes=MAX_ARCHIVE_BYTES,
    )
    return {
        "artifact_path": canonical_artifact,
        "sha256": digest,
    }


def _candidate_evidence(
    root: _DirectoryHandle,
    *,
    version: str,
    target: str,
) -> CandidateEvidence:
    candidate = f"release-candidates/sorafs-cli-{version}-{target}"
    platform_archive_directory = f"{candidate}/platform-archive"
    archive_name = f"sorafs-cli-{version}-{target}.tar.gz"
    archive = f"{platform_archive_directory}/{archive_name}"
    first_summary = f"{platform_archive_directory}/candidate-package-first.json"
    replay_summary = f"{platform_archive_directory}/candidate-package-replay.json"
    first_payload, first_bytes = _load_json(
        root,
        first_summary,
        label=f"{target} first candidate summary",
        max_bytes=MAX_JSON_BYTES,
    )
    replay_payload, replay_bytes = _load_json(
        root,
        replay_summary,
        label=f"{target} replay candidate summary",
        max_bytes=MAX_JSON_BYTES,
    )
    if first_bytes != replay_bytes or first_payload != replay_payload:
        _fail(f"{target} candidate replay summary must be byte-identical")
    if not isinstance(first_payload, dict) or set(first_payload) != RELEASE_SUMMARY_FIELDS:
        _fail(f"{target} candidate summary fields must match the closed contract")
    archive_digest, _archive_size = _hash_regular_file(
        root,
        archive,
        label=f"{target} release archive",
        max_bytes=MAX_ARCHIVE_BYTES,
    )
    expected_values = {
        "schema": RELEASE_CANDIDATE_SCHEMA,
        "status": "verified",
        "version": version,
        "target": target,
        "archive": archive_name,
        "archive_sha256": archive_digest,
        "clean_smoke_binary_count": 3,
    }
    for field, expected in expected_values.items():
        if first_payload.get(field) != expected:
            _fail(f"{target} candidate summary {field} is not workflow-derived")

    evidence = CandidateEvidence(
        target=target,
        archive=archive,
        archive_sha256=archive_digest,
        source_sbom=f"{candidate}/sorafs-release.spdx.json",
        source_report=f"{candidate}/sorafs-release-vulnerabilities.sarif",
        platform_sbom=f"{candidate}/sorafs-cli-{target}.spdx.json",
        platform_report=f"{candidate}/sorafs-cli-{target}-vulnerabilities.sarif",
        attestation_bundle=f"github-attestations/{target}.json",
        cosign_bundle=f"{archive}.sigstore.json",
        sha256sums=f"{candidate}/SHA256SUMS",
        sha256sums_cosign_bundle=f"{candidate}/SHA256SUMS.sigstore.json",
    )
    for label, path in (
        ("source SBOM", evidence.source_sbom),
        ("source vulnerability report", evidence.source_report),
        ("platform SBOM", evidence.platform_sbom),
        ("platform vulnerability report", evidence.platform_report),
        (
            "stable aggregate multi-subject GitHub attestation copy",
            evidence.attestation_bundle,
        ),
        ("cosign bundle", evidence.cosign_bundle),
        ("SHA256SUMS cosign bundle", evidence.sha256sums_cosign_bundle),
    ):
        _load_json(
            root,
            path,
            label=f"{target} {label}",
            max_bytes=MAX_JSON_BYTES,
        )
    _hash_regular_file(
        root,
        evidence.sha256sums,
        label=f"{target} SHA256SUMS",
        max_bytes=MAX_JSON_BYTES,
    )
    return evidence


def _require_identical_source_scans(
    root: _DirectoryHandle,
    candidates: list[CandidateEvidence],
) -> None:
    for field, label in (
        ("source_sbom", "source SBOM"),
        ("source_report", "source vulnerability report"),
    ):
        observed: set[str] = set()
        for candidate in candidates:
            digest, _size = _hash_regular_file(
                root,
                getattr(candidate, field),
                label=f"{candidate.target} {label}",
                max_bytes=MAX_JSON_BYTES,
            )
            observed.add(digest)
        if len(observed) != 1:
            _fail(f"all target candidates must carry the same {label}")


def _external_receipt_inventory(
    root: _DirectoryHandle,
    subdirectory: str,
) -> tuple[_DirectoryHandle, dict[str, str]]:
    directory = _open_child_directory(
        root,
        subdirectory,
        label=f"external {subdirectory} receipt directory",
    )
    expected = {f"{target}.json" for target in REQUIRED_RELEASE_TARGETS}
    try:
        observed = set(os.listdir(directory.fd))
    except OSError as error:
        _close_directory(directory)
        raise SourceBuildError(
            f"external {subdirectory} receipt directory cannot be enumerated"
        ) from error
    except BaseException:
        _close_directory(directory)
        raise
    if observed != expected:
        _close_directory(directory)
        _fail(
            f"external {subdirectory} receipts must contain exactly five "
            "canonical target files"
        )
    return (
        directory,
        {target: f"{target}.json" for target in REQUIRED_RELEASE_TARGETS},
    )


def _verify_external_receipt_inventory(
    root: _DirectoryHandle,
    subdirectory: str,
    directory: _DirectoryHandle,
) -> None:
    """Recheck the retained external directory and its exact fixed inventory."""

    _verify_child_binding(
        root,
        subdirectory,
        directory,
        label=f"external {subdirectory} receipt directory",
    )
    expected = {f"{target}.json" for target in REQUIRED_RELEASE_TARGETS}
    try:
        observed = set(os.listdir(directory.fd))
    except OSError as error:
        raise SourceBuildError(
            f"external {subdirectory} receipt directory cannot be enumerated"
        ) from error
    if observed != expected:
        _fail(
            f"external {subdirectory} receipts changed during source assembly"
        )


def _copy_receipt(
    source_directory: _DirectoryHandle,
    source_name: str,
    destination_directory: _DirectoryHandle,
    destination_name: str,
    *,
    label: str,
) -> _CreatedFile:
    payload, raw = _load_json(
        source_directory,
        source_name,
        label=label,
        max_bytes=MAX_RECEIPT_BYTES,
    )
    if not isinstance(payload, dict):
        _fail(f"{label} must be a JSON object")
    return _write_exclusive(
        destination_directory,
        destination_name,
        raw,
        label=f"staged {label}",
    )


def _common_source_fields(
    *,
    schema: str,
    generated_at_unix: int,
    deployment_id: str,
    environment: str,
    release_manifest_digest_hex: str,
) -> dict[str, Any]:
    return {
        "schema": schema,
        "generated_at_unix": generated_at_unix,
        "deployment_id": deployment_id,
        "environment": environment,
        "deployment_context_reviewed": True,
        "release_manifest_digest_hex": release_manifest_digest_hex,
    }


def _entry_exists(directory: _DirectoryHandle, name: str, *, label: str) -> bool:
    component = _single_component(name, label=label)
    try:
        os.stat(
            component,
            dir_fd=directory.fd,
            follow_symlinks=False,
        )
    except FileNotFoundError:
        return False
    except OSError as error:
        raise SourceBuildError(f"{label} cannot be inspected safely") from error
    return True


def build_sources(
    *,
    source_root: Path,
    external_receipts_root: Path,
    version: str,
    deployment_id: str,
    environment: str,
    generated_at_unix: int,
    now_unix: int,
    provenance_certificate_identity: str,
    provenance_oidc_issuer: str,
    provenance_verification_public_key_hex: str,
) -> dict[str, Any]:
    """Build, re-open, and verify the exact canonical source bundle."""

    if VERSION_PATTERN.fullmatch(version) is None:
        _fail("release version must be canonical")
    deployment_id = _canonical_text(deployment_id, label="deployment id")
    environment = _canonical_text(environment, label="environment")
    _validate_rollout_context(deployment_id, environment)
    generated_at_unix = _positive_timestamp(
        generated_at_unix,
        label="source generation timestamp",
    )
    now_unix = _positive_timestamp(now_unix, label="source validation clock")
    if generated_at_unix > now_unix:
        _fail("source generation timestamp must not be in the future")
    certificate_identity = _canonical_text(
        provenance_certificate_identity,
        label="provenance certificate identity",
    )
    oidc_issuer = _canonical_text(
        provenance_oidc_issuer,
        label="provenance OIDC issuer",
    )
    if not certificate_identity.startswith("https://"):
        _fail("provenance certificate identity must use HTTPS")
    if not oidc_issuer.startswith("https://"):
        _fail("provenance OIDC issuer must use HTTPS")
    public_key = _decode_public_key(provenance_verification_public_key_hex)
    key_fingerprint = hashlib.sha256(public_key).hexdigest()

    retained_directories: list[_DirectoryHandle] = []
    created_files: list[_CreatedFile] = []
    created_directories: list[_CreatedDirectory] = []
    root: _DirectoryHandle | None = None
    receipts_root: _DirectoryHandle | None = None
    # V1 deliberately excludes availability recovery and in-place retry after
    # process death because no reserved-namespace transaction/journal protocol
    # is part of the release wire. Every failure therefore leaves generated
    # names untouched so the caller can discard the protected workflow's
    # entire fresh workspace; a rerun refuses partial names instead of guessing
    # ownership and deleting them.
    try:
        root = _open_root_directory(
            source_root,
            label="supply-chain source root",
        )
        retained_directories.append(root)
        receipts_root = _open_root_directory(
            external_receipts_root,
            label="external receipt root",
        )
        retained_directories.append(receipts_root)
        assert root.path is not None and receipts_root.path is not None
        if (
            root.identity == receipts_root.identity
            or root.path == receipts_root.path
            or root.path in receipts_root.path.parents
            or receipts_root.path in root.path.parents
        ):
            _fail("external receipts and workflow source roots must not overlap")

        output_names = {
            kind: _single_component(
                relative,
                label=f"{kind} canonical source index",
            )
            for kind, relative in DEFAULT_SOURCE_ARTIFACT_PATHS.items()
        }
        if _entry_exists(
            root,
            EVIDENCE_SUBDIRECTORY,
            label="generated reference-SDK evidence directory",
        ):
            _fail("generated reference-SDK evidence directory must not already exist")
        if any(
            _entry_exists(
                root,
                name,
                label=f"{kind} canonical source index",
            )
            for kind, name in output_names.items()
        ):
            _fail("canonical source indexes must not already exist")

        candidates = [
            _candidate_evidence(root, version=version, target=target)
            for target in REQUIRED_RELEASE_TARGETS
        ]
        _require_identical_source_scans(root, candidates)
        release_manifest = "release-authentication/release_manifest.json"
        release_manifest_digest, _manifest_size = _hash_regular_file(
            root,
            release_manifest,
            label="authenticated release manifest",
            max_bytes=MAX_JSON_BYTES,
        )
        _load_json(
            root,
            release_manifest,
            label="authenticated release manifest",
            max_bytes=MAX_JSON_BYTES,
        )
        release_receipt_source, release_receipts = _external_receipt_inventory(
            receipts_root,
            RELEASE_RECEIPT_SUBDIRECTORY,
        )
        retained_directories.append(release_receipt_source)
        (
            provenance_receipt_source,
            provenance_receipts,
        ) = _external_receipt_inventory(
            receipts_root,
            PROVENANCE_RECEIPT_SUBDIRECTORY,
        )
        retained_directories.append(provenance_receipt_source)

        evidence_directory = _create_child_directory(
            root,
            EVIDENCE_SUBDIRECTORY,
            label="generated reference-SDK evidence directory",
        )
        retained_directories.append(evidence_directory)
        created_directories.append(
            _CreatedDirectory(
                root,
                EVIDENCE_SUBDIRECTORY,
                evidence_directory,
                "generated reference-SDK evidence directory",
            )
        )
        release_receipt_directory = _create_child_directory(
            evidence_directory,
            RELEASE_RECEIPT_SUBDIRECTORY,
            label="staged release rehearsal receipt directory",
        )
        retained_directories.append(release_receipt_directory)
        created_directories.append(
            _CreatedDirectory(
                evidence_directory,
                RELEASE_RECEIPT_SUBDIRECTORY,
                release_receipt_directory,
                "staged release rehearsal receipt directory",
            )
        )
        provenance_receipt_directory = _create_child_directory(
            evidence_directory,
            PROVENANCE_RECEIPT_SUBDIRECTORY,
            label="staged provenance verification receipt directory",
        )
        retained_directories.append(provenance_receipt_directory)
        created_directories.append(
            _CreatedDirectory(
                evidence_directory,
                PROVENANCE_RECEIPT_SUBDIRECTORY,
                provenance_receipt_directory,
                "staged provenance verification receipt directory",
            )
        )
        for target in REQUIRED_RELEASE_TARGETS:
            target_receipt = f"{target}.json"
            created_files.append(
                _copy_receipt(
                    release_receipt_source,
                    release_receipts[target],
                    release_receipt_directory,
                    target_receipt,
                    label=f"{target} external release rehearsal receipt",
                )
            )
            created_files.append(
                _copy_receipt(
                    provenance_receipt_source,
                    provenance_receipts[target],
                    provenance_receipt_directory,
                    target_receipt,
                    label=f"{target} external provenance verification receipt",
                )
            )
        _verify_external_receipt_inventory(
            receipts_root,
            RELEASE_RECEIPT_SUBDIRECTORY,
            release_receipt_source,
        )
        _verify_external_receipt_inventory(
            receipts_root,
            PROVENANCE_RECEIPT_SUBDIRECTORY,
            provenance_receipt_source,
        )
        _verify_root_binding(receipts_root)

        common = {
            "generated_at_unix": generated_at_unix,
            "deployment_id": deployment_id,
            "environment": environment,
            "release_manifest_digest_hex": release_manifest_digest,
        }
        release_rows: list[dict[str, Any]] = []
        sbom_rows: list[dict[str, Any]] = []
        vulnerability_rows: list[dict[str, Any]] = []
        provenance_rows: list[dict[str, Any]] = []
        for candidate in candidates:
            target = candidate.target
            release_rows.append(
                {
                    "target": target,
                    "release_artifact": {
                        "artifact_path": candidate.archive,
                        "sha256": candidate.archive_sha256,
                    },
                    "receipt": _file_reference(
                        release_receipt_directory,
                        f"{target}.json",
                        label=f"{target} staged release rehearsal receipt",
                        artifact_path=(
                            f"{EVIDENCE_SUBDIRECTORY}/"
                            f"{RELEASE_RECEIPT_SUBDIRECTORY}/{target}.json"
                        ),
                    ),
                }
            )
            sbom_rows.append(
                {
                    "target": target,
                    "platform_sbom": _file_reference(
                        root,
                        candidate.platform_sbom,
                        label=f"{target} platform SBOM",
                    ),
                }
            )
            vulnerability_rows.append(
                {
                    "target": target,
                    "platform_report": _file_reference(
                        root,
                        candidate.platform_report,
                        label=f"{target} platform vulnerability report",
                    ),
                }
            )
            provenance_rows.append(
                {
                    "target": target,
                    "attestation_bundle": _file_reference(
                        root,
                        candidate.attestation_bundle,
                        label=(
                            f"{target} stable aggregate multi-subject GitHub "
                            "attestation copy"
                        ),
                    ),
                    "cosign_bundle": _file_reference(
                        root,
                        candidate.cosign_bundle,
                        label=f"{target} cosign bundle",
                    ),
                    "sha256sums": _file_reference(
                        root,
                        candidate.sha256sums,
                        label=f"{target} SHA256SUMS",
                    ),
                    "sha256sums_cosign_bundle": _file_reference(
                        root,
                        candidate.sha256sums_cosign_bundle,
                        label=f"{target} SHA256SUMS cosign bundle",
                    ),
                    "verification_receipt": _file_reference(
                        provenance_receipt_directory,
                        f"{target}.json",
                        label=f"{target} staged provenance verification receipt",
                        artifact_path=(
                            f"{EVIDENCE_SUBDIRECTORY}/"
                            f"{PROVENANCE_RECEIPT_SUBDIRECTORY}/{target}.json"
                        ),
                    ),
                }
            )

        release_rehearsal = _common_source_fields(
            schema=RELEASE_REHEARSAL_SCHEMA,
            **common,
        )
        release_rehearsal["targets"] = release_rows
        sbom_index = _common_source_fields(schema=SBOM_INDEX_SCHEMA, **common)
        sbom_index.update(
            {
                "source_sbom": _file_reference(
                    root,
                    candidates[0].source_sbom,
                    label="source release SBOM",
                ),
                "targets": sbom_rows,
            }
        )
        vulnerability_report = _common_source_fields(
            schema=VULNERABILITY_REPORT_SCHEMA,
            **common,
        )
        vulnerability_report.update(
            {
                "source_report": _file_reference(
                    root,
                    candidates[0].source_report,
                    label="source release vulnerability report",
                ),
                "targets": vulnerability_rows,
            }
        )
        provenance_bundle = _common_source_fields(
            schema=PROVENANCE_BUNDLE_SCHEMA,
            **common,
        )
        provenance_bundle.update(
            {
                "certificate_identity": certificate_identity,
                "oidc_issuer": oidc_issuer,
                "verification_key_fingerprint_hex": key_fingerprint,
                "targets": provenance_rows,
            }
        )
        payloads = {
            "release_rehearsal": release_rehearsal,
            "sbom_index": sbom_index,
            "vulnerability_report": vulnerability_report,
            "provenance_bundle": provenance_bundle,
        }
        created_source_indexes: dict[str, _CreatedFile] = {}
        for kind, payload in payloads.items():
            created = _write_exclusive(
                root,
                output_names[kind],
                _canonical_json_bytes(payload),
                label=f"{kind} canonical source index",
            )
            created_files.append(created)
            created_source_indexes[kind] = created

        def authenticate(
            claimed_fingerprint: str,
            message: bytes,
            signature: bytes,
        ) -> bool:
            return secrets.compare_digest(
                claimed_fingerprint,
                key_fingerprint,
            ) and verify_ed25519(public_key, signature, message)

        _verify_root_binding(root)
        result, source_errors = validate_supply_chain_sources(
            root.path,
            expected_deployment_id=deployment_id,
            expected_environment=environment,
            expected_release_manifest_digest_hex=release_manifest_digest,
            expected_certificate_identity=certificate_identity,
            expected_verification_key_fingerprint_hex=key_fingerprint,
            verification_receipt_authenticator=authenticate,
            now_unix=now_unix,
            expected_oidc_issuer=oidc_issuer,
        )
        _verify_root_binding(root)
        if source_errors or not isinstance(result, SupplyChainSourceResult):
            _fail("assembled source bundle failed canonical source validation")
        validated_source_artifacts = {
            artifact.kind: artifact for artifact in result.source_artifacts
        }
        if (
            len(validated_source_artifacts) != len(result.source_artifacts)
            or set(validated_source_artifacts) != set(created_source_indexes)
        ):
            _fail("canonical source validation returned the wrong source artifacts")
        for kind, created in created_source_indexes.items():
            artifact = validated_source_artifacts[kind]
            if (
                artifact.artifact_path != output_names[kind]
                or not secrets.compare_digest(artifact.sha256, created.sha256)
            ):
                _fail(
                    f"{kind} canonical source index changed during source validation"
                )

        staged_receipt_names = {
            f"{target}.json" for target in REQUIRED_RELEASE_TARGETS
        }
        staged_evidence_names = {
            RELEASE_RECEIPT_SUBDIRECTORY,
            PROVENANCE_RECEIPT_SUBDIRECTORY,
        }
        _fsync_directory(release_receipt_directory)
        _fsync_directory(provenance_receipt_directory)
        _fsync_directory(evidence_directory)
        _fsync_directory(root)
        _verify_created_directory_bindings(created_directories)
        _verify_directory_inventory(
            evidence_directory,
            staged_evidence_names,
            label="generated reference-SDK evidence directory",
        )
        _verify_directory_inventory(
            release_receipt_directory,
            staged_receipt_names,
            label="staged release rehearsal receipt directory",
        )
        _verify_directory_inventory(
            provenance_receipt_directory,
            staged_receipt_names,
            label="staged provenance verification receipt directory",
        )
        for created in created_files:
            _reverify_created_file(created)
        _verify_created_directory_bindings(created_directories)
        _verify_directory_inventory(
            evidence_directory,
            staged_evidence_names,
            label="generated reference-SDK evidence directory",
        )
        _verify_directory_inventory(
            release_receipt_directory,
            staged_receipt_names,
            label="staged release rehearsal receipt directory",
        )
        _verify_directory_inventory(
            provenance_receipt_directory,
            staged_receipt_names,
            label="staged provenance verification receipt directory",
        )
        _verify_root_binding(root)
        return {
            "schema": SUMMARY_SCHEMA,
            "status": "validated",
            "generated_at_unix": result.generated_at_unix,
            "now_unix": now_unix,
            "deployment_id": deployment_id,
            "environment": environment,
            "release_manifest_digest_hex": release_manifest_digest,
            "provenance_verification_key_fingerprint_hex": key_fingerprint,
            "source_artifacts": [
                artifact.to_dict() for artifact in result.source_artifacts
            ],
        }
    finally:
        for directory in reversed(retained_directories):
            _close_directory(directory)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    """Parse the strict workflow-only source assembly interface."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--external-receipts-root", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--deployment-id", required=True)
    parser.add_argument("--environment", required=True)
    parser.add_argument("--generated-at-unix", required=True, type=int)
    parser.add_argument("--now-unix", required=True, type=int)
    parser.add_argument("--provenance-certificate-identity", required=True)
    parser.add_argument("--provenance-oidc-issuer", required=True)
    parser.add_argument(
        "--provenance-verification-public-key-hex",
        required=True,
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    """Build the source indexes and emit one payload-free validation summary."""

    args = parse_args(argv)
    try:
        summary = build_sources(
            source_root=args.source_root,
            external_receipts_root=args.external_receipts_root,
            version=args.version,
            deployment_id=args.deployment_id,
            environment=args.environment,
            generated_at_unix=args.generated_at_unix,
            now_unix=args.now_unix,
            provenance_certificate_identity=args.provenance_certificate_identity,
            provenance_oidc_issuer=args.provenance_oidc_issuer,
            provenance_verification_public_key_hex=(
                args.provenance_verification_public_key_hex
            ),
        )
    except (SourceBuildError, OSError, RuntimeError) as error:
        print(
            f"error: SF-11 supply-chain source assembly failed: {error}",
            file=sys.stderr,
        )
        return 2
    print(
        json.dumps(
            summary,
            allow_nan=False,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
