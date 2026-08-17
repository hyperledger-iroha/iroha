#!/usr/bin/env python3
"""Publish one frozen Taira candidate and close a signed OCI receipt handoff.

This installed authority command is intentionally not a generic ORAS or signing
wrapper.  It accepts one frozen candidate plus independently pinned semantic
identity, qualification, repository, tool, and trust inputs.  It runs as a
dedicated non-root authority UID in an authority-private scratch directory.
Registry credentials are read only from one preprovisioned mode-0400 config;
username, password, token, arbitrary layer, arbitrary JSON, and arbitrary sign
surfaces do not exist.

The command re-admits both the original and registry-pulled candidate, validates
the raw OCI manifests, constructs and semantically validates the sole
publication-receipt schema, signs only that controller-created payload, verifies
the pulled receipt, and atomically installs an exact seven-file terminal handoff.
Every child inherits the already-checked authority UID and a closed environment.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import selectors
import stat
import subprocess
import sys
import tempfile
import time
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any, NoReturn

try:
    from . import taira_privacy_rollout_contract as rollout_observation
    from . import taira_rollout_admission as admission
    from .release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_write_bytes,
    )
    from .release_manifest_signing import (
        ReleaseManifestSignatureError,
        sign_release_manifest,
        verify_release_manifest,
    )
except ImportError:
    import taira_privacy_rollout_contract as rollout_observation
    import taira_rollout_admission as admission
    from release_artifact_contract import (
        ReleaseArtifactError,
        canonical_json_bytes,
        exclusive_write_bytes,
    )
    from release_manifest_signing import (
        ReleaseManifestSignatureError,
        sign_release_manifest,
        verify_release_manifest,
    )


PUBLICATION_SCHEMA = "iroha.taira.publication_receipt"
PUBLICATION_SCHEMA_VERSION = 1
PRIMARY_ARTIFACT_TYPE = (
    "application/vnd.hyperledger.iroha.taira.rollout-admission.v1"
)
ADMISSION_ARCHIVE_MEDIA_TYPE = (
    "application/vnd.hyperledger.iroha.taira.rollout-admission.archive.v1+tar+gzip"
)
AUTHORITY_MANIFEST_MEDIA_TYPE = (
    "application/vnd.hyperledger.iroha.release-manifest.v1+json"
)
AUTHORITY_SIGNATURE_MEDIA_TYPE = (
    "application/vnd.hyperledger.iroha.release-manifest.signature.v1+ed25519"
)
AUTHORITY_PUBLIC_KEY_MEDIA_TYPE = (
    "application/vnd.hyperledger.iroha.ed25519-public-key.v1"
)
PUBLICATION_ARTIFACT_TYPE = (
    "application/vnd.hyperledger.iroha.taira.publication-receipt.v1"
)
PUBLICATION_RECEIPT_MEDIA_TYPE = (
    "application/vnd.hyperledger.iroha.taira.publication-receipt.v1+json"
)
OCI_MANIFEST_MEDIA_TYPE = "application/vnd.oci.image.manifest.v1+json"
OCI_EMPTY_CONFIG_MEDIA_TYPE = "application/vnd.oci.empty.v1+json"
OCI_EMPTY_CONFIG_DIGEST = (
    "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
)
OCI_EMPTY_CONFIG_DATA = "e30="
HANDOFF_MANIFEST = "handoff-inventory-v1.json"
SOURCE_IDENTITY_NAME = "taira-source-identity-v1.json"
RECEIPT_ID_NAME = "receipt-id"
PUBLICATION_RECEIPT_NAME = "publication-receipt-v1.json"
PUBLICATION_SIGNATURE_NAME = "publication-receipt-v1.json.sig"
PUBLICATION_PUBLIC_KEY_NAME = "publication-receipt-v1.json.pub"
PRIMARY_MANIFEST_NAME = "taira-primary-oci-manifest.json"
RECEIPT_MANIFEST_NAME = "taira-publication-receipt-oci-manifest.json"
PRIMARY_DIGEST_NAME = "published-primary-digest"
RECEIPT_DIGEST_NAME = "published-receipt-digest"
TERMINAL_FILES = (
    PRIMARY_DIGEST_NAME,
    PUBLICATION_PUBLIC_KEY_NAME,
    PUBLICATION_RECEIPT_NAME,
    PUBLICATION_SIGNATURE_NAME,
    RECEIPT_DIGEST_NAME,
    RECEIPT_MANIFEST_NAME,
    PRIMARY_MANIFEST_NAME,
)

SHA256_RE = re.compile(r"[0-9a-f]{64}")
OCI_DIGEST_RE = re.compile(r"sha256:[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
VERSION_RE = re.compile(r"[1-9][0-9]*\.[0-9]+\.[0-9]+")
REPOSITORY_RE = re.compile(
    r"[a-z0-9.-]+(?::[1-9][0-9]{0,4})?/[a-z0-9]+(?:[._/-][a-z0-9]+)*"
)
SUFFIX_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,47}")
MAX_SMALL_FILE_BYTES = 16 * 1024 * 1024
MAX_REGISTRY_CONFIG_BYTES = 1024 * 1024
MAX_ORAS_JSON_BYTES = 4 * 1024 * 1024
MAX_OCI_MANIFEST_BYTES = 16 * 1024 * 1024
MAX_PUBLICATION_UNIX = 253_402_300_799
CHILD_TIMEOUT_SECONDS = 15 * 60
ROOT_OWNER_UID = 0
FORBIDDEN_CREDENTIAL_ENV = frozenset(
    {
        "DOCKER_CONFIG",
        "ORAS_IDENTITY_TOKEN",
        "ORAS_PASSWORD",
        "ORAS_TOKEN",
        "ORAS_USERNAME",
        "REGISTRY_AUTH_FILE",
        "TAIRA_OCI_IDENTITY_TOKEN",
        "TAIRA_OCI_PASSWORD",
        "TAIRA_OCI_TOKEN",
        "TAIRA_OCI_USERNAME",
    }
)
FORBIDDEN_ORAS_FLAGS = frozenset(
    {
        "--cert-file",
        "--header",
        "--identity-token",
        "--identity-token-stdin",
        "--key-file",
        "--password",
        "--password-stdin",
        "--username",
    }
)


class TairaPublicationError(RuntimeError):
    """The candidate publication violated the installed authority contract."""


def _require_authenticated_rollout_observation_authority() -> None:
    """Translate the independent observation provisioning barrier."""

    try:
        rollout_observation.require_authenticated_rollout_observation_authority_provisioned()
    except rollout_observation.RolloutContractError as exc:
        raise TairaPublicationError(str(exc)) from exc


@dataclass(frozen=True)
class PublishRequest:
    """Fixed semantic and trust inputs for one publication attempt."""

    candidate_root: Path
    expected_source: admission.SourceIdentity
    expected_qualification_receipt_id: str
    repository: str
    suffix: str
    authority_uid: int
    scratch_parent: Path
    registry_config: Path
    oras_path: Path
    trusted_oras_sha256: str
    expected_oras_version: str
    external_signer_path: Path
    trusted_external_signer_sha256: str
    signing_public_key_path: Path
    trusted_signing_fingerprint: str
    release_manifest_verifier_path: Path
    trusted_release_manifest_verifier_sha256: str
    terminal_handoff: Path
    rollout_plan: Path
    rollout_result: Path
    rollout_authority_envelope: Path
    rollout_durable_receipt: Path


@dataclass(frozen=True)
class FileIdentity:
    """Stable identity for one inspected regular file."""

    device: int
    inode: int
    mode: int
    links: int
    uid: int
    gid: int
    size: int
    mtime_ns: int
    ctime_ns: int


@dataclass(frozen=True)
class CapturedFile:
    """Stable digest, size, and inode identity for one file."""

    path: Path
    sha256: str
    size: int
    identity: FileIdentity


@dataclass(frozen=True)
class Layer:
    """One exact OCI file layer."""

    path: str
    media_type: str
    sha256: str
    size: int

    def receipt_row(self) -> dict[str, object]:
        return {
            "media_type": self.media_type,
            "path": self.path,
            "sha256": self.sha256,
            "size": self.size,
        }


@dataclass(frozen=True)
class Candidate:
    """Pinned, frozen candidate inventory."""

    root: Path
    archive_relative: str
    files: Mapping[str, CapturedFile]

    @property
    def archive(self) -> Path:
        return self.root / self.archive_relative

    @property
    def authority(self) -> Path:
        return self.root / "authority"


def _fail(message: str) -> NoReturn:
    raise TairaPublicationError(message)


def _sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    return value


def _oci_digest(value: object, label: str) -> str:
    if not isinstance(value, str) or OCI_DIGEST_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase sha256 OCI digest")
    return value


def _commit(value: object, label: str) -> str:
    if not isinstance(value, str) or COMMIT_RE.fullmatch(value) is None:
        _fail(f"{label} must be one full lowercase 40-hex commit")
    return value


def _integer(
    value: object,
    label: str,
    *,
    minimum: int = 0,
    maximum: int | None = None,
) -> int:
    if (
        type(value) is not int
        or value < minimum
        or (maximum is not None and value > maximum)
    ):
        upper = "" if maximum is None else f" and <= {maximum}"
        _fail(f"{label} must be an integer >= {minimum}{upper}")
    return value


def _identity(info: os.stat_result) -> FileIdentity:
    return FileIdentity(
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _is_root_owned(info: os.stat_result) -> bool:
    return info.st_uid == ROOT_OWNER_UID


def _absolute_lexical(path: Path, label: str, *, exists: bool = True) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} must use one absolute lexical path")
    try:
        if exists:
            resolved = path.resolve(strict=True)
            if resolved != path:
                _fail(f"{label} must use one canonical non-symlink path")
        else:
            parent = path.parent.resolve(strict=True)
            if parent != path.parent:
                _fail(f"{label} parent must use one canonical non-symlink path")
    except OSError as exc:
        raise TairaPublicationError(f"cannot resolve {label}: {exc}") from exc
    return path


def _require_path_chain(
    path: Path,
    label: str,
    *,
    root_owned: bool,
) -> None:
    components = list(reversed(path.parents)) + [path]
    for component in components[1:]:
        try:
            info = component.lstat()
        except OSError as exc:
            raise TairaPublicationError(
                f"cannot inspect {label} path component: {component}"
            ) from exc
        if stat.S_ISLNK(info.st_mode):
            _fail(f"{label} path must not contain symlinks")
        if component != path and (
            not stat.S_ISDIR(info.st_mode) or info.st_mode & 0o022
        ):
            _fail(f"{label} parent path is not a protected directory")
        if root_owned and not _is_root_owned(info):
            _fail(f"{label} path must be entirely root-owned")


def _open_regular(path: Path, label: str) -> tuple[int, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as exc:
        raise TairaPublicationError(f"cannot inspect {label}: {exc}") from exc
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        _fail(f"{label} must be one non-symlink regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise TairaPublicationError(f"cannot open {label}: {exc}") from exc
    opened = os.fstat(descriptor)
    if _identity(opened) != _identity(before):
        os.close(descriptor)
        _fail(f"{label} changed while opening")
    return descriptor, opened


def _capture_file(
    path: Path,
    label: str,
    *,
    maximum: int | None = None,
) -> CapturedFile:
    descriptor, opened = _open_regular(path, label)
    digest = hashlib.sha256()
    total = 0
    try:
        while chunk := os.read(descriptor, 1024 * 1024):
            total += len(chunk)
            if maximum is not None and total > maximum:
                _fail(f"{label} exceeds its {maximum}-byte bound")
            digest.update(chunk)
        closed = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if _identity(closed) != _identity(opened) or total != opened.st_size:
        _fail(f"{label} changed while reading")
    return CapturedFile(path, digest.hexdigest(), total, _identity(opened))


def _read_captured(captured: CapturedFile, label: str) -> bytes:
    if captured.size > MAX_SMALL_FILE_BYTES:
        _fail(f"{label} exceeds the in-memory validation bound")
    descriptor, opened = _open_regular(captured.path, label)
    payload = bytearray()
    try:
        while chunk := os.read(descriptor, 64 * 1024):
            payload.extend(chunk)
            if len(payload) > captured.size:
                _fail(f"{label} grew while reading")
        closed = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    value = bytes(payload)
    if (
        _identity(opened) != captured.identity
        or _identity(closed) != captured.identity
        or len(value) != captured.size
        or hashlib.sha256(value).hexdigest() != captured.sha256
    ):
        _fail(f"{label} changed after capture")
    return value


def _assert_file_unchanged(captured: CapturedFile, label: str) -> None:
    current = _capture_file(captured.path, label)
    if current != captured:
        _fail(f"{label} changed during publication")


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"JSON contains duplicate key {key!r}")
        result[key] = value
    return result


def _reject_nonfinite(value: str) -> NoReturn:
    _fail(f"JSON contains non-finite number {value!r}")


def _strict_json(payload: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_nonfinite,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise TairaPublicationError(f"{label} is not strict UTF-8 JSON") from exc
    if not isinstance(value, dict):
        _fail(f"{label} root must be an object")
    return value


def _exact(value: object, fields: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    if set(value) != fields:
        _fail(
            f"{label} fields differ: missing={sorted(fields - set(value))}, "
            f"extra={sorted(set(value) - fields)}"
        )
    return value


def _canonical_compact(value: object) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def _validate_source(source: admission.SourceIdentity) -> admission.SourceIdentity:
    return admission.SourceIdentity(
        commit=_commit(source.commit, "expected source commit"),
        dpn_validator_release_commit=_commit(
            source.dpn_validator_release_commit,
            "expected DPN validator release commit",
        ),
        cargo_lock_sha256=_sha256(
            source.cargo_lock_sha256, "expected Cargo.lock digest"
        ),
        workspace_source_manifest_sha256=_sha256(
            source.workspace_source_manifest_sha256,
            "expected workspace source digest",
        ),
    )


def _source_identity_digest(source: admission.SourceIdentity) -> str:
    digest = hashlib.sha256()
    digest.update(b"iroha.taira.source_identity.v1\0")
    digest.update(
        json.dumps(
            source.as_dict(),
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("ascii")
    )
    return digest.hexdigest()


def _validate_repository(repository: str, suffix: str) -> tuple[str, str, str]:
    if (
        not isinstance(repository, str)
        or REPOSITORY_RE.fullmatch(repository) is None
        or ".." in repository
        or "//" in repository
    ):
        _fail("OCI repository is not one canonical lowercase repository")
    if not isinstance(suffix, str) or (
        suffix and (SUFFIX_RE.fullmatch(suffix) is None or ".." in suffix)
    ):
        _fail("OCI artifact suffix is not canonical")
    registry = repository.split("/", 1)[0]
    return repository, suffix, registry


def _scan_candidate_inventory(root: Path) -> tuple[list[str], list[str]]:
    files: list[str] = []
    directories: list[str] = []
    for current, names, filenames in os.walk(root, followlinks=False):
        names.sort()
        filenames.sort()
        current_path = Path(current)
        current_info = current_path.lstat()
        if (
            not stat.S_ISDIR(current_info.st_mode)
            or stat.S_IMODE(current_info.st_mode) != 0o555
            or not _is_root_owned(current_info)
        ):
            _fail("candidate directories must be root-owned exact mode 0555")
        for name in names:
            path = current_path / name
            info = path.lstat()
            if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
                _fail("candidate inventory contains an unsafe directory")
            directories.append(path.relative_to(root).as_posix())
        for name in filenames:
            path = current_path / name
            info = path.lstat()
            if (
                stat.S_ISLNK(info.st_mode)
                or not stat.S_ISREG(info.st_mode)
                or info.st_nlink != 1
                or not _is_root_owned(info)
                or stat.S_IMODE(info.st_mode) != 0o444
                or info.st_size <= 0
            ):
                _fail("candidate inventory contains an unsafe or writable file")
            files.append(path.relative_to(root).as_posix())
    return sorted(directories), sorted(files)


def _capture_candidate(
    root: Path,
    source: admission.SourceIdentity,
    expected_receipt_id: str,
) -> Candidate:
    root = _absolute_lexical(root, "candidate root")
    _require_path_chain(root, "candidate root", root_owned=False)
    archive_name = (
        f"taira-admission-{source.workspace_source_manifest_sha256[:16]}-"
        "macos-arm64.tar.gz"
    )
    archive_relative = f"admission/{archive_name}"
    payload_paths = sorted(
        {
            archive_relative,
            "authority/release_manifest.json",
            "authority/release_manifest.json.pub",
            "authority/release_manifest.json.sig",
            RECEIPT_ID_NAME,
            SOURCE_IDENTITY_NAME,
        }
    )
    directories, files = _scan_candidate_inventory(root)
    if directories != ["admission", "authority"] or files != sorted(
        [HANDOFF_MANIFEST, *payload_paths]
    ):
        _fail("candidate handoff inventory is not exactly closed")
    captured = {
        relative: _capture_file(
            root / relative,
            f"candidate file {relative}",
            maximum=(
                admission.MAX_FINAL_ARCHIVE_LOGICAL_BYTES
                if relative == archive_relative
                else MAX_SMALL_FILE_BYTES
            ),
        )
        for relative in files
    }

    manifest_payload = _read_captured(
        captured[HANDOFF_MANIFEST], "candidate handoff manifest"
    )
    manifest = _strict_json(manifest_payload, "candidate handoff manifest")
    if manifest_payload != _canonical_compact(manifest):
        _fail("candidate handoff manifest is not canonical compact JSON")
    _exact(
        manifest,
        {"files", "kind", "schema", "schema_version"},
        "candidate handoff manifest",
    )
    if (
        manifest["schema"] != "iroha.taira.release_handoff"
        or manifest["schema_version"] != 1
        or manifest["kind"] != "candidate"
    ):
        _fail("candidate handoff identity differs")
    rows = manifest["files"]
    if not isinstance(rows, list) or len(rows) != len(payload_paths):
        _fail("candidate handoff rows differ")
    expected_rows = []
    for relative in payload_paths:
        row = captured[relative]
        expected_rows.append(
            {"path": relative, "sha256": row.sha256, "size": row.size}
        )
    if rows != expected_rows:
        _fail("candidate handoff rows do not bind the exact frozen inventory")

    identity_payload = _read_captured(
        captured[SOURCE_IDENTITY_NAME], "candidate source identity"
    )
    identity = _strict_json(identity_payload, "candidate source identity")
    if identity_payload != _canonical_compact(identity):
        _fail("candidate source identity is not canonical compact JSON")
    _exact(identity, {"source", "source_date_epoch"}, "candidate source identity")
    if identity["source"] != source.as_dict():
        _fail("candidate source identity differs from all four expected fields")
    _integer(identity["source_date_epoch"], "candidate source epoch", minimum=1)
    receipt_payload = _read_captured(
        captured[RECEIPT_ID_NAME], "candidate qualification receipt ID"
    )
    if receipt_payload != (expected_receipt_id + "\n").encode("ascii"):
        _fail("candidate qualification receipt differs from the expected receipt")
    return Candidate(root, archive_relative, captured)


def _assert_candidate_unchanged(candidate: Candidate) -> None:
    directories, files = _scan_candidate_inventory(candidate.root)
    if directories != ["admission", "authority"] or files != sorted(
        candidate.files
    ):
        _fail("candidate inventory changed during publication")
    for relative, captured in candidate.files.items():
        _assert_file_unchanged(captured, f"candidate file {relative}")


def _capture_pinned_executable(
    path: Path,
    expected_sha256: str,
    label: str,
) -> CapturedFile:
    path = _absolute_lexical(path, label)
    _require_path_chain(path, label, root_owned=True)
    info = path.lstat()
    if (
        not stat.S_ISREG(info.st_mode)
        or info.st_nlink != 1
        or not _is_root_owned(info)
        or stat.S_IMODE(info.st_mode) != 0o555
        or info.st_mode & (stat.S_ISUID | stat.S_ISGID)
    ):
        _fail(f"{label} must be one root-owned non-writable mode-0555 executable")
    captured = _capture_file(path, label)
    if captured.sha256 != _sha256(expected_sha256, f"trusted {label} digest"):
        _fail(f"{label} differs from its independently trusted SHA-256")
    return captured


def _capture_public_key(
    path: Path,
    expected_fingerprint: str,
) -> CapturedFile:
    path = _absolute_lexical(path, "signing public key")
    _require_path_chain(path, "signing public key", root_owned=True)
    info = path.lstat()
    if (
        not stat.S_ISREG(info.st_mode)
        or info.st_nlink != 1
        or not _is_root_owned(info)
        or stat.S_IMODE(info.st_mode) != 0o444
        or info.st_size != 32
    ):
        _fail("signing public key must be root-owned mode 0444 and exactly 32 bytes")
    captured = _capture_file(path, "signing public key", maximum=32)
    if captured.sha256 != _sha256(expected_fingerprint, "signing fingerprint"):
        _fail("signing public key differs from its trusted fingerprint")
    return captured


def _capture_registry_config(path: Path, authority_uid: int) -> CapturedFile:
    path = _absolute_lexical(path, "registry config")
    _require_path_chain(path, "registry config", root_owned=False)
    info = path.lstat()
    parent = path.parent.lstat()
    if (
        not stat.S_ISDIR(parent.st_mode)
        or parent.st_uid != authority_uid
        or stat.S_IMODE(parent.st_mode) != 0o700
        or not stat.S_ISREG(info.st_mode)
        or info.st_nlink != 1
        or info.st_uid != authority_uid
        or stat.S_IMODE(info.st_mode) != 0o400
        or info.st_size <= 0
    ):
        _fail("registry config must be preprovisioned authority-only mode 0400")
    return _capture_file(
        path, "registry config", maximum=MAX_REGISTRY_CONFIG_BYTES
    )


def _require_authority(authority_uid: int) -> None:
    if authority_uid <= 0 or os.geteuid() != authority_uid:
        _fail("publisher must run entirely as the dedicated non-root authority UID")


def _closed_child_environment(scratch: Path) -> dict[str, str]:
    return {
        "HOME": str(scratch),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.defpath,
        "TMPDIR": str(scratch),
        "XDG_CACHE_HOME": str(scratch / "cache"),
        "XDG_CONFIG_HOME": str(scratch / "config-home"),
    }


def _stop_child(process: subprocess.Popen[bytes]) -> None:
    try:
        process.terminate()
        process.wait(timeout=2)
    except (OSError, subprocess.TimeoutExpired):
        try:
            process.kill()
            process.wait(timeout=2)
        except (OSError, subprocess.TimeoutExpired):
            pass


def _run_child(
    argv: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    output_limit: int,
    timeout: int = CHILD_TIMEOUT_SECONDS,
) -> bytes:
    """Run one fixed child and capture bounded stdout without inherited input."""

    try:
        process = subprocess.Popen(
            list(argv),
            cwd=cwd,
            env=dict(environment),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            close_fds=True,
        )
    except OSError as exc:
        raise TairaPublicationError(f"cannot execute pinned child: {exc}") from exc
    assert process.stdout is not None
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    deadline = time.monotonic() + timeout
    payload = bytearray()
    try:
        while selector.get_map():
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                _stop_child(process)
                _fail("pinned child timed out")
            events = selector.select(remaining)
            if not events:
                _stop_child(process)
                _fail("pinned child timed out")
            for key, _mask in events:
                chunk = os.read(key.fd, 64 * 1024)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                payload.extend(chunk)
                if len(payload) > output_limit:
                    _stop_child(process)
                    _fail("pinned child output exceeds its fixed bound")
        remaining = deadline - time.monotonic()
        try:
            status = process.wait(timeout=max(remaining, 0.001))
        except subprocess.TimeoutExpired:
            _stop_child(process)
            _fail("pinned child timed out")
    finally:
        selector.close()
        process.stdout.close()
    if status != 0:
        _fail(f"pinned child failed with status {status}")
    return bytes(payload)


def _run_oras(
    oras: CapturedFile,
    arguments: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    authority_uid: int,
    output_limit: int = MAX_ORAS_JSON_BYTES,
) -> bytes:
    for argument in arguments:
        if argument.split("=", 1)[0] in FORBIDDEN_ORAS_FLAGS:
            _fail("ORAS credential/header/key CLI surfaces are forbidden")
    _require_authority(authority_uid)
    _assert_file_unchanged(oras, "pinned ORAS executable")
    payload = _run_child(
        [str(oras.path), *arguments],
        cwd=cwd,
        environment=environment,
        output_limit=output_limit,
    )
    _require_authority(authority_uid)
    _assert_file_unchanged(oras, "pinned ORAS executable")
    return payload


def _validate_oras_version(payload: bytes, expected: str) -> None:
    if VERSION_RE.fullmatch(expected) is None:
        _fail("expected ORAS version is not canonical semantic version")
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise TairaPublicationError("ORAS version output is not UTF-8") from exc
    if not text.endswith("\n") or "\x00" in text or "\r" in text:
        _fail("ORAS version output is truncated or noncanonical")
    rows = [line for line in text.splitlines() if line]
    version_rows = [line for line in rows if line.startswith("Version:")]
    if len(version_rows) != 1 or version_rows[0].split(":", 1)[1].strip() != expected:
        _fail("ORAS version output differs from the pinned version")
    keys = [line.split(":", 1)[0] for line in rows if ":" in line]
    if len(keys) != len(rows) or len(keys) != len(set(keys)):
        _fail("ORAS version output contains duplicate or noncanonical rows")


def _oras_result(
    payload: bytes,
    *,
    repository: str,
    tagged_reference: str | None,
    artifact_type: str,
    created: str,
) -> tuple[str, int]:
    value = _strict_json(payload, "ORAS publication result")
    fields = {
        "annotations",
        "artifactType",
        "digest",
        "mediaType",
        "reference",
        "size",
    }
    if tagged_reference is not None:
        fields.add("referenceByTags")
    _exact(value, fields, "ORAS publication result")
    digest = _oci_digest(value["digest"], "ORAS publication result digest")
    size = _integer(value["size"], "ORAS publication result size", minimum=1)
    if (
        value["mediaType"] != OCI_MANIFEST_MEDIA_TYPE
        or value["artifactType"] != artifact_type
        or value["annotations"] != {"org.opencontainers.image.created": created}
        or value["reference"] != f"{repository}@{digest}"
        or (
            tagged_reference is not None
            and value["referenceByTags"] != [tagged_reference]
        )
    ):
        _fail("ORAS publication result differs from the fixed request")
    return digest, size


def _manifest_layers(
    value: object,
    expected: Sequence[Layer],
    label: str,
) -> None:
    if not isinstance(value, list) or len(value) != len(expected):
        _fail(f"{label} must contain exactly {len(expected)} layers")
    for index, (row, layer) in enumerate(zip(value, expected)):
        descriptor = _exact(
            row,
            {"annotations", "digest", "mediaType", "size"},
            f"{label} layer {index}",
        )
        if descriptor != {
            "annotations": {"org.opencontainers.image.title": layer.path},
            "digest": f"sha256:{layer.sha256}",
            "mediaType": layer.media_type,
            "size": layer.size,
        }:
            _fail(f"{label} layer {index} differs from the exact local bytes")


def _validate_raw_manifest(
    payload: bytes,
    *,
    digest: str,
    expected_size: int,
    artifact_type: str,
    layers: Sequence[Layer],
    created: str,
    subject: tuple[str, int] | None,
    label: str,
) -> dict[str, Any]:
    if len(payload) > MAX_OCI_MANIFEST_BYTES:
        _fail(f"{label} exceeds its fixed size bound")
    if f"sha256:{hashlib.sha256(payload).hexdigest()}" != digest:
        _fail(f"{label} bytes differ from the immutable digest")
    if len(payload) != expected_size:
        _fail(f"{label} size differs from the ORAS descriptor")
    manifest = _strict_json(payload, label)
    fields = {
        "annotations",
        "artifactType",
        "config",
        "layers",
        "mediaType",
        "schemaVersion",
    }
    if subject is not None:
        fields.add("subject")
    _exact(manifest, fields, label)
    if (
        manifest["schemaVersion"] != 2
        or manifest["mediaType"] != OCI_MANIFEST_MEDIA_TYPE
        or manifest["artifactType"] != artifact_type
        or manifest["annotations"]
        != {"org.opencontainers.image.created": created}
    ):
        _fail(f"{label} identity differs")
    config = _exact(
        manifest["config"],
        {"data", "digest", "mediaType", "size"},
        f"{label} config",
    )
    if config != {
        "data": OCI_EMPTY_CONFIG_DATA,
        "digest": OCI_EMPTY_CONFIG_DIGEST,
        "mediaType": OCI_EMPTY_CONFIG_MEDIA_TYPE,
        "size": 2,
    }:
        _fail(f"{label} empty config descriptor differs")
    _manifest_layers(manifest["layers"], layers, label)
    if subject is not None:
        subject_digest, subject_size = subject
        subject_value = _exact(
            manifest["subject"],
            {"digest", "mediaType", "size"},
            f"{label} subject",
        )
        if subject_value != {
            "digest": subject_digest,
            "mediaType": OCI_MANIFEST_MEDIA_TYPE,
            "size": subject_size,
        }:
            _fail(f"{label} subject differs from the immutable primary manifest")
    return manifest


def _exact_digest_output(payload: bytes, expected: str, label: str) -> None:
    if payload != (expected + "\n").encode("ascii"):
        _fail(f"{label} output is duplicate, truncated, or noncanonical")


def _candidate_layers(candidate: Candidate) -> list[Layer]:
    rows = (
        (candidate.archive_relative, ADMISSION_ARCHIVE_MEDIA_TYPE),
        ("authority/release_manifest.json", AUTHORITY_MANIFEST_MEDIA_TYPE),
        ("authority/release_manifest.json.sig", AUTHORITY_SIGNATURE_MEDIA_TYPE),
        ("authority/release_manifest.json.pub", AUTHORITY_PUBLIC_KEY_MEDIA_TYPE),
    )
    return [
        Layer(path, media_type, candidate.files[path].sha256, candidate.files[path].size)
        for path, media_type in rows
    ]


def _admission_bytes(
    archive: Path,
    authority: Path,
    request: PublishRequest,
    scratch: Path,
    now_unix: int,
    label: str,
) -> bytes:
    ledger = scratch / f"{label}-replay-ledger.json"
    exclusive_write_bytes(
        ledger, admission.canonical_replay_ledger_bytes(()), mode=0o600
    )
    _require_authority(request.authority_uid)
    result = admission.verify_admission(
        archive_path=archive,
        authority_dir=authority,
        expected_source=request.expected_source,
        expected_receipt_id=request.expected_qualification_receipt_id,
        replay_ledger_path=ledger,
        trusted_signing_fingerprint=request.trusted_signing_fingerprint,
        release_manifest_verifier_path=request.release_manifest_verifier_path,
        trusted_release_manifest_verifier_sha256=(
            request.trusted_release_manifest_verifier_sha256
        ),
        now_unix=now_unix,
    )
    _require_authority(request.authority_uid)
    expected_fields = {
        "artifact_handoff_sha256",
        "archive_sha256",
        "boi_artifact_inventory_sha256",
        "deployment_performed",
        "linux_authority_manifest_sha256",
        "macos_end_block_hash",
        "macos_end_height",
        "peer_count",
        "privacy_protocol_receipt_id",
        "receipt_id",
        "receipt_signers",
        "release_manifest_sha256",
        "release_manifest_verifier_sha256",
        "reset_manifest_sha256",
        "restart_generation",
        "schema",
        "schema_version",
        "signer_fingerprint_sha256",
        "source",
        "supervisor_sha256",
        "validator_binary_sha256",
        "validator_config_sha256",
        "verified",
    }
    _exact(result, expected_fields, f"{label} admission result")
    _sha256(
        result["boi_artifact_inventory_sha256"],
        f"{label} BOI artifact inventory digest",
    )
    _sha256(
        result["privacy_protocol_receipt_id"],
        f"{label} privacy protocol receipt ID",
    )
    try:
        receipt_signers = admission._receipt_signers(
            result["receipt_signers"], f"{label} receipt signer map"
        )
    except admission.TairaRolloutAdmissionError as error:
        raise TairaPublicationError(str(error)) from error
    archive_sha = _capture_file(archive, f"{label} admitted archive").sha256
    if (
        result["schema"] != admission.VERIFICATION_SCHEMA
        or result["schema_version"] != admission.VERIFICATION_SCHEMA_VERSION
        or result["verified"] is not True
        or result["deployment_performed"] is not False
        or result["peer_count"] != admission.PEER_COUNT
        or result["source"] != request.expected_source.as_dict()
        or result["receipt_id"] != request.expected_qualification_receipt_id
        or result["archive_sha256"] != archive_sha
        or result["signer_fingerprint_sha256"]
        != request.trusted_signing_fingerprint
        or result["release_manifest_verifier_sha256"]
        != request.trusted_release_manifest_verifier_sha256
        or result["receipt_signers"] != receipt_signers
    ):
        _fail(f"{label} admission result differs from the current candidate")
    return canonical_json_bytes(result)


def _safe_pull_inventory(root: Path, expected: Sequence[str], label: str) -> None:
    actual_files: list[str] = []
    for current, names, files in os.walk(root, followlinks=False):
        names.sort()
        files.sort()
        current_path = Path(current)
        info = current_path.lstat()
        if (
            not stat.S_ISDIR(info.st_mode)
            or info.st_uid != os.geteuid()
            or info.st_mode & 0o022
        ):
            _fail(f"{label} contains an unsafe directory")
        for name in names:
            child = (current_path / name).lstat()
            if stat.S_ISLNK(child.st_mode) or not stat.S_ISDIR(child.st_mode):
                _fail(f"{label} contains a symlink or special directory")
        for name in files:
            path = current_path / name
            child = path.lstat()
            if (
                stat.S_ISLNK(child.st_mode)
                or not stat.S_ISREG(child.st_mode)
                or child.st_nlink != 1
                or child.st_uid != os.geteuid()
                or child.st_mode & 0o022
            ):
                _fail(f"{label} contains an unsafe pulled file")
            actual_files.append(path.relative_to(root).as_posix())
    if sorted(actual_files) != sorted(expected):
        _fail(f"{label} inventory differs from the exact layer titles")


def _compare_files(left: Path, right: Path, label: str) -> None:
    left_fd, left_info = _open_regular(left, f"{label} source")
    right_fd, right_info = _open_regular(right, f"{label} pull")
    try:
        if left_info.st_size != right_info.st_size:
            _fail(f"{label} pulled size differs")
        while True:
            left_chunk = os.read(left_fd, 1024 * 1024)
            right_chunk = os.read(right_fd, 1024 * 1024)
            if left_chunk != right_chunk:
                _fail(f"{label} pulled bytes differ")
            if not left_chunk:
                break
        if (
            _identity(os.fstat(left_fd)) != _identity(left_info)
            or _identity(os.fstat(right_fd)) != _identity(right_info)
        ):
            _fail(f"{label} changed while byte-comparing")
    finally:
        os.close(right_fd)
        os.close(left_fd)


def _receipt_value(
    *,
    request: PublishRequest,
    admission_payload: bytes,
    layers: Sequence[Layer],
    primary_digest: str,
    primary_size: int,
    tagged_reference: str,
    immutable_reference: str,
    tag: str,
    issued_at_unix: int,
) -> dict[str, object]:
    return {
        "admission_sha256": hashlib.sha256(admission_payload).hexdigest(),
        "immutable_reference": immutable_reference,
        "issued_at_unix": issued_at_unix,
        "layers": [layer.receipt_row() for layer in layers],
        "oras": {
            "executable_sha256": request.trusted_oras_sha256,
            "version": request.expected_oras_version,
        },
        "qualification_receipt_id": request.expected_qualification_receipt_id,
        "repository": request.repository,
        "schema": PUBLICATION_SCHEMA,
        "schema_version": PUBLICATION_SCHEMA_VERSION,
        "signing": {
            "native_verifier_sha256": (
                request.trusted_release_manifest_verifier_sha256
            ),
            "signer_fingerprint_sha256": request.trusted_signing_fingerprint,
        },
        "source": request.expected_source.as_dict(),
        "subject": {
            "digest": primary_digest,
            "media_type": OCI_MANIFEST_MEDIA_TYPE,
            "size": primary_size,
        },
        "suffix": request.suffix,
        "tag": tag,
        "tagged_reference": tagged_reference,
    }


def _validate_publication_receipt(
    payload: bytes,
    *,
    expected: Mapping[str, object],
) -> dict[str, Any]:
    value = _strict_json(payload, "publication receipt")
    if payload != canonical_json_bytes(value):
        _fail("publication receipt is not canonical deterministic JSON")
    _exact(
        value,
        {
            "admission_sha256",
            "immutable_reference",
            "issued_at_unix",
            "layers",
            "oras",
            "qualification_receipt_id",
            "repository",
            "schema",
            "schema_version",
            "signing",
            "source",
            "subject",
            "suffix",
            "tag",
            "tagged_reference",
        },
        "publication receipt",
    )
    if value != expected:
        _fail("publication receipt semantics differ from the current immutable subject")
    _sha256(value["admission_sha256"], "publication admission digest")
    _integer(
        value["issued_at_unix"],
        "publication issue time",
        minimum=1,
        maximum=MAX_PUBLICATION_UNIX,
    )
    _sha256(value["qualification_receipt_id"], "publication qualification receipt")
    return value


def _rollback_terminal_directory(
    path: Path,
    *,
    expected_device: int,
    expected_inode: int,
    authority_uid: int,
) -> bool:
    """Remove only the exact generated handoff directory, wherever rename left it."""

    try:
        directory_info = path.lstat()
    except FileNotFoundError:
        return False
    if (directory_info.st_dev, directory_info.st_ino) != (
        expected_device,
        expected_inode,
    ):
        return False
    if not stat.S_ISDIR(directory_info.st_mode) or directory_info.st_uid != authority_uid:
        _fail("refusing to roll back a substituted terminal handoff directory")
    path.chmod(0o700)
    entries = list(path.iterdir())
    for entry in entries:
        info = entry.lstat()
        if (
            entry.name not in TERMINAL_FILES
            or stat.S_ISLNK(info.st_mode)
            or not stat.S_ISREG(info.st_mode)
            or info.st_nlink != 1
            or info.st_uid != authority_uid
        ):
            _fail("refusing to roll back a substituted terminal handoff entry")
    for entry in entries:
        entry.chmod(0o600)
        entry.unlink()
    path.rmdir()
    return True


def _write_terminal_handoff(
    output: Path,
    payloads: Mapping[str, bytes],
    authority_uid: int,
) -> dict[str, object]:
    output = _absolute_lexical(output, "terminal handoff", exists=False)
    if set(payloads) != set(TERMINAL_FILES):
        _fail("terminal handoff payload inventory is not exactly seven files")
    parent_info = output.parent.lstat()
    if (
        not stat.S_ISDIR(parent_info.st_mode)
        or parent_info.st_uid != authority_uid
        or stat.S_IMODE(parent_info.st_mode) != 0o700
    ):
        _fail("terminal handoff parent must be authority-private exact mode 0700")
    staging = Path(
        tempfile.mkdtemp(prefix=f".{output.name}.pending-", dir=output.parent)
    )
    staging_info = staging.lstat()
    staging_device = staging_info.st_dev
    staging_inode = staging_info.st_ino
    committed = False
    try:
        if (
            not stat.S_ISDIR(staging_info.st_mode)
            or staging_info.st_uid != authority_uid
            or stat.S_IMODE(staging_info.st_mode) != 0o700
        ):
            _fail("terminal handoff staging root is not authority-private")
        staging.chmod(0o700)
        installed: dict[str, CapturedFile] = {}
        for name in sorted(payloads):
            path = staging / name
            exclusive_write_bytes(path, payloads[name], mode=0o600)
            path.chmod(0o444)
            installed[name] = _capture_file(path, f"terminal handoff {name}")
        if sorted(path.name for path in staging.iterdir()) != sorted(TERMINAL_FILES):
            _fail("terminal handoff staging inventory differs")
        for name, captured in installed.items():
            if captured.sha256 != hashlib.sha256(payloads[name]).hexdigest():
                _fail(f"terminal handoff {name} changed before freeze")
            info = (staging / name).lstat()
            if (
                info.st_uid != authority_uid
                or info.st_nlink != 1
                or stat.S_IMODE(info.st_mode) != 0o444
            ):
                _fail(f"terminal handoff {name} identity differs")
        directory_fd = os.open(
            staging,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            os.fchmod(directory_fd, 0o555)
            os.fsync(directory_fd)
            frozen = os.fstat(directory_fd)
        finally:
            os.close(directory_fd)
        if frozen.st_uid != authority_uid or stat.S_IMODE(frozen.st_mode) != 0o555:
            _fail("terminal handoff staging root did not freeze")
        try:
            output.lstat()
        except FileNotFoundError:
            pass
        else:
            _fail("terminal handoff already exists")
        os.rename(staging, output)
        parent_fd = os.open(
            output.parent,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            os.fsync(parent_fd)
        finally:
            os.close(parent_fd)
        if sorted(path.name for path in output.iterdir()) != sorted(TERMINAL_FILES):
            _fail("terminal handoff inventory changed after atomic installation")
        committed = True
        return {
            "files": [
                {
                    "path": name,
                    "sha256": hashlib.sha256(payloads[name]).hexdigest(),
                    "size": len(payloads[name]),
                }
                for name in sorted(payloads)
            ],
            "output": str(output),
        }
    finally:
        if not committed:
            try:
                _rollback_terminal_directory(
                    staging,
                    expected_device=staging_device,
                    expected_inode=staging_inode,
                    authority_uid=authority_uid,
                )
                removed_output = _rollback_terminal_directory(
                    output,
                    expected_device=staging_device,
                    expected_inode=staging_inode,
                    authority_uid=authority_uid,
                )
                if removed_output:
                    parent_fd = os.open(
                        output.parent,
                        os.O_RDONLY
                        | getattr(os, "O_CLOEXEC", 0)
                        | getattr(os, "O_DIRECTORY", 0)
                        | getattr(os, "O_NOFOLLOW", 0),
                    )
                    try:
                        os.fsync(parent_fd)
                    finally:
                        os.close(parent_fd)
            except (OSError, TairaPublicationError) as exc:
                raise TairaPublicationError(
                    "failed to roll back the incomplete terminal handoff"
                ) from exc


def _validate_request(request: PublishRequest) -> PublishRequest:
    source = _validate_source(request.expected_source)
    receipt_id = _sha256(
        request.expected_qualification_receipt_id,
        "expected qualification receipt ID",
    )
    repository, suffix, _registry = _validate_repository(
        request.repository, request.suffix
    )
    for name in FORBIDDEN_CREDENTIAL_ENV:
        if os.environ.get(name):
            _fail(f"credential environment surface {name} is forbidden")
    _require_authority(request.authority_uid)
    if VERSION_RE.fullmatch(request.expected_oras_version) is None:
        _fail("expected ORAS version is not canonical")
    return PublishRequest(
        candidate_root=request.candidate_root,
        expected_source=source,
        expected_qualification_receipt_id=receipt_id,
        repository=repository,
        suffix=suffix,
        authority_uid=request.authority_uid,
        scratch_parent=request.scratch_parent,
        registry_config=request.registry_config,
        oras_path=request.oras_path,
        trusted_oras_sha256=_sha256(
            request.trusted_oras_sha256, "trusted ORAS digest"
        ),
        expected_oras_version=request.expected_oras_version,
        external_signer_path=request.external_signer_path,
        trusted_external_signer_sha256=_sha256(
            request.trusted_external_signer_sha256,
            "trusted external signer digest",
        ),
        signing_public_key_path=request.signing_public_key_path,
        trusted_signing_fingerprint=_sha256(
            request.trusted_signing_fingerprint,
            "trusted signing fingerprint",
        ),
        release_manifest_verifier_path=request.release_manifest_verifier_path,
        trusted_release_manifest_verifier_sha256=_sha256(
            request.trusted_release_manifest_verifier_sha256,
            "trusted release verifier digest",
        ),
        terminal_handoff=request.terminal_handoff,
        rollout_plan=request.rollout_plan,
        rollout_result=request.rollout_result,
        rollout_authority_envelope=request.rollout_authority_envelope,
        rollout_durable_receipt=request.rollout_durable_receipt,
    )


def _publish_after_authenticated_rollout_observation(
    request: PublishRequest, *, now_unix: int | None = None
) -> dict[str, object]:
    """Execute publication after an independently authenticated observation."""

    request = _validate_request(request)
    current_time = int(time.time()) if now_unix is None else now_unix
    _integer(
        current_time,
        "publication issue time",
        minimum=1,
        maximum=MAX_PUBLICATION_UNIX,
    )
    created = (
        dt.datetime(1970, 1, 1, tzinfo=dt.timezone.utc)
        + dt.timedelta(seconds=current_time)
    ).strftime("%Y-%m-%dT%H:%M:%SZ")
    scratch_parent = _absolute_lexical(request.scratch_parent, "scratch parent")
    scratch_info = scratch_parent.lstat()
    if (
        not stat.S_ISDIR(scratch_info.st_mode)
        or scratch_info.st_uid != request.authority_uid
        or stat.S_IMODE(scratch_info.st_mode) != 0o700
    ):
        _fail("scratch parent must be authority-owned exact mode 0700")
    _absolute_lexical(request.terminal_handoff, "terminal handoff", exists=False)
    candidate = _capture_candidate(
        request.candidate_root,
        request.expected_source,
        request.expected_qualification_receipt_id,
    )
    oras = _capture_pinned_executable(
        request.oras_path, request.trusted_oras_sha256, "ORAS executable"
    )
    signer = _capture_pinned_executable(
        request.external_signer_path,
        request.trusted_external_signer_sha256,
        "external signer",
    )
    verifier = _capture_pinned_executable(
        request.release_manifest_verifier_path,
        request.trusted_release_manifest_verifier_sha256,
        "release-manifest verifier",
    )
    public_key = _capture_public_key(
        request.signing_public_key_path, request.trusted_signing_fingerprint
    )
    registry_config = _capture_registry_config(
        request.registry_config, request.authority_uid
    )
    all_paths = {
        candidate.root,
        scratch_parent,
        request.terminal_handoff,
        oras.path,
        signer.path,
        verifier.path,
        public_key.path,
        registry_config.path,
    }
    if len(all_paths) != 8:
        _fail("candidate, scratch, output, tool, key, and config paths must differ")

    tag = f"taira-{_source_identity_digest(request.expected_source)}"
    if request.suffix:
        tag += f"-{request.suffix}"
    tagged_reference = f"{request.repository}:{tag}"
    _, _, _registry = _validate_repository(request.repository, request.suffix)

    with tempfile.TemporaryDirectory(
        prefix="taira-publish-authority-", dir=scratch_parent
    ) as raw_scratch:
        scratch = Path(raw_scratch).resolve(strict=True)
        scratch.chmod(0o700)
        if (
            scratch.lstat().st_uid != request.authority_uid
            or stat.S_IMODE(scratch.lstat().st_mode) != 0o700
        ):
            _fail("publication scratch is not authority-private")
        (scratch / "cache").mkdir(mode=0o700)
        (scratch / "config-home").mkdir(mode=0o700)
        environment = _closed_child_environment(scratch)
        config_snapshot = scratch / "registry-config.json"
        exclusive_write_bytes(
            config_snapshot,
            _read_captured(registry_config, "registry config"),
            mode=0o600,
        )
        config_snapshot.chmod(0o400)
        config_snapshot_capture = _capture_file(
            config_snapshot,
            "private registry-config snapshot",
            maximum=MAX_REGISTRY_CONFIG_BYTES,
        )

        version_output = _run_oras(
            oras,
            ("version",),
            cwd=scratch,
            environment=environment,
            authority_uid=request.authority_uid,
        )
        _validate_oras_version(version_output, request.expected_oras_version)
        admission_payload = _admission_bytes(
            candidate.archive,
            candidate.authority,
            request,
            scratch,
            current_time,
            "original",
        )
        _assert_candidate_unchanged(candidate)

        layers = _candidate_layers(candidate)
        push_output = _run_oras(
            oras,
            (
                "push",
                "--registry-config",
                str(config_snapshot),
                "--image-spec",
                "v1.1",
                "--artifact-type",
                PRIMARY_ARTIFACT_TYPE,
                "--annotation",
                f"org.opencontainers.image.created={created}",
                "--format",
                "json",
                tagged_reference,
                *tuple(f"{layer.path}:{layer.media_type}" for layer in layers),
            ),
            cwd=candidate.root,
            environment=environment,
            authority_uid=request.authority_uid,
        )
        primary_digest, primary_size = _oras_result(
            push_output,
            repository=request.repository,
            tagged_reference=tagged_reference,
            artifact_type=PRIMARY_ARTIFACT_TYPE,
            created=created,
        )
        immutable_reference = f"{request.repository}@{primary_digest}"
        resolved = _run_oras(
            oras,
            (
                "resolve",
                "--registry-config",
                str(config_snapshot),
                tagged_reference,
            ),
            cwd=scratch,
            environment=environment,
            authority_uid=request.authority_uid,
            output_limit=1024,
        )
        _exact_digest_output(resolved, primary_digest, "ORAS primary resolve")
        raw_primary = _run_oras(
            oras,
            (
                "manifest",
                "fetch",
                "--registry-config",
                str(config_snapshot),
                "--output",
                "-",
                immutable_reference,
            ),
            cwd=scratch,
            environment=environment,
            authority_uid=request.authority_uid,
            output_limit=MAX_OCI_MANIFEST_BYTES,
        )
        _validate_raw_manifest(
            raw_primary,
            digest=primary_digest,
            expected_size=primary_size,
            artifact_type=PRIMARY_ARTIFACT_TYPE,
            layers=layers,
            created=created,
            subject=None,
            label="primary OCI manifest",
        )

        primary_pull = scratch / "primary-pull"
        primary_pull.mkdir(mode=0o700)
        _run_oras(
            oras,
            (
                "pull",
                "--registry-config",
                str(config_snapshot),
                "--output",
                str(primary_pull),
                immutable_reference,
            ),
            cwd=scratch,
            environment=environment,
            authority_uid=request.authority_uid,
        )
        _safe_pull_inventory(
            primary_pull,
            [layer.path for layer in layers],
            "primary OCI pull",
        )
        for layer in layers:
            _compare_files(
                candidate.root / layer.path,
                primary_pull / layer.path,
                f"primary layer {layer.path}",
            )
        pulled_admission_payload = _admission_bytes(
            primary_pull / candidate.archive_relative,
            primary_pull / "authority",
            request,
            scratch,
            current_time,
            "pulled",
        )
        if pulled_admission_payload != admission_payload:
            _fail("pulled candidate admission differs from current original admission")

        receipt_dir = scratch / "receipt"
        receipt_dir.mkdir(mode=0o700)
        receipt_path = receipt_dir / PUBLICATION_RECEIPT_NAME
        signature_path = receipt_dir / PUBLICATION_SIGNATURE_NAME
        receipt_public_key = receipt_dir / PUBLICATION_PUBLIC_KEY_NAME
        receipt_value = _receipt_value(
            request=request,
            admission_payload=pulled_admission_payload,
            layers=layers,
            primary_digest=primary_digest,
            primary_size=len(raw_primary),
            tagged_reference=tagged_reference,
            immutable_reference=immutable_reference,
            tag=tag,
            issued_at_unix=current_time,
        )
        receipt_payload = canonical_json_bytes(receipt_value)
        _validate_publication_receipt(receipt_payload, expected=receipt_value)
        exclusive_write_bytes(receipt_path, receipt_payload, mode=0o600)
        _require_authority(request.authority_uid)
        sign_release_manifest(
            receipt_path,
            signer.path,
            public_key.path,
            request.trusted_signing_fingerprint,
            signature_path,
            receipt_public_key,
            verifier.path,
            request.trusted_release_manifest_verifier_sha256,
        )
        _require_authority(request.authority_uid)
        _assert_file_unchanged(signer, "external signer")
        _assert_file_unchanged(verifier, "release-manifest verifier")
        _assert_file_unchanged(public_key, "signing public key")
        if receipt_path.read_bytes() != receipt_payload:
            _fail("publication receipt changed during signing")
        _validate_publication_receipt(receipt_path.read_bytes(), expected=receipt_value)

        receipt_files = (
            (PUBLICATION_RECEIPT_NAME, PUBLICATION_RECEIPT_MEDIA_TYPE),
            (PUBLICATION_SIGNATURE_NAME, AUTHORITY_SIGNATURE_MEDIA_TYPE),
            (PUBLICATION_PUBLIC_KEY_NAME, AUTHORITY_PUBLIC_KEY_MEDIA_TYPE),
        )
        receipt_layers = [
            Layer(
                name,
                media_type,
                _capture_file(receipt_dir / name, f"receipt layer {name}").sha256,
                (receipt_dir / name).stat().st_size,
            )
            for name, media_type in receipt_files
        ]
        attach_output = _run_oras(
            oras,
            (
                "attach",
                "--registry-config",
                str(config_snapshot),
                "--artifact-type",
                PUBLICATION_ARTIFACT_TYPE,
                "--annotation",
                f"org.opencontainers.image.created={created}",
                "--format",
                "json",
                immutable_reference,
                *tuple(f"{layer.path}:{layer.media_type}" for layer in receipt_layers),
            ),
            cwd=receipt_dir,
            environment=environment,
            authority_uid=request.authority_uid,
        )
        receipt_digest, receipt_manifest_size = _oras_result(
            attach_output,
            repository=request.repository,
            tagged_reference=None,
            artifact_type=PUBLICATION_ARTIFACT_TYPE,
            created=created,
        )
        receipt_reference = f"{request.repository}@{receipt_digest}"
        receipt_resolved = _run_oras(
            oras,
            (
                "resolve",
                "--registry-config",
                str(config_snapshot),
                receipt_reference,
            ),
            cwd=scratch,
            environment=environment,
            authority_uid=request.authority_uid,
            output_limit=1024,
        )
        _exact_digest_output(
            receipt_resolved, receipt_digest, "ORAS receipt resolve"
        )
        raw_receipt = _run_oras(
            oras,
            (
                "manifest",
                "fetch",
                "--registry-config",
                str(config_snapshot),
                "--output",
                "-",
                receipt_reference,
            ),
            cwd=scratch,
            environment=environment,
            authority_uid=request.authority_uid,
            output_limit=MAX_OCI_MANIFEST_BYTES,
        )
        _validate_raw_manifest(
            raw_receipt,
            digest=receipt_digest,
            expected_size=receipt_manifest_size,
            artifact_type=PUBLICATION_ARTIFACT_TYPE,
            layers=receipt_layers,
            created=created,
            subject=(primary_digest, len(raw_primary)),
            label="publication receipt OCI manifest",
        )

        receipt_pull = scratch / "receipt-pull"
        receipt_pull.mkdir(mode=0o700)
        _run_oras(
            oras,
            (
                "pull",
                "--registry-config",
                str(config_snapshot),
                "--output",
                str(receipt_pull),
                receipt_reference,
            ),
            cwd=scratch,
            environment=environment,
            authority_uid=request.authority_uid,
        )
        _safe_pull_inventory(
            receipt_pull,
            [layer.path for layer in receipt_layers],
            "publication receipt OCI pull",
        )
        for layer in receipt_layers:
            _compare_files(
                receipt_dir / layer.path,
                receipt_pull / layer.path,
                f"publication receipt layer {layer.path}",
            )
        _require_authority(request.authority_uid)
        verify_release_manifest(
            receipt_pull / PUBLICATION_RECEIPT_NAME,
            receipt_pull / PUBLICATION_SIGNATURE_NAME,
            receipt_pull / PUBLICATION_PUBLIC_KEY_NAME,
            request.trusted_signing_fingerprint,
            verifier.path,
            request.trusted_release_manifest_verifier_sha256,
        )
        _require_authority(request.authority_uid)
        pulled_receipt_payload = (
            receipt_pull / PUBLICATION_RECEIPT_NAME
        ).read_bytes()
        _validate_publication_receipt(
            pulled_receipt_payload,
            expected=_receipt_value(
                request=request,
                admission_payload=pulled_admission_payload,
                layers=layers,
                primary_digest=primary_digest,
                primary_size=len(raw_primary),
                tagged_reference=tagged_reference,
                immutable_reference=immutable_reference,
                tag=tag,
                issued_at_unix=current_time,
            ),
        )

        _assert_candidate_unchanged(candidate)
        for captured, label in (
            (oras, "pinned ORAS executable"),
            (signer, "external signer"),
            (verifier, "release-manifest verifier"),
            (public_key, "signing public key"),
            (registry_config, "registry config"),
            (config_snapshot_capture, "private registry-config snapshot"),
        ):
            _assert_file_unchanged(captured, label)

        signature_payload = (
            receipt_pull / PUBLICATION_SIGNATURE_NAME
        ).read_bytes()
        public_key_payload = (
            receipt_pull / PUBLICATION_PUBLIC_KEY_NAME
        ).read_bytes()
        terminal = _write_terminal_handoff(
            request.terminal_handoff,
            {
                PUBLICATION_RECEIPT_NAME: pulled_receipt_payload,
                PUBLICATION_SIGNATURE_NAME: signature_payload,
                PUBLICATION_PUBLIC_KEY_NAME: public_key_payload,
                PRIMARY_MANIFEST_NAME: raw_primary,
                RECEIPT_MANIFEST_NAME: raw_receipt,
                PRIMARY_DIGEST_NAME: (primary_digest + "\n").encode("ascii"),
                RECEIPT_DIGEST_NAME: (receipt_digest + "\n").encode("ascii"),
            },
            request.authority_uid,
        )
        terminal.update(
            {
                "immutable_reference": immutable_reference,
                "primary_digest": primary_digest,
                "receipt_digest": receipt_digest,
                "receipt_reference": receipt_reference,
                "tagged_reference": tagged_reference,
            }
        )
        return terminal


def publish(request: PublishRequest, *, now_unix: int | None = None) -> dict[str, object]:
    """Historically verify the observation, then publish without re-signing it."""

    _require_authenticated_rollout_observation_authority()
    try:
        rollout_observation.verify_authenticated_result_files(
            plan_path=request.rollout_plan,
            result_path=request.rollout_result,
            authority_envelope_path=request.rollout_authority_envelope,
            durable_receipt_path=request.rollout_durable_receipt,
        )
    except rollout_observation.RolloutContractError as exc:
        raise TairaPublicationError(str(exc)) from exc
    return _publish_after_authenticated_rollout_observation(
        request, now_unix=now_unix
    )


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--candidate-root", type=Path, required=True)
    parser.add_argument("--expected-source-commit", required=True)
    parser.add_argument("--expected-dpn-validator-release-commit", required=True)
    parser.add_argument("--expected-cargo-lock-sha256", required=True)
    parser.add_argument(
        "--expected-workspace-source-manifest-sha256", required=True
    )
    parser.add_argument("--expected-qualification-receipt-id", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--suffix", required=True)
    parser.add_argument("--authority-uid", type=int, required=True)
    parser.add_argument("--scratch-parent", type=Path, required=True)
    parser.add_argument("--registry-config", type=Path, required=True)
    parser.add_argument("--oras", type=Path, required=True)
    parser.add_argument("--trusted-oras-sha256", required=True)
    parser.add_argument("--expected-oras-version", required=True)
    parser.add_argument("--external-signer", type=Path, required=True)
    parser.add_argument("--trusted-external-signer-sha256", required=True)
    parser.add_argument("--signing-public-key", type=Path, required=True)
    parser.add_argument("--trusted-signing-fingerprint", required=True)
    parser.add_argument("--release-manifest-verifier", type=Path, required=True)
    parser.add_argument(
        "--trusted-release-manifest-verifier-sha256", required=True
    )
    parser.add_argument("--terminal-handoff", type=Path, required=True)
    parser.add_argument("--rollout-plan", type=Path, required=True)
    parser.add_argument("--rollout-result", type=Path, required=True)
    parser.add_argument("--rollout-authority-envelope", type=Path, required=True)
    parser.add_argument("--rollout-durable-receipt", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    request = PublishRequest(
        candidate_root=args.candidate_root,
        expected_source=admission.SourceIdentity(
            args.expected_source_commit,
            args.expected_dpn_validator_release_commit,
            args.expected_cargo_lock_sha256,
            args.expected_workspace_source_manifest_sha256,
        ),
        expected_qualification_receipt_id=(
            args.expected_qualification_receipt_id
        ),
        repository=args.repository,
        suffix=args.suffix,
        authority_uid=args.authority_uid,
        scratch_parent=args.scratch_parent,
        registry_config=args.registry_config,
        oras_path=args.oras,
        trusted_oras_sha256=args.trusted_oras_sha256,
        expected_oras_version=args.expected_oras_version,
        external_signer_path=args.external_signer,
        trusted_external_signer_sha256=args.trusted_external_signer_sha256,
        signing_public_key_path=args.signing_public_key,
        trusted_signing_fingerprint=args.trusted_signing_fingerprint,
        release_manifest_verifier_path=args.release_manifest_verifier,
        trusted_release_manifest_verifier_sha256=(
            args.trusted_release_manifest_verifier_sha256
        ),
        terminal_handoff=args.terminal_handoff,
        rollout_plan=args.rollout_plan,
        rollout_result=args.rollout_result,
        rollout_authority_envelope=args.rollout_authority_envelope,
        rollout_durable_receipt=args.rollout_durable_receipt,
    )
    try:
        result = publish(request)
    except (
        OSError,
        ReleaseArtifactError,
        ReleaseManifestSignatureError,
        TairaPublicationError,
        admission.TairaRolloutAdmissionError,
    ) as exc:
        print(f"Taira publication refused: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=True, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
