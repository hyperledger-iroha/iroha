#!/usr/bin/env python3
"""Validate the externally authenticated Sumeragi v2 bootstrap contract.

This validator is candidate code, but the bootstrap authenticates the candidate
before launching it.  The runner must invoke this file with the archived Python
interpreter, isolated and without site initialization, before it invokes any
other candidate helper.  The completion-marker digest is accepted only from
the two equal out-of-band environment aliases installed by the bootstrap.
"""

from __future__ import annotations

import argparse
import base64
import binascii
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import time
from typing import Any, Iterable


_DIGEST_RE = re.compile(r"[0-9a-f]{64}")
_FINGERPRINT_RE = re.compile(r"SHA256:[A-Za-z0-9+/]{43}")
_OBJECT_ID_RE = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_MODE_RE = re.compile(r"[0-7]{4}")
_ENV_NAME_RE = re.compile(r"[A-Z_][A-Z0-9_]*")
_IDENTITY_KEYS = {
    "schema_version",
    "head_commit",
    "head_tree",
    "index_tree",
    "workspace_source_manifest_sha256",
    "cargo_lock_sha256",
}
_MARKER_KEYS = {
    "schema_version",
    "trust_boundary",
    "candidate_root",
    "candidate_identity",
    "candidate_identity_sha256",
    "trusted_inputs",
    "identity_verification",
    "runner",
    "trusted_execution_probes",
}
_TRUSTED_INPUT_KEYS = {
    "allowed_signers",
    "bash",
    "bootstrap",
    "git",
    "identity_verifier",
    "manifest_helper",
    "python",
    "receipt_validator",
    "revocation",
    "runner_tool_manifest",
    "ssh_keygen",
}
_TRUSTED_ARCHIVE_NAMES = {
    "allowed_signers": "bootstrap-allowed-signers",
    "bash": "bash",
    "bootstrap": "trusted-bootstrap.py",
    "git": "git",
    "identity_verifier": "verify-identity.py",
    "manifest_helper": "compute-manifest.py",
    "python": "python3",
    "receipt_validator": "validate-receipt.py",
    "revocation": "bootstrap-revocation",
    "runner_tool_manifest": "runner-tool-manifest.json",
    "ssh_keygen": "ssh-keygen",
}
_EXECUTABLE_INPUTS = {"bash", "git", "python", "ssh_keygen"}
_IDENTITY_ARCHIVE_NAMES = {
    "cargo_lock": "identity-Cargo.lock",
    "git": "identity-git",
    "raw_commit": "identity-raw-commit",
    "ssh_allowed_signers": "identity-allowed-signers",
    "ssh_keygen": "identity-ssh-keygen",
    "ssh_revocation": "identity-revocation",
    "verify_transcript": "identity-transcript.json",
}
_IDENTITY_RECORD_NAMES = {
    "identity_attestation": "identity-attestation.json",
    "identity_transcript": "identity-transcript.json",
    **_IDENTITY_ARCHIVE_NAMES,
}
_DATA_MODE = 0o400
_TOOL_MODE = 0o500
_DIRECTORY_MODE = 0o700
_MAX_IDENTITY_BYTES = 64 * 1024
_MAX_HELPER_BYTES = 16 * 1024 * 1024
_MAX_POLICY_BYTES = 16 * 1024 * 1024
_MAX_EVIDENCE_BYTES = 128 * 1024 * 1024
_MAX_TOOL_BYTES = 512 * 1024 * 1024
_MAX_HELPER_OUTPUT_BYTES = 16 * 1024 * 1024
_COMMAND_TIMEOUT_SECONDS = 120
_RUNNER_EXTRA_ENV = {
    "CARGO_HOME",
    "CARGO_NET_GIT_FETCH_WITH_CLI",
    "CARGO_NET_OFFLINE",
    "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
    "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST",
    "IROHA_RELEASE_SCALING_IROHAD_SHA256",
    "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
    "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
    "NIX_SSL_CERT_FILE",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "SSL_CERT_FILE",
}
_BASE_ENVIRONMENT = {
    "GIT_CONFIG_COUNT": "2",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_KEY_0": "core.hooksPath",
    "GIT_CONFIG_KEY_1": "core.fsmonitor",
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_CONFIG_VALUE_0": os.devnull,
    "GIT_CONFIG_VALUE_1": "false",
    "GIT_TERMINAL_PROMPT": "0",
    "LANG": "C",
    "LC_ALL": "C",
    "TZ": "UTC",
}
_BOOTSTRAP_ALIAS_SUFFIXES = (
    "COMPLETION",
    "IDENTITY_ATTESTATION",
    "IDENTITY_TRANSCRIPT",
    "IDENTITY",
    "EVIDENCE_DIR",
)
_SELF_DIGEST_VARIABLES = [
    "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
    "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
]
_TRAILER_VERSION = "Sumeragi-V2-Release-Identity-Version"
_TRAILER_MANIFEST = "Sumeragi-V2-Source-Manifest-SHA256"
_TRAILER_LOCK = "Sumeragi-V2-Cargo-Lock-SHA256"
_SSH_BEGIN = b"-----BEGIN SSH SIGNATURE-----"
_SSH_END = b"-----END SSH SIGNATURE-----"


class ValidationError(RuntimeError):
    """The bootstrap completion contract failed closed."""


@dataclass(frozen=True)
class Snapshot:
    """Bytes and inode metadata captured from one stable regular file."""

    path: Path
    data: bytes
    device: int
    inode: int
    mode: int
    uid: int
    links: int
    size: int
    mtime_ns: int
    ctime_ns: int

    @property
    def sha256(self) -> str:
        """Return the SHA-256 digest of the captured bytes."""

        return hashlib.sha256(self.data).hexdigest()


@dataclass(frozen=True)
class DirectorySnapshot:
    """An open directory and its stable pathname identity."""

    path: Path
    descriptor: int
    device: int
    inode: int
    mode: int
    uid: int


@dataclass(frozen=True)
class AliasSnapshot:
    """Stable private symlink alias for one authenticated runner tool."""

    path: Path
    target: str
    device: int
    inode: int
    uid: int
    links: int
    mtime_ns: int
    ctime_ns: int


def _runner_alias(path: Path, target: Path, label: str) -> AliasSnapshot:
    metadata = path.lstat()
    if (
        not stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_nlink != 1
        or os.readlink(path) != str(target)
        or path.resolve(strict=True) != target
    ):
        raise ValidationError(f"{label} is not an exact protected alias")
    return AliasSnapshot(
        path=path,
        target=str(target),
        device=metadata.st_dev,
        inode=metadata.st_ino,
        uid=metadata.st_uid,
        links=metadata.st_nlink,
        mtime_ns=metadata.st_mtime_ns,
        ctime_ns=metadata.st_ctime_ns,
    )


@dataclass(frozen=True)
class CommandResult:
    """One bounded child-process result."""

    returncode: int
    stdout: bytes
    stderr: bytes


def _canonical_json(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def _exact_dict(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ValidationError(f"{label} has the wrong exact schema")
    return value


def _strict_int(value: Any, label: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum:
        raise ValidationError(f"{label} must be an integer of at least {minimum}")
    return value


def _string(value: Any, label: str, *, nonempty: bool = True) -> str:
    if not isinstance(value, str) or (nonempty and not value) or "\0" in value:
        raise ValidationError(f"{label} must be a valid string")
    return value


def _digest(value: Any, label: str) -> str:
    value = _string(value, label)
    if _DIGEST_RE.fullmatch(value) is None:
        raise ValidationError(f"{label} must be one lowercase SHA-256 digest")
    return value


def _mode(value: Any, label: str) -> int:
    value = _string(value, label)
    if _MODE_RE.fullmatch(value) is None:
        raise ValidationError(f"{label} must be a four-digit octal mode")
    return int(value, 8)


def _canonical_existing(path: Path, label: str) -> Path:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        raise ValidationError(f"{label} must be an absolute normalized path")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise ValidationError(f"{label} is unavailable") from error
    if path != resolved:
        raise ValidationError(f"{label} must not contain symlinks")
    return path


def _inside(path: Path, root: Path) -> bool:
    return path == root or root in path.parents


def _open_directory(
    path: Path,
    label: str,
    *,
    expected_mode: int | None = None,
    require_private_parent: bool = False,
) -> DirectorySnapshot:
    path = _canonical_existing(path, label)
    if require_private_parent:
        parent = _canonical_existing(path.parent, f"{label} parent")
        parent_stat = parent.lstat()
        if (
            not stat.S_ISDIR(parent_stat.st_mode)
            or parent_stat.st_uid != os.getuid()
            or stat.S_IMODE(parent_stat.st_mode) != _DIRECTORY_MODE
        ):
            raise ValidationError(f"{label} parent must be owner-owned mode 0700")
    before = path.lstat()
    if not stat.S_ISDIR(before.st_mode) or before.st_uid != os.getuid():
        raise ValidationError(f"{label} must be an owner-owned directory")
    if expected_mode is not None and stat.S_IMODE(before.st_mode) != expected_mode:
        raise ValidationError(f"{label} must have exact mode {expected_mode:04o}")
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ValidationError(f"{label} could not be opened safely") from error
    opened = os.fstat(descriptor)
    if (
        not stat.S_ISDIR(opened.st_mode)
        or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
        or opened.st_uid != before.st_uid
        or stat.S_IMODE(opened.st_mode) != stat.S_IMODE(before.st_mode)
    ):
        os.close(descriptor)
        raise ValidationError(f"{label} changed while it was opened")
    return DirectorySnapshot(
        path,
        descriptor,
        opened.st_dev,
        opened.st_ino,
        stat.S_IMODE(opened.st_mode),
        opened.st_uid,
    )


def _read_descriptor(
    descriptor: int,
    path: Path,
    label: str,
    *,
    maximum_bytes: int,
    expected_mode: int | None,
    before: os.stat_result,
    allowed_uids: frozenset[int],
    expected_links: int | None,
) -> Snapshot:
    opened = os.fstat(descriptor)
    if (
        not stat.S_ISREG(opened.st_mode)
        or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
        or opened.st_uid not in allowed_uids
        or (expected_links is not None and opened.st_nlink != expected_links)
        or stat.S_IMODE(opened.st_mode) != stat.S_IMODE(before.st_mode)
    ):
        raise ValidationError(f"{label} does not satisfy its stable owner/link contract")
    if expected_mode is not None and stat.S_IMODE(opened.st_mode) != expected_mode:
        raise ValidationError(f"{label} must have exact mode {expected_mode:04o}")
    if opened.st_size > maximum_bytes:
        raise ValidationError(f"{label} exceeds its size limit")
    chunks: list[bytes] = []
    total = 0
    while True:
        chunk = os.read(descriptor, min(1024 * 1024, maximum_bytes + 1 - total))
        if not chunk:
            break
        chunks.append(chunk)
        total += len(chunk)
        if total > maximum_bytes:
            raise ValidationError(f"{label} exceeds its size limit")
    after = os.fstat(descriptor)
    stable = (
        opened.st_dev,
        opened.st_ino,
        opened.st_uid,
        opened.st_nlink,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        stat.S_IMODE(opened.st_mode),
    )
    if stable != (
        after.st_dev,
        after.st_ino,
        after.st_uid,
        after.st_nlink,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
        stat.S_IMODE(after.st_mode),
    ):
        raise ValidationError(f"{label} changed while it was read")
    return Snapshot(
        path,
        b"".join(chunks),
        opened.st_dev,
        opened.st_ino,
        stat.S_IMODE(opened.st_mode),
        opened.st_uid,
        opened.st_nlink,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
    )


def _read_at(
    directory: DirectorySnapshot,
    name: str,
    label: str,
    *,
    maximum_bytes: int,
    expected_mode: int | None = None,
    allowed_uids: frozenset[int] | None = None,
    expected_links: int | None = 1,
) -> Snapshot:
    if not name or name in {".", ".."} or "/" in name or "\0" in name:
        raise ValidationError(f"{label} has an unsafe archive name")
    try:
        before = os.stat(name, dir_fd=directory.descriptor, follow_symlinks=False)
    except OSError as error:
        raise ValidationError(f"{label} is unavailable") from error
    if not stat.S_ISREG(before.st_mode):
        raise ValidationError(f"{label} must be a non-symlink regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(name, flags, dir_fd=directory.descriptor)
    except OSError as error:
        raise ValidationError(f"{label} could not be opened safely") from error
    try:
        return _read_descriptor(
            descriptor,
            directory.path / name,
            label,
            maximum_bytes=maximum_bytes,
            expected_mode=expected_mode,
            before=before,
            allowed_uids=allowed_uids or frozenset({os.getuid()}),
            expected_links=expected_links,
        )
    finally:
        os.close(descriptor)


def _read_path(
    path: Path,
    label: str,
    *,
    maximum_bytes: int,
    expected_mode: int | None = None,
    allowed_uids: frozenset[int] | None = None,
    expected_links: int | None = 1,
) -> Snapshot:
    path = _canonical_existing(path, label)
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode):
        raise ValidationError(f"{label} must be a non-symlink regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ValidationError(f"{label} could not be opened safely") from error
    try:
        return _read_descriptor(
            descriptor,
            path,
            label,
            maximum_bytes=maximum_bytes,
            expected_mode=expected_mode,
            before=before,
            allowed_uids=allowed_uids or frozenset({os.getuid()}),
            expected_links=expected_links,
        )
    finally:
        os.close(descriptor)


def _revalidate_snapshot(
    snapshot: Snapshot,
    label: str,
    maximum_bytes: int,
    *,
    external_source: bool = False,
) -> None:
    current = _read_path(
        snapshot.path,
        label,
        maximum_bytes=maximum_bytes,
        expected_mode=snapshot.mode,
        allowed_uids=(
            frozenset({0, os.getuid()})
            if external_source
            else frozenset({os.getuid()})
        ),
        expected_links=snapshot.links if external_source else 1,
    )
    if current != snapshot:
        raise ValidationError(f"{label} changed during bootstrap validation")


def _revalidate_directory(directory: DirectorySnapshot, label: str) -> None:
    opened = os.fstat(directory.descriptor)
    pathname = directory.path.lstat()
    expected = (directory.device, directory.inode, directory.uid, directory.mode)
    for observed in (opened, pathname):
        if not stat.S_ISDIR(observed.st_mode) or (
            observed.st_dev,
            observed.st_ino,
            observed.st_uid,
            stat.S_IMODE(observed.st_mode),
        ) != expected:
            raise ValidationError(f"{label} changed during bootstrap validation")


def _paired_environment(suffix: str) -> str:
    sumeragi = f"SUMERAGI_V2_RELEASE_BOOTSTRAP_{suffix}"
    iroha = f"IROHA_RELEASE_BOOTSTRAP_{suffix}"
    left = os.environ.get(sumeragi)
    right = os.environ.get(iroha)
    if not left or left != right:
        raise ValidationError(f"bootstrap path aliases for {suffix} are absent or unequal")
    return left


def _completion_digest_environment() -> str:
    values = [os.environ.get(name) for name in _SELF_DIGEST_VARIABLES]
    if values[0] is None or values[0] != values[1]:
        raise ValidationError("bootstrap completion digest aliases are absent or unequal")
    return _digest(values[0], "out-of-band bootstrap completion digest")


def _parse_canonical(snapshot: Snapshot, label: str) -> dict[str, Any]:
    try:
        value = json.loads(snapshot.data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValidationError(f"{label} is not valid UTF-8 JSON") from error
    if not isinstance(value, dict) or snapshot.data != _canonical_json(value):
        raise ValidationError(f"{label} must be one canonical JSON object")
    return value


def _load_identity(value: Any, label: str) -> dict[str, Any]:
    identity = _exact_dict(value, _IDENTITY_KEYS, label)
    if _strict_int(identity["schema_version"], f"{label} schema_version") != 1:
        raise ValidationError(f"{label} must use schema version 1")
    lengths: set[int] = set()
    for name in ("head_commit", "head_tree", "index_tree"):
        field = _string(identity[name], f"{label} {name}")
        if _OBJECT_ID_RE.fullmatch(field) is None:
            raise ValidationError(f"{label} has an invalid {name}")
        lengths.add(len(field))
    if len(lengths) != 1 or identity["head_tree"] != identity["index_tree"]:
        raise ValidationError(f"{label} does not bind one clean Git object format/tree")
    _digest(identity["workspace_source_manifest_sha256"], f"{label} source manifest")
    _digest(identity["cargo_lock_sha256"], f"{label} Cargo.lock")
    return identity


def _artifact_limit(label: str) -> int:
    if label in _EXECUTABLE_INPUTS:
        return _MAX_TOOL_BYTES
    if label in {"allowed_signers", "revocation"}:
        return _MAX_POLICY_BYTES
    return _MAX_HELPER_BYTES


def _trusted_inputs(
    value: Any,
    evidence: DirectorySnapshot,
    candidate: Path,
) -> tuple[dict[str, Snapshot], list[Snapshot]]:
    records = _exact_dict(value, _TRUSTED_INPUT_KEYS, "trusted_inputs")
    archives: dict[str, Snapshot] = {}
    sources: list[Snapshot] = []
    record_keys = {
        "archive_name",
        "archive_mode",
        "observed_sha256",
        "protected_sha256",
        "size_bytes",
        "source_mode",
        "source_path",
    }
    for label in sorted(records):
        record = _exact_dict(records[label], record_keys, f"trusted input {label}")
        if record["archive_name"] != _TRUSTED_ARCHIVE_NAMES[label]:
            raise ValidationError(f"trusted input {label} has the wrong archive alias")
        expected_mode = _TOOL_MODE if label in _EXECUTABLE_INPUTS else _DATA_MODE
        if _mode(record["archive_mode"], f"trusted input {label} archive mode") != expected_mode:
            raise ValidationError(f"trusted input {label} has the wrong archive mode")
        source_mode = _mode(record["source_mode"], f"trusted input {label} source mode")
        if label in _EXECUTABLE_INPUTS and source_mode & 0o111 == 0:
            raise ValidationError(f"trusted executable source {label} is not executable")
        size = _strict_int(record["size_bytes"], f"trusted input {label} size")
        observed = _digest(record["observed_sha256"], f"trusted input {label} digest")
        protected = _digest(record["protected_sha256"], f"trusted input {label} protected digest")
        if observed != protected:
            raise ValidationError(f"trusted input {label} digests disagree")
        archive = _read_at(
            evidence,
            _TRUSTED_ARCHIVE_NAMES[label],
            f"archived trusted input {label}",
            maximum_bytes=_artifact_limit(label),
            expected_mode=expected_mode,
        )
        if len(archive.data) != size or archive.sha256 != observed:
            raise ValidationError(f"archived trusted input {label} does not match its marker")
        source_path = _canonical_existing(
            Path(_string(record["source_path"], f"trusted input {label} source path")),
            f"trusted input {label} source path",
        )
        if _inside(source_path, candidate) or _inside(source_path, evidence.path):
            raise ValidationError(f"trusted input {label} source entered an untrusted root")
        source = _read_path(
            source_path,
            f"trusted input {label} source",
            maximum_bytes=_artifact_limit(label),
            expected_mode=source_mode,
            allowed_uids=frozenset({0, os.getuid()}),
            expected_links=None,
        )
        if len(source.data) != size or source.sha256 != protected:
            raise ValidationError(f"trusted input {label} source changed after bootstrap")
        archives[label] = archive
        sources.append(source)
    return archives, sources


def _identity_records(
    value: Any,
    evidence: DirectorySnapshot,
) -> dict[str, Snapshot]:
    records = _exact_dict(value, set(_IDENTITY_RECORD_NAMES), "identity_verification")
    snapshots: dict[str, Snapshot] = {}
    record_keys = {"archive_name", "mode", "sha256", "size_bytes"}
    by_name: dict[str, Snapshot] = {}
    for label in sorted(records):
        record = _exact_dict(records[label], record_keys, f"identity record {label}")
        expected_name = _IDENTITY_RECORD_NAMES[label]
        if record["archive_name"] != expected_name:
            raise ValidationError(f"identity record {label} has the wrong archive name")
        expected_mode = _TOOL_MODE if label in {"git", "ssh_keygen"} else _DATA_MODE
        if _mode(record["mode"], f"identity record {label} mode") != expected_mode:
            raise ValidationError(f"identity record {label} has the wrong mode")
        size = _strict_int(record["size_bytes"], f"identity record {label} size")
        digest = _digest(record["sha256"], f"identity record {label} digest")
        snapshot = by_name.get(expected_name)
        if snapshot is None:
            snapshot = _read_at(
                evidence,
                expected_name,
                f"identity evidence {label}",
                maximum_bytes=_MAX_EVIDENCE_BYTES,
                expected_mode=expected_mode,
            )
            by_name[expected_name] = snapshot
        if len(snapshot.data) != size or snapshot.sha256 != digest:
            raise ValidationError(f"identity record {label} does not match its marker")
        snapshots[label] = snapshot
    if records["identity_transcript"] != records["verify_transcript"]:
        raise ValidationError("duplicate transcript marker records disagree")
    return snapshots


def _validate_artifact_record(
    value: Any,
    label: str,
    *,
    archive_name: str,
    expected_mode: int,
    snapshot: Snapshot,
    protected_digest: str | None = None,
) -> dict[str, Any]:
    keys = {"archive_name", "mode", "sha256", "size_bytes"}
    protected = protected_digest is not None
    if protected:
        keys = {"archive_name", "mode", "observed_sha256", "protected_sha256", "size_bytes"}
    record = _exact_dict(value, keys, label)
    if record["archive_name"] != archive_name or _mode(record["mode"], f"{label} mode") != expected_mode:
        raise ValidationError(f"{label} has the wrong archive binding")
    size = _strict_int(record["size_bytes"], f"{label} size")
    if protected:
        observed = _digest(record["observed_sha256"], f"{label} observed digest")
        pinned = _digest(record["protected_sha256"], f"{label} protected digest")
        if observed != pinned or pinned != protected_digest:
            raise ValidationError(f"{label} protected digests disagree")
        digest = observed
    else:
        digest = _digest(record["sha256"], f"{label} digest")
    if snapshot.mode != expected_mode or len(snapshot.data) != size or snapshot.sha256 != digest:
        raise ValidationError(f"{label} does not match its stable evidence")
    return record


def _command_record(value: Any, label: str, *, success: bool) -> dict[str, Any]:
    keys = {
        "argv",
        "replay_argv",
        "exit_status",
        "stdout_base64",
        "stdout_sha256",
        "stdout_size_bytes",
        "stderr_base64",
        "stderr_sha256",
        "stderr_size_bytes",
    }
    record = _exact_dict(value, keys, label)
    for key in ("argv", "replay_argv"):
        arguments = record[key]
        if not isinstance(arguments, list) or not arguments or not all(
            isinstance(argument, str) and "\0" not in argument for argument in arguments
        ):
            raise ValidationError(f"{label} has invalid {key}")
    status = _strict_int(record["exit_status"], f"{label} exit status")
    if success and status != 0:
        raise ValidationError(f"{label} did not succeed")
    for stream in ("stdout", "stderr"):
        encoded = _string(record[f"{stream}_base64"], f"{label} {stream}", nonempty=False)
        digest = _digest(record[f"{stream}_sha256"], f"{label} {stream} digest")
        size = _strict_int(record[f"{stream}_size_bytes"], f"{label} {stream} size")
        try:
            data = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error) as error:
            raise ValidationError(f"{label} has malformed {stream} evidence") from error
        if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
            raise ValidationError(f"{label} has inconsistent {stream} evidence")
    return record


def _validate_raw_commit(raw: bytes, identity: dict[str, Any]) -> None:
    headers, separator, message = raw.partition(b"\n\n")
    if not separator or b"\r" in headers or b"\0" in headers:
        raise ValidationError("identity raw commit has malformed headers")
    records: list[tuple[bytes, list[bytes]]] = []
    for line in headers.split(b"\n"):
        if line.startswith(b" "):
            if not records:
                raise ValidationError("identity raw commit has an orphan folded header")
            records[-1][1].append(line[1:])
            continue
        key, marker, field = line.partition(b" ")
        if not marker or not key or any(byte < 0x21 or byte > 0x7E for byte in key):
            raise ValidationError("identity raw commit has a malformed header")
        records.append((key, [field]))
    trees = [values for key, values in records if key == b"tree"]
    if trees != [[identity["head_tree"].encode("ascii")]]:
        raise ValidationError("identity raw commit tree does not match the candidate")
    signatures = [values for key, values in records if key.startswith(b"gpgsig")]
    if len(signatures) != 1 or not any(key == b"gpgsig" for key, _ in records):
        raise ValidationError("identity raw commit must contain exactly one SSH signature")
    signature = b"\n".join(signatures[0])
    lines = signature.split(b"\n")
    if len(lines) < 3 or lines[0] != _SSH_BEGIN or lines[-1] != _SSH_END:
        raise ValidationError("identity raw commit has invalid SSH signature armor")
    try:
        if not base64.b64decode(b"".join(lines[1:-1]), validate=True):
            raise ValueError
    except (ValueError, binascii.Error) as error:
        raise ValidationError("identity raw commit has malformed SSH signature data") from error
    if b"\r" in message or b"\0" in message:
        raise ValidationError("identity raw commit has a malformed LF-only message")
    try:
        text = message.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValidationError("identity raw commit message is not UTF-8") from error
    expected = [
        f"{_TRAILER_VERSION}: 1",
        f"{_TRAILER_MANIFEST}: {identity['workspace_source_manifest_sha256']}",
        f"{_TRAILER_LOCK}: {identity['cargo_lock_sha256']}",
    ]
    lines_text = text[:-1].split("\n") if text.endswith("\n") else []
    trailer_keys = {_TRAILER_VERSION.casefold(), _TRAILER_MANIFEST.casefold(), _TRAILER_LOCK.casefold()}
    recognized = [
        index
        for index, line in enumerate(lines_text)
        if ":" in line and line.partition(":")[0].casefold() in trailer_keys
    ]
    terminal = list(range(len(lines_text) - 3, len(lines_text)))
    if (
        len(lines_text) < 5
        or lines_text[-4] != ""
        or not lines_text[-5]
        or lines_text[-3:] != expected
        or recognized != terminal
    ):
        raise ValidationError("identity raw commit has the wrong release trailer block")
    framed = b"commit " + str(len(raw)).encode("ascii") + b"\0" + raw
    observed_oid = (
        hashlib.sha1(framed, usedforsecurity=False).hexdigest()
        if len(identity["head_commit"]) == 40
        else hashlib.sha256(framed).hexdigest()
    )
    if observed_oid != identity["head_commit"]:
        raise ValidationError("identity raw commit bytes do not reproduce HEAD")


def _validate_identity_semantics(
    snapshots: dict[str, Snapshot],
    identity: dict[str, Any],
    identity_bytes: bytes,
    trusted: dict[str, Snapshot],
) -> str:
    attestation = _parse_canonical(snapshots["identity_attestation"], "identity attestation")
    transcript = _parse_canonical(snapshots["identity_transcript"], "identity transcript")
    _exact_dict(
        attestation,
        {"schema_version", "release_identity", "release_identity_sha256", "tools", "policies", "verification", "evidence"},
        "identity attestation",
    )
    _exact_dict(
        transcript,
        {"schema_version", "archive_names", "candidate_commit_oid", "environment", "policy_overrides", "policies", "replay", "tools", "commands", "tool_probes"},
        "identity transcript",
    )
    if _strict_int(attestation["schema_version"], "identity attestation schema") != 2:
        raise ValidationError("identity attestation must use schema 2")
    if _strict_int(transcript["schema_version"], "identity transcript schema") != 2:
        raise ValidationError("identity transcript must use schema 2")
    if attestation["release_identity"] != identity or attestation["release_identity_sha256"] != hashlib.sha256(identity_bytes).hexdigest():
        raise ValidationError("identity attestation does not bind the candidate identity")
    tools = _exact_dict(attestation["tools"], {"git", "ssh_keygen"}, "identity tools")
    tool_keys = {"archive_name", "mode", "observed_sha256", "protected_sha256", "size_bytes", "source_path"}
    for label, record_label, trusted_label in (
        ("git", "git", "git"),
        ("ssh_keygen", "ssh_keygen", "ssh_keygen"),
    ):
        record = _exact_dict(tools[label], tool_keys, f"identity tool {label}")
        _string(record["source_path"], f"identity tool {label} source path")
        compact = {key: value for key, value in record.items() if key != "source_path"}
        _validate_artifact_record(
            compact,
            f"identity tool {label}",
            archive_name=_IDENTITY_ARCHIVE_NAMES[record_label],
            expected_mode=_TOOL_MODE,
            snapshot=snapshots[record_label],
            protected_digest=trusted[trusted_label].sha256,
        )
    policies = _exact_dict(
        attestation["policies"],
        {"expected_signer_fingerprint", "signature_format", "ssh_allowed_signers", "ssh_revocation"},
        "identity policies",
    )
    fingerprint = _string(policies["expected_signer_fingerprint"], "expected signer fingerprint")
    if _FINGERPRINT_RE.fullmatch(fingerprint) is None or policies["signature_format"] != "ssh":
        raise ValidationError("identity policies do not bind first-release SSH verification")
    for label, trusted_label in (("ssh_allowed_signers", "allowed_signers"), ("ssh_revocation", "revocation")):
        _validate_artifact_record(
            policies[label],
            f"identity policy {label}",
            archive_name=_IDENTITY_ARCHIVE_NAMES[label],
            expected_mode=_DATA_MODE,
            snapshot=snapshots[label],
            protected_digest=trusted[trusted_label].sha256,
        )
    verification = _exact_dict(
        attestation["verification"],
        {"status", "signer_fingerprint", "primary_key_fingerprint", "allowed_signers_principal"},
        "identity verification result",
    )
    if (
        verification["status"] != "G"
        or verification["signer_fingerprint"] != fingerprint
        or verification["primary_key_fingerprint"] != ""
        or not isinstance(verification["allowed_signers_principal"], str)
        or not verification["allowed_signers_principal"]
    ):
        raise ValidationError("identity verification result is not one good SSH signature")
    evidence = _exact_dict(attestation["evidence"], set(_IDENTITY_ARCHIVE_NAMES), "identity evidence inventory")
    for label in sorted(evidence):
        expected_mode = _TOOL_MODE if label in {"git", "ssh_keygen"} else _DATA_MODE
        _validate_artifact_record(
            evidence[label],
            f"identity evidence {label}",
            archive_name=_IDENTITY_ARCHIVE_NAMES[label],
            expected_mode=expected_mode,
            snapshot=snapshots[label],
        )
    if transcript["archive_names"] != _IDENTITY_ARCHIVE_NAMES:
        raise ValidationError("identity transcript has the wrong archive mapping")
    if transcript["candidate_commit_oid"] != identity["head_commit"]:
        raise ValidationError("identity transcript has the wrong candidate OID")
    if transcript["tools"] != tools or transcript["policies"] != policies:
        raise ValidationError("identity transcript disagrees with the attestation")
    environment = transcript["environment"]
    if not isinstance(environment, dict) or not environment or not all(
        isinstance(key, str) and isinstance(value, str) for key, value in environment.items()
    ):
        raise ValidationError("identity transcript has an invalid closed environment")
    overrides = transcript["policy_overrides"]
    if not isinstance(overrides, list) or not overrides or not all(isinstance(item, str) for item in overrides):
        raise ValidationError("identity transcript has invalid policy overrides")
    replay = _exact_dict(
        transcript["replay"],
        {"candidate_root", "evidence_directory", "environment", "policy_overrides"},
        "identity replay",
    )
    if replay["candidate_root"] != "${CANDIDATE_ROOT}" or replay["evidence_directory"] != "${EVIDENCE_DIRECTORY}":
        raise ValidationError("identity replay has invalid path placeholders")
    if not isinstance(replay["environment"], dict) or not all(
        isinstance(key, str) and isinstance(value, str) for key, value in replay["environment"].items()
    ):
        raise ValidationError("identity replay has an invalid environment")
    if not isinstance(replay["policy_overrides"], list) or not all(isinstance(item, str) for item in replay["policy_overrides"]):
        raise ValidationError("identity replay has invalid policy overrides")
    commands = _exact_dict(transcript["commands"], {"show_signature_metadata", "verify_commit"}, "identity commands")
    show = _command_record(commands["show_signature_metadata"], "show-signature command", success=True)
    _command_record(commands["verify_commit"], "verify-commit command", success=True)
    probes = _exact_dict(transcript["tool_probes"], {"ssh_keygen_usage"}, "identity tool probes")
    _command_record(probes["ssh_keygen_usage"], "ssh-keygen probe", success=False)
    metadata = base64.b64decode(show["stdout_base64"], validate=True)
    expected_metadata = (
        "G\0"
        + fingerprint
        + "\0\0"
        + verification["allowed_signers_principal"]
        + "\0\n"
    ).encode()
    if metadata != expected_metadata:
        raise ValidationError("signature transcript metadata disagrees with its attestation")
    _validate_raw_commit(snapshots["raw_commit"].data, identity)
    try:
        policy_text = snapshots["ssh_allowed_signers"].data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValidationError("allowed-signers policy is not UTF-8") from error
    active_policy = [
        line for line in policy_text.splitlines() if line and not line.startswith("#")
    ]
    folded_policy = "\n".join(active_policy).casefold()
    if (
        not policy_text.endswith("\n")
        or "\r" in policy_text
        or "\0" in policy_text
        or len(active_policy) != 1
        or "cert-authority" in folded_policy
        or "-cert-v01@openssh.com" in folded_policy
        or "valid-after=" in folded_policy
        or "valid-before=" in folded_policy
    ):
        raise ValidationError("allowed-signers policy is not accepted first-release policy")
    return fingerprint


def _abort(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except (OSError, ProcessLookupError):
        try:
            process.kill()
        except OSError:
            pass
    try:
        process.wait(timeout=5)
    except (OSError, subprocess.TimeoutExpired):
        pass


def _run_bounded(
    executable: Path,
    arguments: Iterable[str],
    *,
    cwd: Path,
    environment: dict[str, str],
) -> CommandResult:
    try:
        process = subprocess.Popen(
            [str(executable), *arguments],
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
    except OSError as error:
        raise ValidationError("trusted manifest helper could not execute") from error
    assert process.stdout is not None and process.stderr is not None
    os.set_blocking(process.stdout.fileno(), False)
    os.set_blocking(process.stderr.fileno(), False)
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    total = 0
    deadline = time.monotonic() + _COMMAND_TIMEOUT_SECONDS
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError
            for key, _ in selector.select(min(remaining, 1.0)):
                try:
                    chunk = os.read(key.fileobj.fileno(), 64 * 1024)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                total += len(chunk)
                if total > _MAX_HELPER_OUTPUT_BYTES:
                    raise ValidationError("trusted manifest helper exceeded its output limit")
                buffers[key.data].extend(chunk)
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError
        returncode = process.wait(timeout=remaining)
    except TimeoutError as error:
        _abort(process)
        raise ValidationError("trusted manifest helper exceeded its runtime limit") from error
    except BaseException:
        _abort(process)
        raise
    finally:
        selector.close()
        process.stdout.close()
        process.stderr.close()
    return CommandResult(returncode, bytes(buffers["stdout"]), bytes(buffers["stderr"]))


def _runner_contract(
    value: Any,
    evidence: DirectorySnapshot,
    candidate: Path,
    runner_argument: Path,
    archives: dict[str, Snapshot],
) -> tuple[Snapshot, dict[str, str], Path, list[AliasSnapshot], list[Snapshot]]:
    keys = {
        "argv",
        "closed_path_resolution",
        "environment_without_self_digest",
        "mode",
        "output",
        "path",
        "self_digest_environment_variables",
        "sha256",
        "size_bytes",
        "tool_directory",
        "tools",
    }
    runner = _exact_dict(value, keys, "runner contract")
    expected_runner = candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"
    runner_path = _canonical_existing(Path(_string(runner["path"], "runner path")), "runner path")
    runner_argument = _canonical_existing(runner_argument, "current runner path")
    if runner_path != expected_runner or runner_argument != runner_path:
        raise ValidationError("current runner path does not match the bootstrap candidate runner")
    runner_mode = _mode(runner["mode"], "runner mode")
    snapshot = _read_path(
        runner_path,
        "signed candidate runner",
        maximum_bytes=_MAX_HELPER_BYTES,
        expected_mode=runner_mode,
    )
    if (
        snapshot.sha256 != _digest(runner["sha256"], "runner digest")
        or len(snapshot.data) != _strict_int(runner["size_bytes"], "runner size")
    ):
        raise ValidationError("current runner bytes do not match the bootstrap marker")
    expected_argv = [str(archives["bash"].path), str(runner_path), "--release"]
    if runner["argv"] != expected_argv:
        raise ValidationError("bootstrap runner argv is not the exact release invocation")
    resolutions = _exact_dict(
        runner["closed_path_resolution"], {"bash", "git", "python3"}, "closed PATH resolutions"
    )
    expected_resolutions = {
        "bash": str(archives["bash"].path),
        "git": str(archives["git"].path),
        "python3": str(archives["python"].path),
    }
    if resolutions != expected_resolutions:
        raise ValidationError("closed PATH resolutions do not bind the trusted aliases")
    output = _exact_dict(
        runner["output"],
        {"stderr_path", "stdout_path", "active_mode", "sealed_mode"},
        "runner output contract",
    )
    if output != {
        "stderr_path": str(evidence.path / "runner-stderr.log"),
        "stdout_path": str(evidence.path / "runner-stdout.log"),
        "active_mode": "0600",
        "sealed_mode": "0400",
    }:
        raise ValidationError("runner output contract is not exact")
    for name in ("runner-stdout.log", "runner-stderr.log"):
        metadata = os.stat(name, dir_fd=evidence.descriptor, follow_symlinks=False)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.getuid()
            or metadata.st_nlink != 1
            or stat.S_IMODE(metadata.st_mode) != 0o600
        ):
            raise ValidationError("active runner log metadata is not exact")

    tool_directory_path = _canonical_existing(
        Path(_string(runner["tool_directory"], "runner tool directory")),
        "runner tool directory",
    )
    if tool_directory_path != evidence.path / "runner-bin":
        raise ValidationError("runner tool directory is not the private archive")
    tool_directory = _open_directory(
        tool_directory_path,
        "runner tool directory",
        expected_mode=_DIRECTORY_MODE,
    )
    tool_aliases: list[AliasSnapshot] = []
    tool_sources: list[Snapshot] = []
    try:
        manifest_snapshot = archives["runner_tool_manifest"]
        manifest = _exact_dict(
            _parse_canonical(manifest_snapshot, "runner tool manifest"),
            {"schema_version", "tools"},
            "runner tool manifest",
        )
        manifest_tools = manifest["tools"]
        runner_tools = runner["tools"]
        if (
            _strict_int(manifest["schema_version"], "runner tool schema") != 1
            or not isinstance(manifest_tools, dict)
            or not manifest_tools
            or len(manifest_tools) > 256
            or not isinstance(runner_tools, dict)
            or set(runner_tools) != set(manifest_tools)
        ):
            raise ValidationError("runner tool inventory is not exact")
        observed_names = set(os.listdir(tool_directory.descriptor))
        if observed_names != set(manifest_tools):
            raise ValidationError("runner tool directory inventory is not exact")
        for name in sorted(manifest_tools):
            if (
                re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", name) is None
                or name in {"bash", "git", "python3", "ssh-keygen"}
            ):
                raise ValidationError("runner tool alias is unsafe")
            manifest_record = _exact_dict(
                manifest_tools[name], {"path", "sha256"}, f"runner manifest tool {name}"
            )
            marker_record = _exact_dict(
                runner_tools[name],
                {
                    "alias_name",
                    "alias_path",
                    "sha256",
                    "size_bytes",
                    "source_mode",
                    "source_path",
                },
                f"runner tool {name}",
            )
            source_path = _canonical_existing(
                Path(_string(marker_record["source_path"], f"runner tool {name} path")),
                f"runner tool {name} path",
            )
            if (
                manifest_record.get("path") != str(source_path)
                or _inside(source_path, candidate)
                or _inside(source_path, evidence.path)
            ):
                raise ValidationError(f"runner tool {name} source is outside its policy")
            source_mode = _mode(
                marker_record["source_mode"], f"runner tool {name} source mode"
            )
            source = _read_path(
                source_path,
                f"runner tool source {name}",
                maximum_bytes=_MAX_TOOL_BYTES,
                expected_mode=source_mode,
                allowed_uids=frozenset({0, os.getuid()}),
                expected_links=None,
            )
            alias_path = tool_directory_path / name
            if (
                marker_record["alias_name"] != name
                or marker_record["alias_path"] != str(alias_path)
            ):
                raise ValidationError(f"runner tool {name} alias record is wrong")
            alias = _runner_alias(
                alias_path, source.path, f"runner tool alias {name}"
            )
            if source.mode & 0o022:
                raise ValidationError(f"runner tool {name} source is writable")
            for ancestor in (source.path.parent, *source.path.parent.parents):
                metadata = ancestor.lstat()
                if (
                    stat.S_ISLNK(metadata.st_mode)
                    or not stat.S_ISDIR(metadata.st_mode)
                    or metadata.st_uid not in {0, os.getuid()}
                    or stat.S_IMODE(metadata.st_mode) & 0o022
                ):
                    raise ValidationError(f"runner tool {name} has an unsafe ancestor")
            expected_digest = _digest(
                manifest_record.get("sha256"), f"runner tool {name} manifest digest"
            )
            if (
                expected_digest != source.sha256
                or marker_record
                != {
                    "alias_name": name,
                    "alias_path": str(alias_path),
                    "sha256": source.sha256,
                    "size_bytes": len(source.data),
                    "source_mode": f"{source.mode:04o}",
                    "source_path": str(source.path),
                }
            ):
                raise ValidationError(f"runner tool {name} integrity record is wrong")
            tool_aliases.append(alias)
            tool_sources.append(source)
    finally:
        os.close(tool_directory.descriptor)
    if runner["self_digest_environment_variables"] != _SELF_DIGEST_VARIABLES:
        raise ValidationError("runner self-digest variables have the wrong exact contract")
    environment = runner["environment_without_self_digest"]
    if not isinstance(environment, dict) or not all(
        isinstance(key, str)
        and _ENV_NAME_RE.fullmatch(key) is not None
        and isinstance(item, str)
        and "\0" not in item
        for key, item in environment.items()
    ):
        raise ValidationError("runner closed environment is malformed")
    return snapshot, environment, tool_directory_path, tool_aliases, tool_sources


def _environment_contract(
    environment: dict[str, str],
    evidence: DirectorySnapshot,
    candidate: Path,
    archives: dict[str, Snapshot],
    identity_records: dict[str, Snapshot],
    fingerprint: str,
    marker_digest: str,
    tool_directory: Path,
    checkpoint: str,
) -> None:
    expected = dict(_BASE_ENVIRONMENT)
    expected.update(
        {
            "HOME": str(evidence.path / "home"),
            "PATH": os.pathsep.join([str(evidence.path), str(tool_directory)]),
            "TMPDIR": str(evidence.path / "tmp"),
        }
    )
    policy = {
        "SUMERAGI_V2_RELEASE_SSH_KEYGEN_BIN": str(archives["ssh_keygen"].path),
        "SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256": archives["git"].sha256,
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256": archives["ssh_keygen"].sha256,
        "SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT": fingerprint,
        "SUMERAGI_V2_RELEASE_SSH_ALLOWED_SIGNERS": str(archives["allowed_signers"].path),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256": archives["allowed_signers"].sha256,
        "SUMERAGI_V2_RELEASE_SSH_REVOCATION_FILE": str(archives["revocation"].path),
        "SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256": archives["revocation"].sha256,
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION": str(evidence.path / "BOOTSTRAP_COMPLETED.json"),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION": str(identity_records["identity_attestation"].path),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT": str(identity_records["identity_transcript"].path),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY": str(evidence.path / "candidate-identity.json"),
        "SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR": str(evidence.path),
    }
    aliases = {
        key.replace("SUMERAGI_V2_RELEASE_", "IROHA_RELEASE_", 1): item
        for key, item in policy.items()
        if key.startswith("SUMERAGI_V2_RELEASE_BOOTSTRAP_")
    }
    expected.update(policy)
    expected.update(aliases)
    allowed_keys = set(expected) | _RUNNER_EXTRA_ENV
    if set(environment) - allowed_keys or not set(expected).issubset(environment):
        raise ValidationError("runner closed environment has unapproved or missing variables")
    for key, item in expected.items():
        if environment.get(key) != item:
            raise ValidationError(f"runner closed environment has the wrong {key}")
    if any(key in environment for key in _SELF_DIGEST_VARIABLES):
        raise ValidationError("runner environment embeds its circular marker digest")
    # Recheck the values supplied to this validator independently of the marker map.
    for suffix in _BOOTSTRAP_ALIAS_SUFFIXES:
        paired = _paired_environment(suffix)
        expected_value = aliases[f"IROHA_RELEASE_BOOTSTRAP_{suffix}"]
        if paired != expected_value:
            raise ValidationError(f"current bootstrap alias {suffix} disagrees with the marker")
    if _completion_digest_environment() != marker_digest:
        raise ValidationError("current completion digest aliases changed during validation")
    for key, item in policy.items():
        if os.environ.get(key) != item:
            raise ValidationError(f"current release policy variable {key} disagrees with the marker")
    if checkpoint == "entry":
        runtime = {
            "PWD": str(candidate),
            "SHLVL": "1",
            "_": str(archives["python"].path),
        }
        if sys.platform == "darwin":
            runtime["__CF_USER_TEXT_ENCODING"] = f"0x{os.geteuid():X}:0x1:0xE"
        current_expected = {**environment, **{name: marker_digest for name in _SELF_DIGEST_VARIABLES}, **runtime}
        if dict(os.environ) != current_expected:
            raise ValidationError("entry checkpoint does not have the exact bootstrap launch environment")
        for name, path in (
            ("bash", archives["bash"].path),
            ("git", archives["git"].path),
            ("python3", archives["python"].path),
        ):
            discovered = shutil.which(name, path=environment["PATH"])
            if discovered is None or Path(discovered).resolve(strict=True) != path:
                raise ValidationError(f"entry PATH does not resolve {name} to its archive")


def _inventory(evidence: DirectorySnapshot, checkpoint: str) -> None:
    required = {
        "BOOTSTRAP_COMPLETED.json",
        "candidate-identity.json",
        "home",
        "runner-bin",
        "runner-stderr.log",
        "runner-stdout.log",
        "tmp",
        *_TRUSTED_ARCHIVE_NAMES.values(),
        *set(_IDENTITY_RECORD_NAMES.values()),
    }
    try:
        observed = set(os.listdir(evidence.descriptor))
    except OSError as error:
        raise ValidationError("bootstrap evidence inventory is unavailable") from error
    permitted = set(required)
    if checkpoint == "sealed":
        permitted.add("release-runner")
    if observed != permitted:
        raise ValidationError("bootstrap evidence directory has an unexpected top-level inventory")
    for name in ("home", "tmp", "runner-bin"):
        item = os.stat(name, dir_fd=evidence.descriptor, follow_symlinks=False)
        if (
            not stat.S_ISDIR(item.st_mode)
            or item.st_uid != os.getuid()
            or stat.S_IMODE(item.st_mode) != _DIRECTORY_MODE
        ):
            raise ValidationError(f"bootstrap {name} directory is not private")
    for name in ("runner-stdout.log", "runner-stderr.log"):
        item = os.stat(name, dir_fd=evidence.descriptor, follow_symlinks=False)
        if (
            not stat.S_ISREG(item.st_mode)
            or item.st_uid != os.getuid()
            or item.st_nlink != 1
            or stat.S_IMODE(item.st_mode) != 0o600
        ):
            raise ValidationError(f"bootstrap {name} is not one active private log")
    if checkpoint == "sealed":
        item = os.stat("release-runner", dir_fd=evidence.descriptor, follow_symlinks=False)
        if (
            not stat.S_ISDIR(item.st_mode)
            or item.st_uid != os.getuid()
            or stat.S_IMODE(item.st_mode) != _DIRECTORY_MODE
        ):
            raise ValidationError("sealed release-runner subtree is not one private directory")


def validate(args: argparse.Namespace) -> None:
    if not sys.flags.isolated or not sys.flags.no_site:
        raise ValidationError("validator must run under archived Python with -I and -S")
    if os.getuid() != os.geteuid():
        raise ValidationError("set-ID execution is not supported")
    candidate = _canonical_existing(args.candidate_root, "current candidate root")
    candidate_directory = _open_directory(candidate, "current candidate root")
    evidence_path = _canonical_existing(Path(_paired_environment("EVIDENCE_DIR")), "bootstrap evidence directory")
    evidence = _open_directory(
        evidence_path,
        "bootstrap evidence directory",
        expected_mode=_DIRECTORY_MODE,
        require_private_parent=True,
    )
    snapshots: list[tuple[Snapshot, str, int]] = []
    try:
        completion = Path(_paired_environment("COMPLETION"))
        if completion != evidence.path / "BOOTSTRAP_COMPLETED.json":
            raise ValidationError("completion marker aliases do not name the evidence marker")
        marker = _read_at(
            evidence,
            "BOOTSTRAP_COMPLETED.json",
            "bootstrap completion marker",
            maximum_bytes=_MAX_EVIDENCE_BYTES,
            expected_mode=_DATA_MODE,
        )
        marker_digest = _completion_digest_environment()
        if marker.sha256 != marker_digest:
            raise ValidationError("completion marker does not match its out-of-band digest")
        marker_value = _exact_dict(_parse_canonical(marker, "bootstrap completion marker"), _MARKER_KEYS, "bootstrap completion marker")
        if _strict_int(marker_value["schema_version"], "bootstrap schema_version") != 1:
            raise ValidationError("bootstrap completion marker must use schema 1")
        if marker_value["trust_boundary"] != {
            "bootstrap_authentication": "external prerequisite",
            "release_image_and_dynamic_loader": "external prerequisite",
            "same_uid_and_trusted_ancestor_owners": True,
        }:
            raise ValidationError("bootstrap completion marker has the wrong trust boundary")
        if marker_value["candidate_root"] != str(candidate):
            raise ValidationError("current candidate root disagrees with the bootstrap marker")
        identity = _load_identity(marker_value["candidate_identity"], "marker candidate identity")
        identity_file = _read_at(
            evidence,
            "candidate-identity.json",
            "candidate identity evidence",
            maximum_bytes=_MAX_IDENTITY_BYTES,
            expected_mode=_DATA_MODE,
        )
        if identity_file.data != _canonical_json(identity) or identity_file.sha256 != _digest(
            marker_value["candidate_identity_sha256"], "candidate identity digest"
        ):
            raise ValidationError("candidate identity evidence disagrees with the marker")
        archives, source_snapshots = _trusted_inputs(marker_value["trusted_inputs"], evidence, candidate)
        resolved_python = Path(sys.executable).resolve(strict=True)
        if resolved_python != archives["python"].path:
            raise ValidationError("validator is not running under the archived Python")
        records = _identity_records(marker_value["identity_verification"], evidence)
        fingerprint = _validate_identity_semantics(records, identity, identity_file.data, archives)
        (
            runner,
            environment,
            tool_directory,
            runner_tool_aliases,
            runner_tool_sources,
        ) = _runner_contract(
            marker_value["runner"], evidence, candidate, args.runner, archives
        )
        probes = _exact_dict(marker_value["trusted_execution_probes"], {"bash", "python"}, "trusted execution probes")
        if probes != {
            "bash": {"argv": [str(archives["bash"].path), "-c", ":"], "exit_status": 0},
            "python": {
                "argv": [str(archives["python"].path), "-I", "-S", "-c", "raise SystemExit(0)"],
                "exit_status": 0,
            },
        }:
            raise ValidationError("trusted execution probes have the wrong exact contract")
        _environment_contract(
            environment,
            evidence,
            candidate,
            archives,
            records,
            fingerprint,
            marker_digest,
            tool_directory,
            args.checkpoint,
        )
        # The manifest helper ran before policy aliases were added.  Recreate that
        # exact smaller environment, using only the bootstrap base plus PATH.
        helper_keys = set(_BASE_ENVIRONMENT) | {"HOME", "PATH", "TMPDIR"}
        helper_environment = {key: environment[key] for key in helper_keys}
        result = _run_bounded(
            archives["python"].path,
            ["-I", "-S", str(archives["manifest_helper"].path), "--root", str(candidate), "--release-identity-json"],
            cwd=candidate,
            environment=helper_environment,
        )
        if result.returncode != 0 or result.stderr:
            raise ValidationError("trusted manifest helper rejected the current candidate")
        try:
            recomputed_value = json.loads(result.stdout)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ValidationError("trusted manifest helper returned invalid identity JSON") from error
        recomputed = _load_identity(recomputed_value, "recomputed candidate identity")
        if result.stdout != _canonical_json(recomputed) or recomputed != identity:
            raise ValidationError("current candidate identity changed after bootstrap")
        cargo_lock = _read_path(
            candidate / "Cargo.lock",
            "current candidate Cargo.lock",
            maximum_bytes=_MAX_EVIDENCE_BYTES,
        )
        if cargo_lock.data != records["cargo_lock"].data or cargo_lock.sha256 != identity["cargo_lock_sha256"]:
            raise ValidationError("current Cargo.lock disagrees with signed identity evidence")
        expected_validator_path = (
            candidate / "scripts" / "validate_sumeragi_v2_release_bootstrap.py"
        )
        expected_validator = _read_path(
            expected_validator_path,
            "authenticated candidate bootstrap validator",
            maximum_bytes=_MAX_HELPER_BYTES,
        )
        current_validator_path = _canonical_existing(
            Path(__file__), "executing bootstrap validator"
        )
        sealed_runner: Snapshot | None = None
        if args.checkpoint == "entry":
            if current_validator_path != expected_validator_path:
                raise ValidationError("entry validator is not the candidate script")
            current_validator = expected_validator
        else:
            sealed_root = evidence.path / "release-runner" / "source"
            if current_validator_path != (
                sealed_root / "scripts" / "validate_sumeragi_v2_release_bootstrap.py"
            ):
                raise ValidationError(
                    "sealed validator is not the authenticated runner-owned source copy"
                )
            current_validator = _read_path(
                current_validator_path,
                "sealed bootstrap validator",
                maximum_bytes=_MAX_HELPER_BYTES,
                expected_mode=expected_validator.mode,
            )
            if current_validator.data != expected_validator.data:
                raise ValidationError(
                    "sealed bootstrap validator differs from the authenticated candidate"
                )
            sealed_runner = _read_path(
                sealed_root / "scripts" / "run_sumeragi_v2_release_gates.sh",
                "sealed candidate runner",
                maximum_bytes=_MAX_HELPER_BYTES,
                expected_mode=runner.mode,
            )
            if sealed_runner.data != runner.data:
                raise ValidationError(
                    "sealed runner differs from the authenticated candidate runner"
                )
        _inventory(evidence, args.checkpoint)
        unique_records = {snapshot.path: snapshot for snapshot in records.values()}
        all_snapshots = [
            marker,
            identity_file,
            runner,
            cargo_lock,
            expected_validator,
            *archives.values(),
            *source_snapshots,
            *runner_tool_sources,
            *unique_records.values(),
        ]
        if current_validator.path != expected_validator.path:
            all_snapshots.append(current_validator)
        if sealed_runner is not None:
            all_snapshots.append(sealed_runner)
        source_paths = {
            snapshot.path for snapshot in [*source_snapshots, *runner_tool_sources]
        }
        inode_set: set[tuple[int, int]] = set()
        for snapshot in all_snapshots:
            if snapshot.path in source_paths:
                continue
            key = (snapshot.device, snapshot.inode)
            if key in inode_set:
                raise ValidationError("distinct bootstrap contract files share one inode")
            inode_set.add(key)
        for snapshot in all_snapshots:
            maximum = (
                _MAX_TOOL_BYTES
                if snapshot.mode == _TOOL_MODE
                else max(_MAX_EVIDENCE_BYTES, len(snapshot.data))
            )
            _revalidate_snapshot(
                snapshot,
                snapshot.path.name,
                maximum,
                external_source=snapshot.path in source_paths,
            )
        for alias in runner_tool_aliases:
            if _runner_alias(
                alias.path, Path(alias.target), alias.path.name
            ) != alias:
                raise ValidationError("runner tool alias changed during validation")
        _revalidate_directory(candidate_directory, "current candidate root")
        _revalidate_directory(evidence, "bootstrap evidence directory")
        _inventory(evidence, args.checkpoint)
    finally:
        os.close(evidence.descriptor)
        os.close(candidate_directory.descriptor)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-root", type=Path, required=True)
    parser.add_argument("--runner", type=Path, required=True)
    parser.add_argument("--profile", action="store_true", required=True)
    parser.add_argument("--release", action="store_true", required=True)
    parser.add_argument("--checkpoint", choices=("entry", "sealed"), default="entry")
    return parser


def main() -> int:
    args = _parser().parse_args()
    try:
        validate(args)
    except (ValidationError, OSError) as error:
        print(f"Sumeragi v2 release bootstrap validation failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
