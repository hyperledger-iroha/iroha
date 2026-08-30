#!/usr/bin/env python3
"""Build, package, and verify the isolated Generic11 privacy wallet worker.

Release packages are built only from one raw-clean SSH-signed workspace and
are content-addressed by the exact executable SHA-256.  The manifest binds the
Cargo lock, whole-workspace source manifest, critical worker source closure,
closed Generic11 operation registry, frozen build command, every effective
environment value, resolved build tools, Rust component closures, target,
process-hardening contract, and an authenticated IPWW ping against the
packaged bytes.  A supplied prebuilt may be packaged for inspection, but it
is permanently candidate-only.

This script never accepts or reads an owner bundle, wallet key, proof witness,
or transaction signer secret.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import importlib.util
import json
import os
import re
import secrets
import shutil
import stat
import struct
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Mapping, NoReturn, Sequence


SCHEMA = "iroha.privacy.generic11_wallet_worker_package.v2"
SCHEMA_VERSION = 2
ARTIFACT_FILE = "iroha_privacy_wallet_worker"
WORKER_ROLE = "exact12-generic11-owner-bundle-worker-v1"
PROTOCOL_MAGIC = "IPWW"
PROTOCOL_VERSION = 2
RELEASE_TARGET = "aarch64-unknown-linux-gnu"
PROCESS_HARDENING_CONTRACT = (
    "owner-only-local-pipes-core-dump-disabled-linux-nondumpable-v1"
)
AUTHENTICATED_SOURCE_BUILD_V2 = "cargo-iroha-fast-frozen-release-v2"
PREBUILT_CANDIDATE_BUILD_V1 = "prebuilt-artifact-candidate-v1"
SOURCE_CLOSURE_MANIFEST = Path(
    "ci/privacy_generic11_worker_source_closure_v1.txt"
)
SOURCE_CLOSURE_SCHEMA = (
    "path-and-length-framed-sha256("
    "ci/privacy_generic11_worker_source_closure_v1.txt):v1"
)

GENERIC11_OPERATION_REGISTRY_V1: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("zk-ace-pq-authorization-v0", ("zk_ace_authorization_action_v1",)),
    ("anonymous-pgc-k-out-of-n-v1", ("anonymous_pgc_payment_action_v1",)),
    ("verange-transparent-range-v1", ("verange_range_proof_v1",)),
    (
        "iroha-zk-ams-v1",
        (
            "zk_ams_batch_admission_action_v1",
            "zk_ams_provision_account_action_v1",
        ),
    ),
    ("vega-existing-credential-zk-v0", ("vega_credential_presentation_v1",)),
    (
        "iroha-jindo-polynomial-commitment-v0",
        ("jindo_polynomial_evaluation_v1",),
    ),
    (
        "iroha-bootle-lantern-anoncred-v1",
        ("bootle_lantern_credential_presentation_v1",),
    ),
    ("orchard-halo2-actions-v1", ("orchard_note_action_v1",)),
    ("monero-fcmp-plus-plus-v1", ("fcmp_membership_payment_v1",)),
    ("iroha-ivm-private-note-stark-v1", ("ivm_private_note_action_v1",)),
    ("pq-masp-stark-v0", ("pq_masp_note_action_v1",)),
)

_EXPECTED_SOURCE_CLOSURE = (
    PurePosixPath(".cargo/config.toml"),
    PurePosixPath("Cargo.lock"),
    PurePosixPath("Cargo.toml"),
    PurePosixPath("ci/privacy_generic11_worker_source_closure_v1.txt"),
    PurePosixPath("python/iroha_python/iroha_python_rs/Cargo.toml"),
    PurePosixPath("python/iroha_python/iroha_python_rs/build.rs"),
    PurePosixPath(
        "python/iroha_python/iroha_python_rs/src/bin/iroha_privacy_wallet_worker.rs"
    ),
    PurePosixPath("python/iroha_python/iroha_python_rs/src/lib.rs"),
    PurePosixPath("python/iroha_python/iroha_python_rs/src/privacy_native_actions.rs"),
    PurePosixPath("python/iroha_python/iroha_python_rs/src/privacy_wallet_bundle.rs"),
    PurePosixPath("python/iroha_python/iroha_python_rs/src/privacy_wallet_worker.rs"),
    PurePosixPath("python/iroha_python/src/iroha_python/privacy_wallet_worker.py"),
    PurePosixPath("scripts/check_privacy_python_witness_boundary.py"),
    PurePosixPath("scripts/compute_workspace_source_manifest.py"),
    PurePosixPath("scripts/package_privacy_wallet_worker.py"),
)

_WORKSPACE_MANIFEST_HELPER = (
    Path(__file__).resolve().parent / "compute_workspace_source_manifest.py"
)
_SOURCE_CLOSURE_DOMAIN = b"iroha.privacy.generic11.worker-source-closure.v1"
_REGISTRY_DOMAIN = b"iroha.privacy.generic11.worker-operation-registry.v1"
_BUILD_COMMAND_DOMAIN = b"iroha.privacy.generic11.worker-build-command.v1"
_BUILD_ENVIRONMENT_DOMAIN = b"iroha.privacy.generic11.worker-build-environment.v2"
_BUILD_TOOLCHAIN_DOMAIN = b"iroha.privacy.generic11.worker-build-toolchain.v2"
_BUILD_PROVENANCE_SCHEMA = "iroha.privacy.generic11.worker-build-provenance.v2"
_BUILD_TOOLCHAIN_SCHEMA = "iroha.privacy.generic11.worker-build-toolchain.v2"
_RUST_COMPONENT_CLOSURE_DOMAIN = (
    b"iroha.privacy.generic11.worker-rust-component-closure.v1"
)
_FRAME_MAGIC = b"IPWW"
_PING_COMMAND = 1
_AUTH_KEY_BYTES = 32
_AUTH_TAG_BYTES = 32
_MAX_FRAME_BYTES = 34 * 1_024 * 1_024
_CONTROLLER_SOURCE = (
    Path(__file__).parents[1]
    / "python/iroha_python/src/iroha_python/privacy_wallet_worker.py"
)
_MAX_ARTIFACT_BYTES = 512 * 1_024 * 1_024
_MAX_MANIFEST_BYTES = 128 * 1_024
_MAX_POLICY_BYTES = 16 * 1_024 * 1_024
_MAX_SOURCE_FILE_BYTES = 128 * 1_024 * 1_024
_MAX_TOOL_FILE_BYTES = 512 * 1_024 * 1_024
_MAX_TOOL_OUTPUT_BYTES = 64 * 1_024
_SYSTEM_GIT = "/usr/bin/git"
_SYSTEM_SSH_KEYGEN = "/usr/bin/ssh-keygen"
_TARGET_RE = re.compile(r"[a-z0-9][a-z0-9._+-]{0,127}")
_ENVIRONMENT_NAME_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]{0,127}")
_INHERITED_BUILD_ENVIRONMENT_NAMES = (
    "CARGO_HOME",
    "CARGO_TARGET_DIR",
    "HOME",
    "PATH",
    "RUSTUP_HOME",
    "SCCACHE_DIR",
    "TMPDIR",
)
_BUILD_TOOL_ROLES = (
    "archiver",
    "cargo",
    "cargo_iroha_fast",
    "dirname",
    "env",
    "git",
    "grep",
    "linker",
    "linker_driver",
    "rustc",
    "rustc_wrapper",
    "shell",
    "uname",
)
_RUST_COMPONENT_ROLES = ("cargo", "rust_std", "rustc")


class PrivacyWalletWorkerPackageError(RuntimeError):
    """One fail-closed Generic11 worker package error."""


def _fail(message: str) -> NoReturn:
    raise PrivacyWalletWorkerPackageError(message)


@dataclass(frozen=True)
class StableFileV1:
    path: Path
    device: int
    inode: int
    mode: int
    owner: int
    links: int
    size: int
    modified_ns: int
    sha256: str


@dataclass(frozen=True)
class SourceEvidenceV1:
    allowed_signers_sha256: str
    cargo_lock_sha256: str
    commit: str
    revocation_sha256: str
    source_closure_sha256: str
    source_date_epoch: int
    workspace_source_manifest_sha256: str


@dataclass(frozen=True)
class AuthenticatedBuildCorridorV2:
    """Exact effective inputs used for one authenticated Cargo build."""

    cargo: Path
    environment: dict[str, str]
    provenance: dict[str, object]


def _require_lower_hex(value: object, digits: int, label: str) -> str:
    if (
        type(value) is not str
        or len(value) != digits
        or any(character not in "0123456789abcdef" for character in value)
    ):
        _fail(f"{label} must be exactly {digits} lowercase hexadecimal digits")
    if value == "0" * digits:
        _fail(f"{label} must be nonzero")
    return value


def _require_sha256(value: object, label: str) -> str:
    return _require_lower_hex(value, 64, label)


def _require_commit(value: object, label: str) -> str:
    return _require_lower_hex(value, 40, label)


def _plain_object(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict:
        _fail(f"{label} must be a JSON object")
    return value


def _reject_duplicate_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            _fail(f"Generic11 worker package contains duplicate key {key!r}")
        value[key] = item
    return value


def _canonical_json_bytes(value: object) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=True,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")


def _canonical_absolute_path(path: Path, label: str, *, existing: bool = True) -> Path:
    if not path.is_absolute():
        _fail(f"{label} must be an absolute path")
    try:
        resolved = path.resolve(strict=existing)
    except OSError as error:
        raise PrivacyWalletWorkerPackageError(f"{label} is unavailable") from error
    if resolved != path:
        _fail(f"{label} must already be canonical")
    return path


def _read_stable_file(
    path: Path,
    *,
    label: str,
    maximum: int,
    allow_empty: bool = False,
    require_executable: bool = False,
    require_owner: bool = False,
    capture_payload: bool = False,
) -> tuple[StableFileV1, bytes | None]:
    try:
        before = path.lstat()
    except OSError as error:
        raise PrivacyWalletWorkerPackageError(f"{label} is unavailable: {path}") from error
    minimum = 0 if allow_empty else 1
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or not minimum <= before.st_size <= maximum
        or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or (require_executable and not before.st_mode & stat.S_IXUSR)
        or (require_owner and before.st_uid != os.geteuid())
    ):
        _fail(f"{label} must be one bounded owner-controlled regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise PrivacyWalletWorkerPackageError(f"{label} could not be opened safely") from error
    payload = bytearray() if capture_payload else None
    digest = hashlib.sha256()
    size = 0
    try:
        opened = os.fstat(descriptor)
        if (
            opened.st_dev,
            opened.st_ino,
            opened.st_mode,
            opened.st_uid,
            opened.st_nlink,
            opened.st_size,
            opened.st_mtime_ns,
        ) != (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_uid,
            before.st_nlink,
            before.st_size,
            before.st_mtime_ns,
        ):
            _fail(f"{label} changed before it was opened")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            size += len(chunk)
            if size > maximum:
                _fail(f"{label} exceeds its size bound")
            digest.update(chunk)
            if payload is not None:
                payload.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        size != before.st_size
        or (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_uid,
            after.st_nlink,
            after.st_size,
            after.st_mtime_ns,
        )
        != (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_uid,
            before.st_nlink,
            before.st_size,
            before.st_mtime_ns,
        )
    ):
        _fail(f"{label} changed while it was read")
    identity = StableFileV1(
        path=path,
        device=before.st_dev,
        inode=before.st_ino,
        mode=before.st_mode,
        owner=before.st_uid,
        links=before.st_nlink,
        size=size,
        modified_ns=before.st_mtime_ns,
        sha256=digest.hexdigest(),
    )
    return identity, bytes(payload) if payload is not None else None


def _stable_file(path: Path, **kwargs: object) -> StableFileV1:
    identity, _ = _read_stable_file(path, **kwargs)
    return identity


def _stable_bytes(path: Path, *, label: str, maximum: int, allow_empty: bool = False) -> bytes:
    identity, payload = _read_stable_file(
        path,
        label=label,
        maximum=maximum,
        allow_empty=allow_empty,
        capture_payload=True,
    )
    if payload is None or len(payload) != identity.size:
        _fail(f"{label} could not be read completely")
    return payload


def _source_closure_paths(source_root: Path) -> tuple[PurePosixPath, ...]:
    payload = _stable_bytes(
        source_root / SOURCE_CLOSURE_MANIFEST,
        label="Generic11 worker source-closure manifest",
        maximum=64 * 1024,
    )
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise PrivacyWalletWorkerPackageError(
            "Generic11 worker source-closure manifest is not UTF-8"
        ) from error
    if not text.endswith("\n") or "\r" in text:
        _fail("Generic11 worker source-closure manifest is not canonical LF text")
    paths: list[PurePosixPath] = []
    for raw in text.splitlines():
        path = PurePosixPath(raw)
        if (
            not raw
            or raw.startswith("/")
            or path.as_posix() != raw
            or any(part in ("", ".", "..") for part in path.parts)
        ):
            _fail("Generic11 worker source-closure manifest has a non-canonical path")
        paths.append(path)
    if tuple(paths) != _EXPECTED_SOURCE_CLOSURE:
        _fail("Generic11 worker source-closure path inventory is not exact")
    return tuple(paths)


def source_closure_sha256(source_root: Path) -> str:
    source_root = source_root.resolve(strict=True)
    manifest_payload = _stable_bytes(
        source_root / SOURCE_CLOSURE_MANIFEST,
        label="Generic11 worker source-closure manifest",
        maximum=64 * 1024,
    )
    paths = _source_closure_paths(source_root)
    digest = hashlib.sha256()
    digest.update(_SOURCE_CLOSURE_DOMAIN)
    digest.update(len(manifest_payload).to_bytes(8, "big"))
    digest.update(manifest_payload)
    digest.update(len(paths).to_bytes(4, "big"))
    for path in paths:
        encoded_path = path.as_posix().encode("utf-8")
        payload = _stable_bytes(
            source_root.joinpath(*path.parts),
            label=f"Generic11 worker source {path}",
            maximum=_MAX_SOURCE_FILE_BYTES,
        )
        digest.update(len(encoded_path).to_bytes(2, "big"))
        digest.update(encoded_path)
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)
    return digest.hexdigest()


def _git_environment() -> dict[str, str]:
    return {
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_NO_LAZY_FETCH": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "HOME": os.path.abspath(os.sep),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
    }


def _git(source_root: Path, arguments: Sequence[str], *, timeout: int = 60) -> str:
    try:
        result = subprocess.run(
            [_SYSTEM_GIT, "-C", os.fspath(source_root), *arguments],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout,
            env=_git_environment(),
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise PrivacyWalletWorkerPackageError("Git source authentication failed") from error
    if result.returncode != 0:
        detail = result.stderr.strip()
        _fail("Git source authentication failed" + (f": {detail}" if detail else ""))
    return result.stdout


def _verify_source_signature(
    source_root: Path,
    commit: str,
    allowed_signers: Path,
    revocation: Path,
) -> None:
    _git(
        source_root,
        (
            "-c",
            "gpg.format=ssh",
            "-c",
            f"gpg.ssh.program={_SYSTEM_SSH_KEYGEN}",
            "-c",
            f"gpg.ssh.allowedSignersFile={allowed_signers}",
            "-c",
            f"gpg.ssh.revocationFile={revocation}",
            "verify-commit",
            commit,
        ),
    )


def _raw_release_source_identity(source_root: Path) -> dict[str, object]:
    environment = {
        "HOME": os.path.abspath(os.sep),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
    }
    try:
        completed = subprocess.run(
            [
                sys.executable,
                "-I",
                "-S",
                os.fspath(_WORKSPACE_MANIFEST_HELPER),
                "--root",
                os.fspath(source_root),
                "--release-identity-json",
            ],
            cwd=os.path.abspath(os.sep),
            env=environment,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=600,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise PrivacyWalletWorkerPackageError(
            "raw clean-source identity capture failed"
        ) from error
    if completed.returncode != 0 or not 1 <= len(completed.stdout) <= 64 * 1024:
        _fail("raw clean-source identity capture failed")
    try:
        value = json.loads(
            completed.stdout.decode("ascii"), object_pairs_hook=_reject_duplicate_pairs
        )
    except (UnicodeError, json.JSONDecodeError) as error:
        raise PrivacyWalletWorkerPackageError(
            "raw clean-source identity is not canonical JSON"
        ) from error
    identity = _plain_object(value, "raw clean-source identity")
    if completed.stdout != _canonical_json_bytes(identity):
        _fail("raw clean-source identity is not canonical JSON")
    return identity


def collect_source_evidence(
    source_root: Path,
    *,
    allowed_signers: Path,
    expected_allowed_signers_sha256: str,
    revocation: Path,
    expected_revocation_sha256: str,
) -> SourceEvidenceV1:
    source_root = source_root.resolve(strict=True)
    allowed_signers = _canonical_absolute_path(allowed_signers, "allowed_signers")
    revocation = _canonical_absolute_path(revocation, "revocation")
    allowed = _stable_file(
        allowed_signers,
        label="SSH allowed-signers policy",
        maximum=_MAX_POLICY_BYTES,
    )
    revoked = _stable_file(
        revocation,
        label="SSH revocation policy",
        maximum=_MAX_POLICY_BYTES,
        allow_empty=True,
    )
    if not hmac.compare_digest(
        allowed.sha256,
        _require_sha256(expected_allowed_signers_sha256, "allowed-signers SHA-256"),
    ):
        _fail("SSH allowed-signers policy does not match its trusted SHA-256")
    if not hmac.compare_digest(
        revoked.sha256,
        _require_sha256(expected_revocation_sha256, "revocation SHA-256"),
    ):
        _fail("SSH revocation policy does not match its trusted SHA-256")
    identity = _raw_release_source_identity(source_root)
    commit = _require_commit(identity.get("head_commit"), "source commit")
    workspace_digest = _require_sha256(
        identity.get("workspace_source_manifest_sha256"),
        "workspace source-manifest SHA-256",
    )
    cargo_lock_sha256 = _require_sha256(
        identity.get("cargo_lock_sha256"), "Cargo.lock SHA-256"
    )
    timestamp_text = _git(source_root, ("show", "-s", "--format=%ct", commit)).strip()
    if not timestamp_text.isascii() or not timestamp_text.isdigit():
        _fail("signed source commit timestamp is not canonical")
    source_date_epoch = int(timestamp_text)
    if source_date_epoch <= 0:
        _fail("signed source commit timestamp must be positive")
    _verify_source_signature(source_root, commit, allowed_signers, revocation)
    closure_digest = source_closure_sha256(source_root)
    repeated_identity = _raw_release_source_identity(source_root)
    repeated_allowed = _stable_file(
        allowed_signers,
        label="SSH allowed-signers policy",
        maximum=_MAX_POLICY_BYTES,
    )
    repeated_revoked = _stable_file(
        revocation,
        label="SSH revocation policy",
        maximum=_MAX_POLICY_BYTES,
        allow_empty=True,
    )
    if (
        repeated_identity != identity
        or repeated_allowed != allowed
        or repeated_revoked != revoked
        or _git(source_root, ("show", "-s", "--format=%ct", commit)).strip()
        != timestamp_text
    ):
        _fail("source or SSH policy changed while worker evidence was collected")
    return SourceEvidenceV1(
        allowed_signers_sha256=allowed.sha256,
        cargo_lock_sha256=cargo_lock_sha256,
        commit=commit,
        revocation_sha256=revoked.sha256,
        source_closure_sha256=closure_digest,
        source_date_epoch=source_date_epoch,
        workspace_source_manifest_sha256=workspace_digest,
    )


def operation_registry_manifest_v1() -> list[dict[str, object]]:
    return [
        {
            "operation_schemas": list(operation_schemas),
            "protocol_id": protocol_id,
        }
        for protocol_id, operation_schemas in GENERIC11_OPERATION_REGISTRY_V1
    ]


def operation_registry_sha256_v1() -> str:
    digest = hashlib.sha256()
    digest.update(_REGISTRY_DOMAIN)
    digest.update(_canonical_json_bytes(operation_registry_manifest_v1()))
    return digest.hexdigest()


def _cargo_build_command(target: str) -> tuple[str, ...]:
    if _TARGET_RE.fullmatch(target) is None:
        _fail("Generic11 worker target is not canonical")
    return (
        "cargo",
        "iroha-fast",
        "--",
        "build",
        "--frozen",
        "--profile",
        "release",
        "--package",
        "iroha_python_rs",
        "--bin",
        ARTIFACT_FILE,
        "--target",
        target,
    )


def _framed_sha256(domain: bytes, values: Sequence[str]) -> str:
    digest = hashlib.sha256()
    digest.update(domain)
    digest.update(len(values).to_bytes(4, "big"))
    for value in values:
        encoded = value.encode("utf-8")
        digest.update(len(encoded).to_bytes(4, "big"))
        digest.update(encoded)
    return digest.hexdigest()


def _build_command_sha256(target: str) -> str:
    return _framed_sha256(_BUILD_COMMAND_DOMAIN, _cargo_build_command(target))


def _build_environment_values(source: SourceEvidenceV1) -> dict[str, str]:
    if source.source_date_epoch <= 0:
        _fail("source date epoch must be positive")
    return {
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "IROHA_PRIVACY_GENERIC11_ALLOWED_SIGNERS_SHA256": source.allowed_signers_sha256,
        "IROHA_PRIVACY_GENERIC11_CARGO_LOCK_SHA256": source.cargo_lock_sha256,
        "IROHA_PRIVACY_GENERIC11_REVOCATION_SHA256": source.revocation_sha256,
        "IROHA_PRIVACY_GENERIC11_SIGNED_SOURCE_COMMIT": source.commit,
        "IROHA_PRIVACY_GENERIC11_SOURCE_CLOSURE_SHA256": source.source_closure_sha256,
        "IROHA_PRIVACY_GENERIC11_WORKSPACE_SOURCE_MANIFEST_SHA256": (
            source.workspace_source_manifest_sha256
        ),
        "IROHA_PYTHON_SKIP_RUNTIME_LINK": "1",
        "LANG": "C",
        "LC_ALL": "C",
        "SCCACHE_CLIENT_SIDE": "1",
        "SOURCE_DATE_EPOCH": str(source.source_date_epoch),
        "VERGEN_GIT_SHA": source.commit,
    }


def _canonical_build_environment(
    value: object,
    *,
    source: SourceEvidenceV1 | None = None,
    target: str | None = None,
) -> dict[str, str]:
    environment = _plain_object(value, "Generic11 worker build environment")
    canonical: dict[str, str] = {}
    for name, item in environment.items():
        if (
            _ENVIRONMENT_NAME_RE.fullmatch(name) is None
            or type(item) is not str
            or not item
            or len(item.encode("utf-8")) > 32 * 1_024
            or "\0" in item
        ):
            _fail("Generic11 worker build environment is not canonical")
        canonical[name] = item
    if not canonical.get("HOME") or not canonical.get("PATH"):
        _fail("Generic11 worker build environment requires HOME and PATH")
    if source is not None:
        semantic = _build_environment_values(source)
        if any(canonical.get(name) != item for name, item in semantic.items()):
            _fail("Generic11 worker semantic build environment is not exact")
    if target is not None:
        if _TARGET_RE.fullmatch(target) is None:
            _fail("Generic11 worker target is not canonical")
        suffix = target.upper().replace("-", "_").replace(".", "_")
        cc_suffix = target.replace("-", "_").replace(".", "_")
        required_derived = {
            "AR",
            f"AR_{cc_suffix}",
            "CC",
            f"CC_{cc_suffix}",
            f"CARGO_TARGET_{suffix}_LINKER",
            "RUSTC",
            "RUSTC_WRAPPER",
        }
        allowed = (
            set(_INHERITED_BUILD_ENVIRONMENT_NAMES)
            | set(_build_environment_values(source))
            | required_derived
        ) if source is not None else set(canonical)
        if set(canonical) - allowed or not required_derived <= set(canonical):
            _fail("Generic11 worker effective build environment inventory is not exact")
    return dict(sorted(canonical.items()))


def _build_environment_sha256(environment: Mapping[str, str]) -> str:
    canonical = _canonical_build_environment(dict(environment))
    flattened = [f"{name}={item}" for name, item in canonical.items()]
    return _framed_sha256(_BUILD_ENVIRONMENT_DOMAIN, flattened)


def _canonical_record_path(value: object, label: str) -> str:
    if type(value) is not str or not value or "\0" in value:
        _fail(f"{label} path is invalid")
    path = Path(value)
    if not path.is_absolute() or os.path.normpath(value) != value:
        _fail(f"{label} path is not canonical")
    return value


def _validate_build_file_record(value: object, label: str) -> dict[str, object]:
    record = _plain_object(value, label)
    if set(record) != {"mode", "owner", "path", "sha256", "size"}:
        _fail(f"{label} field inventory is not exact")
    path = _canonical_record_path(record["path"], label)
    digest = _require_sha256(record["sha256"], f"{label} SHA-256")
    size = record["size"]
    mode = record["mode"]
    owner = record["owner"]
    if (
        type(size) is not int
        or not 1 <= size <= _MAX_TOOL_FILE_BYTES
        or type(mode) is not int
        or not 0 <= mode <= 0o7777
        or mode & (stat.S_IWGRP | stat.S_IWOTH)
        or type(owner) is not int
        or not 0 <= owner <= (1 << 31) - 1
    ):
        _fail(f"{label} metadata is invalid")
    return {
        "mode": mode,
        "owner": owner,
        "path": path,
        "sha256": digest,
        "size": size,
    }


def _validate_component_record(value: object, label: str) -> dict[str, object]:
    record = _plain_object(value, label)
    if set(record) != {
        "closure_sha256",
        "file_count",
        "manifest_path",
        "manifest_sha256",
        "total_bytes",
    }:
        _fail(f"{label} field inventory is not exact")
    manifest_path = _canonical_record_path(record["manifest_path"], label)
    manifest_sha256 = _require_sha256(
        record["manifest_sha256"], f"{label} manifest SHA-256"
    )
    closure_sha256 = _require_sha256(
        record["closure_sha256"], f"{label} closure SHA-256"
    )
    file_count = record["file_count"]
    total_bytes = record["total_bytes"]
    if (
        type(file_count) is not int
        or not 1 <= file_count <= 100_000
        or type(total_bytes) is not int
        or not 1 <= total_bytes <= 16 * 1_024 * 1_024 * 1_024
    ):
        _fail(f"{label} bounds are invalid")
    return {
        "closure_sha256": closure_sha256,
        "file_count": file_count,
        "manifest_path": manifest_path,
        "manifest_sha256": manifest_sha256,
        "total_bytes": total_bytes,
    }


def _canonical_build_toolchain(value: object, target: str) -> dict[str, object]:
    toolchain = _plain_object(value, "Generic11 worker build toolchain")
    if set(toolchain) != {
        "cargo_version_sha256",
        "cargo_configuration",
        "components",
        "host",
        "rustc_version_sha256",
        "schema",
        "sysroot",
        "target",
        "tools",
    }:
        _fail("Generic11 worker build toolchain field inventory is not exact")
    if (
        toolchain["schema"] != _BUILD_TOOLCHAIN_SCHEMA
        or toolchain["target"] != target
        or type(toolchain["host"]) is not str
        or _TARGET_RE.fullmatch(toolchain["host"]) is None
    ):
        _fail("Generic11 worker build toolchain identity is invalid")
    sysroot = _canonical_record_path(toolchain["sysroot"], "Rust sysroot")
    tools = _plain_object(toolchain["tools"], "Generic11 worker build tools")
    if set(tools) != set(_BUILD_TOOL_ROLES):
        _fail("Generic11 worker build tool inventory is not exact")
    canonical_tools = {
        role: _validate_build_file_record(tools[role], f"build tool {role}")
        for role in _BUILD_TOOL_ROLES
    }
    if any(not int(record["mode"]) & stat.S_IXUSR for record in canonical_tools.values()):
        _fail("Generic11 worker captured build tools must be owner-executable")
    cargo_configuration = toolchain["cargo_configuration"]
    if type(cargo_configuration) is not list or len(cargo_configuration) > 64:
        _fail("Generic11 worker Cargo configuration inventory is invalid")
    canonical_cargo_configuration = [
        _validate_build_file_record(item, "Cargo configuration")
        for item in cargo_configuration
    ]
    configuration_paths = [item["path"] for item in canonical_cargo_configuration]
    if configuration_paths != sorted(set(configuration_paths)):
        _fail("Generic11 worker Cargo configuration inventory is not canonical")
    components = _plain_object(
        toolchain["components"], "Generic11 worker Rust components"
    )
    if set(components) != set(_RUST_COMPONENT_ROLES):
        _fail("Generic11 worker Rust component inventory is not exact")
    canonical_components = {
        role: _validate_component_record(
            components[role], f"Rust component {role}"
        )
        for role in _RUST_COMPONENT_ROLES
    }
    expected_manifests = {
        "cargo": os.fspath(
            Path(sysroot) / "lib" / "rustlib" / f"manifest-cargo-{toolchain['host']}"
        ),
        "rust_std": os.fspath(
            Path(sysroot) / "lib" / "rustlib" / f"manifest-rust-std-{target}"
        ),
        "rustc": os.fspath(
            Path(sysroot) / "lib" / "rustlib" / f"manifest-rustc-{toolchain['host']}"
        ),
    }
    if any(
        canonical_components[role]["manifest_path"] != expected_path
        for role, expected_path in expected_manifests.items()
    ):
        _fail("Generic11 worker Rust component paths are not toolchain-bound")
    cargo_version = _require_sha256(
        toolchain["cargo_version_sha256"], "Cargo version output SHA-256"
    )
    rustc_version = _require_sha256(
        toolchain["rustc_version_sha256"], "rustc version output SHA-256"
    )
    return {
        "cargo_version_sha256": cargo_version,
        "cargo_configuration": canonical_cargo_configuration,
        "components": canonical_components,
        "host": toolchain["host"],
        "rustc_version_sha256": rustc_version,
        "schema": _BUILD_TOOLCHAIN_SCHEMA,
        "sysroot": sysroot,
        "target": target,
        "tools": canonical_tools,
    }


def _build_toolchain_sha256(toolchain: object, target: str) -> str:
    canonical = _canonical_build_toolchain(toolchain, target)
    digest = hashlib.sha256()
    digest.update(_BUILD_TOOLCHAIN_DOMAIN)
    digest.update(_canonical_json_bytes(canonical))
    return digest.hexdigest()


def _build_provenance_v2(
    environment: Mapping[str, str],
    toolchain: object,
    *,
    source: SourceEvidenceV1,
    target: str,
) -> dict[str, object]:
    canonical_environment = _canonical_build_environment(
        dict(environment), source=source, target=target
    )
    canonical_toolchain = _canonical_build_toolchain(toolchain, target)
    suffix = target.upper().replace("-", "_").replace(".", "_")
    cc_suffix = target.replace("-", "_").replace(".", "_")
    tools = canonical_toolchain["tools"]
    assert isinstance(tools, dict)
    sysroot = Path(str(canonical_toolchain["sysroot"]))
    if (
        tools["cargo"]["path"] != os.fspath(sysroot / "bin" / "cargo")
        or tools["rustc"]["path"] != os.fspath(sysroot / "bin" / "rustc")
    ):
        _fail("Generic11 worker Cargo/rustc paths are not sysroot-bound")
    if (
        canonical_environment["RUSTC"] != tools["rustc"]["path"]
        or canonical_environment["RUSTC_WRAPPER"]
        != tools["rustc_wrapper"]["path"]
    ):
        _fail("Generic11 worker RUSTC does not match the captured toolchain")
    if (
        canonical_environment["CC"] != tools["linker_driver"]["path"]
        or canonical_environment[f"CC_{cc_suffix}"]
        != tools["linker_driver"]["path"]
        or canonical_environment[f"CARGO_TARGET_{suffix}_LINKER"]
        != tools["linker_driver"]["path"]
        or canonical_environment["AR"] != tools["archiver"]["path"]
        or canonical_environment[f"AR_{cc_suffix}"] != tools["archiver"]["path"]
    ):
        _fail("Generic11 worker compiler/linker environment is not tool-bound")
    return {
        "environment": canonical_environment,
        "environment_sha256": _build_environment_sha256(canonical_environment),
        "schema": _BUILD_PROVENANCE_SCHEMA,
        "target": target,
        "toolchain": canonical_toolchain,
        "toolchain_sha256": _build_toolchain_sha256(canonical_toolchain, target),
    }


def _validate_build_provenance_v2(
    value: object,
    *,
    source: SourceEvidenceV1,
    target: str,
) -> dict[str, object]:
    provenance = _plain_object(value, "Generic11 worker build provenance")
    if set(provenance) != {
        "environment",
        "environment_sha256",
        "schema",
        "target",
        "toolchain",
        "toolchain_sha256",
    } or provenance.get("schema") != _BUILD_PROVENANCE_SCHEMA:
        _fail("Generic11 worker build provenance field inventory is not exact")
    canonical = _build_provenance_v2(
        _plain_object(provenance["environment"], "build environment"),
        provenance["toolchain"],
        source=source,
        target=target,
    )
    if provenance["target"] != target:
        _fail("Generic11 worker build provenance target is invalid")
    if not hmac.compare_digest(
        _require_sha256(
            provenance["environment_sha256"], "build environment SHA-256"
        ),
        canonical["environment_sha256"],
    ) or not hmac.compare_digest(
        _require_sha256(
            provenance["toolchain_sha256"], "build toolchain SHA-256"
        ),
        canonical["toolchain_sha256"],
    ):
        _fail("Generic11 worker authenticated build provenance is invalid")
    return canonical


def _encode_ping_frame(sequence: int, auth_key: bytes) -> bytes:
    if sequence <= 0 or len(auth_key) != _AUTH_KEY_BYTES or not any(auth_key):
        _fail("Generic11 worker ping inputs are invalid")
    authenticated = b"".join(
        (
            _FRAME_MAGIC,
            bytes((PROTOCOL_VERSION, _PING_COMMAND)),
            sequence.to_bytes(8, "big"),
            (0).to_bytes(4, "big"),
        )
    )
    body = authenticated + hmac.new(auth_key, authenticated, hashlib.sha256).digest()
    return len(body).to_bytes(4, "big") + body


def _decode_ping_response(encoded: bytes, sequence: int, auth_key: bytes) -> None:
    if not 4 + 18 + _AUTH_TAG_BYTES <= len(encoded) <= _MAX_FRAME_BYTES:
        _fail("Generic11 worker ping response length is invalid")
    declared = int.from_bytes(encoded[:4], "big")
    if declared != len(encoded) - 4:
        _fail("Generic11 worker ping response framing is invalid")
    body = encoded[4:]
    authenticated, tag = body[:-_AUTH_TAG_BYTES], body[-_AUTH_TAG_BYTES:]
    if not hmac.compare_digest(
        tag, hmac.new(auth_key, authenticated, hashlib.sha256).digest()
    ):
        _fail("Generic11 worker ping response authentication failed")
    if (
        authenticated[:4] != _FRAME_MAGIC
        or authenticated[4] != PROTOCOL_VERSION
        or authenticated[5] != _PING_COMMAND
        or int.from_bytes(authenticated[6:14], "big") != sequence
        or int.from_bytes(authenticated[14:18], "big") != 1
        or authenticated[18:] != b"\x00"
    ):
        _fail("Generic11 worker ping response is not canonical")


def _load_exact_worker_launch_module():
    """Load the source-closure-pinned exact-inode launch implementation."""

    module_name = "_iroha_privacy_wallet_worker_package_launch"
    loaded = sys.modules.get(module_name)
    if loaded is not None:
        return loaded
    spec = importlib.util.spec_from_file_location(module_name, _CONTROLLER_SOURCE)
    if spec is None or spec.loader is None:
        _fail("Generic11 worker exact launch module is unavailable")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


def probe_worker_ping(artifact: Path) -> None:
    artifact = _canonical_absolute_path(artifact, "Generic11 worker artifact")
    before = _stable_file(
        artifact,
        label="Generic11 worker artifact",
        maximum=_MAX_ARTIFACT_BYTES,
        require_executable=True,
        require_owner=True,
    )
    auth_key = secrets.token_bytes(_AUTH_KEY_BYTES)
    if len(auth_key) != _AUTH_KEY_BYTES or not any(auth_key):
        _fail("secure Generic11 worker ping authentication key is unavailable")
    request = auth_key + _encode_ping_frame(1, auth_key)
    launch_module = _load_exact_worker_launch_module()
    try:
        launch = launch_module._prepare_verified_worker_launch_v1(
            artifact,
            before.sha256,
        )
    except (OSError, ValueError) as error:
        raise PrivacyWalletWorkerPackageError(
            "Generic11 worker exact authenticated launch failed"
        ) from error
    try:
        try:
            completed = subprocess.run(
                [os.fspath(launch.invocation)],
                input=request,
                cwd=os.path.abspath(os.sep),
                env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=15,
                close_fds=True,
                pass_fds=launch.pass_fds,
                start_new_session=True,
            )
            launch.authenticate()
        except (OSError, ValueError, subprocess.TimeoutExpired) as error:
            raise PrivacyWalletWorkerPackageError(
                "Generic11 worker authenticated ping failed"
            ) from error
    finally:
        launch.close()
    if completed.returncode != 0 or completed.stderr or len(completed.stdout) > _MAX_FRAME_BYTES:
        _fail("Generic11 worker authenticated ping failed")
    _decode_ping_response(completed.stdout, 1, auth_key)
    after = _stable_file(
        artifact,
        label="Generic11 worker artifact",
        maximum=_MAX_ARTIFACT_BYTES,
        require_executable=True,
        require_owner=True,
    )
    if after != before:
        _fail("Generic11 worker artifact changed during authenticated ping")


def _source_from_manifest(manifest: dict[str, object]) -> SourceEvidenceV1:
    source_date_epoch = manifest["source_date_epoch"]
    if type(source_date_epoch) is not int or source_date_epoch <= 0:
        _fail("Generic11 worker source date epoch is invalid")
    return SourceEvidenceV1(
        allowed_signers_sha256=_require_sha256(
            manifest["source_allowed_signers_sha256"], "allowed-signers SHA-256"
        ),
        cargo_lock_sha256=_require_sha256(
            manifest["cargo_lock_sha256"], "Cargo.lock SHA-256"
        ),
        commit=_require_commit(manifest["source_commit"], "source commit"),
        revocation_sha256=_require_sha256(
            manifest["source_revocation_sha256"], "revocation SHA-256"
        ),
        source_closure_sha256=_require_sha256(
            manifest["source_closure_sha256"], "source closure SHA-256"
        ),
        source_date_epoch=source_date_epoch,
        workspace_source_manifest_sha256=_require_sha256(
            manifest["workspace_source_manifest_sha256"],
            "workspace source-manifest SHA-256",
        ),
    )


def _release_ready(
    *,
    source: SourceEvidenceV1,
    target: str,
    build_method: str,
    build_command_sha256: str | None,
    build_provenance: dict[str, object] | None,
    ping_verified: bool,
) -> bool:
    return (
        target == RELEASE_TARGET
        and build_method == AUTHENTICATED_SOURCE_BUILD_V2
        and build_command_sha256 == _build_command_sha256(target)
        and build_provenance is not None
        and build_provenance.get("target") == target
        and ping_verified
    )


def validate_manifest(value: object) -> dict[str, object]:
    manifest = _plain_object(value, "Generic11 worker package manifest")
    expected_keys = {
        "artifact_build_command_sha256",
        "artifact_build_environment_sha256",
        "artifact_build_method",
        "artifact_build_provenance",
        "artifact_build_toolchain_sha256",
        "artifact_file",
        "artifact_sha256",
        "artifact_size",
        "cargo_lock_sha256",
        "operation_registry",
        "operation_registry_sha256",
        "process_hardening_contract",
        "protocol_magic",
        "protocol_version",
        "release_ready",
        "schema",
        "schema_version",
        "smoke_ping_verified",
        "source_allowed_signers_sha256",
        "source_closure_schema",
        "source_closure_sha256",
        "source_commit",
        "source_commit_signature_verified",
        "source_date_epoch",
        "source_revocation_sha256",
        "source_tree_clean",
        "target",
        "worker_role",
        "workspace_source_manifest_sha256",
    }
    if set(manifest) != expected_keys:
        _fail("Generic11 worker package manifest field inventory is not exact")
    if (
        manifest["schema"] != SCHEMA
        or manifest["schema_version"] != SCHEMA_VERSION
        or manifest["artifact_file"] != ARTIFACT_FILE
        or manifest["worker_role"] != WORKER_ROLE
        or manifest["protocol_magic"] != PROTOCOL_MAGIC
        or manifest["protocol_version"] != PROTOCOL_VERSION
        or manifest["process_hardening_contract"] != PROCESS_HARDENING_CONTRACT
        or manifest["source_closure_schema"] != SOURCE_CLOSURE_SCHEMA
        or manifest["source_commit_signature_verified"] is not True
        or manifest["source_tree_clean"] is not True
    ):
        _fail("Generic11 worker package identity or source state is invalid")
    _require_sha256(manifest["artifact_sha256"], "artifact SHA-256")
    artifact_size = manifest["artifact_size"]
    if type(artifact_size) is not int or not 1 <= artifact_size <= _MAX_ARTIFACT_BYTES:
        _fail("Generic11 worker artifact size is invalid")
    target = manifest["target"]
    if type(target) is not str or _TARGET_RE.fullmatch(target) is None:
        _fail("Generic11 worker target is invalid")
    registry = manifest["operation_registry"]
    if registry != operation_registry_manifest_v1():
        _fail("Generic11 worker operation registry is not exact")
    if not hmac.compare_digest(
        _require_sha256(
            manifest["operation_registry_sha256"], "operation registry SHA-256"
        ),
        operation_registry_sha256_v1(),
    ):
        _fail("Generic11 worker operation registry digest is invalid")
    source = _source_from_manifest(manifest)
    method = manifest["artifact_build_method"]
    command_digest = manifest["artifact_build_command_sha256"]
    environment_digest = manifest["artifact_build_environment_sha256"]
    toolchain_digest = manifest["artifact_build_toolchain_sha256"]
    raw_provenance = manifest["artifact_build_provenance"]
    build_provenance: dict[str, object] | None = None
    if method == PREBUILT_CANDIDATE_BUILD_V1:
        if (
            command_digest is not None
            or environment_digest is not None
            or toolchain_digest is not None
            or raw_provenance is not None
        ):
            _fail("prebuilt Generic11 worker candidates cannot claim build provenance")
    elif method == AUTHENTICATED_SOURCE_BUILD_V2:
        build_provenance = _validate_build_provenance_v2(
            raw_provenance, source=source, target=target
        )
        if not hmac.compare_digest(
            _require_sha256(command_digest, "build command SHA-256"),
            _build_command_sha256(target),
        ) or not hmac.compare_digest(
            _require_sha256(environment_digest, "build environment SHA-256"),
            build_provenance["environment_sha256"],
        ) or not hmac.compare_digest(
            _require_sha256(toolchain_digest, "build toolchain SHA-256"),
            build_provenance["toolchain_sha256"],
        ):
            _fail("Generic11 worker authenticated build provenance is invalid")
    else:
        _fail("Generic11 worker artifact build method is invalid")
    ping_verified = manifest["smoke_ping_verified"]
    release_ready = manifest["release_ready"]
    if type(ping_verified) is not bool or type(release_ready) is not bool:
        _fail("Generic11 worker readiness fields must be booleans")
    expected_release = _release_ready(
        source=source,
        target=target,
        build_method=method,
        build_command_sha256=command_digest,
        build_provenance=build_provenance,
        ping_verified=ping_verified,
    )
    if release_ready is not expected_release:
        _fail("Generic11 worker release-ready claim is inconsistent")
    return dict(manifest)


def build_manifest(
    *,
    artifact: StableFileV1,
    source: SourceEvidenceV1,
    target: str,
    build_method: str = PREBUILT_CANDIDATE_BUILD_V1,
    build_command_sha256: str | None = None,
    build_provenance: dict[str, object] | None = None,
    ping_verified: bool = True,
) -> dict[str, object]:
    environment_sha256 = None
    toolchain_sha256 = None
    if build_provenance is not None:
        build_provenance = _validate_build_provenance_v2(
            build_provenance,
            source=source,
            target=target,
        )
        environment_sha256 = build_provenance["environment_sha256"]
        toolchain_sha256 = build_provenance["toolchain_sha256"]
    manifest: dict[str, object] = {
        "artifact_build_command_sha256": build_command_sha256,
        "artifact_build_environment_sha256": environment_sha256,
        "artifact_build_method": build_method,
        "artifact_build_provenance": build_provenance,
        "artifact_build_toolchain_sha256": toolchain_sha256,
        "artifact_file": ARTIFACT_FILE,
        "artifact_sha256": artifact.sha256,
        "artifact_size": artifact.size,
        "cargo_lock_sha256": source.cargo_lock_sha256,
        "operation_registry": operation_registry_manifest_v1(),
        "operation_registry_sha256": operation_registry_sha256_v1(),
        "process_hardening_contract": PROCESS_HARDENING_CONTRACT,
        "protocol_magic": PROTOCOL_MAGIC,
        "protocol_version": PROTOCOL_VERSION,
        "release_ready": _release_ready(
            source=source,
            target=target,
            build_method=build_method,
            build_command_sha256=build_command_sha256,
            build_provenance=build_provenance,
            ping_verified=ping_verified,
        ),
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "smoke_ping_verified": ping_verified,
        "source_allowed_signers_sha256": source.allowed_signers_sha256,
        "source_closure_schema": SOURCE_CLOSURE_SCHEMA,
        "source_closure_sha256": source.source_closure_sha256,
        "source_commit": source.commit,
        "source_commit_signature_verified": True,
        "source_date_epoch": source.source_date_epoch,
        "source_revocation_sha256": source.revocation_sha256,
        "source_tree_clean": True,
        "target": target,
        "worker_role": WORKER_ROLE,
        "workspace_source_manifest_sha256": source.workspace_source_manifest_sha256,
    }
    return validate_manifest(manifest)


def canonical_manifest_bytes(value: object) -> bytes:
    return _canonical_json_bytes(validate_manifest(value))


def load_manifest(path: Path) -> dict[str, object]:
    payload = _stable_bytes(
        path,
        label="Generic11 worker package manifest",
        maximum=_MAX_MANIFEST_BYTES,
    )
    try:
        value = json.loads(payload, object_pairs_hook=_reject_duplicate_pairs)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise PrivacyWalletWorkerPackageError(
            "Generic11 worker package manifest is not JSON"
        ) from error
    manifest = validate_manifest(value)
    if payload != canonical_manifest_bytes(manifest):
        _fail("Generic11 worker package manifest is not canonical JSON")
    return manifest


def _copy_artifact(source: Path, destination: Path, expected: StableFileV1) -> None:
    source_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    destination_flags = (
        os.O_CREAT
        | os.O_EXCL
        | os.O_WRONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    source_fd = os.open(source, source_flags)
    destination_fd = os.open(destination, destination_flags, 0o500)
    digest = hashlib.sha256()
    size = 0
    try:
        opened = os.fstat(source_fd)
        if (opened.st_dev, opened.st_ino, opened.st_size) != (
            expected.device,
            expected.inode,
            expected.size,
        ):
            _fail("Generic11 worker artifact changed before packaging")
        while True:
            chunk = os.read(source_fd, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
            size += len(chunk)
            offset = 0
            while offset < len(chunk):
                offset += os.write(destination_fd, chunk[offset:])
        os.fchmod(destination_fd, 0o500)
        os.fsync(destination_fd)
        closed = os.fstat(source_fd)
    finally:
        os.close(destination_fd)
        os.close(source_fd)
    if (
        (closed.st_dev, closed.st_ino, closed.st_size, closed.st_mtime_ns)
        != (expected.device, expected.inode, expected.size, expected.modified_ns)
        or size != expected.size
        or not hmac.compare_digest(digest.hexdigest(), expected.sha256)
    ):
        _fail("Generic11 worker artifact changed while it was packaged")


def _fsync_directory(path: Path, label: str) -> None:
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(path, flags)
    try:
        if not stat.S_ISDIR(os.fstat(descriptor).st_mode):
            _fail(f"{label} is not a directory")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def write_package(
    *,
    artifact_path: Path,
    manifest: dict[str, object],
    output_root: Path,
) -> Path:
    manifest = validate_manifest(manifest)
    output_root = _canonical_absolute_path(output_root, "output_root")
    root_metadata = output_root.lstat()
    if (
        not stat.S_ISDIR(root_metadata.st_mode)
        or stat.S_ISLNK(root_metadata.st_mode)
        or root_metadata.st_uid != os.geteuid()
        or root_metadata.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or root_metadata.st_mode & 0o700 != 0o700
    ):
        _fail("output_root must be one owner-controlled real existing directory")
    package = output_root / str(manifest["artifact_sha256"])
    if package.exists() or package.is_symlink():
        _fail("Generic11 worker package output must be fresh")
    temporary = output_root / f".generic11-worker-{os.getpid()}-{secrets.token_hex(8)}"
    temporary.mkdir(mode=0o700)
    temporary.chmod(0o700)
    try:
        artifact = _stable_file(
            artifact_path,
            label="Generic11 worker artifact",
            maximum=_MAX_ARTIFACT_BYTES,
            require_executable=True,
            require_owner=True,
        )
        if (
            artifact.sha256 != manifest["artifact_sha256"]
            or artifact.size != manifest["artifact_size"]
        ):
            _fail("Generic11 worker artifact does not match its package manifest")
        _copy_artifact(artifact_path, temporary / ARTIFACT_FILE, artifact)
        manifest_path = temporary / "manifest.json"
        descriptor = os.open(
            manifest_path,
            os.O_CREAT
            | os.O_EXCL
            | os.O_WRONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            0o400,
        )
        try:
            payload = canonical_manifest_bytes(manifest)
            offset = 0
            while offset < len(payload):
                offset += os.write(descriptor, payload[offset:])
            os.fchmod(descriptor, 0o400)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        temporary.chmod(0o500)
        _fsync_directory(temporary, "temporary Generic11 worker package")
        temporary.rename(package)
        _fsync_directory(output_root, "Generic11 worker package output root")
    except BaseException:
        if temporary.exists():
            temporary.chmod(0o700)
            shutil.rmtree(temporary)
        raise
    return package


def verify_package(
    package: Path,
    *,
    require_release_ready: bool = False,
) -> dict[str, object]:
    package = _canonical_absolute_path(package, "package")
    metadata = package.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) != 0o500
    ):
        _fail("Generic11 worker package directory must be owner-controlled mode 0500")
    if {entry.name for entry in package.iterdir()} != {ARTIFACT_FILE, "manifest.json"}:
        _fail("Generic11 worker package file inventory is not exact")
    manifest_identity = _stable_file(
        package / "manifest.json",
        label="Generic11 worker package manifest",
        maximum=_MAX_MANIFEST_BYTES,
        require_owner=True,
    )
    if stat.S_IMODE(manifest_identity.mode) != 0o400:
        _fail("Generic11 worker package manifest must have mode 0400")
    manifest = load_manifest(package / "manifest.json")
    if package.name != manifest["artifact_sha256"]:
        _fail("Generic11 worker package directory is not content-addressed")
    artifact = _stable_file(
        package / ARTIFACT_FILE,
        label="packaged Generic11 worker artifact",
        maximum=_MAX_ARTIFACT_BYTES,
        require_executable=True,
        require_owner=True,
    )
    if stat.S_IMODE(artifact.mode) != 0o500:
        _fail("packaged Generic11 worker artifact must have mode 0500")
    if (
        artifact.sha256 != manifest["artifact_sha256"]
        or artifact.size != manifest["artifact_size"]
    ):
        _fail("packaged Generic11 worker artifact does not match its manifest")
    probe_worker_ping(package / ARTIFACT_FILE)
    if require_release_ready and manifest["release_ready"] is not True:
        _fail("Generic11 worker package is a non-release candidate")
    return manifest


def _collect_from_args(args: argparse.Namespace) -> SourceEvidenceV1:
    return collect_source_evidence(
        args.source_root,
        allowed_signers=args.allowed_signers,
        expected_allowed_signers_sha256=args.allowed_signers_sha256,
        revocation=args.revocation,
        expected_revocation_sha256=args.revocation_sha256,
    )


def _create_package(
    args: argparse.Namespace,
    artifact_path: Path,
    source: SourceEvidenceV1,
    *,
    build_method: str,
    build_command_sha256: str | None,
    build_provenance: dict[str, object] | None,
) -> Path:
    source_root = args.source_root.resolve(strict=True)
    output_root = args.output_root.resolve(strict=True)
    try:
        output_root.relative_to(source_root)
    except ValueError:
        pass
    else:
        _fail("Generic11 worker package output must be outside the source tree")
    artifact_path = artifact_path.resolve(strict=True)
    artifact = _stable_file(
        artifact_path,
        label="Generic11 worker artifact",
        maximum=_MAX_ARTIFACT_BYTES,
        require_executable=True,
        require_owner=True,
    )
    probe_worker_ping(artifact_path)
    repeated = _stable_file(
        artifact_path,
        label="Generic11 worker artifact",
        maximum=_MAX_ARTIFACT_BYTES,
        require_executable=True,
        require_owner=True,
    )
    if repeated != artifact:
        _fail("Generic11 worker artifact changed before packaging")
    manifest = build_manifest(
        artifact=artifact,
        source=source,
        target=args.target,
        build_method=build_method,
        build_command_sha256=build_command_sha256,
        build_provenance=build_provenance,
        ping_verified=True,
    )
    if args.require_release_ready and manifest["release_ready"] is not True:
        _fail("Generic11 worker release package requires the authenticated build corridor")
    package = write_package(
        artifact_path=artifact_path,
        manifest=manifest,
        output_root=output_root,
    )
    verify_package(package, require_release_ready=args.require_release_ready)
    return package


def _stable_build_input_record(
    path: Path,
    *,
    label: str,
    require_executable: bool = False,
) -> dict[str, object]:
    """Hash one root- or process-owner-controlled tool without rejecting hardlinks."""

    path = path.resolve(strict=True)
    try:
        before = path.lstat()
    except OSError as error:
        raise PrivacyWalletWorkerPackageError(f"{label} is unavailable") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink < 1
        or not 1 <= before.st_size <= _MAX_TOOL_FILE_BYTES
        or before.st_uid not in (0, os.geteuid())
        or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
        or (require_executable and not before.st_mode & stat.S_IXUSR)
    ):
        _fail(f"{label} is not an admissible build input")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise PrivacyWalletWorkerPackageError(f"{label} could not be opened safely") from error
    digest = hashlib.sha256()
    size = 0
    try:
        opened = os.fstat(descriptor)
        expected = (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_uid,
            before.st_nlink,
            before.st_size,
            before.st_mtime_ns,
        )
        if (
            opened.st_dev,
            opened.st_ino,
            opened.st_mode,
            opened.st_uid,
            opened.st_nlink,
            opened.st_size,
            opened.st_mtime_ns,
        ) != expected:
            _fail(f"{label} changed before it was opened")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            size += len(chunk)
            if size > _MAX_TOOL_FILE_BYTES:
                _fail(f"{label} exceeds its size bound")
            digest.update(chunk)
        after = os.fstat(descriptor)
        if (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_uid,
            after.st_nlink,
            after.st_size,
            after.st_mtime_ns,
        ) != expected:
            _fail(f"{label} changed while it was read")
    finally:
        os.close(descriptor)
    if size != before.st_size:
        _fail(f"{label} changed while it was read")
    return _validate_build_file_record(
        {
            "mode": stat.S_IMODE(before.st_mode),
            "owner": before.st_uid,
            "path": os.fspath(path),
            "sha256": digest.hexdigest(),
            "size": size,
        },
        label,
    )


def _resolve_build_executable(
    name: str,
    environment: Mapping[str, str],
    *,
    label: str,
) -> Path:
    path_value = environment.get("PATH")
    if not path_value:
        _fail(f"{label} cannot be resolved without PATH")
    try:
        located = shutil.which(name, path=path_value)
    except (OSError, ValueError) as error:
        raise PrivacyWalletWorkerPackageError(f"{label} could not be resolved") from error
    if located is None:
        _fail(f"{label} is unavailable on the frozen PATH")
    try:
        resolved = Path(located).resolve(strict=True)
    except OSError as error:
        raise PrivacyWalletWorkerPackageError(f"{label} could not be resolved") from error
    _stable_build_input_record(resolved, label=label, require_executable=True)
    return resolved


def _locate_build_executable(
    name: str,
    environment: Mapping[str, str],
    *,
    label: str,
) -> Path:
    """Return the PATH spelling, preserving dispatch-sensitive symlink argv[0]."""

    path_value = environment.get("PATH")
    located = shutil.which(name, path=path_value) if path_value else None
    if located is None:
        _fail(f"{label} is unavailable on the frozen PATH")
    invocation = Path(located)
    if not invocation.is_absolute():
        _fail(f"{label} did not resolve to an absolute invocation path")
    _stable_build_input_record(
        invocation.resolve(strict=True),
        label=label,
        require_executable=True,
    )
    return invocation


def _run_build_tool(
    executable: Path,
    arguments: Sequence[str],
    *,
    source_root: Path,
    environment: Mapping[str, str],
    label: str,
) -> bytes:
    try:
        completed = subprocess.run(
            [os.fspath(executable), *arguments],
            cwd=source_root,
            env=dict(environment),
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise PrivacyWalletWorkerPackageError(f"{label} probe failed") from error
    if (
        completed.returncode != 0
        or not 1 <= len(completed.stdout) <= _MAX_TOOL_OUTPUT_BYTES
        or len(completed.stderr) > _MAX_TOOL_OUTPUT_BYTES
    ):
        _fail(f"{label} probe failed")
    return completed.stdout


def _rust_component_closure_record(
    sysroot: Path,
    manifest_name: str,
    *,
    label: str,
) -> dict[str, object]:
    manifest = sysroot / "lib" / "rustlib" / manifest_name
    manifest_payload = _stable_bytes(
        manifest,
        label=f"{label} manifest",
        maximum=4 * 1_024 * 1_024,
    )
    try:
        text = manifest_payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise PrivacyWalletWorkerPackageError(f"{label} manifest is not UTF-8") from error
    if not text.endswith("\n") or "\r" in text:
        _fail(f"{label} manifest is not canonical LF text")
    relative_paths: list[PurePosixPath] = []
    for line in text.splitlines():
        if not line.startswith("file:"):
            _fail(f"{label} manifest contains an unsupported entry")
        raw_path = line[5:]
        relative = PurePosixPath(raw_path)
        if (
            not raw_path
            or raw_path.startswith("/")
            or relative.as_posix() != raw_path
            or any(part in ("", ".", "..") for part in relative.parts)
        ):
            _fail(f"{label} manifest contains a non-canonical path")
        relative_paths.append(relative)
    if len(relative_paths) != len(set(relative_paths)) or not relative_paths:
        _fail(f"{label} manifest inventory is invalid")
    digest = hashlib.sha256()
    digest.update(_RUST_COMPONENT_CLOSURE_DOMAIN)
    encoded_name = manifest_name.encode("utf-8")
    digest.update(len(encoded_name).to_bytes(2, "big"))
    digest.update(encoded_name)
    digest.update(len(manifest_payload).to_bytes(8, "big"))
    digest.update(manifest_payload)
    total_bytes = len(manifest_payload)
    for relative in relative_paths:
        candidate = sysroot.joinpath(*relative.parts)
        try:
            resolved = candidate.resolve(strict=True)
            resolved.relative_to(sysroot)
        except (OSError, ValueError) as error:
            raise PrivacyWalletWorkerPackageError(
                f"{label} file escapes the Rust sysroot"
            ) from error
        if resolved != candidate:
            _fail(f"{label} contains a symlinked component")
        record = _stable_build_input_record(resolved, label=f"{label} file")
        encoded_path = relative.as_posix().encode("utf-8")
        digest.update(len(encoded_path).to_bytes(2, "big"))
        digest.update(encoded_path)
        digest.update(int(record["size"]).to_bytes(8, "big"))
        digest.update(bytes.fromhex(str(record["sha256"])))
        total_bytes += int(record["size"])
    return _validate_component_record(
        {
            "closure_sha256": digest.hexdigest(),
            "file_count": len(relative_paths),
            "manifest_path": os.fspath(manifest),
            "manifest_sha256": hashlib.sha256(manifest_payload).hexdigest(),
            "total_bytes": total_bytes,
        },
        label,
    )


def _frozen_build_environment(source: SourceEvidenceV1) -> dict[str, str]:
    environment: dict[str, str] = {}
    for name in _INHERITED_BUILD_ENVIRONMENT_NAMES:
        value = os.environ.get(name)
        if value is not None:
            environment[name] = value
    if not environment.get("HOME") or not environment.get("PATH"):
        _fail("frozen Generic11 worker build requires explicit HOME and PATH")
    environment.update(_build_environment_values(source))
    return _canonical_build_environment(environment)


def _path_with_build_tools_first(original: str, *directories: Path) -> str:
    parts = original.split(os.pathsep)
    if any(not part or not Path(part).is_absolute() for part in parts):
        _fail("frozen Generic11 worker PATH must contain only absolute entries")
    prefixed: list[str] = []
    for item in (*map(os.fspath, directories), *parts):
        if item not in prefixed:
            prefixed.append(item)
    return os.pathsep.join(prefixed)


def _target_compiler_names(host: str, target: str) -> tuple[tuple[str, ...], tuple[str, ...]]:
    if host == target:
        return ("cc", "clang", "gcc"), ("ar", "llvm-ar")
    return (
        (f"{target}-gcc", f"{target}-cc"),
        (f"{target}-ar", "llvm-ar"),
    )


def _first_resolved_build_executable(
    names: Sequence[str],
    environment: Mapping[str, str],
    *,
    label: str,
) -> Path:
    for name in names:
        if shutil.which(name, path=environment.get("PATH")) is not None:
            return _resolve_build_executable(name, environment, label=label)
    _fail(f"{label} is unavailable on the frozen PATH")


def _cargo_configuration_records(
    source_root: Path,
    environment: Mapping[str, str],
) -> list[dict[str, object]]:
    cargo_home = Path(
        environment.get("CARGO_HOME", os.fspath(Path(environment["HOME"]) / ".cargo"))
    )
    if not cargo_home.is_absolute():
        _fail("CARGO_HOME must be absolute when it is inherited")
    candidates: set[Path] = set()
    for directory in (source_root, *source_root.parents, cargo_home):
        cargo_directory = directory if directory == cargo_home else directory / ".cargo"
        for name in ("config", "config.toml"):
            path = cargo_directory / name
            if path.exists() or path.is_symlink():
                if path.is_symlink():
                    _fail("Cargo configuration cannot be a symlink")
                candidates.add(path.resolve(strict=True))
    records = [
        _stable_build_input_record(path, label="Cargo configuration")
        for path in candidates
    ]
    return sorted(records, key=lambda record: str(record["path"]))


def _prepare_authenticated_build_corridor_v2(
    source_root: Path,
    source: SourceEvidenceV1,
    target: str,
    base_environment: Mapping[str, str],
) -> AuthenticatedBuildCorridorV2:
    """Resolve and bind every ambient input retained by the build process."""

    source_root = source_root.resolve(strict=True)
    base = _canonical_build_environment(dict(base_environment))
    rustc_dispatcher = _locate_build_executable(
        "rustc", base, label="rustc dispatcher"
    )
    sysroot_output = _run_build_tool(
        rustc_dispatcher,
        ("--print", "sysroot"),
        source_root=source_root,
        environment=base,
        label="rustc sysroot",
    )
    try:
        sysroot_text = sysroot_output.decode("utf-8").strip()
        sysroot = Path(sysroot_text).resolve(strict=True)
    except (UnicodeError, OSError) as error:
        raise PrivacyWalletWorkerPackageError("Rust sysroot is invalid") from error
    if (
        not sysroot_text
        or not Path(sysroot_text).is_absolute()
        or os.fspath(sysroot) != sysroot_text
    ):
        _fail("Rust sysroot must be one canonical absolute path")
    cargo = (sysroot / "bin" / "cargo").resolve(strict=True)
    rustc = (sysroot / "bin" / "rustc").resolve(strict=True)
    _stable_build_input_record(cargo, label="Cargo", require_executable=True)
    _stable_build_input_record(rustc, label="rustc", require_executable=True)
    cargo_iroha_fast = _resolve_build_executable(
        "cargo-iroha-fast", base, label="cargo-iroha-fast"
    )
    effective = dict(base)
    effective["PATH"] = _path_with_build_tools_first(
        effective["PATH"], cargo.parent, cargo_iroha_fast.parent
    )
    effective["RUSTC"] = os.fspath(rustc)
    rustc_wrapper = _resolve_build_executable(
        "sccache", effective, label="sccache rustc wrapper"
    )
    effective["RUSTC_WRAPPER"] = os.fspath(rustc_wrapper)
    rustc_version = _run_build_tool(
        rustc,
        ("-vV",),
        source_root=source_root,
        environment=effective,
        label="rustc version",
    )
    try:
        rustc_version_text = rustc_version.decode("utf-8")
    except UnicodeDecodeError as error:
        raise PrivacyWalletWorkerPackageError("rustc version is not UTF-8") from error
    host_matches = re.findall(r"^host: ([a-z0-9][a-z0-9._+-]{0,127})$", rustc_version_text, re.M)
    if len(host_matches) != 1:
        _fail("rustc host identity is invalid")
    host = host_matches[0]
    compiler_names, archiver_names = _target_compiler_names(host, target)
    linker_driver = _first_resolved_build_executable(
        compiler_names, effective, label="target linker driver"
    )
    archiver = _first_resolved_build_executable(
        archiver_names, effective, label="target archiver"
    )
    linker_output = _run_build_tool(
        linker_driver,
        ("-print-prog-name=ld",),
        source_root=source_root,
        environment=effective,
        label="target linker resolution",
    )
    try:
        linker_name = linker_output.decode("utf-8").strip()
    except UnicodeDecodeError as error:
        raise PrivacyWalletWorkerPackageError("target linker path is not UTF-8") from error
    if not linker_name or "\0" in linker_name or "\n" in linker_name:
        _fail("target linker path is invalid")
    if Path(linker_name).is_absolute():
        linker = Path(linker_name).resolve(strict=True)
    else:
        linker = _resolve_build_executable(
            linker_name, effective, label="target linker"
        )
    suffix = target.upper().replace("-", "_").replace(".", "_")
    cc_suffix = target.replace("-", "_").replace(".", "_")
    effective.update(
        {
            "AR": os.fspath(archiver),
            f"AR_{cc_suffix}": os.fspath(archiver),
            "CC": os.fspath(linker_driver),
            f"CC_{cc_suffix}": os.fspath(linker_driver),
            f"CARGO_TARGET_{suffix}_LINKER": os.fspath(linker_driver),
        }
    )
    effective = _canonical_build_environment(
        effective, source=source, target=target
    )
    tool_names = {
        "archiver": archiver,
        "cargo": cargo,
        "cargo_iroha_fast": cargo_iroha_fast,
        "dirname": _resolve_build_executable("dirname", effective, label="dirname"),
        "env": _resolve_build_executable("env", effective, label="env"),
        "git": _resolve_build_executable("git", effective, label="Git"),
        "grep": _resolve_build_executable("grep", effective, label="grep"),
        "linker": linker,
        "linker_driver": linker_driver,
        "rustc": rustc,
        "rustc_wrapper": rustc_wrapper,
        "shell": _resolve_build_executable("bash", effective, label="Bash"),
        "uname": _resolve_build_executable("uname", effective, label="uname"),
    }
    tools = {
        role: _stable_build_input_record(
            path,
            label=f"build tool {role}",
            require_executable=True,
        )
        for role, path in tool_names.items()
    }
    cargo_version = _run_build_tool(
        cargo,
        ("--version", "--verbose"),
        source_root=source_root,
        environment=effective,
        label="Cargo version",
    )
    components = {
        "cargo": _rust_component_closure_record(
            sysroot,
            f"manifest-cargo-{host}",
            label="Cargo Rust component",
        ),
        "rust_std": _rust_component_closure_record(
            sysroot,
            f"manifest-rust-std-{target}",
            label="target Rust standard library component",
        ),
        "rustc": _rust_component_closure_record(
            sysroot,
            f"manifest-rustc-{host}",
            label="rustc component",
        ),
    }
    toolchain: dict[str, object] = {
        "cargo_configuration": _cargo_configuration_records(source_root, effective),
        "cargo_version_sha256": hashlib.sha256(cargo_version).hexdigest(),
        "components": components,
        "host": host,
        "rustc_version_sha256": hashlib.sha256(rustc_version).hexdigest(),
        "schema": _BUILD_TOOLCHAIN_SCHEMA,
        "sysroot": os.fspath(sysroot),
        "target": target,
        "tools": tools,
    }
    provenance = _build_provenance_v2(
        effective,
        toolchain,
        source=source,
        target=target,
    )
    return AuthenticatedBuildCorridorV2(
        cargo=cargo,
        environment=effective,
        provenance=provenance,
    )


def _cargo_target_directory(
    source_root: Path,
    cargo: Path,
    environment: dict[str, str],
) -> Path:
    try:
        completed = subprocess.run(
            [
                os.fspath(cargo),
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--frozen",
            ],
            cwd=source_root,
            env=environment,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=60,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise PrivacyWalletWorkerPackageError("Cargo target directory is unavailable") from error
    if completed.returncode != 0:
        _fail("Cargo metadata failed while locating the Generic11 worker artifact")
    try:
        target = Path(json.loads(completed.stdout)["target_directory"])
    except (KeyError, TypeError, json.JSONDecodeError) as error:
        raise PrivacyWalletWorkerPackageError("Cargo target directory is invalid") from error
    return target.resolve(strict=False)


def _build(args: argparse.Namespace) -> Path:
    source = _collect_from_args(args)
    base_environment = _frozen_build_environment(source)
    corridor = _prepare_authenticated_build_corridor_v2(
        args.source_root,
        source,
        args.target,
        base_environment,
    )
    target_directory = _cargo_target_directory(
        args.source_root,
        corridor.cargo,
        corridor.environment,
    )
    command = _cargo_build_command(args.target)
    actual_command = [os.fspath(corridor.cargo), *command[1:]]
    try:
        result = subprocess.run(
            actual_command,
            cwd=args.source_root,
            env=corridor.environment,
            check=False,
        )
    except OSError as error:
        raise PrivacyWalletWorkerPackageError("Generic11 worker Cargo build failed") from error
    if result.returncode != 0:
        _fail("Generic11 worker Cargo build failed")
    if _collect_from_args(args) != source:
        _fail("authenticated source changed while the Generic11 worker was built")
    repeated_corridor = _prepare_authenticated_build_corridor_v2(
        args.source_root,
        source,
        args.target,
        base_environment,
    )
    if repeated_corridor != corridor:
        _fail("Generic11 worker build inputs changed during compilation")
    artifact = target_directory / args.target / "release" / ARTIFACT_FILE
    return _create_package(
        args,
        artifact,
        source,
        build_method=AUTHENTICATED_SOURCE_BUILD_V2,
        build_command_sha256=_build_command_sha256(args.target),
        build_provenance=corridor.provenance,
    )


def _add_source_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--allowed-signers", type=Path, required=True)
    parser.add_argument("--allowed-signers-sha256", required=True)
    parser.add_argument("--revocation", type=Path, required=True)
    parser.add_argument("--revocation-sha256", required=True)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    package = commands.add_parser("package", help="package an existing candidate artifact")
    _add_source_arguments(package)
    package.add_argument("--artifact", type=Path, required=True)
    package.add_argument("--target", required=True)
    package.add_argument("--output-root", type=Path, required=True)
    package.add_argument("--require-release-ready", action="store_true")
    build = commands.add_parser("build", help="build and package through cargo iroha-fast")
    _add_source_arguments(build)
    build.add_argument("--target", required=True)
    build.add_argument("--output-root", type=Path, required=True)
    build.add_argument("--require-release-ready", action="store_true")
    verify = commands.add_parser("verify", help="verify one installed package")
    verify.add_argument("--package", type=Path, required=True)
    verify.add_argument("--require-release-ready", action="store_true")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "verify":
            package = args.package.resolve(strict=True)
            verify_package(package, require_release_ready=args.require_release_ready)
        elif args.command == "build":
            package = _build(args)
        else:
            source = _collect_from_args(args)
            package = _create_package(
                args,
                args.artifact,
                source,
                build_method=PREBUILT_CANDIDATE_BUILD_V1,
                build_command_sha256=None,
                build_provenance=None,
            )
        print(package)
        return 0
    except (OSError, PrivacyWalletWorkerPackageError, ValueError) as error:
        print(f"Generic11 worker package refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
