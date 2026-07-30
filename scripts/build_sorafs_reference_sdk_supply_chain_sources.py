#!/usr/bin/env python3
"""Assemble and verify canonical SF-11 supply-chain source indexes.

The release workflow already owns the immutable release archives, package
replay summaries, SPDX documents, SARIF reports, GitHub attestations, cosign
bundles, and authenticated release manifest. This helper binds those files into
the four schema-closed source indexes consumed by the SF-11 canary builder.

Release rehearsal receipts and Ed25519-signed provenance verification receipts
remain external runtime evidence. This helper copies and verifies them; it
never creates success receipts or receives signing material. The external root
must contain exactly:

``release-rehearsal/<target>.json`` and
``provenance-verification/<target>.json``

for each canonical native target. Provenance receipts sign the exact bytes from
``provenance_receipt_signing_bytes`` in
``sorafs_reference_sdk_supply_chain.py``.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import secrets
import shutil
import stat
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sccp_release_common import verify_ed25519  # noqa: E402
from sorafs_evidence_validation import (  # noqa: E402
    require_rollout_deployment_id,
    require_rollout_environment,
)
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
    """Exact workflow-owned files for one native release target."""

    target: str
    archive: Path
    archive_sha256: str
    source_sbom: Path
    source_report: Path
    platform_sbom: Path
    platform_report: Path
    attestation_bundle: Path
    cosign_bundle: Path


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


def _absolute_lexical(path: Path) -> Path:
    return path if path.is_absolute() else Path.cwd() / path


def _require_directory(path: Path, *, label: str) -> Path:
    absolute = _absolute_lexical(path)
    try:
        resolved = absolute.resolve(strict=True)
        metadata = absolute.lstat()
    except (OSError, RuntimeError) as error:
        raise SourceBuildError(f"{label} must be an existing directory") from error
    if resolved != absolute or stat.S_ISLNK(metadata.st_mode):
        _fail(f"{label} must not contain symlink or non-canonical components")
    if not stat.S_ISDIR(metadata.st_mode):
        _fail(f"{label} must be an existing directory")
    return resolved


def _open_regular(path: Path, *, label: str) -> tuple[int, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise SourceBuildError(f"{label} must be an existing regular file") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        _fail(f"{label} must be a non-symlink regular file")
    if before.st_nlink != 1:
        _fail(f"{label} must have exactly one hard link")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise SourceBuildError(f"{label} could not be opened safely") from error
    opened = os.fstat(descriptor)
    if _stat_identity(opened) != _stat_identity(before):
        os.close(descriptor)
        _fail(f"{label} changed while it was opened")
    return descriptor, opened


def _read_regular_bytes(path: Path, *, label: str, max_bytes: int) -> bytes:
    descriptor, opened = _open_regular(path, label=label)
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
    path: Path,
    *,
    label: str,
    max_bytes: int,
) -> tuple[str, int]:
    descriptor, opened = _open_regular(path, label=label)
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


def _load_json(path: Path, *, label: str, max_bytes: int) -> tuple[Any, bytes]:
    raw = _read_regular_bytes(path, label=label, max_bytes=max_bytes)
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


def _write_exclusive(path: Path, payload: bytes) -> None:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as error:
        raise SourceBuildError("generated source output already exists") from error
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise OSError("short write")
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


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


def _relative_path(root: Path, path: Path, *, label: str) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError as error:
        raise SourceBuildError(f"{label} must remain under the source root") from error


def _file_reference(root: Path, path: Path, *, label: str) -> dict[str, str]:
    digest, _size = _hash_regular_file(
        path,
        label=label,
        max_bytes=MAX_ARCHIVE_BYTES,
    )
    return {
        "artifact_path": _relative_path(root, path, label=label),
        "sha256": digest,
    }


def _candidate_evidence(
    root: Path,
    *,
    version: str,
    target: str,
) -> CandidateEvidence:
    candidate = root / "release-candidates" / f"sorafs-cli-{version}-{target}"
    _require_directory(candidate, label=f"{target} candidate directory")
    platform_archive_directory = _require_directory(
        candidate / "platform-archive",
        label=f"{target} platform archive directory",
    )
    archive_name = f"sorafs-cli-{version}-{target}.tar.gz"
    archive = platform_archive_directory / archive_name
    first_summary = platform_archive_directory / "candidate-package-first.json"
    replay_summary = platform_archive_directory / "candidate-package-replay.json"
    first_payload, first_bytes = _load_json(
        first_summary,
        label=f"{target} first candidate summary",
        max_bytes=MAX_JSON_BYTES,
    )
    replay_payload, replay_bytes = _load_json(
        replay_summary,
        label=f"{target} replay candidate summary",
        max_bytes=MAX_JSON_BYTES,
    )
    if first_bytes != replay_bytes or first_payload != replay_payload:
        _fail(f"{target} candidate replay summary must be byte-identical")
    if not isinstance(first_payload, dict) or set(first_payload) != RELEASE_SUMMARY_FIELDS:
        _fail(f"{target} candidate summary fields must match the closed contract")
    archive_digest, _archive_size = _hash_regular_file(
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
        source_sbom=candidate / "sorafs-release.spdx.json",
        source_report=candidate / "sorafs-release-vulnerabilities.sarif",
        platform_sbom=candidate / f"sorafs-cli-{target}.spdx.json",
        platform_report=(
            candidate / f"sorafs-cli-{target}-vulnerabilities.sarif"
        ),
        attestation_bundle=root / "github-attestations" / f"{target}.json",
        cosign_bundle=archive.with_name(archive.name + ".sigstore.json"),
    )
    for label, path in (
        ("source SBOM", evidence.source_sbom),
        ("source vulnerability report", evidence.source_report),
        ("platform SBOM", evidence.platform_sbom),
        ("platform vulnerability report", evidence.platform_report),
        ("GitHub attestation bundle", evidence.attestation_bundle),
        ("cosign bundle", evidence.cosign_bundle),
    ):
        _load_json(
            path,
            label=f"{target} {label}",
            max_bytes=MAX_JSON_BYTES,
        )
    return evidence


def _require_identical_source_scans(candidates: list[CandidateEvidence]) -> None:
    for field, label in (
        ("source_sbom", "source SBOM"),
        ("source_report", "source vulnerability report"),
    ):
        observed: set[str] = set()
        for candidate in candidates:
            digest, _size = _hash_regular_file(
                getattr(candidate, field),
                label=f"{candidate.target} {label}",
                max_bytes=MAX_JSON_BYTES,
            )
            observed.add(digest)
        if len(observed) != 1:
            _fail(f"all target candidates must carry the same {label}")


def _external_receipt_inventory(root: Path, subdirectory: str) -> dict[str, Path]:
    directory = _require_directory(
        root / subdirectory,
        label=f"external {subdirectory} receipt directory",
    )
    expected = {f"{target}.json" for target in REQUIRED_RELEASE_TARGETS}
    try:
        observed = {entry.name for entry in os.scandir(directory)}
    except OSError as error:
        raise SourceBuildError(
            f"external {subdirectory} receipt directory cannot be enumerated"
        ) from error
    if observed != expected:
        _fail(
            f"external {subdirectory} receipts must contain exactly five "
            "canonical target files"
        )
    return {target: directory / f"{target}.json" for target in REQUIRED_RELEASE_TARGETS}


def _copy_receipt(
    source: Path,
    destination: Path,
    *,
    label: str,
) -> None:
    payload, raw = _load_json(source, label=label, max_bytes=MAX_RECEIPT_BYTES)
    if not isinstance(payload, dict):
        _fail(f"{label} must be a JSON object")
    _write_exclusive(destination, raw)


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


def _remove_generated_outputs(
    evidence_directory: Path,
    created_output_paths: list[Path],
    *,
    remove_evidence_directory: bool,
) -> None:
    for path in created_output_paths:
        try:
            path.unlink()
        except FileNotFoundError:
            pass
    if (
        remove_evidence_directory
        and evidence_directory.exists()
        and not evidence_directory.is_symlink()
    ):
        shutil.rmtree(evidence_directory)


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

    root = _require_directory(source_root, label="supply-chain source root")
    receipts_root = _require_directory(
        external_receipts_root,
        label="external receipt root",
    )
    if root == receipts_root or root in receipts_root.parents or receipts_root in root.parents:
        _fail("external receipts and workflow source roots must not overlap")
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

    evidence_directory = root / EVIDENCE_SUBDIRECTORY
    output_paths = {
        kind: root / relative
        for kind, relative in DEFAULT_SOURCE_ARTIFACT_PATHS.items()
    }
    if evidence_directory.exists() or evidence_directory.is_symlink():
        _fail("generated reference-SDK evidence directory must not already exist")
    if any(path.exists() or path.is_symlink() for path in output_paths.values()):
        _fail("canonical source indexes must not already exist")

    candidates = [
        _candidate_evidence(root, version=version, target=target)
        for target in REQUIRED_RELEASE_TARGETS
    ]
    _require_identical_source_scans(candidates)
    release_authentication_directory = _require_directory(
        root / "release-authentication",
        label="release authentication directory",
    )
    release_manifest = release_authentication_directory / "release_manifest.json"
    release_manifest_digest, _manifest_size = _hash_regular_file(
        release_manifest,
        label="authenticated release manifest",
        max_bytes=MAX_JSON_BYTES,
    )
    _load_json(
        release_manifest,
        label="authenticated release manifest",
        max_bytes=MAX_JSON_BYTES,
    )
    release_receipts = _external_receipt_inventory(
        receipts_root,
        RELEASE_RECEIPT_SUBDIRECTORY,
    )
    provenance_receipts = _external_receipt_inventory(
        receipts_root,
        PROVENANCE_RECEIPT_SUBDIRECTORY,
    )

    release_receipt_directory = evidence_directory / RELEASE_RECEIPT_SUBDIRECTORY
    provenance_receipt_directory = (
        evidence_directory / PROVENANCE_RECEIPT_SUBDIRECTORY
    )
    created_output_paths: list[Path] = []
    evidence_directory_created = False
    try:
        evidence_directory.mkdir(mode=0o700)
        evidence_directory_created = True
        release_receipt_directory.mkdir(mode=0o700)
        provenance_receipt_directory.mkdir(mode=0o700)
        for target in REQUIRED_RELEASE_TARGETS:
            _copy_receipt(
                release_receipts[target],
                release_receipt_directory / f"{target}.json",
                label=f"{target} external release rehearsal receipt",
            )
            _copy_receipt(
                provenance_receipts[target],
                provenance_receipt_directory / f"{target}.json",
                label=f"{target} external provenance verification receipt",
            )

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
                        "artifact_path": _relative_path(
                            root,
                            candidate.archive,
                            label=f"{target} release archive",
                        ),
                        "sha256": candidate.archive_sha256,
                    },
                    "receipt": _file_reference(
                        root,
                        release_receipt_directory / f"{target}.json",
                        label=f"{target} staged release rehearsal receipt",
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
                        label=f"{target} GitHub attestation bundle",
                    ),
                    "cosign_bundle": _file_reference(
                        root,
                        candidate.cosign_bundle,
                        label=f"{target} cosign bundle",
                    ),
                    "verification_receipt": _file_reference(
                        root,
                        provenance_receipt_directory / f"{target}.json",
                        label=f"{target} staged provenance verification receipt",
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
        for kind, payload in payloads.items():
            _write_exclusive(output_paths[kind], _canonical_json_bytes(payload))
            created_output_paths.append(output_paths[kind])

        def authenticate(
            claimed_fingerprint: str,
            message: bytes,
            signature: bytes,
        ) -> bool:
            return secrets.compare_digest(
                claimed_fingerprint,
                key_fingerprint,
            ) and verify_ed25519(public_key, signature, message)

        result, source_errors = validate_supply_chain_sources(
            root,
            expected_deployment_id=deployment_id,
            expected_environment=environment,
            expected_release_manifest_digest_hex=release_manifest_digest,
            expected_certificate_identity=certificate_identity,
            expected_verification_key_fingerprint_hex=key_fingerprint,
            verification_receipt_authenticator=authenticate,
            now_unix=now_unix,
            expected_oidc_issuer=oidc_issuer,
        )
        if source_errors or not isinstance(result, SupplyChainSourceResult):
            _fail("assembled source bundle failed canonical source validation")
    except BaseException:
        _remove_generated_outputs(
            evidence_directory,
            created_output_paths,
            remove_evidence_directory=evidence_directory_created,
        )
        raise

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
