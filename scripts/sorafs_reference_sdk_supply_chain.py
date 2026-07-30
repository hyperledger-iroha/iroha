"""Validate source-bound SoraFS reference SDK supply-chain evidence.

The public :func:`validate_supply_chain_sources` entry point opens four
schema-closed indexes under one reviewed source root, verifies every indexed
file digest, and derives the existing SF-11 per-target result fields.  It does
not accept precomputed canary booleans or vulnerability totals.  Release
rehearsal and provenance receipts must be signed, and callers supply the
trusted Ed25519 authenticator; the module never treats an unsigned status as
authoritative.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
import stat
import unicodedata
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


REQUIRED_RELEASE_TARGETS = (
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
)
SOURCE_ARTIFACT_KINDS = (
    "release_rehearsal",
    "sbom_index",
    "vulnerability_report",
    "provenance_bundle",
)
DEFAULT_SOURCE_ARTIFACT_PATHS = {
    "release_rehearsal": "release-rehearsal.json",
    "sbom_index": "sbom-index.json",
    "vulnerability_report": "vulnerability-report.json",
    "provenance_bundle": "provenance-bundle.json",
}

RELEASE_REHEARSAL_SCHEMA = "sorafs.reference_sdk.release_rehearsal.v1"
RELEASE_REHEARSAL_RECEIPT_SCHEMA = (
    "sorafs.reference_sdk.release_rehearsal.receipt.v1"
)
SBOM_INDEX_SCHEMA = "sorafs.reference_sdk.sbom_index.v1"
VULNERABILITY_REPORT_SCHEMA = (
    "sorafs.reference_sdk.vulnerability_report_index.v1"
)
PROVENANCE_BUNDLE_SCHEMA = "sorafs.reference_sdk.provenance_bundle_index.v1"
PROVENANCE_VERIFICATION_RECEIPT_SCHEMA = (
    "sorafs.reference_sdk.provenance_verification.receipt.v1"
)

DEFAULT_OIDC_ISSUER = "https://token.actions.githubusercontent.com"
DEFAULT_MAX_SOURCE_AGE_SECS = 14 * 24 * 60 * 60
MAX_SOURCE_ARTIFACT_BYTES = 2 * 1024 * 1024
MAX_INDEXED_JSON_BYTES = 16 * 1024 * 1024
MAX_RELEASE_ARTIFACT_BYTES = 512 * 1024 * 1024
MAX_TIMESTAMP = (1 << 63) - 1

_HEX64 = re.compile(r"^[0-9a-f]{64}\Z")
_HEX128 = re.compile(r"^[0-9a-f]{128}\Z")
_RELEASE_REHEARSAL_RECEIPT_SIGNATURE_DOMAIN = (
    b"sorafs-reference-sdk-release-rehearsal-receipt-v1\x00"
)
_PROVENANCE_RECEIPT_SIGNATURE_DOMAIN = (
    b"sorafs-reference-sdk-provenance-verification-receipt-v1\x00"
)
_COMMON_SOURCE_FIELDS = frozenset(
    {
        "schema",
        "generated_at_unix",
        "deployment_id",
        "environment",
        "deployment_context_reviewed",
        "release_manifest_digest_hex",
    }
)
_FILE_REFERENCE_FIELDS = frozenset({"artifact_path", "sha256"})
_RELEASE_OPERATIONS = (
    "binary_smoke",
    "deterministic_archive_replay",
    "installation",
    "rollback",
    "yank",
)
_RELEASE_REHEARSAL_FIELDS = _COMMON_SOURCE_FIELDS | {"targets"}
_RELEASE_REHEARSAL_TARGET_FIELDS = frozenset(
    {"target", "release_artifact", "receipt"}
)
_RELEASE_RECEIPT_FIELDS = _COMMON_SOURCE_FIELDS | {
    "target",
    "subject_sha256",
    "operations",
    "verification_key_fingerprint_hex",
    "signature_algorithm",
    "signature_hex",
}
_SBOM_INDEX_FIELDS = _COMMON_SOURCE_FIELDS | {"source_sbom", "targets"}
_SBOM_TARGET_FIELDS = frozenset({"target", "platform_sbom"})
_VULNERABILITY_INDEX_FIELDS = _COMMON_SOURCE_FIELDS | {
    "source_report",
    "targets",
}
_VULNERABILITY_TARGET_FIELDS = frozenset({"target", "platform_report"})
_PROVENANCE_INDEX_FIELDS = _COMMON_SOURCE_FIELDS | {
    "certificate_identity",
    "oidc_issuer",
    "verification_key_fingerprint_hex",
    "targets",
}
_PROVENANCE_TARGET_FIELDS = frozenset(
    {
        "target",
        "attestation_bundle",
        "cosign_bundle",
        "sha256sums",
        "sha256sums_cosign_bundle",
        "verification_receipt",
    }
)
_PROVENANCE_RECEIPT_FIELDS = _COMMON_SOURCE_FIELDS | {
    "target",
    "certificate_identity",
    "oidc_issuer",
    "verification_key_fingerprint_hex",
    "subject_sha256",
    "attestation_bundle_sha256",
    "cosign_bundle_sha256",
    "sha256sums_sha256",
    "sha256sums_cosign_bundle_sha256",
    "oidc_identity_status",
    "cosign_provenance_status",
    "signature_algorithm",
    "signature_hex",
}

VerificationReceiptAuthenticator = Callable[[str, bytes, bytes], bool]


@dataclass(frozen=True)
class SourceArtifactBinding:
    """One opened top-level source artifact and its exact byte digest."""

    kind: str
    artifact_path: str
    sha256: str

    def to_dict(self) -> dict[str, str]:
        """Return the canonical payload-free source binding."""

        return {
            "kind": self.kind,
            "artifact_path": self.artifact_path,
            "sha256": self.sha256,
        }


@dataclass(frozen=True)
class SupplyChainTargetResult:
    """Source-derived SF-11 supply-chain result for one native target."""

    target: str
    binary_smoke_passed: bool
    deterministic_archive_replay_passed: bool
    installation_verified: bool
    rollback_verified: bool
    yank_verified: bool
    sbom_generated: bool
    critical_vulnerability_count: int
    high_vulnerability_count: int
    oidc_identity_verified: bool
    cosign_provenance_verified: bool

    def to_dict(self) -> dict[str, object]:
        """Return the existing schema-closed SF-11 target result shape."""

        return {
            "target": self.target,
            "binary_smoke_passed": self.binary_smoke_passed,
            "deterministic_archive_replay_passed": (
                self.deterministic_archive_replay_passed
            ),
            "installation_verified": self.installation_verified,
            "rollback_verified": self.rollback_verified,
            "yank_verified": self.yank_verified,
            "sbom_generated": self.sbom_generated,
            "critical_vulnerability_count": self.critical_vulnerability_count,
            "high_vulnerability_count": self.high_vulnerability_count,
            "oidc_identity_verified": self.oidc_identity_verified,
            "cosign_provenance_verified": self.cosign_provenance_verified,
        }


@dataclass(frozen=True)
class SupplyChainSourceResult:
    """Deterministic, source-derived fields reusable by builder and checker."""

    generated_at_unix: int
    deployment_id: str
    environment: str
    release_manifest_digest_hex: str
    source_artifacts: tuple[SourceArtifactBinding, ...]
    target_results: tuple[SupplyChainTargetResult, ...]
    sbom_index_digest_hex: str
    vulnerability_report_digest_hex: str
    provenance_bundle_digest_hex: str

    def to_dict(self) -> dict[str, object]:
        """Return all deterministic source-validation output fields."""

        return {
            "generated_at_unix": self.generated_at_unix,
            "deployment_id": self.deployment_id,
            "environment": self.environment,
            "release_manifest_digest_hex": self.release_manifest_digest_hex,
            "source_artifacts": [
                artifact.to_dict() for artifact in self.source_artifacts
            ],
            "target_count": len(self.target_results),
            "target_results": [result.to_dict() for result in self.target_results],
            "sbom_index_digest_hex": self.sbom_index_digest_hex,
            "vulnerability_report_digest_hex": (
                self.vulnerability_report_digest_hex
            ),
            "provenance_bundle_digest_hex": self.provenance_bundle_digest_hex,
        }

    def canary_fields(self) -> dict[str, object]:
        """Return only fields consumed by the SF-11 supply-chain canary."""

        return {
            "target_count": len(self.target_results),
            "target_results": [result.to_dict() for result in self.target_results],
            "release_manifest_digest_hex": self.release_manifest_digest_hex,
            "sbom_index_digest_hex": self.sbom_index_digest_hex,
            "vulnerability_report_digest_hex": (
                self.vulnerability_report_digest_hex
            ),
            "provenance_bundle_digest_hex": self.provenance_bundle_digest_hex,
            "source_artifacts": [
                artifact.to_dict() for artifact in self.source_artifacts
            ],
        }


@dataclass(frozen=True)
class _LoadedJson:
    artifact_path: str
    sha256: str
    payload: Any


@dataclass(frozen=True)
class _SourceRoot:
    path: Path
    signature: tuple[int, ...]


class _DuplicateJsonKey(ValueError):
    """Raised internally when an input JSON object repeats a key."""


class _FileRegistry:
    """Reject duplicate paths and hard-linked substitutions across one bundle."""

    def __init__(self) -> None:
        self._paths: set[Path] = set()
        self._identities: set[tuple[int, int]] = set()

    def record(
        self,
        resolved_path: Path,
        file_stat: os.stat_result,
        label: str,
        errors: list[str],
    ) -> None:
        identity = (file_stat.st_dev, file_stat.st_ino)
        if resolved_path in self._paths or identity in self._identities:
            errors.append(f"{label} must not duplicate another source file")
            return
        self._paths.add(resolved_path)
        self._identities.add(identity)


def _canonical_string(value: Any) -> str | None:
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or value != unicodedata.normalize("NFC", value)
        or any(unicodedata.category(character).startswith("C") for character in value)
    ):
        return None
    return value


def _canonical_relative_path(value: Any) -> str | None:
    text = _canonical_string(value)
    if (
        text is None
        or "\\" in text
        or "%" in text
        or text.startswith("/")
        or "//" in text
    ):
        return None
    path = PurePosixPath(text)
    if (
        not path.parts
        or str(path) != text
        or any(part in {"", ".", ".."} or ":" in part for part in path.parts)
    ):
        return None
    return text


def _canonical_hex64(value: Any) -> str | None:
    if not isinstance(value, str) or _HEX64.fullmatch(value) is None:
        return None
    if not any(character != "0" for character in value):
        return None
    return value


def _canonical_hex128(value: Any) -> str | None:
    if not isinstance(value, str) or _HEX128.fullmatch(value) is None:
        return None
    if not any(character != "0" for character in value):
        return None
    return value


def release_rehearsal_receipt_signing_bytes(
    receipt: Mapping[str, Any],
) -> bytes:
    """Return domain-separated canonical release-rehearsal receipt bytes."""

    if set(receipt) != _RELEASE_RECEIPT_FIELDS:
        raise ValueError("release rehearsal receipt has the wrong exact schema")
    unsigned = dict(receipt)
    unsigned.pop("signature_hex")
    try:
        encoded = json.dumps(
            unsigned,
            allow_nan=False,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ValueError(
            "release rehearsal receipt is not canonically encodable"
        ) from error
    return _RELEASE_REHEARSAL_RECEIPT_SIGNATURE_DOMAIN + encoded


def provenance_receipt_signing_bytes(receipt: Mapping[str, Any]) -> bytes:
    """Return the domain-separated canonical bytes an authenticator must sign."""

    if set(receipt) != _PROVENANCE_RECEIPT_FIELDS:
        raise ValueError("provenance verification receipt has the wrong exact schema")
    unsigned = dict(receipt)
    unsigned.pop("signature_hex")
    try:
        encoded = json.dumps(
            unsigned,
            allow_nan=False,
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise ValueError(
            "provenance verification receipt is not canonically encodable"
        ) from error
    return _PROVENANCE_RECEIPT_SIGNATURE_DOMAIN + encoded


def _closed_object(
    value: Any,
    fields: frozenset[str],
    label: str,
    errors: list[str],
) -> Mapping[str, Any] | None:
    if not isinstance(value, Mapping):
        errors.append(f"{label} must be an object")
        return None
    if set(value) != fields:
        errors.append(f"{label} fields must match the schema-closed contract")
    return value


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


def _decode_json(data: bytes, label: str, errors: list[str]) -> Any | None:
    try:
        text = data.decode("utf-8", errors="strict")
        return json.loads(
            text,
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
    ):
        errors.append(
            f"{label} must be strict UTF-8 JSON without duplicate keys or non-finite values"
        )
        return None


def _stat_signature(metadata: os.stat_result) -> tuple[int, ...]:
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


def _is_link_like(metadata: os.stat_result) -> bool:
    reparse_flag = getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)
    file_attributes = getattr(metadata, "st_file_attributes", 0)
    return stat.S_ISLNK(metadata.st_mode) or bool(
        reparse_flag and file_attributes & reparse_flag
    )


def _directory_open_flags() -> int:
    return (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def _prepare_source_root(
    source_root: Path,
    errors: list[str],
) -> _SourceRoot | None:
    if not isinstance(source_root, Path):
        errors.append("supply-chain source root must be a path")
        return None
    if (
        os.open not in os.supports_dir_fd
        or os.stat not in os.supports_dir_fd
        or os.stat not in os.supports_follow_symlinks
    ):
        errors.append(
            "supply-chain validation requires descriptor-relative file access"
        )
        return None
    descriptor = -1
    try:
        if source_root.is_symlink():
            errors.append("supply-chain source root must not be a symlink")
            return None
        if not source_root.is_dir():
            errors.append("supply-chain source root must be an existing directory")
            return None
        resolved = source_root.resolve(strict=True)
        before = resolved.lstat()
        if _is_link_like(before) or not stat.S_ISDIR(before.st_mode):
            errors.append(
                "supply-chain source root must be a non-symlink directory"
            )
            return None
        descriptor = os.open(resolved, _directory_open_flags())
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISDIR(opened.st_mode)
            or _stat_signature(before) != _stat_signature(opened)
        ):
            errors.append("supply-chain source root changed while it was opened")
            return None
        return _SourceRoot(resolved, _stat_signature(opened))
    except (OSError, RuntimeError):
        errors.append("supply-chain source root cannot be inspected")
        return None
    finally:
        if descriptor >= 0:
            try:
                os.close(descriptor)
            except OSError:
                pass


def _open_source_root(
    root: _SourceRoot,
    label: str,
    errors: list[str],
) -> int:
    descriptor = -1
    try:
        before = root.path.lstat()
        if (
            _is_link_like(before)
            or not stat.S_ISDIR(before.st_mode)
            or _stat_signature(before) != root.signature
        ):
            errors.append(f"{label} source root changed during validation")
            return -1
        descriptor = os.open(root.path, _directory_open_flags())
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISDIR(opened.st_mode)
            or _stat_signature(opened) != root.signature
        ):
            errors.append(f"{label} source root changed during validation")
            os.close(descriptor)
            return -1
        return descriptor
    except (OSError, RuntimeError):
        if descriptor >= 0:
            try:
                os.close(descriptor)
            except OSError:
                pass
        errors.append(f"{label} source root could not be opened safely")
        return -1


def _read_relative_file(
    root: _SourceRoot,
    artifact_path: Any,
    *,
    label: str,
    max_bytes: int,
    registry: _FileRegistry,
    errors: list[str],
    capture_bytes: bool = True,
) -> tuple[str, bytes | None, str] | None:
    relative = _canonical_relative_path(artifact_path)
    if relative is None:
        errors.append(f"{label} artifact_path must be a canonical relative path")
        return None

    parts = PurePosixPath(relative).parts
    directory_fd = _open_source_root(root, label, errors)
    if directory_fd < 0:
        return None
    fd = -1
    try:
        for component in parts[:-1]:
            before_directory = os.stat(
                component,
                dir_fd=directory_fd,
                follow_symlinks=False,
            )
            if _is_link_like(before_directory):
                errors.append(f"{label} path must not contain symlinks")
                return None
            if not stat.S_ISDIR(before_directory.st_mode):
                errors.append(f"{label} must reference an existing regular file")
                return None
            child_fd = os.open(
                component,
                _directory_open_flags(),
                dir_fd=directory_fd,
            )
            opened_directory = os.fstat(child_fd)
            if (
                not stat.S_ISDIR(opened_directory.st_mode)
                or _stat_signature(before_directory)
                != _stat_signature(opened_directory)
            ):
                os.close(child_fd)
                errors.append(f"{label} path changed while it was opened")
                return None
            os.close(directory_fd)
            directory_fd = child_fd

        before = os.stat(
            parts[-1],
            dir_fd=directory_fd,
            follow_symlinks=False,
        )
        if _is_link_like(before):
            errors.append(f"{label} path must not contain symlinks")
            return None
        if not stat.S_ISREG(before.st_mode):
            errors.append(f"{label} must reference a regular file")
            return None
        flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
        )
        fd = os.open(parts[-1], flags, dir_fd=directory_fd)
        opened = os.fstat(fd)
        if (
            not stat.S_ISREG(opened.st_mode)
            or _stat_signature(before) != _stat_signature(opened)
        ):
            errors.append(f"{label} changed while it was opened")
            return None
        if opened.st_size == 0:
            errors.append(f"{label} must not be empty")
            return None
        if opened.st_size > max_bytes:
            errors.append(f"{label} exceeds the bounded file-size limit")
            return None
        chunks: list[bytes] = []
        hasher = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(fd, min(64 * 1024, max_bytes + 1 - total))
            if not chunk:
                break
            hasher.update(chunk)
            if capture_bytes:
                chunks.append(chunk)
            total += len(chunk)
            if total > max_bytes:
                errors.append(f"{label} exceeds the bounded file-size limit")
                return None
        after = os.fstat(fd)
        if _stat_signature(opened) != _stat_signature(after) or total != after.st_size:
            errors.append(f"{label} changed while it was being read")
            return None
        resolved = root.path.joinpath(*parts)
        registry.record(resolved, after, label, errors)
        data = b"".join(chunks) if capture_bytes else None
        return relative, data, hasher.hexdigest()
    except FileNotFoundError:
        errors.append(f"{label} must reference an existing regular file")
        return None
    except (OSError, RuntimeError):
        errors.append(f"{label} could not be read safely")
        return None
    finally:
        if fd >= 0:
            try:
                os.close(fd)
            except OSError:
                pass
        if directory_fd >= 0:
            try:
                os.close(directory_fd)
            except OSError:
                pass


def _load_json_file(
    root: _SourceRoot,
    artifact_path: Any,
    *,
    label: str,
    max_bytes: int,
    registry: _FileRegistry,
    errors: list[str],
    expected_sha256: str | None = None,
) -> _LoadedJson | None:
    opened = _read_relative_file(
        root,
        artifact_path,
        label=label,
        max_bytes=max_bytes,
        registry=registry,
        errors=errors,
    )
    if opened is None:
        return None
    relative, data, digest = opened
    if data is None:  # pragma: no cover - guarded by capture_bytes default
        errors.append(f"{label} JSON bytes were not retained")
        return None
    if expected_sha256 is not None and digest != expected_sha256:
        errors.append(f"{label} sha256 must match the indexed file bytes")
    payload = _decode_json(data, label, errors)
    if payload is None:
        return None
    return _LoadedJson(relative, digest, payload)


def _file_reference(
    value: Any,
    label: str,
    errors: list[str],
) -> tuple[str, str] | None:
    payload = _closed_object(value, _FILE_REFERENCE_FIELDS, label, errors)
    if payload is None:
        return None
    artifact_path = _canonical_relative_path(payload.get("artifact_path"))
    if artifact_path is None:
        errors.append(f"{label}.artifact_path must be a canonical relative path")
    sha256 = _canonical_hex64(payload.get("sha256"))
    if sha256 is None:
        errors.append(f"{label}.sha256 must be non-zero lowercase SHA-256")
    if artifact_path is None or sha256 is None:
        return None
    return artifact_path, sha256


def _positive_timestamp(
    value: Any,
    label: str,
    *,
    now_unix: int,
    max_age_secs: int,
    errors: list[str],
) -> int | None:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or value <= 0
        or value > MAX_TIMESTAMP
    ):
        errors.append(f"{label} must be a positive bounded timestamp")
        return None
    if value > now_unix:
        errors.append(f"{label} must not be in the future")
    elif now_unix - value > max_age_secs:
        errors.append(f"{label} exceeds the maximum source age")
    return value


def _validate_common_source(
    payload: Mapping[str, Any],
    *,
    schema: str,
    label: str,
    expected_deployment_id: str,
    expected_environment: str,
    expected_release_manifest_digest_hex: str,
    now_unix: int,
    max_age_secs: int,
    errors: list[str],
) -> int | None:
    if payload.get("schema") != schema:
        errors.append(f"{label}.schema must match the canonical v1 source schema")
    if payload.get("deployment_id") != expected_deployment_id:
        errors.append(f"{label}.deployment_id must match the reviewed context")
    if payload.get("environment") != expected_environment:
        errors.append(f"{label}.environment must match the reviewed context")
    if payload.get("deployment_context_reviewed") is not True:
        errors.append(f"{label}.deployment_context_reviewed must be true")
    digest = _canonical_hex64(payload.get("release_manifest_digest_hex"))
    if digest is None:
        errors.append(
            f"{label}.release_manifest_digest_hex must be non-zero lowercase SHA-256"
        )
    elif digest != expected_release_manifest_digest_hex:
        errors.append(
            f"{label}.release_manifest_digest_hex must match the reviewed release manifest"
        )
    return _positive_timestamp(
        payload.get("generated_at_unix"),
        f"{label}.generated_at_unix",
        now_unix=now_unix,
        max_age_secs=max_age_secs,
        errors=errors,
    )


def _target_rows(
    value: Any,
    *,
    fields: frozenset[str],
    label: str,
    errors: list[str],
) -> list[Mapping[str, Any] | None]:
    if isinstance(value, (str, bytes, bytearray, Mapping)) or not isinstance(
        value, Sequence
    ):
        errors.append(f"{label} must be an array")
        return []
    rows = list(value)
    if len(rows) != len(REQUIRED_RELEASE_TARGETS):
        errors.append(f"{label} must contain exactly five target rows")
    checked: list[Mapping[str, Any] | None] = []
    observed: list[str] = []
    for index, row in enumerate(rows):
        row_label = f"{label}[{index}]"
        payload = _closed_object(row, fields, row_label, errors)
        checked.append(payload)
        if payload is None:
            continue
        target = _canonical_string(payload.get("target"))
        if target is None:
            errors.append(f"{row_label}.target must be canonical")
            continue
        observed.append(target)
        if index >= len(REQUIRED_RELEASE_TARGETS) or target != REQUIRED_RELEASE_TARGETS[
            index
        ]:
            errors.append(f"{row_label}.target must match the canonical target order")
    if len(observed) != len(set(observed)):
        errors.append(f"{label} must not contain duplicate targets")
    return checked


def _load_indexed_json(
    root: _SourceRoot,
    reference_value: Any,
    *,
    label: str,
    registry: _FileRegistry,
    errors: list[str],
) -> _LoadedJson | None:
    reference = _file_reference(reference_value, label, errors)
    if reference is None:
        return None
    artifact_path, sha256 = reference
    return _load_json_file(
        root,
        artifact_path,
        label=label,
        max_bytes=MAX_INDEXED_JSON_BYTES,
        registry=registry,
        errors=errors,
        expected_sha256=sha256,
    )


def _load_indexed_file_digest(
    root: _SourceRoot,
    reference_value: Any,
    *,
    label: str,
    max_bytes: int,
    registry: _FileRegistry,
    errors: list[str],
) -> str | None:
    reference = _file_reference(reference_value, label, errors)
    if reference is None:
        return None
    artifact_path, expected_sha256 = reference
    opened = _read_relative_file(
        root,
        artifact_path,
        label=label,
        max_bytes=max_bytes,
        registry=registry,
        errors=errors,
        capture_bytes=False,
    )
    if opened is None:
        return None
    _relative, _data, digest = opened
    if digest != expected_sha256:
        errors.append(f"{label} sha256 must match the indexed file bytes")
    return digest


def _load_indexed_release_artifact(
    root: _SourceRoot,
    reference_value: Any,
    *,
    label: str,
    registry: _FileRegistry,
    errors: list[str],
) -> str | None:
    return _load_indexed_file_digest(
        root,
        reference_value,
        label=label,
        max_bytes=MAX_RELEASE_ARTIFACT_BYTES,
        registry=registry,
        errors=errors,
    )


def _validate_release_receipt(
    payload: Any,
    *,
    target: str,
    expected_subject_sha256: str,
    label: str,
    expected_deployment_id: str,
    expected_environment: str,
    expected_release_manifest_digest_hex: str,
    expected_verification_key_fingerprint_hex: str,
    verification_receipt_authenticator: VerificationReceiptAuthenticator,
    now_unix: int,
    max_age_secs: int,
    errors: list[str],
) -> tuple[dict[str, bool], int | None] | None:
    receipt = _closed_object(payload, _RELEASE_RECEIPT_FIELDS, label, errors)
    if receipt is None:
        return None
    generated_at = _validate_common_source(
        receipt,
        schema=RELEASE_REHEARSAL_RECEIPT_SCHEMA,
        label=label,
        expected_deployment_id=expected_deployment_id,
        expected_environment=expected_environment,
        expected_release_manifest_digest_hex=expected_release_manifest_digest_hex,
        now_unix=now_unix,
        max_age_secs=max_age_secs,
        errors=errors,
    )
    if receipt.get("target") != target:
        errors.append(f"{label}.target must match its release-rehearsal row")
    subject_sha256 = _canonical_hex64(receipt.get("subject_sha256"))
    if subject_sha256 is None:
        errors.append(f"{label}.subject_sha256 must be non-zero lowercase SHA-256")
    elif subject_sha256 != expected_subject_sha256:
        errors.append(
            f"{label}.subject_sha256 must match the indexed release artifact"
        )
    verification_key_fingerprint = _canonical_hex64(
        receipt.get("verification_key_fingerprint_hex")
    )
    if verification_key_fingerprint is None:
        errors.append(
            f"{label}.verification_key_fingerprint_hex must be non-zero lowercase SHA-256"
        )
    elif verification_key_fingerprint != expected_verification_key_fingerprint_hex:
        errors.append(
            f"{label}.verification_key_fingerprint_hex must match the trusted verifier"
        )
    signature_algorithm = receipt.get("signature_algorithm")
    if signature_algorithm != "ed25519":
        errors.append(f"{label}.signature_algorithm must be `ed25519`")
    signature_hex = _canonical_hex128(receipt.get("signature_hex"))
    if signature_hex is None:
        errors.append(
            f"{label}.signature_hex must be a non-zero lowercase Ed25519 signature"
        )
    if (
        verification_key_fingerprint is not None
        and verification_key_fingerprint
        == expected_verification_key_fingerprint_hex
        and signature_algorithm == "ed25519"
        and signature_hex is not None
    ):
        try:
            signing_bytes = release_rehearsal_receipt_signing_bytes(receipt)
            authenticated = verification_receipt_authenticator(
                verification_key_fingerprint,
                signing_bytes,
                bytes.fromhex(signature_hex),
            )
        except Exception:
            errors.append(
                f"{label} signature could not be authenticated by the trusted verifier"
            )
        else:
            if authenticated is not True:
                errors.append(
                    f"{label} signature must authenticate with the trusted verifier"
                )
    operations = _closed_object(
        receipt.get("operations"),
        frozenset(_RELEASE_OPERATIONS),
        f"{label}.operations",
        errors,
    )
    if operations is None:
        return None
    derived: dict[str, bool] = {}
    for operation in _RELEASE_OPERATIONS:
        status_value = operations.get(operation)
        if status_value not in ("passed", "failed"):
            errors.append(
                f"{label}.operations.{operation} must be `passed` or `failed`"
            )
            continue
        derived[operation] = status_value == "passed"
    if len(derived) != len(_RELEASE_OPERATIONS):
        return None
    return derived, generated_at


def _validate_spdx(payload: Any, label: str, errors: list[str]) -> bool:
    if not isinstance(payload, Mapping):
        errors.append(f"{label} must be an SPDX JSON object")
        return False
    valid = True
    if payload.get("spdxVersion") != "SPDX-2.3":
        errors.append(f"{label}.spdxVersion must be `SPDX-2.3`")
        valid = False
    if payload.get("SPDXID") != "SPDXRef-DOCUMENT":
        errors.append(f"{label}.SPDXID must identify the SPDX document")
        valid = False
    if _canonical_string(payload.get("name")) is None:
        errors.append(f"{label}.name must be canonical")
        valid = False
    if not isinstance(payload.get("creationInfo"), Mapping):
        errors.append(f"{label}.creationInfo must be an object")
        valid = False
    packages = payload.get("packages")
    if not isinstance(packages, list) or not packages:
        errors.append(f"{label}.packages must be a non-empty array")
        valid = False
    return valid


def _severity_value(value: Any) -> str | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        if value < 0 or value > 10:
            return None
        score = float(value)
    elif isinstance(value, float):
        score = value
    elif isinstance(value, str):
        normalized = value.strip().lower()
        labels = {
            "critical": "critical",
            "high": "high",
            "medium": "other",
            "moderate": "other",
            "low": "other",
            "negligible": "other",
            "unknown": "other",
            "none": "other",
        }
        if normalized in labels:
            return labels[normalized]
        try:
            score = float(normalized)
        except ValueError:
            return None
    else:
        return None
    if not math.isfinite(score) or score < 0 or score > 10:
        return None
    if score >= 9:
        return "critical"
    if score >= 7:
        return "high"
    return "other"


def _property_severity(
    properties: Any,
    *,
    label: str,
    errors: list[str],
) -> str | None:
    if properties is None:
        return None
    if not isinstance(properties, Mapping):
        errors.append(f"{label} must be an object when present")
        return None
    values: list[str] = []
    for field in ("security-severity", "security_severity", "severity"):
        if field not in properties:
            continue
        severity = _severity_value(properties.get(field))
        if severity is None:
            errors.append(f"{label}.{field} must carry a recognized severity")
        else:
            values.append(severity)
    tags = properties.get("tags")
    if tags is not None:
        if not isinstance(tags, list):
            errors.append(f"{label}.tags must be an array when present")
        else:
            for tag in tags:
                severity = _severity_value(tag)
                if severity in {"critical", "high"}:
                    values.append(severity)
    if len(set(values)) > 1:
        errors.append(f"{label} must not carry conflicting severities")
        return None
    return values[0] if values else None


def _sarif_counts(
    payload: Any,
    label: str,
    errors: list[str],
) -> tuple[int, int] | None:
    if not isinstance(payload, Mapping):
        errors.append(f"{label} must be a SARIF JSON object")
        return None
    if payload.get("version") != "2.1.0":
        errors.append(f"{label}.version must be SARIF `2.1.0`")
    runs = payload.get("runs")
    if not isinstance(runs, list) or not runs:
        errors.append(f"{label}.runs must be a non-empty array")
        return None
    critical = 0
    high = 0
    valid = True
    for run_index, run_value in enumerate(runs):
        run_label = f"{label}.runs[{run_index}]"
        if not isinstance(run_value, Mapping):
            errors.append(f"{run_label} must be an object")
            valid = False
            continue
        tool = run_value.get("tool")
        driver = tool.get("driver") if isinstance(tool, Mapping) else None
        if not isinstance(driver, Mapping):
            errors.append(f"{run_label}.tool.driver must be an object")
            valid = False
            continue
        rules_value = driver.get("rules", [])
        if not isinstance(rules_value, list):
            errors.append(f"{run_label}.tool.driver.rules must be an array")
            valid = False
            rules_value = []
        rules: dict[str, str | None] = {}
        for rule_index, rule_value in enumerate(rules_value):
            rule_label = f"{run_label}.tool.driver.rules[{rule_index}]"
            if not isinstance(rule_value, Mapping):
                errors.append(f"{rule_label} must be an object")
                valid = False
                continue
            rule_id = _canonical_string(rule_value.get("id"))
            if rule_id is None:
                errors.append(f"{rule_label}.id must be canonical")
                valid = False
                continue
            if rule_id in rules:
                errors.append(f"{run_label}.tool.driver.rules must not duplicate ids")
                valid = False
                continue
            rules[rule_id] = _property_severity(
                rule_value.get("properties"),
                label=f"{rule_label}.properties",
                errors=errors,
            )
        results_value = run_value.get("results", [])
        if not isinstance(results_value, list):
            errors.append(f"{run_label}.results must be an array when present")
            valid = False
            continue
        for result_index, result_value in enumerate(results_value):
            result_label = f"{run_label}.results[{result_index}]"
            if not isinstance(result_value, Mapping):
                errors.append(f"{result_label} must be an object")
                valid = False
                continue
            rule_id = _canonical_string(result_value.get("ruleId"))
            if rule_id is None or rule_id not in rules:
                errors.append(f"{result_label}.ruleId must reference a declared rule")
                valid = False
                continue
            result_severity = _property_severity(
                result_value.get("properties"),
                label=f"{result_label}.properties",
                errors=errors,
            )
            rule_severity = rules[rule_id]
            if (
                result_severity is not None
                and rule_severity is not None
                and result_severity != rule_severity
            ):
                errors.append(f"{result_label} must not override its rule severity")
                valid = False
                continue
            severity = result_severity or rule_severity
            if severity is None:
                errors.append(f"{result_label} must carry an explicit severity")
                valid = False
                continue
            if severity == "critical":
                critical += 1
            elif severity == "high":
                high += 1
    return (critical, high) if valid else None


def _validate_bundle_json(payload: Any, label: str, errors: list[str]) -> bool:
    if isinstance(payload, Mapping):
        if payload:
            return True
    elif isinstance(payload, list) and payload:
        return True
    errors.append(f"{label} must be a non-empty JSON bundle")
    return False


def _validate_provenance_receipt(
    payload: Any,
    *,
    target: str,
    label: str,
    expected_deployment_id: str,
    expected_environment: str,
    expected_release_manifest_digest_hex: str,
    expected_certificate_identity: str,
    expected_oidc_issuer: str,
    expected_verification_key_fingerprint_hex: str,
    expected_subject_sha256: str,
    verification_receipt_authenticator: VerificationReceiptAuthenticator,
    attestation_bundle_sha256: str,
    cosign_bundle_sha256: str,
    sha256sums_sha256: str,
    sha256sums_cosign_bundle_sha256: str,
    now_unix: int,
    max_age_secs: int,
    errors: list[str],
) -> tuple[bool, bool, int | None] | None:
    receipt = _closed_object(payload, _PROVENANCE_RECEIPT_FIELDS, label, errors)
    if receipt is None:
        return None
    generated_at = _validate_common_source(
        receipt,
        schema=PROVENANCE_VERIFICATION_RECEIPT_SCHEMA,
        label=label,
        expected_deployment_id=expected_deployment_id,
        expected_environment=expected_environment,
        expected_release_manifest_digest_hex=expected_release_manifest_digest_hex,
        now_unix=now_unix,
        max_age_secs=max_age_secs,
        errors=errors,
    )
    if receipt.get("target") != target:
        errors.append(f"{label}.target must match its provenance row")
    if receipt.get("certificate_identity") != expected_certificate_identity:
        errors.append(f"{label}.certificate_identity must match the trusted identity")
    if receipt.get("oidc_issuer") != expected_oidc_issuer:
        errors.append(f"{label}.oidc_issuer must match the trusted issuer")
    subject_sha256 = _canonical_hex64(receipt.get("subject_sha256"))
    if subject_sha256 is None:
        errors.append(f"{label}.subject_sha256 must be non-zero lowercase SHA-256")
    elif subject_sha256 != expected_subject_sha256:
        errors.append(
            f"{label}.subject_sha256 must match the indexed release artifact"
        )
    verification_key_fingerprint = _canonical_hex64(
        receipt.get("verification_key_fingerprint_hex")
    )
    if verification_key_fingerprint is None:
        errors.append(
            f"{label}.verification_key_fingerprint_hex must be non-zero lowercase SHA-256"
        )
    elif verification_key_fingerprint != expected_verification_key_fingerprint_hex:
        errors.append(
            f"{label}.verification_key_fingerprint_hex must match the trusted verifier"
        )
    if receipt.get("attestation_bundle_sha256") != attestation_bundle_sha256:
        errors.append(
            f"{label}.attestation_bundle_sha256 must match the opened bundle"
        )
    if receipt.get("cosign_bundle_sha256") != cosign_bundle_sha256:
        errors.append(f"{label}.cosign_bundle_sha256 must match the opened bundle")
    if receipt.get("sha256sums_sha256") != sha256sums_sha256:
        errors.append(
            f"{label}.sha256sums_sha256 must match the opened SHA256SUMS"
        )
    if (
        receipt.get("sha256sums_cosign_bundle_sha256")
        != sha256sums_cosign_bundle_sha256
    ):
        errors.append(
            f"{label}.sha256sums_cosign_bundle_sha256 must match the opened bundle"
        )
    oidc_status = receipt.get("oidc_identity_status")
    cosign_status = receipt.get("cosign_provenance_status")
    if oidc_status not in ("verified", "failed"):
        errors.append(
            f"{label}.oidc_identity_status must be `verified` or `failed`"
        )
    if cosign_status not in ("verified", "failed"):
        errors.append(
            f"{label}.cosign_provenance_status must be `verified` or `failed`"
        )
    signature_algorithm = receipt.get("signature_algorithm")
    if signature_algorithm != "ed25519":
        errors.append(f"{label}.signature_algorithm must be `ed25519`")
    signature_hex = _canonical_hex128(receipt.get("signature_hex"))
    if signature_hex is None:
        errors.append(
            f"{label}.signature_hex must be a non-zero lowercase Ed25519 signature"
        )
    if (
        verification_key_fingerprint is not None
        and verification_key_fingerprint
        == expected_verification_key_fingerprint_hex
        and signature_algorithm == "ed25519"
        and signature_hex is not None
    ):
        try:
            signing_bytes = provenance_receipt_signing_bytes(receipt)
            authenticated = verification_receipt_authenticator(
                verification_key_fingerprint,
                signing_bytes,
                bytes.fromhex(signature_hex),
            )
        except Exception:
            errors.append(
                f"{label} signature could not be authenticated by the trusted verifier"
            )
        else:
            if authenticated is not True:
                errors.append(
                    f"{label} signature must authenticate with the trusted verifier"
                )
    if oidc_status not in ("verified", "failed") or cosign_status not in (
        "verified",
        "failed",
    ):
        return None
    return oidc_status == "verified", cosign_status == "verified", generated_at


def validate_supply_chain_sources(
    source_root: Path,
    *,
    expected_deployment_id: str,
    expected_environment: str,
    expected_release_manifest_digest_hex: str,
    expected_certificate_identity: str,
    expected_verification_key_fingerprint_hex: str,
    verification_receipt_authenticator: VerificationReceiptAuthenticator,
    now_unix: int,
    max_source_age_secs: int = DEFAULT_MAX_SOURCE_AGE_SECS,
    expected_oidc_issuer: str = DEFAULT_OIDC_ISSUER,
    release_rehearsal_path: str = DEFAULT_SOURCE_ARTIFACT_PATHS[
        "release_rehearsal"
    ],
    sbom_index_path: str = DEFAULT_SOURCE_ARTIFACT_PATHS["sbom_index"],
    vulnerability_report_path: str = DEFAULT_SOURCE_ARTIFACT_PATHS[
        "vulnerability_report"
    ],
    provenance_bundle_path: str = DEFAULT_SOURCE_ARTIFACT_PATHS[
        "provenance_bundle"
    ],
) -> tuple[SupplyChainSourceResult | None, list[str]]:
    """Open, bind, and derive one exact five-target supply-chain source bundle.

    The returned error list is deterministic and payload-free.  A result is
    returned only when every top-level source, indexed receipt, SBOM, SARIF
    report, and provenance bundle validates.  The receipt authenticator is the
    explicit trust boundary for release-rehearsal and externally verified
    OIDC/cosign results, and must verify domain-separated Ed25519 signatures
    against the expected key fingerprint.
    """

    errors: list[str] = []
    deployment_id = _canonical_string(expected_deployment_id)
    environment = _canonical_string(expected_environment)
    release_manifest_digest = _canonical_hex64(
        expected_release_manifest_digest_hex
    )
    certificate_identity = _canonical_string(expected_certificate_identity)
    verification_key_fingerprint = _canonical_hex64(
        expected_verification_key_fingerprint_hex
    )
    oidc_issuer = _canonical_string(expected_oidc_issuer)
    if deployment_id is None:
        errors.append("expected deployment_id must be canonical")
    if environment is None:
        errors.append("expected environment must be canonical")
    if release_manifest_digest is None:
        errors.append(
            "expected release manifest digest must be non-zero lowercase SHA-256"
        )
    if certificate_identity is None:
        errors.append("expected certificate identity must be canonical")
    if verification_key_fingerprint is None:
        errors.append(
            "expected verification key fingerprint must be non-zero lowercase SHA-256"
        )
    if not callable(verification_receipt_authenticator):
        errors.append("verification receipt authenticator must be callable")
    if oidc_issuer is None:
        errors.append("expected OIDC issuer must be canonical")
    if (
        not isinstance(now_unix, int)
        or isinstance(now_unix, bool)
        or now_unix <= 0
        or now_unix > MAX_TIMESTAMP
    ):
        errors.append("reviewed source-validation clock must be positive and bounded")
    if (
        not isinstance(max_source_age_secs, int)
        or isinstance(max_source_age_secs, bool)
        or max_source_age_secs < 0
        or max_source_age_secs > MAX_TIMESTAMP
    ):
        errors.append("maximum source age must be a non-negative bounded integer")
    root = _prepare_source_root(source_root, errors)
    if errors or root is None:
        return None, errors
    assert deployment_id is not None
    assert environment is not None
    assert release_manifest_digest is not None
    assert certificate_identity is not None
    assert verification_key_fingerprint is not None
    assert oidc_issuer is not None

    registry = _FileRegistry()
    source_specs = (
        (
            "release_rehearsal",
            release_rehearsal_path,
            RELEASE_REHEARSAL_SCHEMA,
            _RELEASE_REHEARSAL_FIELDS,
        ),
        ("sbom_index", sbom_index_path, SBOM_INDEX_SCHEMA, _SBOM_INDEX_FIELDS),
        (
            "vulnerability_report",
            vulnerability_report_path,
            VULNERABILITY_REPORT_SCHEMA,
            _VULNERABILITY_INDEX_FIELDS,
        ),
        (
            "provenance_bundle",
            provenance_bundle_path,
            PROVENANCE_BUNDLE_SCHEMA,
            _PROVENANCE_INDEX_FIELDS,
        ),
    )
    sources: dict[str, _LoadedJson] = {}
    source_timestamps: list[int] = []
    bindings: list[SourceArtifactBinding] = []
    for kind, artifact_path, schema, fields in source_specs:
        loaded = _load_json_file(
            root,
            artifact_path,
            label=f"{kind} source",
            max_bytes=MAX_SOURCE_ARTIFACT_BYTES,
            registry=registry,
            errors=errors,
        )
        if loaded is None:
            continue
        bindings.append(
            SourceArtifactBinding(kind, loaded.artifact_path, loaded.sha256)
        )
        payload = _closed_object(
            loaded.payload,
            fields,
            f"{kind} source",
            errors,
        )
        if payload is None:
            continue
        generated_at = _validate_common_source(
            payload,
            schema=schema,
            label=f"{kind} source",
            expected_deployment_id=deployment_id,
            expected_environment=environment,
            expected_release_manifest_digest_hex=release_manifest_digest,
            now_unix=now_unix,
            max_age_secs=max_source_age_secs,
            errors=errors,
        )
        if generated_at is not None:
            source_timestamps.append(generated_at)
        sources[kind] = _LoadedJson(
            loaded.artifact_path,
            loaded.sha256,
            payload,
        )

    release_results: list[dict[str, bool] | None] = [None] * len(
        REQUIRED_RELEASE_TARGETS
    )
    release_subjects: list[str | None] = [None] * len(REQUIRED_RELEASE_TARGETS)
    sbom_results: list[bool | None] = [None] * len(REQUIRED_RELEASE_TARGETS)
    vulnerability_results: list[tuple[int, int] | None] = [None] * len(
        REQUIRED_RELEASE_TARGETS
    )
    provenance_results: list[tuple[bool, bool] | None] = [None] * len(
        REQUIRED_RELEASE_TARGETS
    )

    release_source = sources.get("release_rehearsal")
    if release_source is not None:
        rows = _target_rows(
            release_source.payload.get("targets"),
            fields=_RELEASE_REHEARSAL_TARGET_FIELDS,
            label="release_rehearsal source.targets",
            errors=errors,
        )
        for index, target in enumerate(REQUIRED_RELEASE_TARGETS):
            if index >= len(rows) or rows[index] is None:
                continue
            release_subject = _load_indexed_release_artifact(
                root,
                rows[index].get("release_artifact"),
                label=f"release_rehearsal release artifact[{index}]",
                registry=registry,
                errors=errors,
            )
            if release_subject is None:
                continue
            release_subjects[index] = release_subject
            loaded = _load_indexed_json(
                root,
                rows[index].get("receipt"),
                label=f"release_rehearsal receipt[{index}]",
                registry=registry,
                errors=errors,
            )
            if loaded is None:
                continue
            validated = _validate_release_receipt(
                loaded.payload,
                target=target,
                expected_subject_sha256=release_subject,
                label=f"release_rehearsal receipt[{index}]",
                expected_deployment_id=deployment_id,
                expected_environment=environment,
                expected_release_manifest_digest_hex=release_manifest_digest,
                expected_verification_key_fingerprint_hex=(
                    verification_key_fingerprint
                ),
                verification_receipt_authenticator=(
                    verification_receipt_authenticator
                ),
                now_unix=now_unix,
                max_age_secs=max_source_age_secs,
                errors=errors,
            )
            if validated is not None:
                release_results[index], generated_at = validated
                if generated_at is not None:
                    source_timestamps.append(generated_at)

    sbom_source = sources.get("sbom_index")
    if sbom_source is not None:
        source_sbom = _load_indexed_json(
            root,
            sbom_source.payload.get("source_sbom"),
            label="sbom_index source SBOM",
            registry=registry,
            errors=errors,
        )
        source_sbom_valid = (
            source_sbom is not None
            and _validate_spdx(source_sbom.payload, "sbom_index source SBOM", errors)
        )
        rows = _target_rows(
            sbom_source.payload.get("targets"),
            fields=_SBOM_TARGET_FIELDS,
            label="sbom_index source.targets",
            errors=errors,
        )
        for index, _target in enumerate(REQUIRED_RELEASE_TARGETS):
            if index >= len(rows) or rows[index] is None:
                continue
            platform_sbom = _load_indexed_json(
                root,
                rows[index].get("platform_sbom"),
                label=f"sbom_index platform SBOM[{index}]",
                registry=registry,
                errors=errors,
            )
            platform_valid = (
                platform_sbom is not None
                and _validate_spdx(
                    platform_sbom.payload,
                    f"sbom_index platform SBOM[{index}]",
                    errors,
                )
            )
            sbom_results[index] = source_sbom_valid and platform_valid

    vulnerability_source = sources.get("vulnerability_report")
    if vulnerability_source is not None:
        source_report = _load_indexed_json(
            root,
            vulnerability_source.payload.get("source_report"),
            label="vulnerability_report source report",
            registry=registry,
            errors=errors,
        )
        source_counts = (
            None
            if source_report is None
            else _sarif_counts(
                source_report.payload,
                "vulnerability_report source report",
                errors,
            )
        )
        rows = _target_rows(
            vulnerability_source.payload.get("targets"),
            fields=_VULNERABILITY_TARGET_FIELDS,
            label="vulnerability_report source.targets",
            errors=errors,
        )
        for index, _target in enumerate(REQUIRED_RELEASE_TARGETS):
            if index >= len(rows) or rows[index] is None:
                continue
            platform_report = _load_indexed_json(
                root,
                rows[index].get("platform_report"),
                label=f"vulnerability_report platform report[{index}]",
                registry=registry,
                errors=errors,
            )
            platform_counts = (
                None
                if platform_report is None
                else _sarif_counts(
                    platform_report.payload,
                    f"vulnerability_report platform report[{index}]",
                    errors,
                )
            )
            if source_counts is not None and platform_counts is not None:
                vulnerability_results[index] = (
                    source_counts[0] + platform_counts[0],
                    source_counts[1] + platform_counts[1],
                )

    provenance_source = sources.get("provenance_bundle")
    if provenance_source is not None:
        if (
            provenance_source.payload.get("certificate_identity")
            != certificate_identity
        ):
            errors.append(
                "provenance_bundle source.certificate_identity must match the trusted identity"
            )
        if provenance_source.payload.get("oidc_issuer") != oidc_issuer:
            errors.append(
                "provenance_bundle source.oidc_issuer must match the trusted issuer"
            )
        if (
            provenance_source.payload.get("verification_key_fingerprint_hex")
            != verification_key_fingerprint
        ):
            errors.append(
                "provenance_bundle source.verification_key_fingerprint_hex "
                "must match the trusted verifier"
            )
        rows = _target_rows(
            provenance_source.payload.get("targets"),
            fields=_PROVENANCE_TARGET_FIELDS,
            label="provenance_bundle source.targets",
            errors=errors,
        )
        for index, target in enumerate(REQUIRED_RELEASE_TARGETS):
            if index >= len(rows) or rows[index] is None:
                continue
            expected_subject = release_subjects[index]
            if expected_subject is None:
                errors.append(
                    f"provenance_bundle target[{index}] requires an indexed release artifact"
                )
                continue
            row = rows[index]
            attestation = _load_indexed_json(
                root,
                row.get("attestation_bundle"),
                label=f"provenance_bundle attestation[{index}]",
                registry=registry,
                errors=errors,
            )
            cosign = _load_indexed_json(
                root,
                row.get("cosign_bundle"),
                label=f"provenance_bundle cosign bundle[{index}]",
                registry=registry,
                errors=errors,
            )
            sha256sums_sha256 = _load_indexed_file_digest(
                root,
                row.get("sha256sums"),
                label=f"provenance_bundle SHA256SUMS[{index}]",
                max_bytes=MAX_INDEXED_JSON_BYTES,
                registry=registry,
                errors=errors,
            )
            sha256sums_cosign_bundle = _load_indexed_json(
                root,
                row.get("sha256sums_cosign_bundle"),
                label=(
                    "provenance_bundle SHA256SUMS cosign bundle"
                    f"[{index}]"
                ),
                registry=registry,
                errors=errors,
            )
            receipt = _load_indexed_json(
                root,
                row.get("verification_receipt"),
                label=f"provenance_bundle verification receipt[{index}]",
                registry=registry,
                errors=errors,
            )
            if attestation is not None:
                _validate_bundle_json(
                    attestation.payload,
                    f"provenance_bundle attestation[{index}]",
                    errors,
                )
            if cosign is not None:
                _validate_bundle_json(
                    cosign.payload,
                    f"provenance_bundle cosign bundle[{index}]",
                    errors,
                )
            if sha256sums_cosign_bundle is not None:
                _validate_bundle_json(
                    sha256sums_cosign_bundle.payload,
                    (
                        "provenance_bundle SHA256SUMS cosign bundle"
                        f"[{index}]"
                    ),
                    errors,
                )
            if (
                attestation is None
                or cosign is None
                or sha256sums_sha256 is None
                or sha256sums_cosign_bundle is None
                or receipt is None
            ):
                continue
            validated = _validate_provenance_receipt(
                receipt.payload,
                target=target,
                label=f"provenance_bundle verification receipt[{index}]",
                expected_deployment_id=deployment_id,
                expected_environment=environment,
                expected_release_manifest_digest_hex=release_manifest_digest,
                expected_certificate_identity=certificate_identity,
                expected_oidc_issuer=oidc_issuer,
                expected_verification_key_fingerprint_hex=(
                    verification_key_fingerprint
                ),
                expected_subject_sha256=expected_subject,
                verification_receipt_authenticator=(
                    verification_receipt_authenticator
                ),
                attestation_bundle_sha256=attestation.sha256,
                cosign_bundle_sha256=cosign.sha256,
                sha256sums_sha256=sha256sums_sha256,
                sha256sums_cosign_bundle_sha256=(
                    sha256sums_cosign_bundle.sha256
                ),
                now_unix=now_unix,
                max_age_secs=max_source_age_secs,
                errors=errors,
            )
            if validated is not None:
                oidc_verified, cosign_verified, generated_at = validated
                provenance_results[index] = (oidc_verified, cosign_verified)
                if generated_at is not None:
                    source_timestamps.append(generated_at)

    if len(bindings) != len(SOURCE_ARTIFACT_KINDS):
        errors.append("source bundle must contain all four canonical source artifacts")
    if errors:
        return None, errors
    if (
        any(result is None for result in release_results)
        or any(result is None for result in sbom_results)
        or any(result is None for result in vulnerability_results)
        or any(result is None for result in provenance_results)
    ):
        return None, ["source bundle did not derive every canonical target result"]

    target_results: list[SupplyChainTargetResult] = []
    for index, target in enumerate(REQUIRED_RELEASE_TARGETS):
        release = release_results[index]
        sbom = sbom_results[index]
        vulnerabilities = vulnerability_results[index]
        provenance = provenance_results[index]
        assert release is not None
        assert sbom is not None
        assert vulnerabilities is not None
        assert provenance is not None
        target_results.append(
            SupplyChainTargetResult(
                target=target,
                binary_smoke_passed=release["binary_smoke"],
                deterministic_archive_replay_passed=release[
                    "deterministic_archive_replay"
                ],
                installation_verified=release["installation"],
                rollback_verified=release["rollback"],
                yank_verified=release["yank"],
                sbom_generated=sbom,
                critical_vulnerability_count=vulnerabilities[0],
                high_vulnerability_count=vulnerabilities[1],
                oidc_identity_verified=provenance[0],
                cosign_provenance_verified=provenance[1],
            )
        )

    binding_by_kind = {binding.kind: binding for binding in bindings}
    return (
        SupplyChainSourceResult(
            generated_at_unix=min(source_timestamps),
            deployment_id=deployment_id,
            environment=environment,
            release_manifest_digest_hex=release_manifest_digest,
            source_artifacts=tuple(binding_by_kind[kind] for kind in SOURCE_ARTIFACT_KINDS),
            target_results=tuple(target_results),
            sbom_index_digest_hex=binding_by_kind["sbom_index"].sha256,
            vulnerability_report_digest_hex=binding_by_kind[
                "vulnerability_report"
            ].sha256,
            provenance_bundle_digest_hex=binding_by_kind[
                "provenance_bundle"
            ].sha256,
        ),
        [],
    )


__all__ = [
    "DEFAULT_MAX_SOURCE_AGE_SECS",
    "DEFAULT_OIDC_ISSUER",
    "DEFAULT_SOURCE_ARTIFACT_PATHS",
    "PROVENANCE_BUNDLE_SCHEMA",
    "PROVENANCE_VERIFICATION_RECEIPT_SCHEMA",
    "RELEASE_REHEARSAL_RECEIPT_SCHEMA",
    "RELEASE_REHEARSAL_SCHEMA",
    "REQUIRED_RELEASE_TARGETS",
    "SBOM_INDEX_SCHEMA",
    "SOURCE_ARTIFACT_KINDS",
    "SourceArtifactBinding",
    "SupplyChainSourceResult",
    "SupplyChainTargetResult",
    "VULNERABILITY_REPORT_SCHEMA",
    "VerificationReceiptAuthenticator",
    "provenance_receipt_signing_bytes",
    "release_rehearsal_receipt_signing_bytes",
    "validate_supply_chain_sources",
]
