"""Authenticate exact SF-11 manifest inputs before hardware release qualification.

The caller independently supplies a SHA-256-pinned source-context file. That
closed context names exact manifest, raw signature/key, policy, custody trust,
completed-operation state, receipt, and native verifier inputs. It is never
read from a canary artifact. Data files are bounded, read through anchored
no-follow descriptors, snapshotted privately, and rechecked after verification.

Raw Ed25519 verification is followed by the distinct native ReleaseManifest
receipt verifier, which authenticates independent policy/attester/state-observer
trust, fresh ACTIVE custody and the exact completed operation. Missing native
support, malformed output or a failed receipt always rejects qualification.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import secrets
import stat
import tempfile
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Any, NoReturn

from release_manifest_signing import (
    MAX_MANIFEST_SIZE,
    NATIVE_VERIFIER_PROTOCOL,
    ReleaseManifestSignatureError,
    _open_release_output_parent,
    verify_release_manifest,
)
from sorafs_evidence_json import read_evidence_bytes
from sorafs_reference_sdk_receipt_verifier import (
    INPUT_LIMITS, ReceiptVerifierError, verify_receipt_snapshots,
)


SOURCE_CONTEXT_SCHEMA = "sorafs.reference_sdk.signed_manifest_sources.v1"
MAX_SOURCE_CONTEXT_BYTES = 32 * 1024
SOURCE_LIMITS = INPUT_LIMITS.copy()
SOURCE_FIELDS = frozenset(SOURCE_LIMITS) | {"native_verifier"}
_HEX64 = re.compile(r"[0-9a-f]{64}\Z")
NATIVE_RECEIPT_SCHEMA = "sorafs.release_manifest_receipt_verification.v1"
NATIVE_SOURCE_FIELDS = {
    "manifest_sha256": "manifest", "signature_sha256": "signature",
    "public_key_fingerprint_sha256": "public_key", "signer_policy_sha256": "signer_policy",
    "custody_trust_sha256": "custody_trust", "completed_operation_state_sha256": "completed_operation_state",
    "operation_receipt_sha256": "operation_receipt",
}
NATIVE_TO_CANARY = {
    **{field: field for field in NATIVE_SOURCE_FIELDS},
    "manifest_sha256": "manifest_digest_hex", "public_key_fingerprint_sha256": "public_key_fingerprint_hex",
    "policy_digest": "policy_digest_hex", "backend": "signing_backend",
    **{field: field for field in (
        "manifest_size", "operation_id", "custody_record_digest", "key_revision", "policy_revision",
        "service_id", "administrator_id", "role", "deployment_id", "chain_id", "network_id",
        "finalized_height", "finalized_block_hash",
    )},
}
NATIVE_RECEIPT_FIELDS = frozenset(NATIVE_TO_CANARY) | {"schema", "status", "verified_at_unix_ms"}
DERIVED_CANARY_FIELDS = {
    "signature_algorithm": "ed25519", "manifest_signature_verified": True,
    "hardware_custody_verified": True, "completed_operation_verified": True,
    "state_observation_verified": True, "raw_manifest_included": False,
    "receipt_verification_schema": NATIVE_RECEIPT_SCHEMA,
}
SIGNED_MANIFEST_CANARY_FIELDS = tuple(sorted(
    (set(NATIVE_TO_CANARY.values()) | set(DERIVED_CANARY_FIELDS)
    | {"source_context_sha256", "native_verifier_sha256"})
    - {"deployment_id"}
))


class SignedManifestSourceError(ValueError):
    """The independent source context or an exact verification input failed."""


@dataclass(frozen=True)
class VerifiedManifestSignatureSources:
    """Exact raw-signature result; this deliberately carries no custody claim."""

    source_context_sha256: str
    manifest_sha256: str
    manifest_size: int
    signature_sha256: str
    public_key_sha256: str
    native_verifier_sha256: str


@dataclass(frozen=True)
class VerifiedSignedManifestSources:
    """Exact closed native result after source, signature and custody authentication."""

    source_context_sha256: str
    native_verifier_sha256: str
    native_fields: tuple[tuple[str, str | int], ...]

    def canary_fields(self) -> dict[str, str | int | bool]:
        """Derive persistent facts; the verifier's current clock is checked per use."""
        native_fields = dict(self.native_fields)
        return {
            **{target: native_fields[source] for source, target in NATIVE_TO_CANARY.items()},
            **DERIVED_CANARY_FIELDS,
            "source_context_sha256": self.source_context_sha256,
            "native_verifier_sha256": self.native_verifier_sha256,
        }


@dataclass(frozen=True)
class _Source:
    path: Path
    payload: bytes
    identity: tuple[int, ...]
    maximum: int


class _PinnedSourceParents:
    """Retain every ancestor inode until the complete source verification finishes."""

    def __init__(self) -> None:
        self.parents: dict[Path, tuple[tuple[tuple[int, int], ...], tuple[int, ...]]] = {}

    def bind(self, path: Path) -> None:
        if path.parent not in self.parents:
            # Reuse the native release helper's descriptor-relative no-follow
            # traversal. Holding descriptors prevents removed inode reuse.
            _, lineage, descriptors = _open_release_output_parent(path.parent)
            self.parents[path.parent] = (lineage, descriptors)

    def assert_unchanged(self) -> None:
        for parent, (expected, _) in self.parents.items():
            _, lineage, descriptors = _open_release_output_parent(parent)
            for descriptor in reversed(descriptors):
                os.close(descriptor)
            if lineage != expected:
                raise SignedManifestSourceError("source ancestor changed during verification")

    def close(self) -> None:
        for _, descriptors in self.parents.values():
            for descriptor in reversed(descriptors):
                os.close(descriptor)


def _identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev, metadata.st_ino, metadata.st_size,
        metadata.st_mtime_ns, metadata.st_ctime_ns, metadata.st_nlink,
        metadata.st_mode, metadata.st_uid,
    )


def _require_digest(value: Any, label: str) -> str:
    if not isinstance(value, str) or _HEX64.fullmatch(value) is None or value == "0" * 64:
        raise SignedManifestSourceError(f"{label} requires a nonzero lowercase SHA-256")
    return value


def _require_path(value: Any, label: str) -> Path:
    if (
        not isinstance(value, str) or not value or value != value.strip()
        or "\x00" in value or not os.path.isabs(value)
        or os.path.normpath(value) != value
    ):
        raise SignedManifestSourceError(f"{label} requires an exact absolute path")
    return Path(value)


def _metadata(path: Path, label: str) -> os.stat_result:
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1
        or metadata.st_mode & 0o022 or metadata.st_uid not in {os.getuid(), 0}
    ):
        raise SignedManifestSourceError(f"{label} must be an owned single-link regular source without group/world write permission")
    return metadata


def _load_source(path: Path, expected: str, maximum: int, label: str) -> _Source:
    before = _metadata(path, label)
    payload = read_evidence_bytes(path, maximum)
    after = _metadata(path, label)
    if _identity(before) != _identity(after):
        raise SignedManifestSourceError(f"{label} changed during its stable read")
    if not payload or not secrets.compare_digest(hashlib.sha256(payload).hexdigest(), expected):
        raise SignedManifestSourceError(f"{label} does not match its independently pinned source digest")
    return _Source(path, payload, _identity(after), maximum)


def _recheck_source(source: _Source, label: str) -> None:
    current = _load_source(
        source.path, hashlib.sha256(source.payload).hexdigest(), source.maximum, label,
    )
    if current.identity != source.identity or current.payload != source.payload:
        raise SignedManifestSourceError(f"{label} changed during source verification")


def _closed_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise SignedManifestSourceError("source context contains a duplicate JSON field")
        result[key] = value
    return result


def _reject_constant(_value: str) -> NoReturn:
    raise SignedManifestSourceError("source context contains a non-JSON constant")


def _write_snapshot(path: Path, payload: bytes) -> None:
    descriptor = os.open(
        path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o400,
    )
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise SignedManifestSourceError("private source snapshot write was incomplete")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


@contextmanager
def _verified_manifest_source_snapshot(
    context_path: Path | None,
    trusted_context_sha256: str | None,
) -> Iterator[tuple[VerifiedManifestSignatureSources, dict[str, Path], Path, dict[str, str]]]:
    """Keep the exact private snapshots alive through every required authenticator."""

    if context_path is None or trusted_context_sha256 is None:
        raise SignedManifestSourceError("signed_manifest requires an independently pinned source context")
    parents = _PinnedSourceParents()
    try:
        context_digest = _require_digest(trusted_context_sha256, "source context")
        context_path = _require_path(os.fspath(context_path), "source context")
        parents.bind(context_path)
        context_source = _load_source(context_path, context_digest, MAX_SOURCE_CONTEXT_BYTES, "source context")
        context = json.loads(
            context_source.payload.decode("utf-8"), object_pairs_hook=_closed_pairs,
            parse_constant=_reject_constant,
        )
        if not isinstance(context, dict) or set(context) != {"schema", "sources"} or context["schema"] != SOURCE_CONTEXT_SCHEMA:
            raise SignedManifestSourceError("source context must use the exact closed V1 schema")
        references = context["sources"]
        if not isinstance(references, dict) or set(references) != SOURCE_FIELDS:
            raise SignedManifestSourceError("source context must name every exact manifest, policy, custody, operation and verifier input")
        paths: dict[str, Path] = {}
        digests: dict[str, str] = {}
        for name, reference in references.items():
            if not isinstance(reference, dict) or set(reference) != {"path", "sha256"}:
                raise SignedManifestSourceError(f"{name} source reference must contain only path and sha256")
            paths[name] = _require_path(reference["path"], name)
            digests[name] = _require_digest(reference["sha256"], name)
        if len(set(paths.values()) | {context_path}) != len(paths) + 1:
            raise SignedManifestSourceError("source context and all source paths must be distinct")
        for path in paths.values():
            parents.bind(path)
        sources = {
            name: _load_source(paths[name], digests[name], maximum, name)
            for name, maximum in SOURCE_LIMITS.items()
        }
        if len(sources["signature"].payload) != 64 or len(sources["public_key"].payload) != 32:
            raise SignedManifestSourceError("manifest signature/key must be exactly 64/32 raw bytes")
        with tempfile.TemporaryDirectory(prefix=".sf11-manifest-sources-", dir=context_path.parent) as staging:
            snapshots = {name: Path(staging) / name for name in SOURCE_LIMITS}
            for name, source in sources.items():
                _write_snapshot(snapshots[name], source.payload)
                parents.bind(snapshots[name])
            private_sources = {
                name: _load_source(snapshots[name], digests[name], source.maximum, f"{name} private snapshot")
                for name, source in sources.items()
            }
            verification = verify_release_manifest(
                snapshots["manifest"], snapshots["signature"], snapshots["public_key"],
                digests["public_key"], paths["native_verifier"], digests["native_verifier"],
            )
            if (
                verification.get("signature_verified") is not True
                or verification.get("signature_algorithm") != "ed25519"
                or verification.get("native_verifier_protocol") != NATIVE_VERIFIER_PROTOCOL
                or verification.get("native_verifier_sha256") != digests["native_verifier"]
                or verification.get("manifest_sha256") != digests["manifest"]
                or verification.get("manifest_size") != len(sources["manifest"].payload)
                or verification.get("signer_fingerprint_sha256") != digests["public_key"]
            ):
                raise SignedManifestSourceError("native signature result does not authenticate the exact pinned sources")
            result = VerifiedManifestSignatureSources(
                context_digest, digests["manifest"], len(sources["manifest"].payload),
                digests["signature"], digests["public_key"], digests["native_verifier"],
            )
            try:
                parents.assert_unchanged()
                yield result, snapshots, paths["native_verifier"], digests
            finally:
                for name, source in sources.items():
                    _recheck_source(private_sources[name], f"{name} private snapshot")
                    _recheck_source(source, name)
                _recheck_source(context_source, "source context")
                parents.assert_unchanged()
    except SignedManifestSourceError:
        raise
    except (OSError, ValueError, RuntimeError, RecursionError, TypeError, ReleaseManifestSignatureError):
        # Source content, paths, and native stderr are runtime-only diagnostics.
        raise SignedManifestSourceError("signed-manifest source or native signature verification failed") from None
    finally:
        parents.close()


def verify_signed_manifest_source_files(
    context_path: Path | None,
    trusted_context_sha256: str | None,
) -> VerifiedManifestSignatureSources:
    """Verify pinned source files and the native raw signature, without a custody claim."""

    with _verified_manifest_source_snapshot(context_path, trusted_context_sha256) as (result, _snapshots, _verifier, _digests):
        return result


def authenticate_signed_manifest_sources(
    context_path: Path | None,
    trusted_context_sha256: str | None,
    now_unix: int,
) -> VerifiedSignedManifestSources:
    """Authenticate a fresh purpose-specific receipt over the same pinned bytes."""
    if type(now_unix) is not int or not 0 < now_unix <= ((1 << 64) - 1) // 1000:
        raise SignedManifestSourceError("signed-manifest trusted Unix clock is outside the canonical millisecond range")
    now_unix_ms = now_unix * 1000
    with _verified_manifest_source_snapshot(context_path, trusted_context_sha256) as (raw, snapshots, verifier, digests):
        try:
            output = verify_receipt_snapshots(snapshots, digests, verifier, raw.native_verifier_sha256, now_unix_ms)
            result = json.loads(output.decode("utf-8"), object_pairs_hook=_closed_pairs, parse_constant=_reject_constant)
            if not isinstance(result, dict) or set(result) != NATIVE_RECEIPT_FIELDS:
                raise SignedManifestSourceError("native release receipt result must use the exact closed schema")
            if result["schema"] != NATIVE_RECEIPT_SCHEMA or result["status"] != "verified" or result["role"] != "release_manifest" or result["backend"] != "hardware":
                raise SignedManifestSourceError("native result does not authenticate the ReleaseManifest hardware purpose")
            for field, source in NATIVE_SOURCE_FIELDS.items():
                if _require_digest(result[field], field) != digests[source]:
                    raise SignedManifestSourceError("native receipt result differs from the exact pinned sources")
            for field in ("operation_id", "custody_record_digest", "policy_digest", "network_id", "finalized_block_hash"):
                _require_digest(result[field], field)
            for field in ("manifest_size", "key_revision", "policy_revision", "finalized_height", "verified_at_unix_ms"):
                if type(result[field]) is not int or not 0 < result[field] <= (1 << 64) - 1:
                    raise SignedManifestSourceError("native receipt result has a noncanonical positive integer")
            if result["manifest_size"] != raw.manifest_size or result["verified_at_unix_ms"] != now_unix_ms:
                raise SignedManifestSourceError("native receipt result differs from the pinned size or independent clock")
            for field in ("service_id", "administrator_id", "deployment_id"):
                value = result[field]
                if not isinstance(value, str) or re.fullmatch(r"[A-Za-z0-9._:-]{1,128}", value) is None or "test" in value.lower():
                    raise SignedManifestSourceError("native receipt result contains an invalid signer or deployment identity")
            chain = result["chain_id"]
            if not isinstance(chain, str) or re.fullmatch(r"[A-Za-z0-9](?:[A-Za-z0-9._:-]{0,126}[A-Za-z0-9])?", chain) is None:
                raise SignedManifestSourceError("native receipt result contains an invalid chain identity")
            if result["service_id"] == result["administrator_id"]:
                raise SignedManifestSourceError("native receipt signer and administrator must be independent")
            return VerifiedSignedManifestSources(raw.source_context_sha256, raw.native_verifier_sha256, tuple(sorted(result.items())))
        except SignedManifestSourceError:
            raise
        except (ReceiptVerifierError, OSError, ValueError, TypeError, RecursionError):
            raise SignedManifestSourceError("native ReleaseManifest receipt verification failed or is unavailable") from None
