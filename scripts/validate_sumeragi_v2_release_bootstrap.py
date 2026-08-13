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
from pathlib import Path, PurePosixPath
import re
import selectors
import shutil
import stat
import subprocess
import sys
import sysconfig
import time
import types
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
    "candidate_identity",
    "candidate_identity_sha256",
    "trusted_inputs",
    "release_approvals",
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
    "receipt_validator_support",
    "runtime_helper",
    "tool_probe_helper",
    "approval_contract",
    "approval_offline_toolchain_sdk",
    "approval_formal_proof_tools",
    "approval_network_scale_soak",
    "approval_final_bootstrap_publication",
    "sdk_dependency_bundle_manifest",
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
    "python": (
        "python-runtime/bin/python3"
        if (
            sys.platform == "darwin"
            and isinstance(sysconfig.get_config_var("PYTHONFRAMEWORK"), str)
            and bool(sysconfig.get_config_var("PYTHONFRAMEWORK"))
        )
        else "python3"
    ),
    "receipt_validator": "validate-receipt.py",
    "receipt_validator_support": "sumeragi_v2_localnet_manifest.py",
    "runtime_helper": "copy-release-runtime.py",
    "tool_probe_helper": "probe-release-tools.py",
    "approval_contract": "release-approval-contract.py",
    "approval_offline_toolchain_sdk": (
        "offline-toolchain-sdk.approval.v1.json"
    ),
    "approval_formal_proof_tools": "formal-proof-tools.approval.v1.json",
    "approval_network_scale_soak": "network-scale-soak.approval.v1.json",
    "approval_final_bootstrap_publication": (
        "final-bootstrap-publication.approval.v1.json"
    ),
    "sdk_dependency_bundle_manifest": "sdk-dependency-bundle-manifest.json",
    "revocation": "bootstrap-revocation",
    "runner_tool_manifest": "runner-tool-manifest.json",
    "ssh_keygen": "ssh-keygen",
}
_RECEIPT_VALIDATOR_COMPONENT_SHA256 = {
    "write_sumeragi_v2_release_receipt_corridor_log.py": (
        "6ff2d5337414bbbf74a9530cc1b2bd59bc62141a82a1319fa2a270b84e64ce8c"
    ),
    "write_sumeragi_v2_release_receipt_formal_artifacts.py": (
        "43a815d4257ad6296a48e125dfab52c5f31aabba5210f4154641164887e48886"
    ),
    "write_sumeragi_v2_release_receipt_gate_evidence.py": (
        "dd67a4f7b7c321238bd08789cb54fb7704c3e309c9f1764baea275ff64a5e5ae"
    ),
    "write_sumeragi_v2_release_receipt_publication.py": (
        "f75a5f2df901408d028605ab11b09f01a77853ecd27deb10a6cdfbd08dda5bed"
    ),
}
_BOOTSTRAP_COMPONENT_SHA256 = {
    "bootstrap_sumeragi_v2_release_receipt_replay.py": (
        "e336273e2a4322d125344b6bd5162fdd1a9dcfce874aa49497a03c30141bfd8b"
    ),
}
_APPROVAL_CLASS_IDS = (
    "offline-toolchain-sdk",
    "formal-proof-tools",
    "network-scale-soak",
    "final-bootstrap-publication",
)
_APPROVAL_INPUT_LABELS = {
    class_id: "approval_" + class_id.replace("-", "_")
    for class_id in _APPROVAL_CLASS_IDS
}
_APPROVAL_ATTESTATION_NAMES = {
    class_id: f"{class_id}.approval-attestation.v1.json"
    for class_id in _APPROVAL_CLASS_IDS
}
_APPROVAL_SET_ATTESTATION_NAME = "release-approval-set-attestation.v1.json"
_APPROVAL_SET_ARCHIVE_ID = "release-approval.set-attestation.v1"
_APPROVAL_OPERATION_COUNTS = {
    "offline-toolchain-sdk": 23,
    "formal-proof-tools": 38,
    "network-scale-soak": 8,
    "final-bootstrap-publication": 8,
}
_FRAMEWORK_PYTHON = _TRUSTED_ARCHIVE_NAMES["python"] != "python3"
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
_IDENTITY_ARCHIVE_IDS = {
    "cargo_lock": "release-identity.cargo-lock.v1",
    "git": "release-identity.git.v1",
    "raw_commit": "release-identity.raw-commit.v1",
    "ssh_allowed_signers": "release-identity.ssh-allowed-signers.v1",
    "ssh_keygen": "release-identity.ssh-keygen.v1",
    "ssh_revocation": "release-identity.ssh-revocation.v1",
    "verify_transcript": "release-identity.verify-transcript.v1",
}
_IDENTITY_ATTESTATION_FORMAT = "iroha-sumeragi-v2-release-identity-attestation"
_IDENTITY_TRANSCRIPT_FORMAT = "iroha-sumeragi-v2-release-identity-transcript"
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
_MAX_SDK_MANIFEST_BYTES = 256 * 1024 * 1024
_MAX_POLICY_BYTES = 16 * 1024 * 1024
_MAX_EVIDENCE_BYTES = 128 * 1024 * 1024
_MAX_TOOL_BYTES = 512 * 1024 * 1024
_MAX_FRAMEWORK_RUNTIME_BYTES = 4 * 1024 * 1024 * 1024
_MAX_FRAMEWORK_RUNTIME_MEMBERS = 250_000
_MAX_HELPER_OUTPUT_BYTES = 16 * 1024 * 1024
_COMMAND_TIMEOUT_SECONDS = 120
_RUNNER_EXTRA_ENV = {
    "CARGO_HOME",
    "CARGO_NET_GIT_FETCH_WITH_CLI",
    "CARGO_NET_OFFLINE",
    "IROHA_RELEASE_CANCEL_REQUEST_PATH",
    "IROHA_RELEASE_INVOCATION_ROOT",
    "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
    "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST",
    "IROHA_RELEASE_SCALING_IROHAD_SHA256",
    "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
    "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
    "IROHA_RELEASE_TLA2TOOLS_JAR",
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
    relative_target = os.path.relpath(target, path.parent)
    metadata = path.lstat()
    if (
        not stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_nlink != 1
        or os.readlink(path) != relative_target
        or path.resolve(strict=True) != target
    ):
        raise ValidationError(f"{label} is not an exact protected alias")
    return AliasSnapshot(
        path=path,
        target=relative_target,
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
    if label == "sdk_dependency_bundle_manifest":
        return _MAX_SDK_MANIFEST_BYTES
    return _MAX_HELPER_BYTES


def _framework_runtime_projection(
    records: Any, label: str,
) -> list[dict[str, Any]]:
    if not isinstance(records, list):
        raise ValidationError(f"{label} records are not a list")
    projected: list[dict[str, Any]] = []
    for value in records:
        if not isinstance(value, dict):
            raise ValidationError(f"{label} member is malformed")
        kind = value.get("kind")
        source_keys = {
            "directory": {"path", "kind", "device", "inode", "mode"},
            "file": {
                "path",
                "kind",
                "device",
                "inode",
                "mode",
                "size",
                "sha256",
            },
            "symlink": {"path", "kind", "mode", "target"},
        }.get(kind)
        sanitized_keys = {
            "directory": {"path", "kind", "mode"},
            "file": {"path", "kind", "mode", "size", "sha256"},
            "symlink": {"path", "kind", "mode", "target"},
        }.get(kind)
        keys = set(value)
        if source_keys is not None and keys == source_keys:
            value = {key: value[key] for key in sanitized_keys or ()}
            keys = set(value)
        if sanitized_keys is None or keys != sanitized_keys:
            raise ValidationError(f"{label} member schema is not exact")
        path = value.get("path")
        if (
            not isinstance(path, str)
            or not path
            or path.startswith("/")
            or PurePosixPath(path).as_posix() != path
            or ".." in PurePosixPath(path).parts
            or not isinstance(value.get("mode"), str)
            or _MODE_RE.fullmatch(value["mode"]) is None
        ):
            raise ValidationError(f"{label} member path or mode is unsafe")
        if kind == "file":
            if (
                type(value["size"]) is not int
                or value["size"] < 0
                or _DIGEST_RE.fullmatch(
                    _string(value["sha256"], f"{label} file digest")
                )
                is None
            ):
                raise ValidationError(f"{label} file metadata is invalid")
        elif kind == "symlink" and (
            not isinstance(value["target"], str) or not value["target"]
        ):
            raise ValidationError(f"{label} symlink target is invalid")
        projected.append(dict(value))
    projected.sort(key=lambda record: record["path"])
    if len(projected) > _MAX_FRAMEWORK_RUNTIME_MEMBERS or len({
        record["path"] for record in projected
    }) != len(projected):
        raise ValidationError(f"{label} member inventory is not unique and bounded")
    return projected


def _validate_framework_python_runtime(
    value: Any, evidence: DirectorySnapshot,
) -> tuple[Snapshot, DirectorySnapshot]:
    """Independently authenticate every archived framework-Python member."""

    runtime = _exact_dict(
        value,
        {
            "format",
            "schema_version",
            "archive_root",
            "root_mode",
            "executable",
            "inventory",
            "record_count",
            "file_bytes",
            "records",
        },
        "framework Python runtime",
    )
    stdlib_name = f"python{sys.version_info.major}.{sys.version_info.minor}"
    if (
        runtime["format"] != "iroha-sumeragi-v2-framework-python-runtime"
        or _strict_int(
            runtime["schema_version"], "framework Python runtime schema"
        )
        != 1
        or runtime["archive_root"] != "python-runtime"
        or runtime["root_mode"] != "0500"
        or runtime["executable"] != "bin/python3"
    ):
        raise ValidationError("framework Python runtime binding is not exact")
    inventory_record = _exact_dict(
        runtime["inventory"],
        {"archive_name", "mode", "sha256", "size_bytes"},
        "framework Python runtime inventory record",
    )
    if (
        inventory_record["archive_name"] != "python-runtime-input.json"
        or _mode(
            inventory_record["mode"],
            "framework Python runtime inventory mode",
        )
        != _DATA_MODE
    ):
        raise ValidationError("framework Python runtime inventory binding is wrong")
    inventory = _read_at(
        evidence,
        "python-runtime-input.json",
        "framework Python runtime inventory",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
        expected_mode=_DATA_MODE,
    )
    if (
        inventory.sha256
        != _digest(
            inventory_record["sha256"],
            "framework Python runtime inventory digest",
        )
        or len(inventory.data)
        != _strict_int(
            inventory_record["size_bytes"],
            "framework Python runtime inventory size",
        )
    ):
        raise ValidationError(
            "framework Python runtime inventory bytes do not match the marker"
        )
    private_inventory = _parse_canonical(
        inventory, "framework Python runtime inventory",
    )
    inventory_keys = {
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
    runtime_path = evidence.path / "python-runtime"
    if (
        set(private_inventory) != inventory_keys
        or private_inventory["format"]
        != "iroha-sumeragi-v2-private-framework-python-runtime"
        or _strict_int(
            private_inventory["schema_version"],
            "private framework Python runtime schema",
        )
        != 1
        or private_inventory["runtime_root"] != str(runtime_path)
        or private_inventory["source_disclosure"] != "withheld"
        or not isinstance(private_inventory["input_records"], list)
    ):
        raise ValidationError(
            "private framework Python runtime inventory contract is wrong"
        )
    expected_records = _framework_runtime_projection(
        runtime["records"], "framework Python runtime",
    )
    private_records = _framework_runtime_projection(
        private_inventory["records"], "private framework Python runtime",
    )
    if expected_records != private_records:
        raise ValidationError(
            "framework Python marker does not bind the private member inventory"
        )
    expected_count = _strict_int(
        runtime["record_count"], "framework Python runtime record count",
    )
    expected_bytes = _strict_int(
        runtime["file_bytes"], "framework Python runtime file bytes",
    )
    if (
        expected_count != len(expected_records)
        or expected_count
        != _strict_int(
            private_inventory["record_count"],
            "private framework Python runtime record count",
        )
        or expected_bytes
        != sum(
            record["size"]
            for record in expected_records
            if record["kind"] == "file"
        )
        or expected_bytes
        != _strict_int(
            private_inventory["file_bytes"],
            "private framework Python runtime file bytes",
        )
        or expected_bytes > _MAX_FRAMEWORK_RUNTIME_BYTES
    ):
        raise ValidationError(
            "framework Python runtime member accounting is not exact"
        )

    directory = _open_directory(
        runtime_path,
        "framework Python runtime",
        expected_mode=0o500,
    )
    observed: list[dict[str, Any]] = []
    total_bytes = 0

    def walk(
        descriptor: int,
        relative: str,
        directory_identity: tuple[int, ...],
    ) -> None:
        nonlocal total_bytes
        try:
            names = tuple(sorted(os.listdir(descriptor)))
        except OSError as error:
            raise ValidationError(
                f"framework Python runtime directory is unreadable: {relative or '.'}"
            ) from error
        if len(observed) + len(names) > _MAX_FRAMEWORK_RUNTIME_MEMBERS:
            raise ValidationError(
                "framework Python runtime contains too many members"
            )
        for name in names:
            path = name if not relative else f"{relative}/{name}"
            if (
                not name
                or name in {".", ".."}
                or "/" in name
                or "\0" in name
            ):
                raise ValidationError(
                    "framework Python runtime has an unsafe member name"
                )
            metadata = os.stat(
                name, dir_fd=descriptor, follow_symlinks=False,
            )
            mode = f"{stat.S_IMODE(metadata.st_mode):04o}"
            if metadata.st_uid != os.getuid():
                raise ValidationError(
                    f"framework Python runtime member has the wrong owner: {path}"
                )
            if stat.S_ISDIR(metadata.st_mode):
                child = os.open(
                    name,
                    os.O_RDONLY
                    | getattr(os, "O_DIRECTORY", 0)
                    | getattr(os, "O_CLOEXEC", 0)
                    | getattr(os, "O_NOFOLLOW", 0),
                    dir_fd=descriptor,
                )
                opened = os.fstat(child)
                identity = (
                    opened.st_dev,
                    opened.st_ino,
                    opened.st_uid,
                    stat.S_IMODE(opened.st_mode),
                    opened.st_mtime_ns,
                    opened.st_ctime_ns,
                )
                if (
                    not stat.S_ISDIR(opened.st_mode)
                    or identity
                    != (
                        metadata.st_dev,
                        metadata.st_ino,
                        metadata.st_uid,
                        stat.S_IMODE(metadata.st_mode),
                        metadata.st_mtime_ns,
                        metadata.st_ctime_ns,
                    )
                ):
                    os.close(child)
                    raise ValidationError(
                        f"framework Python runtime directory changed: {path}"
                    )
                observed.append(
                    {"path": path, "kind": "directory", "mode": mode}
                )
                try:
                    walk(child, path, identity)
                finally:
                    os.close(child)
                after = os.stat(
                    name, dir_fd=descriptor, follow_symlinks=False,
                )
                if (
                    after.st_dev,
                    after.st_ino,
                    after.st_uid,
                    stat.S_IMODE(after.st_mode),
                    after.st_mtime_ns,
                    after.st_ctime_ns,
                ) != identity:
                    raise ValidationError(
                        f"framework Python runtime directory changed: {path}"
                    )
            elif stat.S_ISREG(metadata.st_mode):
                flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
                if hasattr(os, "O_NOFOLLOW"):
                    flags |= os.O_NOFOLLOW
                child = os.open(name, flags, dir_fd=descriptor)
                digest = hashlib.sha256()
                size = 0
                try:
                    opened = os.fstat(child)
                    if (
                        not stat.S_ISREG(opened.st_mode)
                        or opened.st_uid != os.getuid()
                        or opened.st_nlink != 1
                        or (
                            opened.st_dev,
                            opened.st_ino,
                            opened.st_size,
                            opened.st_mtime_ns,
                            opened.st_ctime_ns,
                            stat.S_IMODE(opened.st_mode),
                        )
                        != (
                            metadata.st_dev,
                            metadata.st_ino,
                            metadata.st_size,
                            metadata.st_mtime_ns,
                            metadata.st_ctime_ns,
                            stat.S_IMODE(metadata.st_mode),
                        )
                    ):
                        raise ValidationError(
                            f"framework Python runtime file changed: {path}"
                        )
                    while True:
                        block = os.read(child, 1024 * 1024)
                        if not block:
                            break
                        digest.update(block)
                        size += len(block)
                        if (
                            size > _MAX_TOOL_BYTES
                            or total_bytes + size
                            > _MAX_FRAMEWORK_RUNTIME_BYTES
                        ):
                            raise ValidationError(
                                "framework Python runtime exceeds its byte bound"
                            )
                    after = os.fstat(child)
                    if (
                        size != opened.st_size
                        or (
                            after.st_dev,
                            after.st_ino,
                            after.st_size,
                            after.st_mtime_ns,
                            after.st_ctime_ns,
                            stat.S_IMODE(after.st_mode),
                        )
                        != (
                            opened.st_dev,
                            opened.st_ino,
                            opened.st_size,
                            opened.st_mtime_ns,
                            opened.st_ctime_ns,
                            stat.S_IMODE(opened.st_mode),
                        )
                    ):
                        raise ValidationError(
                            f"framework Python runtime file changed: {path}"
                        )
                finally:
                    os.close(child)
                total_bytes += size
                observed.append({
                    "path": path,
                    "kind": "file",
                    "mode": mode,
                    "size": size,
                    "sha256": digest.hexdigest(),
                })
            elif stat.S_ISLNK(metadata.st_mode):
                target = os.readlink(name, dir_fd=descriptor)
                after = os.stat(
                    name, dir_fd=descriptor, follow_symlinks=False,
                )
                if (
                    not stat.S_ISLNK(after.st_mode)
                    or (
                        after.st_dev,
                        after.st_ino,
                        after.st_uid,
                        after.st_mtime_ns,
                        after.st_ctime_ns,
                    )
                    != (
                        metadata.st_dev,
                        metadata.st_ino,
                        metadata.st_uid,
                        metadata.st_mtime_ns,
                        metadata.st_ctime_ns,
                    )
                    or os.readlink(name, dir_fd=descriptor) != target
                ):
                    raise ValidationError(
                        f"framework Python runtime symlink changed: {path}"
                    )
                observed.append({
                    "path": path,
                    "kind": "symlink",
                    "mode": mode,
                    "target": target,
                })
            else:
                raise ValidationError(
                    f"framework Python runtime contains a special member: {path}"
                )
        current = os.fstat(descriptor)
        current_identity = (
            current.st_dev,
            current.st_ino,
            current.st_uid,
            stat.S_IMODE(current.st_mode),
            current.st_mtime_ns,
            current.st_ctime_ns,
        )
        if current_identity != directory_identity:
            raise ValidationError(
                f"framework Python runtime directory changed: {relative or '.'}"
            )

    try:
        root_metadata = os.fstat(directory.descriptor)
        walk(
            directory.descriptor,
            "",
            (
                root_metadata.st_dev,
                root_metadata.st_ino,
                root_metadata.st_uid,
                stat.S_IMODE(root_metadata.st_mode),
                root_metadata.st_mtime_ns,
                root_metadata.st_ctime_ns,
            ),
        )
        observed.sort(key=lambda record: record["path"])
        if observed != expected_records or total_bytes != expected_bytes:
            raise ValidationError(
                "framework Python runtime members differ from the marker"
            )
        by_path = {record["path"]: record for record in observed}
        required = {
            "bin": "directory",
            "bin/python3": "file",
            "Python3": "file",
            "Resources": "directory",
            "Resources/Python.app/Contents/MacOS/Python": "file",
            "lib": "directory",
            f"lib/{stdlib_name}": "directory",
            f"lib/{stdlib_name}/lib-dynload": "directory",
        }
        if (
            {PurePosixPath(path).parts[0] for path in by_path}
            != {"bin", "Python3", "Resources", "lib"}
            or any(
                not isinstance(by_path.get(path), dict)
                or by_path[path]["kind"] != kind
                for path, kind in required.items()
            )
        ):
            raise ValidationError(
                "framework Python runtime indispensable layout is incomplete"
            )
        for path, record in by_path.items():
            if record["kind"] != "symlink":
                continue
            target = PurePosixPath(record["target"])
            if target.is_absolute():
                raise ValidationError(
                    f"framework Python runtime symlink is absolute: {path}"
                )
            parts = list(PurePosixPath(path).parts[:-1])
            for part in target.parts:
                if part in {"", "."}:
                    continue
                if part == "..":
                    if not parts:
                        raise ValidationError(
                            f"framework Python runtime symlink escapes: {path}"
                        )
                    parts.pop()
                else:
                    parts.append(part)
            if not parts or parts[0] not in {"Python3", "Resources", "lib"}:
                raise ValidationError(
                    f"framework Python runtime symlink leaves its closure: {path}"
                )
            for index in range(1, len(parts) + 1):
                target_path = "/".join(parts[:index])
                target_record = by_path.get(target_path)
                if (
                    not isinstance(target_record, dict)
                    or (
                        index < len(parts)
                        and target_record["kind"] != "directory"
                    )
                    or (
                        index == len(parts)
                        and target_record["kind"] not in {"directory", "file"}
                    )
                ):
                    raise ValidationError(
                        f"framework Python runtime symlink target is not exact: {path}"
                    )
    except BaseException:
        os.close(directory.descriptor)
        raise
    return inventory, directory


def _load_release_approval_contract(snapshot: Snapshot) -> Any:
    """Load the approval API from one authenticated bootstrap archive."""

    module_name = "_sumeragi_v2_release_approval_" + snapshot.sha256
    module = types.ModuleType(module_name)
    module.__file__ = str(snapshot.path)
    module.__package__ = ""
    sys.modules[module_name] = module
    try:
        exec(compile(snapshot.data, str(snapshot.path), "exec"), module.__dict__)
    except BaseException as error:
        raise ValidationError(
            "archived release approval contract could not be loaded"
        ) from error
    finally:
        sys.modules.pop(module_name, None)
    required = (
        "APPROVAL_ARCHIVE_IDS",
        "APPROVAL_CLASS_ORDER",
        "APPROVAL_OPERATION_PLAN_SHA256",
        "APPROVAL_SET_ARCHIVE_FORMAT",
        "ReleaseApprovalClass",
        "ReleaseApprovalError",
        "build_release_approval_expectations",
        "load_protected_release_approval_set",
        "sanitized_release_approval_set_archive",
    )
    if any(not hasattr(module, name) for name in required):
        raise ValidationError("archived release approval contract API is incomplete")
    if tuple(value.value for value in module.APPROVAL_CLASS_ORDER) != _APPROVAL_CLASS_IDS:
        raise ValidationError("archived release approval class order is not exact")
    return module


def _release_approval_archive_record(
    value: Any,
    *,
    label: str,
    archive_id: str,
    archive_name: str,
    snapshot: Snapshot,
) -> None:
    record = _exact_dict(
        value,
        {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
        label,
    )
    if (
        record["archive_id"] != archive_id
        or record["archive_name"] != archive_name
        or _mode(record["mode"], f"{label} mode") != _DATA_MODE
        or _digest(record["sha256"], f"{label} digest") != snapshot.sha256
        or _strict_int(record["size_bytes"], f"{label} size")
        != len(snapshot.data)
    ):
        raise ValidationError(f"{label} archive binding is not exact")


def _release_approvals(
    value: Any,
    *,
    evidence: DirectorySnapshot,
    identity: dict[str, Any],
    archives: dict[str, Snapshot],
) -> list[Snapshot]:
    """Independently replay all four exact approvals and sanitized archives."""

    marker = _exact_dict(
        value,
        {
            "format",
            "schema_version",
            "candidate_oid",
            "candidate_tree",
            "protected_tool_manifest_sha256",
            "evidence_root_id",
            "expected_duration_seconds",
            "operation_plan_sha256",
            "class_attestations",
            "set_attestation",
        },
        "release approvals",
    )
    module = _load_release_approval_contract(archives["approval_contract"])
    if (
        marker["format"] != module.APPROVAL_SET_ARCHIVE_FORMAT
        or _strict_int(marker["schema_version"], "release approval schema") != 1
        or marker["candidate_oid"] != identity["head_commit"]
        or marker["candidate_tree"] != identity["head_tree"]
        or marker["protected_tool_manifest_sha256"]
        != archives["runner_tool_manifest"].sha256
    ):
        raise ValidationError("release approval candidate/tool binding is not exact")
    durations = _exact_dict(
        marker["expected_duration_seconds"],
        set(_APPROVAL_CLASS_IDS),
        "release approval durations",
    )
    if any(type(value) is not int for value in durations.values()):
        raise ValidationError("release approval durations are not exact integers")
    expected_plan_digests = {
        approval_class.value: digest
        for approval_class, digest in module.APPROVAL_OPERATION_PLAN_SHA256.items()
    }
    if marker["operation_plan_sha256"] != expected_plan_digests:
        raise ValidationError("release approval operation plans are not exact")
    try:
        expectations = module.build_release_approval_expectations(
            candidate_oid=identity["head_commit"],
            candidate_tree=identity["head_tree"],
            protected_tool_manifest_sha256=archives[
                "runner_tool_manifest"
            ].sha256,
            evidence_root_id=marker["evidence_root_id"],
            offline_toolchain_sdk_duration_seconds=durations[
                "offline-toolchain-sdk"
            ],
            formal_proof_tools_duration_seconds=durations[
                "formal-proof-tools"
            ],
            network_scale_soak_duration_seconds=durations[
                "network-scale-soak"
            ],
            final_bootstrap_publication_duration_seconds=durations[
                "final-bootstrap-publication"
            ],
        )
        approvals = module.load_protected_release_approval_set(
            {
                module.ReleaseApprovalClass(class_id): archives[
                    _APPROVAL_INPUT_LABELS[class_id]
                ].path
                for class_id in _APPROVAL_CLASS_IDS
            },
            expectations=expectations,
            expected_owner_uid=os.getuid(),
        )
    except module.ReleaseApprovalError as error:
        raise ValidationError(f"release approval replay failed: {error}") from error
    if {
        approval.class_id.value: len(approval.operations)
        for approval in approvals
    } != _APPROVAL_OPERATION_COUNTS:
        raise ValidationError("release approval operation counts are not exact")
    class_records = _exact_dict(
        marker["class_attestations"],
        set(_APPROVAL_CLASS_IDS),
        "release approval class attestations",
    )
    snapshots: list[Snapshot] = []
    for approval in approvals:
        class_id = approval.class_id.value
        archive_name = _APPROVAL_ATTESTATION_NAMES[class_id]
        snapshot = _read_at(
            evidence,
            archive_name,
            f"sanitized release approval {class_id}",
            maximum_bytes=_MAX_EVIDENCE_BYTES,
            expected_mode=_DATA_MODE,
        )
        sanitized = approval.sanitized_archive()
        if snapshot.data != sanitized.canonical_bytes or snapshot.sha256 != sanitized.sha256:
            raise ValidationError(
                f"sanitized release approval {class_id} is not exact"
            )
        _release_approval_archive_record(
            class_records[class_id],
            label=f"release approval {class_id}",
            archive_id=module.APPROVAL_ARCHIVE_IDS[approval.class_id],
            archive_name=archive_name,
            snapshot=snapshot,
        )
        snapshots.append(snapshot)
    set_snapshot = _read_at(
        evidence,
        _APPROVAL_SET_ATTESTATION_NAME,
        "sanitized release approval set",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
        expected_mode=_DATA_MODE,
    )
    sanitized_set = module.sanitized_release_approval_set_archive(approvals)
    if (
        set_snapshot.data != sanitized_set.canonical_bytes
        or set_snapshot.sha256 != sanitized_set.sha256
    ):
        raise ValidationError("sanitized release approval set is not exact")
    _release_approval_archive_record(
        marker["set_attestation"],
        label="release approval set",
        archive_id=_APPROVAL_SET_ARCHIVE_ID,
        archive_name=_APPROVAL_SET_ATTESTATION_NAME,
        snapshot=set_snapshot,
    )
    snapshots.append(set_snapshot)
    return snapshots


def _trusted_inputs(
    value: Any,
    evidence: DirectorySnapshot,
    candidate: Path,
) -> tuple[
    dict[str, Snapshot],
    list[Snapshot],
    DirectorySnapshot | None,
]:
    records = _exact_dict(value, _TRUSTED_INPUT_KEYS, "trusted_inputs")
    archives: dict[str, Snapshot] = {}
    sources: list[Snapshot] = []
    base_record_keys = {
        "archive_id", "archive_name", "mode", "sha256", "size_bytes",
    }
    framework_directory: DirectorySnapshot | None = None
    framework_inventory: Snapshot | None = None
    for label in sorted(records):
        record_keys = base_record_keys
        if label == "python" and _FRAMEWORK_PYTHON:
            record_keys = record_keys | {"runtime"}
        if label == "bootstrap":
            record_keys = record_keys | {"components"}
        if label == "receipt_validator":
            record_keys = record_keys | {"components"}
        record = _exact_dict(
            records[label], record_keys, f"trusted input {label}",
        )
        if record["archive_id"] != f"release-bootstrap.{label.replace('_', '-')}.v1":
            raise ValidationError(f"trusted input {label} has the wrong archive id")
        if record["archive_name"] != _TRUSTED_ARCHIVE_NAMES[label]:
            raise ValidationError(f"trusted input {label} has the wrong archive alias")
        expected_mode = _TOOL_MODE if label in _EXECUTABLE_INPUTS else _DATA_MODE
        if _mode(record["mode"], f"trusted input {label} archive mode") != expected_mode:
            raise ValidationError(f"trusted input {label} has the wrong archive mode")
        size = _strict_int(record["size_bytes"], f"trusted input {label} size")
        digest = _digest(record["sha256"], f"trusted input {label} digest")
        archive_name = _TRUSTED_ARCHIVE_NAMES[label]
        archive = (
            _read_path(
                evidence.path / archive_name,
                f"archived trusted input {label}",
                maximum_bytes=_artifact_limit(label),
                expected_mode=expected_mode,
            )
            if "/" in archive_name
            else _read_at(
                evidence,
                archive_name,
                f"archived trusted input {label}",
                maximum_bytes=_artifact_limit(label),
                expected_mode=expected_mode,
            )
        )
        if len(archive.data) != size or archive.sha256 != digest:
            raise ValidationError(f"archived trusted input {label} does not match its marker")
        if label == "python" and _FRAMEWORK_PYTHON:
            framework_inventory, framework_directory = (
                _validate_framework_python_runtime(record["runtime"], evidence)
            )
        if label == "bootstrap":
            components = _exact_dict(
                record["components"],
                set(_BOOTSTRAP_COMPONENT_SHA256),
                "bootstrap components",
            )
            for name, expected_digest in sorted(
                _BOOTSTRAP_COMPONENT_SHA256.items()
            ):
                component_record = _exact_dict(
                    components[name],
                    {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
                    f"bootstrap component {name}",
                )
                if (
                    component_record["archive_id"]
                    != "release-bootstrap.bootstrap-component.v1:" + name
                    or component_record["archive_name"] != name
                    or _mode(
                        component_record["mode"],
                        f"bootstrap component {name} mode",
                    )
                    != _DATA_MODE
                    or _digest(
                        component_record["sha256"],
                        f"bootstrap component {name} digest",
                    )
                    != expected_digest
                ):
                    raise ValidationError(
                        f"bootstrap component {name} binding is wrong"
                    )
                component = _read_at(
                    evidence,
                    name,
                    f"bootstrap component {name}",
                    maximum_bytes=_MAX_HELPER_BYTES,
                    expected_mode=_DATA_MODE,
                )
                if (
                    component.sha256 != expected_digest
                    or len(component.data)
                    != _strict_int(
                        component_record["size_bytes"],
                        f"bootstrap component {name} size",
                    )
                ):
                    raise ValidationError(
                        f"bootstrap component {name} bytes are wrong"
                    )
                archives["bootstrap_component:" + name] = component
        if label == "receipt_validator":
            components = _exact_dict(
                record["components"],
                set(_RECEIPT_VALIDATOR_COMPONENT_SHA256),
                "receipt validator components",
            )
            for name, expected_digest in sorted(
                _RECEIPT_VALIDATOR_COMPONENT_SHA256.items()
            ):
                component_record = _exact_dict(
                    components[name],
                    {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
                    f"receipt validator component {name}",
                )
                if (
                    component_record["archive_id"]
                    != "release-bootstrap.receipt-validator-component.v1:" + name
                    or component_record["archive_name"] != name
                    or _mode(
                        component_record["mode"],
                        f"receipt validator component {name} mode",
                    )
                    != _DATA_MODE
                    or _digest(
                        component_record["sha256"],
                        f"receipt validator component {name} digest",
                    )
                    != expected_digest
                ):
                    raise ValidationError(
                        f"receipt validator component {name} binding is wrong"
                    )
                component = _read_at(
                    evidence,
                    name,
                    f"receipt validator component {name}",
                    maximum_bytes=_MAX_HELPER_BYTES,
                    expected_mode=_DATA_MODE,
                )
                if (
                    component.sha256 != expected_digest
                    or len(component.data)
                    != _strict_int(
                        component_record["size_bytes"],
                        f"receipt validator component {name} size",
                    )
                ):
                    raise ValidationError(
                        f"receipt validator component {name} bytes are wrong"
                    )
                archives["receipt_validator_component:" + name] = component
        archives[label] = archive
    if framework_inventory is not None:
        archives["python_runtime_inventory"] = framework_inventory
    return archives, sources, framework_directory


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


def _validate_legacy_identity_semantics(
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


def _validate_identity_semantics(
    snapshots: dict[str, Snapshot],
    identity: dict[str, Any],
    identity_bytes: bytes,
    trusted: dict[str, Snapshot],
) -> str:
    """Authenticate the path-free identity documents and their archived bytes."""

    attestation = _parse_canonical(
        snapshots["identity_attestation"], "identity attestation"
    )
    transcript = _parse_canonical(
        snapshots["identity_transcript"], "identity transcript"
    )
    _exact_dict(
        attestation,
        {"format", "schema_version", "candidate", "archives"},
        "identity attestation",
    )
    if (
        attestation["format"] != _IDENTITY_ATTESTATION_FORMAT
        or _strict_int(
            attestation["schema_version"], "identity attestation schema"
        )
        != 3
    ):
        raise ValidationError("identity attestation must use sanitized schema 3")
    candidate = _exact_dict(
        attestation["candidate"],
        {
            "commit_oid",
            "tree_oid",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "release_identity_sha256",
        },
        "identity attestation candidate",
    )
    if candidate != {
        "commit_oid": identity["head_commit"],
        "tree_oid": identity["head_tree"],
        "source_manifest_sha256": identity[
            "workspace_source_manifest_sha256"
        ],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "release_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
    }:
        raise ValidationError("identity attestation does not bind the candidate")

    archives = _exact_dict(
        attestation["archives"],
        set(_IDENTITY_ARCHIVE_IDS),
        "identity attestation archives",
    )
    for label in sorted(archives):
        record = _exact_dict(
            archives[label],
            {"archive_id", "mode", "sha256", "size_bytes"},
            f"identity archive {label}",
        )
        snapshot = snapshots[label]
        expected_mode = _TOOL_MODE if label in {"git", "ssh_keygen"} else _DATA_MODE
        if (
            record["archive_id"] != _IDENTITY_ARCHIVE_IDS[label]
            or _mode(record["mode"], f"identity archive {label} mode")
            != expected_mode
            or _digest(record["sha256"], f"identity archive {label} digest")
            != snapshot.sha256
            or _strict_int(record["size_bytes"], f"identity archive {label} size")
            != len(snapshot.data)
        ):
            raise ValidationError(
                f"identity archive {label} does not match authenticated bytes"
            )
    for label, trusted_label in (
        ("git", "git"),
        ("ssh_keygen", "ssh_keygen"),
        ("ssh_allowed_signers", "allowed_signers"),
        ("ssh_revocation", "revocation"),
    ):
        if snapshots[label].data != trusted[trusted_label].data:
            raise ValidationError(
                f"identity archive {label} differs from its protected input"
            )
    if snapshots["cargo_lock"].sha256 != identity["cargo_lock_sha256"]:
        raise ValidationError("identity Cargo.lock archive has the wrong digest")
    if archives["verify_transcript"]["sha256"] != snapshots[
        "identity_transcript"
    ].sha256:
        raise ValidationError("identity transcript archive binding is inconsistent")

    _exact_dict(
        transcript,
        {
            "format",
            "schema_version",
            "archive_ids",
            "candidate_commit_oid",
            "operations",
        },
        "identity transcript",
    )
    if (
        transcript["format"] != _IDENTITY_TRANSCRIPT_FORMAT
        or _strict_int(transcript["schema_version"], "identity transcript schema")
        != 3
        or transcript["archive_ids"] != _IDENTITY_ARCHIVE_IDS
        or transcript["candidate_commit_oid"] != identity["head_commit"]
    ):
        raise ValidationError("identity transcript binding is not exact")
    operations = _exact_dict(
        transcript["operations"],
        {"show_signature_metadata", "verify_commit", "ssh_keygen_usage"},
        "identity transcript operations",
    )
    expected_operations = {
        "show_signature_metadata": ("git.show-signature-metadata.ssh.v1", 0),
        "verify_commit": ("git.verify-commit.ssh.v1", 0),
        "ssh_keygen_usage": ("ssh-keygen.usage-probe.v1", 1),
    }
    for label, (operation_id, expected_status) in expected_operations.items():
        record = _exact_dict(
            operations[label],
            {
                "operation_id",
                "exit_status",
                "stdout_sha256",
                "stdout_size_bytes",
                "stderr_sha256",
                "stderr_size_bytes",
            },
            f"identity transcript operation {label}",
        )
        if (
            record["operation_id"] != operation_id
            or _strict_int(record["exit_status"], f"{label} exit status")
            != expected_status
        ):
            raise ValidationError(f"identity transcript operation {label} is not exact")
        for stream in ("stdout", "stderr"):
            _digest(record[f"{stream}_sha256"], f"{label} {stream} digest")
            size = _strict_int(
                record[f"{stream}_size_bytes"], f"{label} {stream} size"
            )
            if size < 0 or size > _MAX_HELPER_OUTPUT_BYTES:
                raise ValidationError(
                    f"identity transcript operation {label} has invalid output size"
                )

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
    fingerprint = os.environ.get(
        "SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT"
    )
    if not isinstance(fingerprint, str) or _FINGERPRINT_RE.fullmatch(fingerprint) is None:
        raise ValidationError("protected signer fingerprint is absent or malformed")
    return fingerprint


def _run_bounded(
    executable: Path,
    arguments: Iterable[str],
    *,
    cwd: Path,
    environment: dict[str, str],
) -> CommandResult:
    selector = selectors.DefaultSelector()
    deadline = time.monotonic() + _COMMAND_TIMEOUT_SECONDS
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    # Bounds determine the eventual verdict; they never control the child.
    # Retain only the capped prefix while draining both streams through EOF.
    retained_output_bytes = 0
    output_limit_exceeded = False
    runtime_limit_exceeded = False
    pending_violation: BaseException | None = None

    def latch(violation: BaseException) -> None:
        nonlocal pending_violation
        if pending_violation is None:
            pending_violation = violation

    try:
        process = subprocess.Popen(
            [str(executable), *arguments],
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        selector.close()
        raise ValidationError("trusted manifest helper could not execute") from error
    output_streams = tuple(
        (stream, label)
        for stream, label in (
            (process.stdout, "stdout"),
            (process.stderr, "stderr"),
        )
        if stream is not None
    )
    if len(output_streams) != 2:
        latch(ValidationError("trusted manifest helper pipes are unavailable"))
    for stream, label in output_streams:
        while True:
            descriptor: int | None = None
            try:
                descriptor = stream.fileno()
                os.set_blocking(descriptor, False)
                selector.register(descriptor, selectors.EVENT_READ, label)
                break
            except BaseException as error:
                latch(error)
                if descriptor is not None:
                    try:
                        selector.get_key(descriptor)
                    except KeyError:
                        pass
                    except BaseException as lookup_error:
                        latch(lookup_error)
                    else:
                        break
    while True:
        try:
            if not selector.get_map():
                break
            remaining = deadline - time.monotonic()
            if remaining <= 0 and not runtime_limit_exceeded:
                runtime_limit_exceeded = True
                latch(
                    ValidationError(
                        "trusted manifest helper exceeded its runtime limit"
                    )
                )
            events = selector.select(
                1.0 if runtime_limit_exceeded else min(remaining, 1.0)
            )
        except BaseException as error:
            latch(error)
            continue
        for key, _ in events:
            try:
                try:
                    chunk = os.read(key.fd, 64 * 1024)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fd)
                    continue
                retained_capacity = max(
                    _MAX_HELPER_OUTPUT_BYTES - retained_output_bytes, 0
                )
                retained = chunk[:retained_capacity]
                buffers[key.data].extend(retained)
                retained_output_bytes += len(retained)
                if len(retained) != len(chunk) and not output_limit_exceeded:
                    output_limit_exceeded = True
                    latch(
                        ValidationError(
                            "trusted manifest helper exceeded its output limit"
                        )
                    )
            except BaseException as error:
                latch(error)
    while True:
        try:
            returncode = process.wait()
            break
        except BaseException as error:
            latch(error)
    try:
        if time.monotonic() > deadline and not runtime_limit_exceeded:
            runtime_limit_exceeded = True
            latch(
                ValidationError(
                    "trusted manifest helper exceeded its runtime limit"
                )
            )
    except BaseException as error:
        latch(error)
    try:
        selector.close()
    except BaseException as error:
        latch(error)
    for stream, _ in output_streams:
        try:
            stream.close()
        except BaseException as error:
            latch(error)
    if pending_violation is not None:
        raise pending_violation
    return CommandResult(
        returncode, bytes(buffers["stdout"]), bytes(buffers["stderr"])
    )


def _replay_runner_tool_probes(
    value: Any,
    *,
    evidence: DirectorySnapshot,
    archives: dict[str, Snapshot],
    tools: dict[str, Snapshot],
    environment: dict[str, str],
) -> tuple[Snapshot, Snapshot]:
    """Independently replay the protected 41-command functional closure."""

    closure = _exact_dict(
        value,
        {"manifest", "result", "value"},
        "runner tool functional probes",
    )
    manifest_record = _exact_dict(
        closure["manifest"],
        {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
        "runner tool probe manifest",
    )
    result_record = _exact_dict(
        closure["result"],
        {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
        "runner tool probe result",
    )
    expected_records = (
        (
            manifest_record,
            "runner-tool-probe-manifest.json",
            "release-bootstrap.runner-tool-probe-manifest.v1",
        ),
        (
            result_record,
            "runner-tool-probes.json",
            "release-bootstrap.runner-tool-probes.v1",
        ),
    )
    snapshots: list[Snapshot] = []
    for record, name, archive_id in expected_records:
        if (
            record["archive_id"] != archive_id
            or record["archive_name"] != name
            or _mode(record["mode"], f"{name} mode") != _DATA_MODE
        ):
            raise ValidationError(f"{name} archive binding is wrong")
        snapshot = _read_at(
            evidence,
            name,
            name,
            maximum_bytes=_MAX_HELPER_OUTPUT_BYTES,
            expected_mode=_DATA_MODE,
        )
        if (
            snapshot.sha256 != _digest(record["sha256"], f"{name} digest")
            or len(snapshot.data)
            != _strict_int(record["size_bytes"], f"{name} size")
        ):
            raise ValidationError(f"{name} bytes do not match the marker")
        snapshots.append(snapshot)
    manifest, result_snapshot = snapshots
    result_value = _parse_canonical(result_snapshot, "runner tool probe result")
    if result_value != closure["value"]:
        raise ValidationError("runner tool probe value differs from its archive")
    result_value = _exact_dict(
        result_value,
        {
            "format",
            "host_family",
            "probe_contract_sha256",
            "schema_version",
            "tool_count",
            "tools",
        },
        "runner tool probe value",
    )
    results = result_value["tools"]
    if (
        result_value["format"]
        != "iroha-sumeragi-v2-release-tool-functional-probes"
        or _strict_int(result_value["schema_version"], "tool probe schema") != 1
        or _strict_int(result_value["tool_count"], "tool probe count") != 41
        or not isinstance(results, dict)
        or set(results) != set(tools)
        or len(tools) != 41
    ):
        raise ValidationError("runner tool probe inventory is not exact")
    for name, tool in tools.items():
        record = _exact_dict(
            results[name],
            {
                "archive_id", "exit_status", "invocation_sha256", "mode",
                "operation_id", "postcondition_sha256", "sha256",
                "size_bytes", "stderr_sha256", "stderr_size_bytes",
                "stdout_sha256", "stdout_size_bytes",
            },
            f"runner tool probe {name}",
        )
        if (
            record["archive_id"] != f"release-runner-tool.{name}.v1"
            or record["mode"] != "0500"
            or record["sha256"] != tool.sha256
            or _strict_int(record["size_bytes"], f"tool probe {name} size")
            != len(tool.data)
        ):
            raise ValidationError(f"runner tool probe {name} binding is wrong")
    probe_root = evidence.path / ".validator-runner-tool-probe"
    replay = _run_bounded(
        archives["python"].path,
        (
            "-I",
            "-S",
            str(archives["tool_probe_helper"].path),
            "--tool-manifest",
            str(manifest.path),
            "--expected-tool-manifest-sha256",
            manifest.sha256,
            "--probe-root",
            str(probe_root),
        ),
        cwd=evidence.path,
        environment=environment,
    )
    if (
        replay.returncode != 0
        or replay.stderr
        or replay.stdout != result_snapshot.data
        or probe_root.exists()
        or probe_root.is_symlink()
    ):
        raise ValidationError(
            "independent runner tool functional-probe replay failed"
        )
    return manifest, result_snapshot


def _runner_contract(
    value: Any,
    evidence: DirectorySnapshot,
    candidate: Path,
    runner_argument: Path,
    archives: dict[str, Snapshot],
    checkpoint: str,
) -> tuple[
    Snapshot,
    dict[str, str],
    Path,
    list[AliasSnapshot],
    dict[str, Snapshot],
]:
    keys = {
        "archive_id",
        "invocation",
        "closed_path_resolution",
        "environment_sha256",
        "mode",
        "output",
        "self_digest_environment_variables",
        "sha256",
        "size_bytes",
        "tool_directory",
        "tools",
    }
    runner = _exact_dict(value, keys, "runner contract")
    expected_runner = candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"
    runner_argument = _canonical_existing(runner_argument, "current runner path")
    runner_path = _canonical_existing(expected_runner, "candidate runner path")
    if runner_argument != runner_path:
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
    if runner["archive_id"] != "release-candidate.runner.v1" or runner["invocation"] != {
        "profile": "release",
        "operation_id": "sumeragi-v2.release.v1",
        "arguments": ["--release"],
        "bash_archive_id": "release-bootstrap.bash.v1",
    }:
        raise ValidationError("bootstrap runner argv is not the exact release invocation")
    resolutions = _exact_dict(
        runner["closed_path_resolution"], {"bash", "git", "python3"}, "closed PATH resolutions"
    )
    expected_resolutions = {
        "bash": "release-bootstrap.bash.v1",
        "git": "release-bootstrap.git.v1",
        "python3": "release-bootstrap.python.v1",
    }
    if resolutions != expected_resolutions:
        raise ValidationError("closed PATH resolutions do not bind the trusted aliases")
    output = _exact_dict(
        runner["output"],
        {
            "stderr_archive_id",
            "stderr_name",
            "stdout_archive_id",
            "stdout_name",
            "active_mode",
            "sealed_mode",
        },
        "runner output contract",
    )
    if output != {
        "stderr_archive_id": "release-bootstrap.runner-stderr.v1",
        "stderr_name": "runner-stderr.log",
        "stdout_archive_id": "release-bootstrap.runner-stdout.v1",
        "stdout_name": "runner-stdout.log",
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

    if runner["tool_directory"] != "runner-bin":
        raise ValidationError("runner tool directory identifier is not exact")
    tool_directory_path = evidence.path / "runner-bin"
    if tool_directory_path != evidence.path / "runner-bin":
        raise ValidationError("runner tool directory is not the private archive")
    tool_directory = _open_directory(
        tool_directory_path,
        "runner tool directory",
        expected_mode=_DIRECTORY_MODE,
    )
    tool_aliases: list[AliasSnapshot] = []
    tool_sources: dict[str, Snapshot] = {}
    tool_archive_path = evidence.path / "runner-tools"
    tool_archive_directory = _open_directory(
        tool_archive_path,
        "runner tool archive directory",
        expected_mode=_DIRECTORY_MODE,
    )
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
        archive_names = set(os.listdir(tool_archive_directory.descriptor))
        if observed_names != set(manifest_tools) or archive_names != set(manifest_tools):
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
                    "archive_id",
                    "alias_name",
                    "archive_name",
                    "mode",
                    "sha256",
                    "size_bytes",
                },
                f"runner tool {name}",
            )
            source = _read_at(
                tool_archive_directory,
                name,
                f"archived runner tool {name}",
                maximum_bytes=_MAX_TOOL_BYTES,
                expected_mode=_TOOL_MODE,
            )
            alias_path = tool_directory_path / name
            if (
                marker_record["alias_name"] != name
                or marker_record["archive_id"] != f"release-runner-tool.{name}.v1"
                or marker_record["archive_name"] != f"runner-tools/{name}"
            ):
                raise ValidationError(f"runner tool {name} alias record is wrong")
            alias = _runner_alias(
                alias_path, source.path, f"runner tool alias {name}"
            )
            expected_digest = _digest(
                manifest_record.get("sha256"), f"runner tool {name} manifest digest"
            )
            if (
                expected_digest != source.sha256
                or marker_record
                != {
                    "alias_name": name,
                    "archive_id": f"release-runner-tool.{name}.v1",
                    "archive_name": f"runner-tools/{name}",
                    "mode": "0500",
                    "sha256": source.sha256,
                    "size_bytes": len(source.data),
                }
            ):
                raise ValidationError(f"runner tool {name} integrity record is wrong")
            tool_aliases.append(alias)
            tool_sources[name] = source
    finally:
        os.close(tool_directory.descriptor)
        os.close(tool_archive_directory.descriptor)
    if runner["self_digest_environment_variables"] != _SELF_DIGEST_VARIABLES:
        raise ValidationError("runner self-digest variables have the wrong exact contract")
    environment = {
        key: item
        for key, item in os.environ.items()
        if key not in _SELF_DIGEST_VARIABLES
        and (
            key in set(_BASE_ENVIRONMENT) | {"HOME", "PATH", "TMPDIR"} | _RUNNER_EXTRA_ENV
            or key.startswith("SUMERAGI_V2_RELEASE_")
            or key.startswith("IROHA_RELEASE_")
        )
    }
    if not isinstance(environment, dict) or not all(
        isinstance(key, str)
        and _ENV_NAME_RE.fullmatch(key) is not None
        and isinstance(item, str)
        and "\0" not in item
        for key, item in environment.items()
    ):
        raise ValidationError("runner closed environment is malformed")
    environment_digest = _digest(
        runner["environment_sha256"], "runner environment digest"
    )
    if checkpoint == "entry" and hashlib.sha256(
        _canonical_json(environment)
    ).hexdigest() != environment_digest:
        raise ValidationError("runner closed environment digest is not exact")
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
            "PATH": os.pathsep.join([
                str(evidence.path),
                *(
                    [str(archives["python"].path.parent)]
                    if _FRAMEWORK_PYTHON
                    else []
                ),
                str(tool_directory),
            ]),
            "TMPDIR": str(evidence.path / "tmp"),
        }
    )
    policy = {
        "SUMERAGI_V2_RELEASE_RUNTIME_HELPER": str(archives["runtime_helper"].path),
        "SUMERAGI_V2_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": archives["runtime_helper"].sha256,
        "SUMERAGI_V2_RELEASE_TOOL_PROBE_HELPER": str(
            archives["tool_probe_helper"].path
        ),
        "SUMERAGI_V2_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": archives[
            "tool_probe_helper"
        ].sha256,
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
    aliases.update({
        "IROHA_RELEASE_RUNTIME_HELPER": str(archives["runtime_helper"].path),
        "IROHA_RELEASE_EXPECTED_RUNTIME_HELPER_SHA256": archives["runtime_helper"].sha256,
        "IROHA_RELEASE_TOOL_PROBE_HELPER": str(
            archives["tool_probe_helper"].path
        ),
        "IROHA_RELEASE_EXPECTED_TOOL_PROBE_HELPER_SHA256": archives[
            "tool_probe_helper"
        ].sha256,
        "IROHA_RELEASE_SDK_DEPENDENCY_BUNDLE_MANIFEST": str(
            archives["sdk_dependency_bundle_manifest"].path
        ),
        "IROHA_RELEASE_EXPECTED_SDK_DEPENDENCY_BUNDLE_MANIFEST_SHA256": (
            archives["sdk_dependency_bundle_manifest"].sha256
        ),
    })
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
        "runner-tools",
        "runner-stderr.log",
        "runner-stdout.log",
        "runner-tool-probe-manifest.json",
        "runner-tool-probes.json",
        "tmp",
        *{
            name
            for name in _TRUSTED_ARCHIVE_NAMES.values()
            if "/" not in name
        },
        *set(_IDENTITY_RECORD_NAMES.values()),
        *set(_APPROVAL_ATTESTATION_NAMES.values()),
        *set(_BOOTSTRAP_COMPONENT_SHA256),
        *set(_RECEIPT_VALIDATOR_COMPONENT_SHA256),
        _APPROVAL_SET_ATTESTATION_NAME,
    }
    if _FRAMEWORK_PYTHON:
        required.update({"python-runtime", "python-runtime-input.json"})
    try:
        observed = set(os.listdir(evidence.descriptor))
    except OSError as error:
        raise ValidationError("bootstrap evidence inventory is unavailable") from error
    permitted = set(required)
    retained = {
        "RELEASE_COMPLETED.json",
        "receipt-validation-ack.json",
        "sealed-identity.json",
        "release-retained-inventory.json",
        "release-runner-result.json",
    }
    if checkpoint == "sealed":
        permitted.update(retained if "release-runner-result.json" in observed else {"release-runner"})
    if observed != permitted:
        raise ValidationError("bootstrap evidence directory has an unexpected top-level inventory")
    for name in ("home", "tmp", "runner-bin", "runner-tools"):
        item = os.stat(name, dir_fd=evidence.descriptor, follow_symlinks=False)
        if (
            not stat.S_ISDIR(item.st_mode)
            or item.st_uid != os.getuid()
            or stat.S_IMODE(item.st_mode) != _DIRECTORY_MODE
        ):
            raise ValidationError(f"bootstrap {name} directory is not private")
    if _FRAMEWORK_PYTHON:
        item = os.stat(
            "python-runtime",
            dir_fd=evidence.descriptor,
            follow_symlinks=False,
        )
        if (
            not stat.S_ISDIR(item.st_mode)
            or item.st_uid != os.getuid()
            or stat.S_IMODE(item.st_mode) != 0o500
        ):
            raise ValidationError(
                "bootstrap framework Python runtime is not protected"
            )
    for name in ("runner-stdout.log", "runner-stderr.log"):
        item = os.stat(name, dir_fd=evidence.descriptor, follow_symlinks=False)
        if (
            not stat.S_ISREG(item.st_mode)
            or item.st_uid != os.getuid()
            or item.st_nlink != 1
            or stat.S_IMODE(item.st_mode) != 0o600
        ):
            raise ValidationError(f"bootstrap {name} is not one active private log")
    if checkpoint == "sealed" and "release-runner" in observed:
        item = os.stat("release-runner", dir_fd=evidence.descriptor, follow_symlinks=False)
        if (
            not stat.S_ISDIR(item.st_mode)
            or item.st_uid != os.getuid()
            or stat.S_IMODE(item.st_mode) != _DIRECTORY_MODE
        ):
            raise ValidationError("sealed release-runner subtree is not one private directory")
    for name in retained & observed:
        item = os.stat(name, dir_fd=evidence.descriptor, follow_symlinks=False)
        if (
            not stat.S_ISREG(item.st_mode)
            or item.st_uid != os.getuid()
            or item.st_nlink != 1
            or stat.S_IMODE(item.st_mode) != 0o400
        ):
            raise ValidationError(f"retained release evidence {name} is unsafe")


def _sealed_release_root(evidence: DirectorySnapshot) -> tuple[Path, Snapshot | None]:
    result_path = evidence.path / "release-runner-result.json"
    if not result_path.exists() and not result_path.is_symlink():
        return evidence.path / "release-runner" / "source", None
    snapshot = _read_path(
        result_path,
        "protected outer release result",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
        expected_mode=0o400,
    )
    value = _exact_dict(
        _parse_canonical(snapshot, "protected outer release result"),
        {
            "format", "schema_version", "invocation_archive_id",
            "source_archive_id",
            "source_manifest_sha256", "sealed_identity", "receipt", "inventory",
            "receipt_validation",
        },
        "protected outer release result",
    )
    if (
        value["format"] != "iroha-sumeragi-v2-retained-release-evidence"
        or value["schema_version"] != 2
        or value["invocation_archive_id"] != "release-retained.invocation.v1"
        or value["source_archive_id"] != "release-retained.source.v1"
    ):
        raise ValidationError("protected outer release result schema is not exact")
    invocation_value = os.environ.get("IROHA_RELEASE_INVOCATION_ROOT")
    if not isinstance(invocation_value, str) or not invocation_value:
        raise ValidationError(
            "path-free retained result lacks private invocation provenance"
        )
    invocation = _canonical_existing(
        Path(invocation_value), "retained invocation root"
    )
    source = _canonical_existing(
        invocation / "source", "retained source root"
    )
    if source != invocation / "source" or invocation == evidence.path or invocation in evidence.path.parents or evidence.path in invocation.parents:
        raise ValidationError("retained release source is not external and exact")
    _digest(_string(value["source_manifest_sha256"], "retained source digest"), "retained source digest")
    for field, local_path, name, archive_id in (
        ("receipt", invocation / "output" / "release" / "RELEASE_COMPLETED.json", "RELEASE_COMPLETED.json", "release-terminal.receipt.v1"),
        ("sealed_identity", invocation / "sealed-identity.json", "sealed-identity.json", "release-retained.identity.v1"),
        ("inventory", invocation / "retained-evidence-inventory.json", "release-retained-inventory.json", "release-retained.inventory.v2"),
        ("receipt_validation", invocation / "receipt-validation-ack.json", "receipt-validation-ack.json", "release-retained.receipt-validation-ack.v3"),
    ):
        record = _exact_dict(
            value[field], {"archive_id", "mode", "sha256", "size_bytes"},
            f"retained {field}",
        )
        protected = _read_path(evidence.path / name, f"protected retained {field}", maximum_bytes=256 * 1024 * 1024, expected_mode=0o400)
        local = _read_path(
            local_path,
            f"retained {field}",
            maximum_bytes=256 * 1024 * 1024,
            expected_mode=0o400,
        )
        if (
            record["archive_id"] != archive_id
            or record["mode"] != "0400"
            or _digest(_string(record["sha256"], f"retained {field} digest"), f"retained {field} digest") != protected.sha256
            or _strict_int(record["size_bytes"], f"retained {field} size") != protected.size
            or local.data != protected.data
        ):
            raise ValidationError(f"protected retained {field} binding changed")
    return source, snapshot


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
        if _strict_int(marker_value["schema_version"], "bootstrap schema_version") != 2:
            raise ValidationError("bootstrap completion marker must use schema 2")
        if marker_value["trust_boundary"] != {
            "bootstrap_authentication": "external prerequisite",
            "release_image_and_dynamic_loader": "external prerequisite",
            "same_uid_and_trusted_ancestor_owners": True,
        }:
            raise ValidationError("bootstrap completion marker has the wrong trust boundary")
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
        (
            archives,
            source_snapshots,
            framework_python_directory,
        ) = _trusted_inputs(
            marker_value["trusted_inputs"], evidence, candidate,
        )
        approval_snapshots = _release_approvals(
            marker_value["release_approvals"],
            evidence=evidence,
            identity=identity,
            archives=archives,
        )
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
            marker_value["runner"], evidence, candidate, args.runner, archives,
            args.checkpoint,
        )
        probes = _exact_dict(
            marker_value["trusted_execution_probes"],
            {"bash", "python", "runner_tool_closure"},
            "trusted execution probes",
        )
        python_probe_code = "import sys;sys.stdout.write(sys.executable+'\\n')"
        expected_python_stdout = f"{archives['python'].path}\n".encode()
        if {key: probes[key] for key in ("bash", "python")} != {
            "bash": {"argv": [str(archives["bash"].path), "-c", ":"], "exit_status": 0},
            "python": {
                "argv": [
                    str(archives["python"].path),
                    "-I",
                    "-S",
                    "-c",
                    python_probe_code,
                ],
                "expected_executable": (
                    "python-runtime/bin/python3"
                    if _FRAMEWORK_PYTHON
                    else "python3"
                ),
                "exit_status": 0,
                "stdout_sha256": hashlib.sha256(
                    expected_python_stdout
                ).hexdigest(),
                "stdout_size_bytes": len(expected_python_stdout),
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
        tool_probe_manifest, tool_probe_result = _replay_runner_tool_probes(
            probes["runner_tool_closure"],
            evidence=evidence,
            archives=archives,
            tools=runner_tool_sources,
            environment=environment,
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
            sealed_root, release_result_snapshot = _sealed_release_root(evidence)
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
        if _FRAMEWORK_PYTHON:
            refreshed_inventory, refreshed_directory = (
                _validate_framework_python_runtime(
                    marker_value["trusted_inputs"]["python"]["runtime"],
                    evidence,
                )
            )
            if (
                framework_python_directory is None
                or refreshed_inventory
                != archives["python_runtime_inventory"]
            ):
                os.close(refreshed_directory.descriptor)
                raise ValidationError(
                    "framework Python runtime changed during validation"
                )
            _revalidate_directory(
                framework_python_directory,
                "framework Python runtime",
            )
            os.close(framework_python_directory.descriptor)
            framework_python_directory = refreshed_directory
        unique_records = {snapshot.path: snapshot for snapshot in records.values()}
        all_snapshots = [
            marker,
            identity_file,
            runner,
            cargo_lock,
            expected_validator,
            *archives.values(),
            *source_snapshots,
            *approval_snapshots,
            *runner_tool_sources.values(),
            tool_probe_manifest,
            tool_probe_result,
            *unique_records.values(),
        ]
        if args.checkpoint == "sealed" and release_result_snapshot is not None:
            all_snapshots.append(release_result_snapshot)
        if current_validator.path != expected_validator.path:
            all_snapshots.append(current_validator)
        if sealed_runner is not None:
            all_snapshots.append(sealed_runner)
        source_paths = {snapshot.path for snapshot in source_snapshots}
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
                alias.path,
                (alias.path.parent / alias.target).resolve(strict=True),
                alias.path.name,
            ) != alias:
                raise ValidationError("runner tool alias changed during validation")
        _revalidate_directory(candidate_directory, "current candidate root")
        _revalidate_directory(evidence, "bootstrap evidence directory")
        if framework_python_directory is not None:
            _revalidate_directory(
                framework_python_directory,
                "framework Python runtime",
            )
        _inventory(evidence, args.checkpoint)
    finally:
        if "framework_python_directory" in locals() and framework_python_directory is not None:
            os.close(framework_python_directory.descriptor)
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
