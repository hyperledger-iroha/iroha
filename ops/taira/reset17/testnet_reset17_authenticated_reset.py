#!/usr/bin/env python3
"""Authenticated fail-stop reset17 corridor for public Taira.

The controller accepts one canonical, SSH-signed candidate manifest, verifies
every public byte and every private-file *identity* without reading private
contents, validates the four canonical ``iroha3d_taira`` configurations and
LaunchAgents, and emits a hash-addressed plan.  ``check-plan`` and ``apply``
repeat all authentication and safety checks.  Apply stages a new immutable
release and generation-specific data roots; it never deletes predecessor data.

This file deliberately contains no operator keys, signer bytes, bearer tokens,
or candidate-specific hashes.  The reviewed manifest SHA-256, clean source
commit, and allowed-signers SHA-256 are mandatory out-of-band preflight pins.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import fcntl
import hashlib
import http.client
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import stat
import subprocess
import sys
import time
import tomllib
from typing import Any, Iterable, Mapping, NoReturn, Sequence
from urllib.parse import urlsplit


MANIFEST_SCHEMA = "inori.taira.reset17-public-bundle-manifest.v1"
PLAN_SCHEMA = "inori.taira.reset17-authenticated-plan.v1"
RESULT_SCHEMA = "inori.taira.reset17-authenticated-result.v1"
PREDECESSOR_SCHEMA = "inori.taira.reset17-predecessor-snapshot.v1"
GENERATION = "reset17"
SIGNING_IDENTITY = "taira-reset17-release"
SIGNING_NAMESPACE = "taira-reset17"
EXPECTED_COMMIT_SIGNER_FINGERPRINT = (
    "SHA256:ykCGGqELtdtBpdJ/DTT6ROwpqCCGKYACMhUfdzTxi+g"
)

TAIRA_CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
TAIRA_CHAIN_DISCRIMINANT = 369
TAIRA_VALIDATORS = 4
TAIRA_RUNTIME_SIGNER_BYTES = 71
TAIRA_RUNTIME_SIGNER_REVISION = 1
TAIRA_RUNTIME_SIGNER_HANDLE_PREFIX = "software://taira/inrou/"
TAIRA_NEXUS_LOCAL_BUDGET_BYTES = 68_719_476_736
TAIRA_SORAFS_CAPACITY_BYTES = 13_743_895_347
TAIRA_STORAGE_WEIGHTS = {
    "kura_blocks_bps": 5_500,
    "wsv_snapshots_bps": 2_000,
    "sorafs_bps": 2_000,
    "soranet_spool_bps": 250,
    "soravpn_spool_bps": 250,
}

BPNG_ASSET_ALIAS = "kina#bpng"
BPNG_ASSET_DEFINITION = "839FV3NJC8NfgWQvghXU2hEFQm9a"
BPNG_ASSET_DOMAIN = "bpng.bpng"
BPNG_ASSET_SCALE = 2
BPNG_LANE_ID = 3
BPNG_LANE_ALIAS = "dpn"
BPNG_PHYSICAL_DATASPACE_ID = 10
BPNG_PHYSICAL_DATASPACE_ALIAS = "bpng"

NATIVE_BRIDGE_ABI = 22
KAGEMUSHA_DATA_ABI = 4
EXACT12_PROTOCOLS = (
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
)

MIN_FREE_RESERVE_BYTES = 32 * 1024**3
FREE_RESERVE_BPS = 1_000
MAX_JSON_BYTES = 4 * 1024 * 1024
MAX_SIGNATURE_BYTES = 128 * 1024
MAX_PUBLIC_FILE_BYTES = 1024 * 1024 * 1024
MAX_PRIVATE_FILE_BYTES = 4096
MAX_HELPER_OUTPUT_BYTES = 1024 * 1024
PLAN_CONFIRMATION_PREFIX = "FAIL-STOP-TAIRA-RESET17:"

SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
SSH_FINGERPRINT_RE = re.compile(r"SHA256:[A-Za-z0-9+/]{43}")
RUN_ID_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,95}")
PUBLIC_KEY_RE = re.compile(r"[0-9a-f]{64}")
NETWORK_ID_RE = re.compile(r"[0-9a-f]{64}")

REQUIRED_ARTIFACTS = frozenset(
    {
        "iroha3d_taira",
        "kagami",
        "iroha",
        "taira_operator_status",
        "fd198_supervisor",
        "genesis_manifest",
        "signed_genesis",
        "preparation_tool",
    }
)
PRIVATE_ROLES = frozenset(
    {
        "validator_signer",
        "soranet_transport",
        "streaming_identity",
        "faucet_signer",
        "soracloud_runtime_signer",
    }
)

EXPECTED_BUILD_COMMANDS = (
    (
        "cargo",
        "iroha-fast",
        "--",
        "build",
        "--frozen",
        "--offline",
        "--target",
        "aarch64-apple-darwin",
        "--release",
        "-p",
        "irohad",
        "--bin",
        "iroha3d_taira",
    ),
    (
        "cargo",
        "iroha-fast",
        "--",
        "build",
        "--frozen",
        "--offline",
        "--target",
        "aarch64-apple-darwin",
        "--release",
        "-p",
        "iroha_kagami",
        "--bin",
        "kagami",
    ),
    (
        "cargo",
        "iroha-fast",
        "--",
        "build",
        "--frozen",
        "--offline",
        "--target",
        "aarch64-apple-darwin",
        "--release",
        "-p",
        "iroha_cli",
        "--bin",
        "iroha",
    ),
)


class Reset17Error(RuntimeError):
    """A bounded, payload-free reset17 refusal."""


def _duplicate_rejecting_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise Reset17Error("JSON contains a duplicate key")
        value[key] = item
    return value


def canonical_json_bytes(value: Any) -> bytes:
    try:
        rendered = json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise Reset17Error("value cannot be encoded as canonical JSON") from error
    return (rendered + "\n").encode("utf-8")


def _reject_json_constant(_value: str) -> NoReturn:
    raise Reset17Error("JSON contains a non-finite number")


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _read_bounded(path: Path, maximum: int, label: str) -> bytes:
    try:
        metadata = os.lstat(path)
    except FileNotFoundError as error:
        raise Reset17Error(f"{label} is missing") from error
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or not 1 <= metadata.st_size <= maximum
    ):
        raise Reset17Error(f"{label} is not a bounded single-link regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise Reset17Error(f"{label} cannot be opened safely") from error
    try:
        before = os.fstat(descriptor)
        if (
            before.st_dev != metadata.st_dev
            or before.st_ino != metadata.st_ino
            or before.st_size != metadata.st_size
        ):
            raise Reset17Error(f"{label} changed while opening")
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        payload = b"".join(chunks)
        after = os.fstat(descriptor)
        stable = (
            "st_dev",
            "st_ino",
            "st_uid",
            "st_mode",
            "st_nlink",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if any(getattr(before, field) != getattr(after, field) for field in stable):
            raise Reset17Error(f"{label} changed while reading")
        if len(payload) != metadata.st_size or len(payload) > maximum:
            raise Reset17Error(f"{label} has an invalid bounded length")
        return payload
    finally:
        os.close(descriptor)


def _open_and_read_bounded(
    path: Path, maximum: int, label: str
) -> tuple[int, bytes]:
    """Return one verified open descriptor and the bytes read from that FD."""

    _reject_symlink_components(path, label)
    try:
        metadata = os.lstat(path)
    except FileNotFoundError as error:
        raise Reset17Error(f"{label} is missing") from error
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or not 1 <= metadata.st_size <= maximum
    ):
        raise Reset17Error(f"{label} is not a bounded single-link regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise Reset17Error(f"{label} cannot be opened safely") from error
    try:
        before = os.fstat(descriptor)
        if (
            before.st_dev != metadata.st_dev
            or before.st_ino != metadata.st_ino
            or before.st_size != metadata.st_size
        ):
            raise Reset17Error(f"{label} changed while opening")
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        payload = b"".join(chunks)
        after = os.fstat(descriptor)
        if _file_stat_identity(before) != _file_stat_identity(after):
            raise Reset17Error(f"{label} changed while reading")
        if len(payload) != metadata.st_size or len(payload) > maximum:
            raise Reset17Error(f"{label} has an invalid bounded length")
        os.lseek(descriptor, 0, os.SEEK_SET)
        return descriptor, payload
    except BaseException:
        os.close(descriptor)
        raise


def _file_stat_identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_uid,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def read_json(path: Path, maximum: int = MAX_JSON_BYTES, canonical: bool = True) -> Any:
    payload = _read_bounded(path, maximum, "JSON file")
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_duplicate_rejecting_object,
            parse_constant=_reject_json_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise Reset17Error("JSON file is malformed") from error
    if canonical and canonical_json_bytes(value) != payload:
        raise Reset17Error("JSON file is not canonical")
    return value


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise Reset17Error(f"{label} must be an object")
    return value


def _require_exact_keys(
    value: Mapping[str, Any], required: Iterable[str], label: str
) -> None:
    expected = set(required)
    actual = set(value)
    if actual != expected:
        raise Reset17Error(f"{label} has missing or unexpected fields")


def _require_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise Reset17Error(f"{label} must be a non-empty string")
    return value


def _require_int(value: Any, label: str, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum:
        raise Reset17Error(f"{label} must be an integer >= {minimum}")
    return value


def _absolute_path(value: Any, label: str) -> Path:
    raw = _require_string(value, label)
    path = Path(raw)
    if (
        not path.is_absolute()
        or ".." in path.parts
        or raw != os.path.normpath(raw)
        or "//" in raw
    ):
        raise Reset17Error(f"{label} must be a normalized absolute path")
    return path


def _relative_path(value: Any, label: str) -> Path:
    raw = _require_string(value, label)
    path = Path(raw)
    if (
        path.is_absolute()
        or ".." in path.parts
        or str(path) in ("", ".")
        or raw != os.path.normpath(raw)
        or raw.startswith("./")
        or "//" in raw
    ):
        raise Reset17Error(f"{label} must be a normalized relative path")
    return path


def _is_below(path: Path, parent: Path) -> bool:
    try:
        relative = path.relative_to(parent)
        return relative != Path(".")
    except ValueError:
        return False


def _reject_symlink_components(path: Path, label: str) -> None:
    current = Path(path.anchor)
    for component in path.parts[1:]:
        current /= component
        try:
            metadata = os.lstat(current)
        except FileNotFoundError:
            return
        if stat.S_ISLNK(metadata.st_mode):
            raise Reset17Error(f"{label} contains a symlink component")


def _validate_owner_private_ancestors(path: Path, root: Path, label: str) -> None:
    if not _is_below(path, root):
        raise Reset17Error(f"{label} escapes its private root")
    current = root
    for component in path.relative_to(root).parts[:-1]:
        current /= component
        try:
            metadata = os.lstat(current)
        except FileNotFoundError as error:
            raise Reset17Error(f"{label} has a missing private ancestor") from error
        if (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) != 0o700
        ):
            raise Reset17Error(f"{label} has a non-private ancestor")


@dataclass(frozen=True)
class FileRecord:
    path: Path
    sha256: str
    size: int
    mode: int
    install_relative: Path

    @classmethod
    def parse(cls, value: Any, label: str) -> "FileRecord":
        raw = _require_object(value, label)
        _require_exact_keys(
            raw, ("path", "sha256", "size", "mode", "install_relative"), label
        )
        digest = _require_string(raw["sha256"], f"{label} sha256")
        if SHA256_RE.fullmatch(digest) is None:
            raise Reset17Error(f"{label} sha256 is invalid")
        mode = _require_int(raw["mode"], f"{label} mode")
        if mode not in (0o400, 0o444, 0o500, 0o555):
            raise Reset17Error(f"{label} mode is not immutable public-file mode")
        return cls(
            path=_relative_path(raw["path"], f"{label} path"),
            sha256=digest,
            size=_require_int(raw["size"], f"{label} size", 1),
            mode=mode,
            install_relative=_relative_path(
                raw["install_relative"], f"{label} install path"
            ),
        )

    def source(self, bundle: Path) -> Path:
        return bundle / self.path

    def destination(self, release_dir: Path) -> Path:
        return release_dir / self.install_relative


@dataclass(frozen=True)
class PrivateFileIdentity:
    path: Path
    device: int
    inode: int
    uid: int
    mode: int
    links: int
    size: int
    modified_ns: int
    changed_ns: int

    def json(self) -> dict[str, Any]:
        return {
            "path": str(self.path),
            "device": self.device,
            "inode": self.inode,
            "uid": self.uid,
            "mode": self.mode,
            "links": self.links,
            "size": self.size,
            "modified_ns": self.modified_ns,
            "changed_ns": self.changed_ns,
        }


def _private_identity(path: Path, label: str, exact_size: int | None) -> PrivateFileIdentity:
    _reject_symlink_components(path, label)
    try:
        metadata = os.lstat(path)
    except FileNotFoundError as error:
        raise Reset17Error(f"{label} is missing") from error
    size_ok = (
        metadata.st_size == exact_size
        if exact_size is not None
        else 1 <= metadata.st_size <= MAX_PRIVATE_FILE_BYTES
    )
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o600
        or metadata.st_nlink != 1
        or not size_ok
    ):
        raise Reset17Error(f"{label} has untrusted private-file metadata")
    return PrivateFileIdentity(
        path,
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_uid,
        stat.S_IMODE(metadata.st_mode),
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _verify_file(record: FileRecord, bundle: Path, executable: bool) -> None:
    source = record.source(bundle)
    _reject_symlink_components(source, "candidate public file")
    descriptor, payload = _open_and_read_bounded(
        source, MAX_PUBLIC_FILE_BYTES, "candidate public file"
    )
    try:
        metadata = os.fstat(descriptor)
        if (
            metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) != record.mode
            or metadata.st_size != record.size
            or sha256_bytes(payload) != record.sha256
            or bool(metadata.st_mode & 0o111) != executable
        ):
            raise Reset17Error("candidate public file identity does not match manifest")
    finally:
        os.close(descriptor)


def _verify_external_public_file(
    path: Path, expected_sha256: str, expected_size: int, label: str
) -> None:
    _reject_symlink_components(path, label)
    descriptor, payload = _open_and_read_bounded(path, MAX_PUBLIC_FILE_BYTES, label)
    try:
        metadata = os.fstat(descriptor)
        if (
            metadata.st_uid not in (0, os.geteuid())
            or metadata.st_size != expected_size
            or sha256_bytes(payload) != expected_sha256
            or not (metadata.st_mode & 0o111)
        ):
            raise Reset17Error(f"{label} identity is foreign")
    finally:
        os.close(descriptor)


def _file_record_json(record: FileRecord) -> dict[str, Any]:
    return {
        "path": str(record.path),
        "sha256": record.sha256,
        "size": record.size,
        "mode": record.mode,
        "install_relative": str(record.install_relative),
    }


def verify_manifest_signature(
    manifest_path: Path,
    signature_path: Path,
    allowed_signers_path: Path,
    expected_manifest_sha256: str,
    expected_allowed_signers_sha256: str,
) -> tuple[bytes, str]:
    if SHA256_RE.fullmatch(expected_manifest_sha256) is None:
        raise Reset17Error("expected manifest SHA-256 is invalid")
    if SHA256_RE.fullmatch(expected_allowed_signers_sha256) is None:
        raise Reset17Error("expected allowed-signers SHA-256 is invalid")
    manifest_fd, manifest = _open_and_read_bounded(
        manifest_path, MAX_JSON_BYTES, "reset17 manifest"
    )
    allowed_fd: int | None = None
    signature_fd: int | None = None
    try:
        if sha256_bytes(manifest) != expected_manifest_sha256:
            raise Reset17Error("reset17 manifest does not match the reviewed digest")
        allowed_fd, allowed = _open_and_read_bounded(
            allowed_signers_path,
            MAX_SIGNATURE_BYTES,
            "reset17 allowed-signers file",
        )
        if sha256_bytes(allowed) != expected_allowed_signers_sha256:
            raise Reset17Error(
                "reset17 allowed-signers file does not match reviewed digest"
            )
        signature_fd, signature = _open_and_read_bounded(
            signature_path, MAX_SIGNATURE_BYTES, "reset17 manifest signature"
        )
        command = (
            "/usr/bin/ssh-keygen",
            "-Y",
            "verify",
            "-f",
            f"/dev/fd/{allowed_fd}",
            "-I",
            SIGNING_IDENTITY,
            "-n",
            SIGNING_NAMESPACE,
            "-s",
            f"/dev/fd/{signature_fd}",
        )
        try:
            result = subprocess.run(
                command,
                input=manifest,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=15,
                check=False,
                pass_fds=(allowed_fd, signature_fd),
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise Reset17Error(
                "reset17 manifest signature verification is unavailable"
            ) from error
        if (
            result.returncode != 0
            or len(result.stdout) > MAX_HELPER_OUTPUT_BYTES
            or len(result.stderr) > MAX_HELPER_OUTPUT_BYTES
        ):
            raise Reset17Error("reset17 manifest signature is not trusted")
        return manifest, sha256_bytes(signature)
    finally:
        os.close(manifest_fd)
        if allowed_fd is not None:
            os.close(allowed_fd)
        if signature_fd is not None:
            os.close(signature_fd)


@dataclass(frozen=True)
class ValidatorCandidate:
    index: int
    label: str
    data_root: Path
    torii_url: str
    p2p_port: int
    config: FileRecord
    launch_agent: FileRecord
    private_files: Mapping[str, Path]
    private_identities: Mapping[str, PrivateFileIdentity]
    signer_public_key_hex: str
    signer_handle: str
    signer_authority: str
    signer_policy_digest_hex: str
    signer_launch_path: Path
    validator_public_key: str
    trusted_peer_public_keys: tuple[str, ...]
    trusted_peer_endpoints: tuple[tuple[str, str], ...]
    trusted_pop_public_keys: tuple[str, ...]


@dataclass(frozen=True)
class Candidate:
    bundle: Path
    manifest_path: Path
    signature_path: Path
    allowed_signers_path: Path
    manifest_sha256: str
    signature_sha256: str
    allowed_signers_sha256: str
    raw: Mapping[str, Any]
    release_id: str
    source_commit: str
    release_dir: Path
    launch_agents_dir: Path
    control_root: Path
    python_path: Path
    python_sha256: str
    python_size: int
    require_single_data_volume: bool
    artifacts: Mapping[str, FileRecord]
    validators: tuple[ValidatorCandidate, ...]
    network_id: str


def _parse_source(value: Any, expected_source_commit: str) -> str:
    source = _require_object(value, "source")
    _require_exact_keys(
        source,
        (
            "commit",
            "tree",
            "parent",
            "commit_signer_fingerprint",
            "source_date_epoch",
            "cargo_target_dir",
            "rustc_version",
            "cargo_version",
            "build_commands",
        ),
        "source",
    )
    commit = _require_string(source["commit"], "source commit")
    if COMMIT_RE.fullmatch(commit) is None or commit != expected_source_commit:
        raise Reset17Error("source commit is not the reviewed clean commit")
    for field in ("tree", "parent"):
        if COMMIT_RE.fullmatch(_require_string(source[field], f"source {field}")) is None:
            raise Reset17Error(f"source {field} is invalid")
    fingerprint = _require_string(
        source["commit_signer_fingerprint"], "commit signer fingerprint"
    )
    if (
        SSH_FINGERPRINT_RE.fullmatch(fingerprint) is None
        or fingerprint != EXPECTED_COMMIT_SIGNER_FINGERPRINT
    ):
        raise Reset17Error("clean source commit signer is not trusted")
    _require_int(source["source_date_epoch"], "source date epoch", 1)
    target_dir = _absolute_path(source["cargo_target_dir"], "Cargo target directory")
    lowered = str(target_dir).lower()
    if "reset17" not in lowered or lowered.endswith("/routine") or "/routine/" in lowered:
        raise Reset17Error("reset17 requires a dedicated authenticated Cargo target lane")
    for field in ("rustc_version", "cargo_version"):
        _require_string(source[field], field.replace("_", " "))
    commands = source["build_commands"]
    if not isinstance(commands, list):
        raise Reset17Error("build commands must be a list")
    normalized: list[tuple[str, ...]] = []
    for command in commands:
        if not isinstance(command, list) or not all(
            isinstance(argument, str) and argument for argument in command
        ):
            raise Reset17Error("build command is malformed")
        normalized.append(tuple(command))
    if tuple(normalized) != EXPECTED_BUILD_COMMANDS:
        raise Reset17Error("reset17 build commands are not the exact reviewed sequence")
    return commit


def _validate_protocols(value: Any) -> None:
    protocols = _require_object(value, "protocols")
    _require_exact_keys(
        protocols,
        ("native_bridge_abi", "kagemusha_data_abi", "exact12"),
        "protocols",
    )
    if protocols["native_bridge_abi"] != NATIVE_BRIDGE_ABI:
        raise Reset17Error("candidate is not authenticated ABI22")
    if protocols["kagemusha_data_abi"] != KAGEMUSHA_DATA_ABI:
        raise Reset17Error("candidate changes the Kagemusha V4 data ABI")
    if protocols["exact12"] != list(EXACT12_PROTOCOLS):
        raise Reset17Error("candidate does not bind the ordered Exact12 protocol set")


def _validate_bpng(value: Any) -> None:
    bpng = _require_object(value, "BPNG profile")
    _require_exact_keys(
        bpng,
        (
            "asset_alias",
            "asset_definition",
            "asset_domain",
            "scale",
            "lane_id",
            "lane_alias",
            "physical_dataspace_id",
            "physical_dataspace_alias",
        ),
        "BPNG profile",
    )
    expected = {
        "asset_alias": BPNG_ASSET_ALIAS,
        "asset_definition": BPNG_ASSET_DEFINITION,
        "asset_domain": BPNG_ASSET_DOMAIN,
        "scale": BPNG_ASSET_SCALE,
        "lane_id": BPNG_LANE_ID,
        "lane_alias": BPNG_LANE_ALIAS,
        "physical_dataspace_id": BPNG_PHYSICAL_DATASPACE_ID,
        "physical_dataspace_alias": BPNG_PHYSICAL_DATASPACE_ALIAS,
    }
    if bpng != expected:
        raise Reset17Error("candidate changes the canonical Digital Kina/BPNG routing profile")


def _parse_deployment(value: Any, release_id: str) -> dict[str, Any]:
    deployment = _require_object(value, "deployment")
    _require_exact_keys(
        deployment,
        (
            "uid",
            "launch_domain",
            "python",
            "release_dir",
            "launch_agents_dir",
            "control_root",
            "require_single_data_volume",
            "free_reserve_bytes",
            "free_reserve_bps",
        ),
        "deployment",
    )
    uid = _require_int(deployment["uid"], "deployment uid", 1)
    if uid != os.geteuid() or deployment["launch_domain"] != f"gui/{uid}":
        raise Reset17Error("deployment uid/domain does not match this service user")
    release_dir = _absolute_path(deployment["release_dir"], "release directory")
    if release_dir.name != release_id:
        raise Reset17Error("release directory is not bound to release id")
    launch_agents_dir = _absolute_path(
        deployment["launch_agents_dir"], "LaunchAgents directory"
    )
    control_root = _absolute_path(deployment["control_root"], "control root")
    for path, label in (
        (release_dir, "release directory"),
        (launch_agents_dir, "LaunchAgents directory"),
        (control_root, "control root"),
    ):
        _reject_symlink_components(path, label)
    python = _require_object(deployment["python"], "isolated Python")
    _require_exact_keys(python, ("path", "sha256", "size"), "isolated Python")
    python_path = _absolute_path(python["path"], "isolated Python path")
    python_sha = _require_string(python["sha256"], "isolated Python sha256")
    if SHA256_RE.fullmatch(python_sha) is None:
        raise Reset17Error("isolated Python digest is invalid")
    python_size = _require_int(python["size"], "isolated Python size", 1)
    if deployment["require_single_data_volume"] is not True:
        raise Reset17Error("public Taira reset17 must bind one shared data volume")
    if deployment["free_reserve_bytes"] != MIN_FREE_RESERVE_BYTES:
        raise Reset17Error("reset17 free-space floor was weakened")
    if deployment["free_reserve_bps"] != FREE_RESERVE_BPS:
        raise Reset17Error("reset17 proportional free-space reserve was weakened")
    return {
        "uid": uid,
        "launch_domain": deployment["launch_domain"],
        "release_dir": release_dir,
        "launch_agents_dir": launch_agents_dir,
        "control_root": control_root,
        "python_path": python_path,
        "python_sha256": python_sha,
        "python_size": python_size,
        "require_single_data_volume": True,
    }


def _load_toml(record: FileRecord, bundle: Path) -> dict[str, Any]:
    payload = _read_bounded(record.source(bundle), MAX_JSON_BYTES, "validator config")
    try:
        value = tomllib.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise Reset17Error("validator config is malformed") from error
    if not isinstance(value, dict):
        raise Reset17Error("validator config is not a table")
    return value


def _nested_table(value: Mapping[str, Any], keys: Sequence[str], label: str) -> dict[str, Any]:
    current: Any = value
    for key in keys:
        if not isinstance(current, dict) or not isinstance(current.get(key), dict):
            raise Reset17Error(f"{label} is missing")
        current = current[key]
    return current


def _config_absolute(value: Any, label: str) -> Path:
    return _absolute_path(value, f"config {label}")


def _validate_config(
    config_record: FileRecord,
    bundle: Path,
    data_root: Path,
    torii_url: str,
    p2p_port: int,
    private_files: Mapping[str, Path],
    runtime_signer: Mapping[str, Any],
    signed_genesis_destination: Path,
) -> tuple[
    str,
    tuple[str, ...],
    tuple[tuple[str, str], ...],
    tuple[str, ...],
]:
    config = _load_toml(config_record, bundle)
    if config.get("chain") != TAIRA_CHAIN_ID:
        raise Reset17Error("validator config changes canonical Taira chain id")
    if config.get("chain_discriminant") != TAIRA_CHAIN_DISCRIMINANT:
        raise Reset17Error("validator config changes canonical Taira discriminant")
    if _nested_table(config, ("sumeragi",), "Sumeragi config").get("role") != "validator":
        raise Reset17Error("validator config does not declare the validator role")
    validator_public_key = _require_string(
        config.get("public_key"), "validator public key"
    )
    trusted_peers = config.get("trusted_peers")
    trusted_pop = config.get("trusted_peers_pop")
    if (
        not isinstance(trusted_peers, list)
        or not trusted_peers
        or not all(isinstance(peer, str) and "@" in peer for peer in trusted_peers)
        or not isinstance(trusted_pop, list)
        or not trusted_pop
        or not all(isinstance(item, dict) for item in trusted_pop)
    ):
        raise Reset17Error("validator trusted-peer topology is malformed")
    trusted_peer_endpoints_list: list[tuple[str, str]] = []
    for peer in trusted_peers:
        public_key, endpoint = peer.split("@", 1)
        if (
            not public_key
            or re.fullmatch(r"127\.0\.0\.1:[0-9]{4,5}", endpoint) is None
        ):
            raise Reset17Error("validator trusted-peer endpoint is not canonical loopback")
        endpoint_port = int(endpoint.rsplit(":", 1)[1])
        if not 1_024 <= endpoint_port <= 65_535:
            raise Reset17Error("validator trusted-peer endpoint port is invalid")
        trusted_peer_endpoints_list.append((public_key, endpoint))
    trusted_peer_endpoints = tuple(trusted_peer_endpoints_list)
    trusted_peer_keys = tuple(public_key for public_key, _endpoint in trusted_peer_endpoints)
    trusted_pop_keys_list: list[str] = []
    for item in trusted_pop:
        _require_exact_keys(item, ("public_key", "pop_hex"), "trusted-peer PoP")
        trusted_pop_keys_list.append(
            _require_string(item["public_key"], "trusted-peer PoP public key")
        )
        proof = _require_string(item["pop_hex"], "trusted-peer proof of possession")
        if re.fullmatch(r"[0-9a-f]+", proof) is None or len(proof) % 2:
            raise Reset17Error("trusted-peer proof of possession is malformed")
    trusted_pop_keys = tuple(trusted_pop_keys_list)
    if (
        len(set(trusted_peer_keys)) != len(trusted_peer_keys)
        or len(set(trusted_pop_keys)) != len(trusted_pop_keys)
    ):
        raise Reset17Error("validator trusted-peer topology contains duplicates")
    parsed_torii = urlsplit(torii_url)
    torii_address = _nested_table(config, ("torii",), "Torii config").get("address")
    network_address = _nested_table(config, ("network",), "network config").get(
        "address"
    )
    if not isinstance(torii_address, str) or not isinstance(network_address, str):
        raise Reset17Error("validator config omits Torii or P2P listen address")

    def configured_port(address: str, label: str) -> int:
        untagged = address.removeprefix("addr:").split("#", 1)[0]
        try:
            port = int(untagged.rsplit(":", 1)[1])
        except (IndexError, ValueError) as error:
            raise Reset17Error(f"validator config {label} address is malformed") from error
        if not 1 <= port <= 65_535:
            raise Reset17Error(f"validator config {label} port is invalid")
        return port

    if configured_port(torii_address, "Torii") != parsed_torii.port:
        raise Reset17Error("validator config Torii port disagrees with manifest")
    if configured_port(network_address, "P2P") != p2p_port:
        raise Reset17Error("validator config P2P port disagrees with manifest")
    private_bindings = {
        "validator_signer": config.get("private_key_file"),
        "soranet_transport": config.get("soranet_transport_private_key_file"),
        "streaming_identity": _nested_table(
            config, ("streaming",), "streaming config"
        ).get("identity_private_key_file"),
        "faucet_signer": _nested_table(
            config, ("torii", "faucet"), "faucet config"
        ).get("private_key_file"),
    }
    for role, configured in private_bindings.items():
        if configured != str(private_files[role]):
            raise Reset17Error(f"validator config substitutes {role} private path")
    genesis = _nested_table(config, ("genesis",), "genesis config")
    if genesis.get("file") != str(signed_genesis_destination):
        raise Reset17Error("validator config does not bind installed signed genesis")

    nexus_storage = _nested_table(config, ("nexus", "storage"), "Nexus storage config")
    if nexus_storage.get("local_budget_bytes") != TAIRA_NEXUS_LOCAL_BUDGET_BYTES:
        raise Reset17Error("validator config does not reserve canonical 64-GiB budget")
    weights = _nested_table(
        nexus_storage, ("disk_budget_weights",), "Nexus disk-budget weights"
    )
    if weights != TAIRA_STORAGE_WEIGHTS:
        raise Reset17Error("validator config changes canonical disk-budget weights")
    sorafs = _nested_table(config, ("sorafs", "storage"), "SoraFS storage config")
    if sorafs.get("enabled") is not False or sorafs.get(
        "max_capacity_bytes"
    ) != TAIRA_SORAFS_CAPACITY_BYTES:
        raise Reset17Error("validator config changes fixed-launcher SoraFS policy")
    runtime = _nested_table(config, ("soracloud_runtime",), "Soracloud runtime config")
    if runtime.get("production_mode") is not True:
        raise Reset17Error("validator config disables Soracloud production mode")
    inrou = runtime.get("inrou", {})
    if not isinstance(inrou, dict):
        raise Reset17Error("validator Inrou policy is malformed")
    if (
        inrou.get("enabled", False) is not False
        or inrou.get("backends", []) != []
        or inrou.get("portable_vm_uid") is not None
        or inrou.get("portable_vm_gid") is not None
    ):
        raise Reset17Error("validator config enables forbidden public Taira Inrou hosting")
    signer = _nested_table(
        runtime, ("submission", "signer"), "Soracloud runtime signer binding"
    )
    expected_signer = {
        "handle": runtime_signer["handle"],
        "authority": runtime_signer["authority"],
        "algorithm": "ed25519",
        "public_key_hex": runtime_signer["public_key_hex"],
        "revision": TAIRA_RUNTIME_SIGNER_REVISION,
        "policy_digest_hex": runtime_signer["policy_digest_hex"],
    }
    if signer != expected_signer:
        raise Reset17Error("validator config runtime signer binding is substituted")

    state_paths: list[Path] = []
    for table_path, key, label in (
        (("kura",), "store_dir", "Kura store"),
        (("snapshot",), "store_dir", "snapshot store"),
        (("soracloud_runtime",), "state_dir", "Soracloud runtime state"),
    ):
        table = _nested_table(config, table_path, f"{label} config")
        state_paths.append(_config_absolute(table.get(key), label))
    if len(set(state_paths)) != len(state_paths) or any(
        not _is_below(path, data_root) for path in state_paths
    ):
        raise Reset17Error("validator state path escapes generation-specific data root")

    nexus = _nested_table(config, ("nexus",), "Nexus config")
    lane_catalog = nexus.get("lane_catalog")
    dataspace_catalog = nexus.get("dataspace_catalog")
    routing_policy = nexus.get("routing_policy")
    if (
        not isinstance(lane_catalog, list)
        or not all(isinstance(item, dict) for item in lane_catalog)
        or not isinstance(dataspace_catalog, list)
        or not all(isinstance(item, dict) for item in dataspace_catalog)
        or not isinstance(routing_policy, dict)
        or not isinstance(routing_policy.get("rules"), list)
    ):
        raise Reset17Error("validator Nexus routing catalogs are malformed")
    lane_matches = [
        item
        for item in lane_catalog
        if item.get("index") == BPNG_LANE_ID
        or item.get("alias") == BPNG_LANE_ALIAS
    ]
    if len(lane_matches) != 1 or any(
        lane_matches[0].get(key) != expected
        for key, expected in (
            ("index", BPNG_LANE_ID),
            ("alias", BPNG_LANE_ALIAS),
            ("dataspace", BPNG_PHYSICAL_DATASPACE_ALIAS),
        )
    ):
        raise Reset17Error("validator config changes the BPNG DPN lane binding")
    dataspace_matches = [
        item
        for item in dataspace_catalog
        if item.get("id") == BPNG_PHYSICAL_DATASPACE_ID
        or item.get("alias") == BPNG_PHYSICAL_DATASPACE_ALIAS
    ]
    if len(dataspace_matches) != 1 or any(
        dataspace_matches[0].get(key) != expected
        for key, expected in (
            ("id", BPNG_PHYSICAL_DATASPACE_ID),
            ("alias", BPNG_PHYSICAL_DATASPACE_ALIAS),
        )
    ):
        raise Reset17Error("validator config changes the physical BPNG dataspace")
    expected_routes = {
        "*@bpng": (BPNG_LANE_ID, BPNG_PHYSICAL_DATASPACE_ALIAS),
        "*@mibank.bpng": (BPNG_LANE_ID, BPNG_PHYSICAL_DATASPACE_ALIAS),
    }
    observed_routes: dict[str, tuple[Any, Any]] = {}
    for rule in routing_policy["rules"]:
        if not isinstance(rule, dict) or not isinstance(rule.get("matcher"), dict):
            raise Reset17Error("validator Nexus routing rule is malformed")
        account = rule["matcher"].get("account")
        if account in expected_routes:
            if account in observed_routes:
                raise Reset17Error("validator duplicates one BPNG routing rule")
            observed_routes[account] = (rule.get("lane"), rule.get("dataspace"))
    if observed_routes != expected_routes:
        raise Reset17Error("validator config does not route BPNG/MiBank to lane 3")
    return (
        validator_public_key,
        trusted_peer_keys,
        trusted_peer_endpoints,
        trusted_pop_keys,
    )


def _parse_private_files(
    value: Any, index: int
) -> tuple[dict[str, Path], dict[str, PrivateFileIdentity]]:
    raw = _require_object(value, f"validator {index} private files")
    if set(raw) != PRIVATE_ROLES:
        raise Reset17Error("validator private-file inventory is not the exact five-role set")
    paths: dict[str, Path] = {}
    identities: dict[str, PrivateFileIdentity] = {}
    for role in sorted(PRIVATE_ROLES):
        path = _absolute_path(raw[role], f"validator {index} {role} path")
        try:
            parent_metadata = os.lstat(path.parent)
        except FileNotFoundError as error:
            raise Reset17Error("validator private directory is missing") from error
        if (
            not stat.S_ISDIR(parent_metadata.st_mode)
            or parent_metadata.st_uid != os.geteuid()
            or stat.S_IMODE(parent_metadata.st_mode) != 0o700
        ):
            raise Reset17Error("validator private directory is not owner-0700")
        paths[role] = path
        identities[role] = _private_identity(
            path,
            f"validator {index} {role}",
            TAIRA_RUNTIME_SIGNER_BYTES
            if role == "soracloud_runtime_signer"
            else None,
        )
    if len(set(paths.values())) != len(paths):
        raise Reset17Error("one private file is reused across validator roles")
    return paths, identities


def _parse_runtime_signer(
    value: Any, private_files: Mapping[str, Path], index: int
) -> dict[str, Any]:
    signer = _require_object(value, f"validator {index} runtime signer")
    _require_exact_keys(
        signer,
        (
            "source_path",
            "launch_path",
            "public_key_hex",
            "handle",
            "authority",
            "algorithm",
            "revision",
            "policy_digest_hex",
        ),
        f"validator {index} runtime signer",
    )
    source = _absolute_path(signer["source_path"], "runtime signer source")
    launch = _absolute_path(signer["launch_path"], "runtime signer launch path")
    if source != private_files["soracloud_runtime_signer"] or source == launch:
        raise Reset17Error("runtime signer source/launch custody is inconsistent")
    public_key = _require_string(signer["public_key_hex"], "runtime signer public key")
    if PUBLIC_KEY_RE.fullmatch(public_key) is None:
        raise Reset17Error("runtime signer public key is not lowercase raw Ed25519")
    if signer["handle"] != f"{TAIRA_RUNTIME_SIGNER_HANDLE_PREFIX}{public_key}":
        raise Reset17Error("runtime signer handle does not bind its public key")
    _require_string(signer["authority"], "runtime signer authority")
    revision = _require_int(signer["revision"], "runtime signer revision", 1)
    if signer["algorithm"] != "ed25519" or revision != 1:
        raise Reset17Error("runtime signer algorithm/revision is not canonical")
    policy = _require_string(signer["policy_digest_hex"], "runtime signer policy digest")
    if SHA256_RE.fullmatch(policy) is None:
        # The digest is BLAKE3, but it has the same lower-hex 32-byte shape.
        raise Reset17Error("runtime signer policy digest is malformed")
    _reject_symlink_components(launch, "runtime signer launch path")
    try:
        parent = os.lstat(launch.parent)
    except FileNotFoundError as error:
        raise Reset17Error("runtime signer launch directory is missing") from error
    if (
        not stat.S_ISDIR(parent.st_mode)
        or parent.st_uid != os.geteuid()
        or stat.S_IMODE(parent.st_mode) != 0o700
    ):
        raise Reset17Error("runtime signer launch directory is not owner-0700")
    try:
        stale = os.lstat(launch)
    except FileNotFoundError:
        stale = None
    if stale is not None and (
        not stat.S_ISREG(stale.st_mode)
        or stale.st_uid != os.geteuid()
        or stat.S_IMODE(stale.st_mode) != 0o600
        or stale.st_nlink != 1
        or stale.st_size not in (0, TAIRA_RUNTIME_SIGNER_BYTES)
    ):
        raise Reset17Error("runtime signer launch remnant is untrusted")
    return {
        "source_path": source,
        "launch_path": launch,
        "public_key_hex": public_key,
        "handle": signer["handle"],
        "authority": signer["authority"],
        "algorithm": "ed25519",
        "revision": 1,
        "policy_digest_hex": policy,
    }


def _validate_launch_agent(
    record: FileRecord,
    bundle: Path,
    validator: ValidatorCandidate,
    candidate: Candidate,
) -> None:
    payload = _read_bounded(record.source(bundle), MAX_JSON_BYTES, "LaunchAgent plist")
    try:
        value = plistlib.loads(payload)
    except plistlib.InvalidFileException as error:
        raise Reset17Error("LaunchAgent plist is malformed") from error
    if not isinstance(value, dict):
        raise Reset17Error("LaunchAgent plist is not a dictionary")
    _require_exact_keys(
        value,
        (
            "Label",
            "Program",
            "ProgramArguments",
            "RunAtLoad",
            "KeepAlive",
            "WorkingDirectory",
            "ThrottleInterval",
            "SoftResourceLimits",
            "HardResourceLimits",
            "EnvironmentVariables",
            "Umask",
            "StandardOutPath",
            "StandardErrorPath",
        ),
        "LaunchAgent plist",
    )
    binary = candidate.artifacts["iroha3d_taira"].destination(candidate.release_dir)
    supervisor = candidate.artifacts["fd198_supervisor"].destination(
        candidate.release_dir
    )
    genesis = candidate.artifacts["genesis_manifest"].destination(candidate.release_dir)
    config = validator.config.destination(candidate.release_dir)
    expected_arguments = [
        str(candidate.python_path),
        "-I",
        "-B",
        str(supervisor),
        "run",
        "--binary",
        str(binary),
        "--config",
        str(config),
        "--genesis-manifest",
        str(genesis),
        "--signer-source",
        str(validator.private_files["soracloud_runtime_signer"]),
        "--signer-launch",
        str(validator.signer_launch_path),
    ]
    if value.get("Label") != validator.label:
        raise Reset17Error("LaunchAgent label is foreign")
    if value.get("ProgramArguments") != expected_arguments:
        raise Reset17Error("LaunchAgent does not use the sealed FD198 supervisor argv")
    if value["Program"] != str(candidate.python_path):
        raise Reset17Error("LaunchAgent Program disagrees with sealed Python target")
    if value.get("RunAtLoad") is not True or value.get("KeepAlive") is not True:
        raise Reset17Error("LaunchAgent is not restart-supervised")
    if value.get("WorkingDirectory") != str(candidate.release_dir):
        raise Reset17Error("LaunchAgent working directory is not immutable release")
    if _require_int(value.get("Umask"), "LaunchAgent umask") != 0o077:
        raise Reset17Error("LaunchAgent umask is not owner-private")
    if _require_int(value.get("ThrottleInterval"), "LaunchAgent throttle", 10) > 300:
        raise Reset17Error("LaunchAgent throttle is outside reviewed bounds")
    soft = _require_object(value.get("SoftResourceLimits"), "soft resource limits")
    hard = _require_object(value.get("HardResourceLimits"), "hard resource limits")
    _require_exact_keys(soft, ("NumberOfFiles",), "soft resource limits")
    _require_exact_keys(hard, ("NumberOfFiles",), "hard resource limits")
    soft_files = _require_int(
        soft.get("NumberOfFiles"), "soft NumberOfFiles limit", 4096
    )
    hard_files = _require_int(
        hard.get("NumberOfFiles"), "hard NumberOfFiles limit", 8192
    )
    if soft_files > hard_files:
        raise Reset17Error("LaunchAgent file-descriptor limits are inconsistent")
    if soft_files < 4096 or hard_files < 8192:
        raise Reset17Error("LaunchAgent cannot safely inherit descriptor 198")
    environment = value.get("EnvironmentVariables", {})
    if environment != {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"}:
        raise Reset17Error("LaunchAgent environment is malformed")
    logs_root = candidate.control_root / "logs"
    expected_logs = {
        "StandardOutPath": logs_root / f"{validator.label}.stdout.log",
        "StandardErrorPath": logs_root / f"{validator.label}.stderr.log",
    }
    for key, expected in expected_logs.items():
        if _absolute_path(value[key], f"LaunchAgent {key}") != expected:
            raise Reset17Error("LaunchAgent log path escapes the reset17 control root")


def _verify_macho_arm64(path: Path, label: str) -> None:
    try:
        result = subprocess.run(
            ("/usr/bin/file", "-b", str(path)),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=10,
            check=False,
            text=True,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Reset17Error(f"{label} architecture probe is unavailable") from error
    if result.returncode != 0 or "Mach-O" not in result.stdout or "arm64" not in result.stdout:
        raise Reset17Error(f"{label} is not a macOS arm64 Mach-O executable")


def _run_offline_config_check(candidate: Candidate, validator: ValidatorCandidate) -> None:
    binary = candidate.artifacts["iroha3d_taira"].source(candidate.bundle)
    config = validator.config.source(candidate.bundle)
    genesis = candidate.artifacts["genesis_manifest"].source(candidate.bundle)
    command = (
        str(candidate.python_path),
        "-I",
        "-B",
        str(candidate.artifacts["fd198_supervisor"].source(candidate.bundle)),
        "check-config",
        "--binary",
        str(binary),
        "--config",
        str(config),
        "--genesis-manifest",
        str(genesis),
        "--timeout-seconds",
        "45",
        "--signer-source",
        str(validator.private_files["soracloud_runtime_signer"]),
        "--signer-launch",
        str(
            validator.signer_launch_path.with_name(
                "runtime-signer.check-config.fd198"
            )
        ),
    )
    try:
        result = subprocess.run(
            command,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=55,
            check=False,
            env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Reset17Error("canonical iroha3d_taira config check is unavailable") from error
    if result.returncode != 0:
        raise Reset17Error("canonical iroha3d_taira rejected a rendered validator config")


def _run_supervisor_metadata_check(candidate: Candidate, validator: ValidatorCandidate) -> None:
    command = (
        str(candidate.python_path),
        "-I",
        "-B",
        str(candidate.artifacts["fd198_supervisor"].source(candidate.bundle)),
        "validate",
        "--signer-source",
        str(validator.private_files["soracloud_runtime_signer"]),
        "--signer-launch",
        str(validator.signer_launch_path),
    )
    try:
        result = subprocess.run(
            command,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=15,
            check=False,
            env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Reset17Error("FD198 supervisor metadata check is unavailable") from error
    if result.returncode != 0:
        raise Reset17Error("FD198 supervisor rejected runtime signer custody")


def _parse_canonical_json(payload: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_duplicate_rejecting_object,
            parse_constant=_reject_json_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise Reset17Error(f"{label} is malformed") from error
    result = _require_object(value, label)
    if canonical_json_bytes(result) != payload:
        raise Reset17Error(f"{label} is not canonical JSON")
    return result


def _validate_existing_directory(path: Path, label: str, mode: int | None = None) -> None:
    _reject_symlink_components(path, label)
    try:
        metadata = os.lstat(path)
    except FileNotFoundError as error:
        raise Reset17Error(f"{label} is missing") from error
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) & 0o022
        or (mode is not None and stat.S_IMODE(metadata.st_mode) != mode)
    ):
        raise Reset17Error(f"{label} is not an owned trusted directory")


def _parse_torii_url(value: Any, index: int) -> tuple[str, int]:
    raw = _require_string(value, f"validator {index} Torii URL")
    try:
        parsed = urlsplit(raw)
        port = parsed.port
    except ValueError as error:
        raise Reset17Error(f"validator {index} Torii URL is malformed") from error
    if (
        parsed.scheme != "http"
        or parsed.hostname != "127.0.0.1"
        or port is None
        or not 1_024 <= port <= 65_535
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path not in ("", "/")
        or parsed.query
        or parsed.fragment
    ):
        raise Reset17Error("validator Torii URL must be a bare loopback HTTP origin")
    normalized = f"http://127.0.0.1:{port}"
    if raw.rstrip("/") != normalized:
        raise Reset17Error("validator Torii URL is not canonical")
    return normalized, port


def _candidate_public_records(candidate: Candidate) -> list[tuple[str, FileRecord]]:
    records = [(f"artifact:{name}", record) for name, record in candidate.artifacts.items()]
    for validator in candidate.validators:
        records.extend(
            (
                (f"validator:{validator.index}:config", validator.config),
                (f"validator:{validator.index}:launch_agent", validator.launch_agent),
            )
        )
    return sorted(records, key=lambda item: item[0])


def _validate_public_record_inventory(
    artifacts: Mapping[str, FileRecord], validators: Sequence[ValidatorCandidate]
) -> None:
    records: list[FileRecord] = list(artifacts.values())
    for validator in validators:
        records.extend((validator.config, validator.launch_agent))
    source_paths = [record.path for record in records]
    install_paths = [record.install_relative for record in records]
    if len(set(source_paths)) != len(source_paths):
        raise Reset17Error("candidate reuses one public source path")
    if len(set(install_paths)) != len(install_paths):
        raise Reset17Error("candidate reuses one public install path")
    for left_index, left in enumerate(install_paths):
        for right in install_paths[left_index + 1 :]:
            if _is_below(left, right) or _is_below(right, left):
                raise Reset17Error("candidate public install paths overlap")


def load_candidate(
    *,
    bundle: Path,
    manifest_path: Path,
    signature_path: Path,
    allowed_signers_path: Path,
    expected_manifest_sha256: str,
    expected_allowed_signers_sha256: str,
    expected_source_commit: str,
    expected_control_root: Path,
    expected_launch_agents_dir: Path,
    run_offline_checks: bool = True,
) -> Candidate:
    """Authenticate and fully validate one reset17 public/private candidate."""

    for path, label in (
        (bundle, "candidate bundle"),
        (manifest_path, "candidate manifest"),
        (signature_path, "candidate signature"),
        (allowed_signers_path, "allowed-signers file"),
        (expected_control_root, "operator-pinned control root"),
        (expected_launch_agents_dir, "operator-pinned LaunchAgents directory"),
    ):
        if not path.is_absolute() or str(path) != os.path.normpath(str(path)):
            raise Reset17Error(f"{label} path is not normalized and absolute")
        _reject_symlink_components(path, label)
    _validate_existing_directory(bundle, "candidate bundle")
    if COMMIT_RE.fullmatch(expected_source_commit) is None:
        raise Reset17Error("expected source commit is invalid")

    manifest_payload, signature_sha256 = verify_manifest_signature(
        manifest_path,
        signature_path,
        allowed_signers_path,
        expected_manifest_sha256,
        expected_allowed_signers_sha256,
    )
    raw = _parse_canonical_json(manifest_payload, "reset17 manifest")
    _require_exact_keys(
        raw,
        (
            "schema",
            "generation",
            "release_id",
            "network_id",
            "source",
            "protocols",
            "bpng",
            "deployment",
            "artifacts",
            "validators",
        ),
        "reset17 manifest",
    )
    if raw["schema"] != MANIFEST_SCHEMA or raw["generation"] != GENERATION:
        raise Reset17Error("candidate manifest schema/generation is foreign")
    release_id = _require_string(raw["release_id"], "release id")
    if RUN_ID_RE.fullmatch(release_id) is None or not release_id.startswith("reset17-"):
        raise Reset17Error("release id is not a canonical reset17 identifier")
    network_id = _require_string(raw["network_id"], "Taira NetworkId")
    if NETWORK_ID_RE.fullmatch(network_id) is None:
        raise Reset17Error("Taira NetworkId is not lowercase 32-byte hex")
    source_commit = _parse_source(raw["source"], expected_source_commit)
    _validate_protocols(raw["protocols"])
    _validate_bpng(raw["bpng"])
    deployment = _parse_deployment(raw["deployment"], release_id)

    control_root = deployment["control_root"]
    release_dir = deployment["release_dir"]
    launch_agents_dir = deployment["launch_agents_dir"]
    if (
        control_root != expected_control_root
        or launch_agents_dir != expected_launch_agents_dir
    ):
        raise Reset17Error("deployment roots do not match mandatory operator pins")
    if control_root in (Path("/"), Path.home()) or release_dir != (
        control_root / "releases" / release_id
    ):
        raise Reset17Error("release path is not isolated below the reset17 control root")
    _validate_existing_directory(control_root, "reset17 control root", 0o700)
    _validate_existing_directory(
        launch_agents_dir, "service-user LaunchAgents directory"
    )
    if launch_agents_dir.name != "LaunchAgents" or launch_agents_dir.parent.name != "Library":
        raise Reset17Error("LaunchAgents directory is not a service-user Library path")
    _verify_external_public_file(
        deployment["python_path"],
        deployment["python_sha256"],
        deployment["python_size"],
        "isolated Python executable",
    )

    artifacts_raw = _require_object(raw["artifacts"], "artifact inventory")
    if set(artifacts_raw) != REQUIRED_ARTIFACTS:
        raise Reset17Error("candidate artifact inventory is not the exact reviewed set")
    artifacts = {
        name: FileRecord.parse(artifacts_raw[name], f"artifact {name}")
        for name in sorted(REQUIRED_ARTIFACTS)
    }
    executable_artifacts = {
        "iroha3d_taira",
        "kagami",
        "iroha",
        "taira_operator_status",
    }
    for name, record in artifacts.items():
        _verify_file(record, bundle, name in executable_artifacts)
    for name in sorted(executable_artifacts):
        _verify_macho_arm64(artifacts[name].source(bundle), f"artifact {name}")

    validators_raw = raw["validators"]
    if not isinstance(validators_raw, list) or len(validators_raw) != TAIRA_VALIDATORS:
        raise Reset17Error("candidate must contain exactly four validators")
    validators: list[ValidatorCandidate] = []
    all_private_paths: set[Path] = set()
    all_private_inodes: set[tuple[int, int]] = set()
    all_launch_paths: set[Path] = set()
    torii_origins: set[str] = set()
    all_ports: set[int] = set()
    signed_genesis_destination = artifacts["signed_genesis"].destination(release_dir)
    for position, value in enumerate(validators_raw, start=1):
        entry = _require_object(value, f"validator {position}")
        _require_exact_keys(
            entry,
            (
                "index",
                "label",
                "data_root",
                "torii_url",
                "p2p_port",
                "config",
                "launch_agent",
                "private_files",
                "runtime_signer",
            ),
            f"validator {position}",
        )
        index = _require_int(entry["index"], "validator index", 1)
        if index != position:
            raise Reset17Error("validator inventory is not ordered 1 through 4")
        label = _require_string(entry["label"], f"validator {index} label")
        if label != f"org.sora.taira.user.validator-{index}":
            raise Reset17Error("validator LaunchAgent label is not canonical")
        data_root = _absolute_path(entry["data_root"], f"validator {index} data root")
        if data_root != control_root / "data" / GENERATION / f"validator-{index}":
            raise Reset17Error("validator data root is not generation-specific")
        _reject_symlink_components(data_root, f"validator {index} data root")
        torii_url, torii_port = _parse_torii_url(entry["torii_url"], index)
        p2p_port = _require_int(
            entry["p2p_port"], f"validator {index} P2P port", 1_024
        )
        if p2p_port > 65_535 or torii_url in torii_origins:
            raise Reset17Error("validator Torii/P2P endpoint inventory is invalid")
        if torii_port in all_ports or p2p_port in all_ports or torii_port == p2p_port:
            raise Reset17Error("validator Torii/P2P ports are not globally unique")
        torii_origins.add(torii_url)
        all_ports.update((torii_port, p2p_port))

        config = FileRecord.parse(entry["config"], f"validator {index} config")
        launch_agent = FileRecord.parse(
            entry["launch_agent"], f"validator {index} LaunchAgent"
        )
        _verify_file(config, bundle, False)
        _verify_file(launch_agent, bundle, False)
        if config.install_relative != Path("config") / f"validator-{index}.toml":
            raise Reset17Error("validator config install path is not canonical")
        if launch_agent.install_relative != Path("launch-agents") / f"{label}.plist":
            raise Reset17Error("validator LaunchAgent install path is not canonical")

        private_files, private_identities = _parse_private_files(
            entry["private_files"], index
        )
        expected_private_root = control_root / "private" / f"validator-{index}"
        if any(not _is_below(path, expected_private_root) for path in private_files.values()):
            raise Reset17Error("validator private files escape the derived custody root")
        for path in private_files.values():
            _validate_owner_private_ancestors(
                path, control_root, "validator private file"
            )
        signer = _parse_runtime_signer(entry["runtime_signer"], private_files, index)
        for path, identity in zip(private_files.values(), private_identities.values()):
            inode = (identity.device, identity.inode)
            if path in all_private_paths or inode in all_private_inodes:
                raise Reset17Error("one private file is reused across validators")
            all_private_paths.add(path)
            all_private_inodes.add(inode)
        launch_path = signer["launch_path"]
        if launch_path != (
            control_root
            / "run"
            / GENERATION
            / f"validator-{index}"
            / "runtime-signer.fd198"
        ):
            raise Reset17Error("runtime signer launch path is not derived from control root")
        _validate_owner_private_ancestors(
            launch_path, control_root, "runtime signer launch file"
        )
        if launch_path in all_launch_paths or launch_path in all_private_paths:
            raise Reset17Error("runtime signer launch path is reused")
        all_launch_paths.add(launch_path)

        (
            validator_public_key,
            trusted_peer_keys,
            trusted_peer_endpoints,
            trusted_pop_keys,
        ) = _validate_config(
            config,
            bundle,
            data_root,
            torii_url,
            p2p_port,
            private_files,
            signer,
            signed_genesis_destination,
        )
        validators.append(
            ValidatorCandidate(
                index=index,
                label=label,
                data_root=data_root,
                torii_url=torii_url,
                p2p_port=p2p_port,
                config=config,
                launch_agent=launch_agent,
                private_files=private_files,
                private_identities=private_identities,
                signer_public_key_hex=signer["public_key_hex"],
                signer_handle=signer["handle"],
                signer_authority=signer["authority"],
                signer_policy_digest_hex=signer["policy_digest_hex"],
                signer_launch_path=launch_path,
                validator_public_key=validator_public_key,
                trusted_peer_public_keys=trusted_peer_keys,
                trusted_peer_endpoints=trusted_peer_endpoints,
                trusted_pop_public_keys=trusted_pop_keys,
            )
        )

    _validate_public_record_inventory(artifacts, validators)
    canonical_peer_keys = tuple(sorted(item.validator_public_key for item in validators))
    if len(set(canonical_peer_keys)) != TAIRA_VALIDATORS:
        raise Reset17Error("validator public keys are not globally unique")
    expected_peer_endpoints = {
        item.validator_public_key: f"127.0.0.1:{item.p2p_port}" for item in validators
    }
    for validator in validators:
        if (
            tuple(sorted(validator.trusted_peer_public_keys)) != canonical_peer_keys
            or tuple(sorted(validator.trusted_pop_public_keys)) != canonical_peer_keys
            or dict(validator.trusted_peer_endpoints) != expected_peer_endpoints
        ):
            raise Reset17Error("validator configs do not bind one exact four-peer topology")
    candidate = Candidate(
        bundle=bundle,
        manifest_path=manifest_path,
        signature_path=signature_path,
        allowed_signers_path=allowed_signers_path,
        manifest_sha256=expected_manifest_sha256,
        signature_sha256=signature_sha256,
        allowed_signers_sha256=expected_allowed_signers_sha256,
        raw=raw,
        release_id=release_id,
        source_commit=source_commit,
        release_dir=release_dir,
        launch_agents_dir=launch_agents_dir,
        control_root=control_root,
        python_path=deployment["python_path"],
        python_sha256=deployment["python_sha256"],
        python_size=deployment["python_size"],
        require_single_data_volume=deployment["require_single_data_volume"],
        artifacts=artifacts,
        validators=tuple(validators),
        network_id=network_id,
    )
    for validator in candidate.validators:
        _validate_launch_agent(validator.launch_agent, bundle, validator, candidate)
        _run_supervisor_metadata_check(candidate, validator)
        if run_offline_checks:
            _run_offline_config_check(candidate, validator)
    return candidate


def _nearest_existing_directory(path: Path, label: str) -> Path:
    current = path
    while True:
        try:
            metadata = os.lstat(current)
        except FileNotFoundError:
            parent = current.parent
            if parent == current:
                raise Reset17Error(f"{label} has no resolvable storage ancestor")
            current = parent
            continue
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            raise Reset17Error(f"{label} storage ancestor is not a real directory")
        _reject_symlink_components(current, label)
        return current


def _volume_identity(path: Path, label: str) -> tuple[int, int, int, Path]:
    anchor = _nearest_existing_directory(path, label)
    metadata = os.stat(anchor, follow_symlinks=False)
    try:
        volume = os.statvfs(anchor)
    except OSError as error:
        raise Reset17Error(f"{label} storage capacity is unavailable") from error
    fragment = volume.f_frsize or volume.f_bsize
    capacity = fragment * volume.f_blocks
    available = fragment * volume.f_bavail
    if fragment <= 0 or capacity <= 0 or available < 0:
        raise Reset17Error(f"{label} storage capacity is invalid")
    return metadata.st_dev, capacity, available, anchor


def _service_is_loaded(label: str) -> bool:
    target = f"gui/{os.geteuid()}/{label}"
    try:
        result = subprocess.run(
            ("/bin/launchctl", "print", target),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=15,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Reset17Error("LaunchAgent predecessor state is unavailable") from error
    if len(result.stdout) > MAX_HELPER_OUTPUT_BYTES or len(result.stderr) > MAX_HELPER_OUTPUT_BYTES:
        raise Reset17Error("LaunchAgent predecessor query returned excessive output")
    if result.returncode == 0:
        return True
    combined = (result.stdout + result.stderr).decode("utf-8", "replace").lower()
    if any(
        marker in combined
        for marker in (
            "could not find service",
            "service not found",
            "no such process",
        )
    ):
        return False
    raise Reset17Error("LaunchAgent predecessor state is ambiguous")


def _predecessor_snapshot_path(candidate: Candidate) -> Path:
    return (
        candidate.control_root
        / "backups"
        / candidate.release_id
        / "predecessor.json"
    )


def _validate_predecessor_entries(
    candidate: Candidate, entries: Any
) -> list[dict[str, Any]]:
    if not isinstance(entries, list) or len(entries) != TAIRA_VALIDATORS:
        raise Reset17Error("predecessor snapshot is not the exact four-validator set")
    validated: list[dict[str, Any]] = []
    backup_root = candidate.control_root / "backups" / candidate.release_id / "launch-agents"
    for validator, raw_entry in zip(candidate.validators, entries):
        entry = _require_object(raw_entry, "predecessor entry")
        _require_exact_keys(
            entry,
            (
                "index",
                "label",
                "target",
                "backup",
                "present",
                "sha256",
                "size",
                "mode",
                "loaded",
            ),
            "predecessor entry",
        )
        target = candidate.launch_agents_dir / f"{validator.label}.plist"
        backup = backup_root / f"{validator.label}.plist.predecessor"
        if (
            entry["index"] != validator.index
            or entry["label"] != validator.label
            or entry["target"] != str(target)
            or entry["backup"] != str(backup)
            or type(entry["present"]) is not bool
            or type(entry["loaded"]) is not bool
        ):
            raise Reset17Error("predecessor snapshot identity is foreign")
        if entry["present"]:
            digest = _require_string(entry["sha256"], "predecessor SHA-256")
            if SHA256_RE.fullmatch(digest) is None:
                raise Reset17Error("predecessor SHA-256 is malformed")
            size = _require_int(entry["size"], "predecessor size", 1)
            mode = _require_int(entry["mode"], "predecessor mode")
            if mode not in (0o400, 0o444, 0o600, 0o644):
                raise Reset17Error("predecessor LaunchAgent mode is not reviewed")
        else:
            if (
                entry["sha256"] is not None
                or entry["size"] != 0
                or entry["mode"] is not None
            ):
                raise Reset17Error("missing predecessor carries foreign metadata")
        record = validator.launch_agent
        for path, is_backup in ((backup, True), (target, False)):
            if not (path.exists() or path.is_symlink()):
                continue
            payload, _mode = _file_digest_record(
                path, "predecessor backup" if is_backup else "installed LaunchAgent"
            )
            desired = len(payload) == record.size and sha256_bytes(payload) == record.sha256
            predecessor = (
                entry["present"]
                and len(payload) == entry["size"]
                and sha256_bytes(payload) == entry["sha256"]
            )
            if is_backup and not predecessor:
                raise Reset17Error("predecessor backup does not match its snapshot")
            if not is_backup and not (desired or predecessor):
                raise Reset17Error("installed LaunchAgent drifted from predecessor plan")
        if (
            not entry["present"]
            and entry["loaded"]
            and not (target.exists() or target.is_symlink())
        ):
            raise Reset17Error("missing LaunchAgent cannot be recorded as loaded")
        validated.append(dict(entry))
    return validated


def predecessor_plan(candidate: Candidate) -> list[dict[str, Any]]:
    snapshot_path = _predecessor_snapshot_path(candidate)
    if snapshot_path.exists() or snapshot_path.is_symlink():
        snapshot = read_json(snapshot_path)
        snapshot_object = _require_object(snapshot, "predecessor snapshot")
        _require_exact_keys(
            snapshot_object,
            ("schema", "release_id", "predecessor"),
            "predecessor snapshot",
        )
        if (
            snapshot_object["schema"] != PREDECESSOR_SCHEMA
            or snapshot_object["release_id"] != candidate.release_id
        ):
            raise Reset17Error("predecessor snapshot schema/release is foreign")
        return _validate_predecessor_entries(
            candidate, snapshot_object["predecessor"]
        )

    predecessors: list[dict[str, Any]] = []
    backup_root = candidate.control_root / "backups" / candidate.release_id / "launch-agents"
    for validator in candidate.validators:
        target = candidate.launch_agents_dir / f"{validator.label}.plist"
        backup = backup_root / f"{validator.label}.plist.predecessor"
        record = validator.launch_agent
        selected: Path | None = None
        if backup.exists() or backup.is_symlink():
            selected = backup
            backup_payload, backup_mode = _file_digest_record(
                backup, "predecessor LaunchAgent backup"
            )
            if target.exists() or target.is_symlink():
                target_payload, _target_mode = _file_digest_record(
                    target, "installed LaunchAgent"
                )
                target_is_desired = (
                    len(target_payload) == record.size
                    and sha256_bytes(target_payload) == record.sha256
                )
                if not target_is_desired and target_payload != backup_payload:
                    raise Reset17Error("installed LaunchAgent drifted from plan/backup")
            payload, mode = backup_payload, backup_mode
        elif target.exists() or target.is_symlink():
            payload, mode = _file_digest_record(target, "predecessor LaunchAgent")
            if len(payload) == record.size and sha256_bytes(payload) == record.sha256:
                # The candidate is already installed and there is no evidence
                # that a predecessor existed.
                payload = b""
                mode = 0
            else:
                selected = target
        else:
            payload = b""
            mode = 0
        predecessors.append(
            {
                "index": validator.index,
                "label": validator.label,
                "target": str(target),
                "backup": str(backup),
                "present": selected is not None,
                "sha256": sha256_bytes(payload) if selected is not None else None,
                "size": len(payload) if selected is not None else 0,
                "mode": mode if selected is not None else None,
                "loaded": _service_is_loaded(validator.label)
                if selected is not None or target.exists()
                else False,
            }
        )
    return _validate_predecessor_entries(candidate, predecessors)


def storage_plan(
    candidate: Candidate, predecessors: Sequence[Mapping[str, Any]]
) -> dict[str, Any]:
    """Bind destination devices and reject insufficient physical capacity."""

    volume_paths: list[tuple[Path, str, int, int]] = []
    # Each validator reserves its full Nexus budget on the physical data device.
    for validator in candidate.validators:
        volume_paths.append(
            (
                validator.data_root,
                f"validator {validator.index} data root",
                TAIRA_NEXUS_LOCAL_BUDGET_BYTES,
                0,
            )
        )
    for predecessor in predecessors:
        size = _require_int(predecessor.get("size"), "predecessor size")
        if size:
            volume_paths.append(
                (
                    candidate.control_root / "backups",
                    f"predecessor LaunchAgent backup {predecessor['index']}",
                    0,
                    size,
                )
            )
    # Release bytes are copied once; LaunchAgent payloads are copied a second
    # time into ~/Library/LaunchAgents.  Predecessor backup bytes are included
    # above, making this the exact planned copy footprint.
    for role, record in _candidate_public_records(candidate):
        volume_paths.append(
            (candidate.release_dir, f"release copy {role}", 0, record.size)
        )
    for validator in candidate.validators:
        volume_paths.append(
            (
                candidate.launch_agents_dir,
                f"installed LaunchAgent {validator.index}",
                0,
                validator.launch_agent.size,
            )
        )

    groups: dict[int, dict[str, Any]] = {}
    data_devices: set[int] = set()
    for path, label, budget_bytes, copy_bytes in volume_paths:
        device, capacity, available, anchor = _volume_identity(path, label)
        if budget_bytes:
            data_devices.add(device)
        group = groups.setdefault(
            device,
            {
                "device": device,
                "capacity_bytes": capacity,
                "available_bytes": available,
                "anchors": set(),
                "nexus_budget_bytes": 0,
                "copy_bytes": 0,
            },
        )
        if group["capacity_bytes"] != capacity:
            raise Reset17Error("one physical device reports inconsistent capacity")
        group["available_bytes"] = min(group["available_bytes"], available)
        group["anchors"].add(str(anchor))
        group["nexus_budget_bytes"] += budget_bytes
        group["copy_bytes"] += copy_bytes
    if len(data_devices) != 1:
        raise Reset17Error("validator data roots do not resolve to one physical volume")

    planned_groups: list[dict[str, Any]] = []
    for device in sorted(groups):
        group = groups[device]
        reserve = max(
            MIN_FREE_RESERVE_BYTES,
            (
                group["capacity_bytes"] * FREE_RESERVE_BPS + 9_999
            )
            // 10_000,
        )
        required = group["nexus_budget_bytes"] + group["copy_bytes"] + reserve
        if group["available_bytes"] < required:
            shortfall = required - group["available_bytes"]
            raise Reset17Error(
                f"physical storage gate is short by {shortfall} bytes on device {device}"
            )
        planned_groups.append(
            {
                "device": device,
                "anchors": sorted(group["anchors"]),
                "capacity_bytes": group["capacity_bytes"],
                "nexus_budget_bytes": group["nexus_budget_bytes"],
                "copy_bytes": group["copy_bytes"],
                "reserve_bytes": reserve,
                "required_available_bytes": required,
            }
        )
    return {
        "require_single_data_volume": candidate.require_single_data_volume,
        "data_device": next(iter(data_devices)),
        "groups": planned_groups,
    }


def build_plan(candidate: Candidate) -> dict[str, Any]:
    predecessors = predecessor_plan(candidate)
    public_files = []
    for role, record in _candidate_public_records(candidate):
        public_files.append(
            {
                "role": role,
                "source": str(record.source(candidate.bundle)),
                "destination": str(record.destination(candidate.release_dir)),
                "sha256": record.sha256,
                "size": record.size,
                "mode": record.mode,
            }
        )
    validators = []
    for validator in candidate.validators:
        validators.append(
            {
                "index": validator.index,
                "label": validator.label,
                "data_root": str(validator.data_root),
                "torii_url": validator.torii_url,
                "p2p_port": validator.p2p_port,
                "validator_public_key": validator.validator_public_key,
                "trusted_peer_endpoints": {
                    key: endpoint
                    for key, endpoint in sorted(validator.trusted_peer_endpoints)
                },
                "launch_agent_target": str(
                    candidate.launch_agents_dir / f"{validator.label}.plist"
                ),
                "private_files": {
                    role: validator.private_identities[role].json()
                    for role in sorted(validator.private_identities)
                },
                "runtime_signer": {
                    "launch_path": str(validator.signer_launch_path),
                    "public_key_hex": validator.signer_public_key_hex,
                    "handle": validator.signer_handle,
                    "authority": validator.signer_authority,
                    "policy_digest_hex": validator.signer_policy_digest_hex,
                    "revision": TAIRA_RUNTIME_SIGNER_REVISION,
                },
            }
        )
    return {
        "schema": PLAN_SCHEMA,
        "generation": GENERATION,
        "release_id": candidate.release_id,
        "network_id": candidate.network_id,
        "source_commit": candidate.source_commit,
        "authentication": {
            "manifest_sha256": candidate.manifest_sha256,
            "signature_sha256": candidate.signature_sha256,
            "allowed_signers_sha256": candidate.allowed_signers_sha256,
            "signing_identity": SIGNING_IDENTITY,
            "signing_namespace": SIGNING_NAMESPACE,
        },
        "bpng": {
            "asset_alias": BPNG_ASSET_ALIAS,
            "asset_definition": BPNG_ASSET_DEFINITION,
            "asset_domain": BPNG_ASSET_DOMAIN,
            "scale": BPNG_ASSET_SCALE,
            "lane_id": BPNG_LANE_ID,
            "lane_alias": BPNG_LANE_ALIAS,
            "physical_dataspace_id": BPNG_PHYSICAL_DATASPACE_ID,
            "physical_dataspace_alias": BPNG_PHYSICAL_DATASPACE_ALIAS,
        },
        "protocols": {
            "native_bridge_abi": NATIVE_BRIDGE_ABI,
            "kagemusha_data_abi": KAGEMUSHA_DATA_ABI,
            "exact12": list(EXACT12_PROTOCOLS),
        },
        "release_dir": str(candidate.release_dir),
        "launch_domain": f"gui/{os.geteuid()}",
        "public_files": public_files,
        "validators": validators,
        "storage": storage_plan(candidate, predecessors),
        "predecessor": predecessors,
        "operations": [
            "stage_immutable_release",
            "create_generation_data_roots",
            "backup_and_install_launch_agents",
            "bootout_predecessor_services",
            "bootstrap_reset17_services",
            "verify_four_peer_progress",
            "write_authenticated_result",
        ],
    }


def plan_bytes(candidate: Candidate) -> bytes:
    return canonical_json_bytes(build_plan(candidate))


def _read_and_check_plan(
    candidate: Candidate, plan_path: Path, expected_plan_sha256: str
) -> tuple[dict[str, Any], bytes]:
    if SHA256_RE.fullmatch(expected_plan_sha256) is None:
        raise Reset17Error("expected plan SHA-256 is invalid")
    payload = _read_bounded(plan_path, MAX_JSON_BYTES, "reset17 plan")
    if sha256_bytes(payload) != expected_plan_sha256:
        raise Reset17Error("reset17 plan does not match its reviewed digest")
    parsed = _parse_canonical_json(payload, "reset17 plan")
    expected = plan_bytes(candidate)
    if payload != expected:
        raise Reset17Error("reset17 plan is stale or does not match the authenticated candidate")
    return parsed, payload


def _write_canonical_file(path: Path, payload: bytes, mode: int) -> None:
    if not path.is_absolute() or str(path) != os.path.normpath(str(path)):
        raise Reset17Error("output path must be normalized and absolute")
    _reject_symlink_components(path, "output path")
    try:
        existing_metadata = os.lstat(path)
    except FileNotFoundError:
        existing_metadata = None
    if existing_metadata is not None:
        existing = _read_bounded(path, MAX_JSON_BYTES, "existing output")
        if existing == payload:
            if (
                existing_metadata.st_uid != os.geteuid()
                or stat.S_IMODE(existing_metadata.st_mode) != mode
            ):
                raise Reset17Error("existing output has untrusted metadata")
            return
        raise Reset17Error("refusing to replace a different existing output")
    _validate_existing_directory(path.parent, "output directory")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    created = False
    try:
        descriptor = os.open(path, flags, mode)
        created = True
        try:
            view = memoryview(payload)
            offset = 0
            while offset < len(payload):
                written = os.write(descriptor, view[offset:])
                if written <= 0:
                    raise Reset17Error("short output write")
                offset += written
            view.release()
            os.fchmod(descriptor, mode)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        directory = os.open(
            path.parent,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    except BaseException:
        if created:
            try:
                os.unlink(path)
            except FileNotFoundError:
                pass
        raise


def _ensure_owned_directory(path: Path, mode: int = 0o700) -> None:
    """Create one derived directory and reject any foreign pre-existing node."""

    _reject_symlink_components(path, "reset17 directory")
    try:
        metadata = os.lstat(path)
    except FileNotFoundError:
        _validate_existing_directory(path.parent, "reset17 directory parent")
        try:
            os.mkdir(path, mode)
        except FileExistsError:
            pass
        metadata = os.lstat(path)
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != mode
    ):
        raise Reset17Error("reset17 directory metadata is untrusted")


def _installed_file_matches(path: Path, record: FileRecord, mode: int | None = None) -> bool:
    try:
        payload = _read_bounded(path, MAX_PUBLIC_FILE_BYTES, "installed public file")
    except Reset17Error:
        return False
    metadata = os.lstat(path)
    expected_mode = record.mode if mode is None else mode
    return (
        metadata.st_uid == os.geteuid()
        and stat.S_IMODE(metadata.st_mode) == expected_mode
        and metadata.st_size == record.size
        and sha256_bytes(payload) == record.sha256
    )


def _copy_record_atomic(source: Path, destination: Path, record: FileRecord, mode: int) -> None:
    if destination.exists() or destination.is_symlink():
        if _installed_file_matches(destination, record, mode):
            return
        raise Reset17Error("refusing to replace a foreign staged public file")
    _validate_existing_directory(destination.parent, "public-file destination directory")
    temporary = destination.parent / f".{destination.name}.copying-{os.getpid()}"
    if temporary.exists() or temporary.is_symlink():
        raise Reset17Error("reset17 copy temporary path is occupied")
    created_temporary = False
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
        target_fd = os.open(temporary, flags, 0o600)
        created_temporary = True
        try:
            source_fd, payload = _open_and_read_bounded(
                source, MAX_PUBLIC_FILE_BYTES, "candidate public file"
            )
            try:
                if len(payload) != record.size or sha256_bytes(payload) != record.sha256:
                    raise Reset17Error("candidate public file changed before staging")
                view = memoryview(payload)
                offset = 0
                while offset < len(payload):
                    written = os.write(target_fd, view[offset:])
                    if written <= 0:
                        raise Reset17Error("short reset17 public-file write")
                    offset += written
                view.release()
            finally:
                os.close(source_fd)
            os.fchmod(target_fd, mode)
            os.fsync(target_fd)
        finally:
            os.close(target_fd)
        if not _installed_file_matches(temporary, record, mode):
            raise Reset17Error("staged public file failed destination rehash")
        os.replace(temporary, destination)
        directory = os.open(
            destination.parent,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    except BaseException:
        if created_temporary:
            try:
                os.unlink(temporary)
            except FileNotFoundError:
                pass
        raise


def _verify_release_at(candidate: Candidate, root: Path, sealed: bool) -> None:
    _validate_existing_directory(root, "immutable reset17 release")
    if sealed and stat.S_IMODE(os.lstat(root).st_mode) != 0o555:
        raise Reset17Error("immutable reset17 release root is writable")
    expected_files = {
        record.install_relative for _role, record in _candidate_public_records(candidate)
    }
    expected_directories: set[Path] = set()
    for relative in expected_files:
        parent = relative.parent
        while parent != Path("."):
            expected_directories.add(parent)
            parent = parent.parent
    actual_files: set[Path] = set()
    actual_directories: set[Path] = set()
    for path in root.rglob("*"):
        metadata = os.lstat(path)
        relative = path.relative_to(root)
        if stat.S_ISREG(metadata.st_mode):
            actual_files.add(relative)
        elif stat.S_ISDIR(metadata.st_mode):
            actual_directories.add(relative)
        else:
            raise Reset17Error("immutable reset17 release contains a foreign node")
    if actual_files != expected_files or actual_directories != expected_directories:
        raise Reset17Error("immutable reset17 release inventory is not exact")
    for _role, record in _candidate_public_records(candidate):
        if not _installed_file_matches(root / record.install_relative, record):
            raise Reset17Error("existing reset17 release differs from candidate")
    if sealed:
        for path in (root / relative for relative in actual_directories):
            metadata = os.lstat(path)
            if stat.S_ISDIR(metadata.st_mode) and stat.S_IMODE(metadata.st_mode) != 0o555:
                raise Reset17Error("immutable reset17 release contains a writable directory")


def _verify_installed_release(candidate: Candidate) -> None:
    _verify_release_at(candidate, candidate.release_dir, True)


def _seal_directory_tree(root: Path) -> None:
    directories = [root]
    for path in root.rglob("*"):
        metadata = os.lstat(path)
        if stat.S_ISLNK(metadata.st_mode):
            raise Reset17Error("immutable release contains a symlink")
        if stat.S_ISDIR(metadata.st_mode):
            directories.append(path)
    for directory in sorted(directories, key=lambda item: len(item.parts), reverse=True):
        os.chmod(directory, 0o555, follow_symlinks=False)


def _stage_immutable_release(candidate: Candidate) -> None:
    releases = candidate.control_root / "releases"
    _ensure_owned_directory(releases, 0o700)
    try:
        existing = os.lstat(candidate.release_dir)
    except FileNotFoundError:
        existing = None
    if existing is not None:
        _verify_installed_release(candidate)
        return
    staging = releases / f".{candidate.release_id}.staging"
    try:
        staging_metadata = os.lstat(staging)
    except FileNotFoundError:
        staging_metadata = None
    if staging_metadata is not None and (
        stat.S_ISDIR(staging_metadata.st_mode)
        and staging_metadata.st_uid == os.geteuid()
        and stat.S_IMODE(staging_metadata.st_mode) == 0o555
    ):
        _verify_release_at(candidate, staging, True)
    else:
        _ensure_owned_directory(staging, 0o700)
        for _role, record in _candidate_public_records(candidate):
            destination = staging / record.install_relative
            # Build each parent one level at a time below the controlled staging root.
            parent = staging
            for component in record.install_relative.parent.parts:
                parent /= component
                _ensure_owned_directory(parent, 0o700)
            _copy_record_atomic(record.source(candidate.bundle), destination, record, record.mode)
        _verify_release_at(candidate, staging, False)
        _seal_directory_tree(staging)
        _verify_release_at(candidate, staging, True)
    try:
        os.rename(staging, candidate.release_dir)
    except OSError:
        try:
            os.lstat(candidate.release_dir)
        except FileNotFoundError:
            raise
        _verify_installed_release(candidate)
    directory = os.open(
        releases,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        os.fsync(directory)
    finally:
        os.close(directory)
    _verify_installed_release(candidate)


def _prepare_runtime_directories(candidate: Candidate) -> None:
    data = candidate.control_root / "data"
    generation = data / GENERATION
    logs = candidate.control_root / "logs"
    backups = candidate.control_root / "backups"
    release_backup = backups / candidate.release_id
    launch_backup = release_backup / "launch-agents"
    results = candidate.control_root / "results"
    for path in (data, generation, logs, backups, release_backup, launch_backup, results):
        _ensure_owned_directory(path, 0o700)
    for validator in candidate.validators:
        _ensure_owned_directory(validator.data_root, 0o700)


def _persist_predecessor_snapshot(
    candidate: Candidate, predecessors: Sequence[Mapping[str, Any]]
) -> None:
    validated = _validate_predecessor_entries(candidate, list(predecessors))
    payload = canonical_json_bytes(
        {
            "schema": PREDECESSOR_SCHEMA,
            "release_id": candidate.release_id,
            "predecessor": validated,
        }
    )
    _write_canonical_file(_predecessor_snapshot_path(candidate), payload, 0o400)


@dataclass(frozen=True)
class LaunchAgentInstall:
    validator: ValidatorCandidate
    target: Path
    backup: Path
    had_predecessor: bool
    was_already_desired: bool
    predecessor_was_loaded: bool


def _file_digest_record(path: Path, label: str) -> tuple[bytes, int]:
    _reject_symlink_components(path, label)
    descriptor, payload = _open_and_read_bounded(path, MAX_JSON_BYTES, label)
    try:
        metadata = os.fstat(descriptor)
        if metadata.st_uid != os.geteuid():
            raise Reset17Error(f"{label} is not owned by the service user")
        return payload, stat.S_IMODE(metadata.st_mode)
    finally:
        os.close(descriptor)


def _copy_bytes_exclusive(path: Path, payload: bytes, mode: int) -> None:
    if path.exists() or path.is_symlink():
        existing = _read_bounded(path, MAX_JSON_BYTES, "existing backup")
        if existing == payload and stat.S_IMODE(os.lstat(path).st_mode) == mode:
            return
        raise Reset17Error("recoverable predecessor backup already differs")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    descriptor = os.open(path, flags, mode)
    complete = False
    try:
        view = memoryview(payload)
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, view[offset:])
            if written <= 0:
                raise Reset17Error("short predecessor backup write")
            offset += written
        view.release()
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
        complete = True
    finally:
        os.close(descriptor)
        if not complete:
            try:
                os.unlink(path)
            except FileNotFoundError:
                pass


def _install_launch_agents(
    candidate: Candidate,
    predecessors: Sequence[Mapping[str, Any]],
    installs: list[LaunchAgentInstall] | None = None,
) -> list[LaunchAgentInstall]:
    if installs is None:
        installs = []
    backup_root = candidate.control_root / "backups" / candidate.release_id / "launch-agents"
    predecessor_by_index = {
        _require_int(entry.get("index"), "predecessor index", 1): entry
        for entry in predecessors
    }
    if set(predecessor_by_index) != set(range(1, TAIRA_VALIDATORS + 1)):
        raise Reset17Error("predecessor install inventory is incomplete")
    for validator in candidate.validators:
        record = validator.launch_agent
        source = record.destination(candidate.release_dir)
        target = candidate.launch_agents_dir / f"{validator.label}.plist"
        backup = backup_root / f"{validator.label}.plist.predecessor"
        planned = predecessor_by_index[validator.index]
        desired = _installed_file_matches(target, record, 0o644)
        had_predecessor = planned.get("present") is True
        predecessor_was_loaded = planned.get("loaded") is True
        installs.append(
            LaunchAgentInstall(
                validator=validator,
                target=target,
                backup=backup,
                had_predecessor=had_predecessor,
                was_already_desired=desired,
                predecessor_was_loaded=predecessor_was_loaded,
            )
        )
        if not desired:
            if backup.exists() or backup.is_symlink():
                predecessor, predecessor_mode = _file_digest_record(
                    backup, "predecessor LaunchAgent backup"
                )
            else:
                try:
                    os.lstat(target)
                except FileNotFoundError:
                    predecessor = b""
                    predecessor_mode = 0
                else:
                    predecessor, predecessor_mode = _file_digest_record(
                        target, "predecessor LaunchAgent"
                    )
                if had_predecessor:
                    if (
                        len(predecessor) != planned["size"]
                        or sha256_bytes(predecessor) != planned["sha256"]
                        or predecessor_mode != planned["mode"]
                    ):
                        raise Reset17Error("predecessor LaunchAgent changed before backup")
                    _copy_bytes_exclusive(backup, predecessor, predecessor_mode)
                elif predecessor:
                    raise Reset17Error("an unplanned predecessor LaunchAgent appeared")
            _copy_record_atomic(source, target, record, 0o644)
        if had_predecessor:
            predecessor, predecessor_mode = _file_digest_record(
                backup, "predecessor LaunchAgent backup"
            )
            if (
                len(predecessor) != planned["size"]
                or sha256_bytes(predecessor) != planned["sha256"]
                or predecessor_mode != planned["mode"]
            ):
                raise Reset17Error("predecessor LaunchAgent backup drifted")
    return installs


def _run_launchctl(arguments: Sequence[str], allow_absent: bool = False) -> None:
    command = ("/bin/launchctl", *arguments)
    try:
        result = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Reset17Error("launchctl operation is unavailable") from error
    if len(result.stdout) > MAX_HELPER_OUTPUT_BYTES or len(result.stderr) > MAX_HELPER_OUTPUT_BYTES:
        raise Reset17Error("launchctl returned excessive output")
    combined = (result.stdout + result.stderr).decode("utf-8", "replace").lower()
    absent_markers = (
        "no such process",
        "could not find specified service",
        "service not found",
        "no such file or directory",
    )
    if result.returncode != 0 and not (
        allow_absent and any(marker in combined for marker in absent_markers)
    ):
        raise Reset17Error("launchctl rejected the reset17 service transition")


def _bootout_services(candidate: Candidate) -> None:
    domain = f"gui/{os.geteuid()}"
    for validator in candidate.validators:
        _run_launchctl(("bootout", f"{domain}/{validator.label}"), allow_absent=True)


def _bootstrap_services(candidate: Candidate) -> None:
    domain = f"gui/{os.geteuid()}"
    for validator in candidate.validators:
        target = candidate.launch_agents_dir / f"{validator.label}.plist"
        _run_launchctl(("bootstrap", domain, str(target)))


def _restore_launch_agents(
    candidate: Candidate, installs: Sequence[LaunchAgentInstall]
) -> bool:
    domain = f"gui/{os.geteuid()}"
    complete = True
    for install in installs:
        try:
            _run_launchctl(
                ("bootout", f"{domain}/{install.validator.label}"), allow_absent=True
            )
        except (OSError, Reset17Error):
            complete = False
    for install in installs:
        try:
            if install.had_predecessor:
                payload, mode = _file_digest_record(
                    install.backup, "predecessor LaunchAgent backup"
                )
                temporary = install.target.parent / f".{install.target.name}.rollback-{os.getpid()}"
                if temporary.exists() or temporary.is_symlink():
                    raise Reset17Error("LaunchAgent rollback temporary path is occupied")
                _copy_bytes_exclusive(temporary, payload, mode)
                os.replace(temporary, install.target)
            elif not install.was_already_desired:
                try:
                    os.unlink(install.target)
                except FileNotFoundError:
                    pass
        except (OSError, Reset17Error):
            complete = False
    for install in installs:
        if not install.predecessor_was_loaded or not (
            install.had_predecessor or install.was_already_desired
        ):
            continue
        try:
            _run_launchctl(("bootstrap", domain, str(install.target)))
        except (OSError, Reset17Error):
            complete = False
    return complete


def _torii_get(
    origin: str, path: str, timeout_seconds: float
) -> tuple[int, bytes]:
    parsed = urlsplit(origin)
    connection = http.client.HTTPConnection(parsed.hostname, parsed.port, timeout=timeout_seconds)
    try:
        connection.request("GET", path, headers={"Accept": "application/json"})
        response = connection.getresponse()
        payload = response.read(MAX_HELPER_OUTPUT_BYTES + 1)
        status_code = response.status
    finally:
        connection.close()
    if len(payload) > MAX_HELPER_OUTPUT_BYTES:
        raise Reset17Error("one reset17 Torii endpoint returned excessive output")
    return status_code, payload


def _status_snapshot(candidate: Candidate, timeout_seconds: float = 5.0) -> dict[str, Any]:
    validators: list[dict[str, Any]] = []
    for validator in candidate.validators:
        for readiness_path in ("/health", "/readyz"):
            readiness_status, _readiness_payload = _torii_get(
                validator.torii_url, readiness_path, timeout_seconds
            )
            if readiness_status != 200:
                raise Reset17Error("one reset17 Torii readiness endpoint is unavailable")
        status_code, payload = _torii_get(
            validator.torii_url, "/status", timeout_seconds
        )
        if status_code != 200:
            raise Reset17Error("one reset17 Torii status endpoint is unavailable")
        try:
            status_payload_value = json.loads(
                payload,
                object_pairs_hook=_duplicate_rejecting_object,
                parse_constant=_reject_json_constant,
            )
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise Reset17Error("one reset17 Torii status payload is malformed") from error
        status_payload = _require_object(status_payload_value, "Torii status")
        height_value = status_payload.get("blocks", status_payload.get("height"))
        queue_value = status_payload.get("queue_size", 0)
        if (
            type(height_value) is not int
            or height_value < 1
            or type(queue_value) is not int
            or queue_value < 0
        ):
            raise Reset17Error("one reset17 Torii status payload is malformed")
        if status_payload.get("restart_required") is True or status_payload.get(
            "liveness_blocker"
        ) not in (None, "", [], {}):
            raise Reset17Error("one reset17 validator reports a liveness blocker")
        observed_network = status_payload.get("network_id")
        if observed_network is not None and observed_network != candidate.network_id:
            raise Reset17Error("one reset17 validator reports a foreign NetworkId")
        validators.append(
            {
                "index": validator.index,
                "height": height_value,
                "queue_size": queue_value,
            }
        )
    heights = [entry["height"] for entry in validators]
    return {
        "validators": validators,
        "minimum_height": min(heights),
        "maximum_height": max(heights),
    }


def wait_for_four_peer_progress(
    candidate: Candidate, timeout_seconds: int
) -> dict[str, Any]:
    if type(timeout_seconds) is not int or not 15 <= timeout_seconds <= 600:
        raise Reset17Error("health timeout must be an integer from 15 through 600 seconds")
    deadline = time.monotonic() + timeout_seconds
    first: dict[str, Any] | None = None
    while time.monotonic() < deadline:
        try:
            sample = _status_snapshot(candidate)
        except (OSError, ValueError, json.JSONDecodeError, Reset17Error):
            time.sleep(1)
            continue
        converged = sample["maximum_height"] - sample["minimum_height"] <= 1
        empty_queues = all(
            entry["queue_size"] == 0 for entry in sample["validators"]
        )
        if converged and empty_queues:
            if first is None:
                first = sample
            elif sample["minimum_height"] > first["minimum_height"]:
                return {"initial": first, "progressed": sample}
        time.sleep(1)
    raise Reset17Error("four-peer Taira health did not converge and progress")


def _write_failure_receipt(
    candidate: Candidate, plan_sha256: str, rollback_complete: bool
) -> None:
    failure_path = (
        candidate.control_root
        / "results"
        / f"{candidate.release_id}.failure.{plan_sha256[:12]}.json"
    )
    payload = canonical_json_bytes(
        {
            "schema": RESULT_SCHEMA,
            "generation": GENERATION,
            "status": "rolled_back" if rollback_complete else "rollback_incomplete",
            "failure": "post-mutation reset17 apply failure",
            "release_id": candidate.release_id,
            "release_dir": str(candidate.release_dir),
            "plan_sha256": plan_sha256,
            "manifest_sha256": candidate.manifest_sha256,
            "source_commit": candidate.source_commit,
            "network_id": candidate.network_id,
            "bpng_asset_alias": BPNG_ASSET_ALIAS,
        }
    )
    _write_canonical_file(failure_path, payload, 0o400)


def apply_candidate(
    candidate: Candidate,
    *,
    plan_path: Path,
    expected_plan_sha256: str,
    confirmation: str,
    result_path: Path,
    health_timeout_seconds: int,
) -> dict[str, Any]:
    if confirmation != f"{PLAN_CONFIRMATION_PREFIX}{expected_plan_sha256}":
        raise Reset17Error("reset17 apply confirmation is not exact")
    if result_path != candidate.control_root / "results" / f"{candidate.release_id}.json":
        raise Reset17Error("result path is not the derived reset17 receipt path")
    parsed_plan, _plan_payload = _read_and_check_plan(
        candidate, plan_path, expected_plan_sha256
    )
    predecessors = parsed_plan.get("predecessor")
    if not isinstance(predecessors, list):
        raise Reset17Error("reset17 plan predecessor inventory is malformed")
    installs: list[LaunchAgentInstall] = []
    mutation_started = False
    try:
        _stage_immutable_release(candidate)
        mutation_started = True
        _prepare_runtime_directories(candidate)
        _persist_predecessor_snapshot(candidate, predecessors)
        _install_launch_agents(candidate, predecessors, installs)
        _bootout_services(candidate)
        _bootstrap_services(candidate)
        health = wait_for_four_peer_progress(candidate, health_timeout_seconds)
    except BaseException:
        rollback_complete = (
            _restore_launch_agents(candidate, installs) if installs else True
        )
        if mutation_started:
            try:
                _write_failure_receipt(
                    candidate, expected_plan_sha256, rollback_complete
                )
            except (OSError, Reset17Error):
                pass
        raise
    result = {
        "schema": RESULT_SCHEMA,
        "generation": GENERATION,
        "status": "applied",
        "applied_at_unix": int(time.time()),
        "release_id": candidate.release_id,
        "release_dir": str(candidate.release_dir),
        "plan_sha256": expected_plan_sha256,
        "manifest_sha256": candidate.manifest_sha256,
        "source_commit": candidate.source_commit,
        "network_id": candidate.network_id,
        "bpng": {
            "asset_alias": BPNG_ASSET_ALIAS,
            "asset_definition": BPNG_ASSET_DEFINITION,
            "asset_domain": BPNG_ASSET_DOMAIN,
            "scale": BPNG_ASSET_SCALE,
            "lane_id": BPNG_LANE_ID,
            "lane_alias": BPNG_LANE_ALIAS,
            "physical_dataspace_id": BPNG_PHYSICAL_DATASPACE_ID,
            "physical_dataspace_alias": BPNG_PHYSICAL_DATASPACE_ALIAS,
        },
        "validators": [validator.label for validator in candidate.validators],
        "health": health,
    }
    try:
        _write_canonical_file(result_path, canonical_json_bytes(result), 0o400)
    except BaseException:
        rollback_complete = _restore_launch_agents(candidate, installs)
        try:
            _write_failure_receipt(
                candidate, expected_plan_sha256, rollback_complete
            )
        except (OSError, Reset17Error):
            pass
        raise
    return result


def _add_candidate_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--bundle", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--signature", required=True, type=Path)
    parser.add_argument("--allowed-signers", required=True, type=Path)
    parser.add_argument("--expected-manifest-sha256", required=True)
    parser.add_argument("--expected-allowed-signers-sha256", required=True)
    parser.add_argument("--expected-source-commit", required=True)
    parser.add_argument("--expected-control-root", required=True, type=Path)
    parser.add_argument("--expected-launch-agents-dir", required=True, type=Path)


def _candidate_kwargs(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "bundle": args.bundle,
        "manifest_path": args.manifest,
        "signature_path": args.signature,
        "allowed_signers_path": args.allowed_signers,
        "expected_manifest_sha256": args.expected_manifest_sha256,
        "expected_allowed_signers_sha256": args.expected_allowed_signers_sha256,
        "expected_source_commit": args.expected_source_commit,
        "expected_control_root": args.expected_control_root,
        "expected_launch_agents_dir": args.expected_launch_agents_dir,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    plan_command = subparsers.add_parser(
        "plan", help="authenticate a candidate and write a deterministic plan"
    )
    _add_candidate_arguments(plan_command)
    plan_command.add_argument("--out-plan", required=True, type=Path)

    check_command = subparsers.add_parser(
        "check-plan", help="reauthenticate and verify a reviewed plan"
    )
    _add_candidate_arguments(check_command)
    check_command.add_argument("--plan", required=True, type=Path)
    check_command.add_argument("--expected-plan-sha256", required=True)

    apply_command = subparsers.add_parser(
        "apply", help="apply an exact reviewed plan under a fail-stop lock"
    )
    _add_candidate_arguments(apply_command)
    apply_command.add_argument("--plan", required=True, type=Path)
    apply_command.add_argument("--expected-plan-sha256", required=True)
    apply_command.add_argument("--confirm", required=True)
    apply_command.add_argument("--result", required=True, type=Path)
    apply_command.add_argument(
        "--health-timeout-seconds", type=int, default=180
    )
    return parser


def _open_apply_lock(candidate: Candidate) -> int:
    lock_path = candidate.control_root / "reset17.apply.lock"
    _reject_symlink_components(lock_path, "reset17 apply lock")
    flags = (
        os.O_RDWR
        | os.O_CREAT
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(lock_path, flags, 0o600)
    except OSError as error:
        raise Reset17Error("reset17 apply lock cannot be opened safely") from error
    try:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != os.geteuid()
            or stat.S_IMODE(metadata.st_mode) != 0o600
            or metadata.st_nlink != 1
        ):
            raise Reset17Error("reset17 apply lock metadata is untrusted")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise Reset17Error("another reset17 apply holds the fail-stop lock") from error
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        candidate = load_candidate(**_candidate_kwargs(args))
        if args.command == "plan":
            payload = plan_bytes(candidate)
            _write_canonical_file(args.out_plan, payload, 0o400)
            print(sha256_bytes(payload))
            return 0
        if args.command == "check-plan":
            _read_and_check_plan(candidate, args.plan, args.expected_plan_sha256)
            print(args.expected_plan_sha256)
            return 0
        if args.confirm != f"{PLAN_CONFIRMATION_PREFIX}{args.expected_plan_sha256}":
            raise Reset17Error("reset17 apply confirmation is not exact")
        _read_and_check_plan(candidate, args.plan, args.expected_plan_sha256)
        lock_descriptor = _open_apply_lock(candidate)
        try:
            # Repeat authentication, private identity, helper, topology, and
            # storage checks after acquiring the nonblocking exclusive lock.
            locked_candidate = load_candidate(**_candidate_kwargs(args))
            if locked_candidate.control_root != candidate.control_root:
                raise Reset17Error("reset17 control root changed under apply lock")
            result = apply_candidate(
                locked_candidate,
                plan_path=args.plan,
                expected_plan_sha256=args.expected_plan_sha256,
                confirmation=args.confirm,
                result_path=args.result,
                health_timeout_seconds=args.health_timeout_seconds,
            )
        finally:
            fcntl.flock(lock_descriptor, fcntl.LOCK_UN)
            os.close(lock_descriptor)
        print(sha256_bytes(canonical_json_bytes(result)))
        return 0
    except Reset17Error as error:
        print(f"taira-reset17: {error}", file=sys.stderr)
        return 1
    except OSError:
        print("taira-reset17: operating system refused the sealed request", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
