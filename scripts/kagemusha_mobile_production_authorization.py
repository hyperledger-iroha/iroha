#!/usr/bin/env python3
"""Issue and verify platform-scoped Kagemusha mobile build authorizations.

The authorization document is not self-authenticating.  The protected promotion
workflow attests its exact bytes with GitHub OIDC, and release consumers verify
that attestation before trusting the document or enabling production Cargo code.
"""

from __future__ import annotations

import argparse
from contextlib import ExitStack, contextmanager
import hashlib
import json
import os
from pathlib import Path
from pathlib import PurePosixPath
import re
import stat
import sys
from typing import Any, NoReturn
import zipfile


SCHEMA = "iroha.kagemusha.mobile.production_build_authorization.v1"
PROMOTION_ID_DOMAIN = b"iroha.kagemusha.github-promotion-run.v1"
EXPECTED_REPOSITORY = "hyperledger-iroha/iroha"
EXPECTED_WORKFLOW_REF = (
    "hyperledger-iroha/iroha/.github/workflows/"
    "promote_kagemusha_v4.yml@refs/heads/main"
)
PRODUCTION_FEATURE = "privacy-production-enabled"
KAGEMUSHA_ARTIFACT_ABI_VERSION = 21
NATIVE_BRIDGE_ABI_VERSION = 23
MAX_AUTHORIZATION_BYTES = 32 * 1024
MAX_EVIDENCE_BYTES = 16 * 1024 * 1024
MAX_APPLE_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_ANDROID_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024
MAX_ANDROID_EXPANDED_BYTES = 8 * 1024 * 1024 * 1024
MAX_PACKAGE_METADATA_BYTES = 4 * 1024 * 1024
MAX_ZIP_ENTRIES = 16 * 1024
READ_CHUNK_BYTES = 1024 * 1024
SHA1 = re.compile(r"[0-9a-f]{40}")
SHA256 = re.compile(r"[0-9a-f]{64}")
SEMVER_TAG = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")
PLATFORM_TARGETS = {
    "apple": [
        "aarch64-apple-darwin",
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
        "x86_64-apple-darwin",
        "x86_64-apple-ios",
    ],
    "android": ["aarch64-linux-android", "x86_64-linux-android"],
}
CATALOG_METADATA = {
    "internal-validation-receipt-v1.norito": 1024 * 1024,
    "manifest.json": 32 * 1024 * 1024,
    "manifest.norito": 32 * 1024 * 1024,
    "promotion-record-v4.norito": 1024 * 1024,
    "recursive-step-two-qualification-v4.norito": 1024 * 1024,
    "release-attestation-v4.norito": 1024 * 1024,
    "topup-finality-roster-v4.norito": 1024 * 1024,
}
AUTHORIZATION_FIELDS = {
    "authorization",
    "artifact_manifest_sha256",
    "candidate_sha256",
    "cargo_features",
    "kagemusha_artifact_abi_version",
    "native_bridge_abi_version",
    "platform",
    "platform_evidence_sha256",
    "promotion_id",
    "promotion_record_sha256",
    "release_tag",
    "release_catalog_sha256",
    "release_generation",
    "release_policy_sha256",
    "release_verification_report_sha256",
    "repository",
    "reviewed_source_closure_sha256",
    "run_attempt",
    "run_id",
    "schema",
    "sealed_candidate_build_report_sha256",
    "source_sha",
    "source_tree_sha256",
    "target_triples",
    "version",
    "workflow_ref",
    "workflow_sha",
}
SHARED_AUTHORIZATION_FIELDS = AUTHORIZATION_FIELDS - {
    "platform",
    "platform_evidence_sha256",
    "release_verification_report_sha256",
    "target_triples",
}
KAGAMI_REPORT_FIELDS = {
    "artifacts",
    "asset_definition_id",
    "asset_scale",
    "authenticated_source_seal_projection_sha256",
    "bridge_abi_version",
    "candidate_sha256",
    "envelope_sha256",
    "generation",
    "generation_memory_enforcement_profile",
    "generation_memory_limit_bytes",
    "generator_binary_sha256",
    "internal_validation_receipt_sha256",
    "manifest_body_sha256",
    "network_id",
    "promotion_record_sha256",
    "qualification_receipt_sha256",
    "qualified_candidate_sha256",
    "recursive_step_verifier_commitment",
    "release_policy_sha256",
    "reviewed_cargo_binary_sha256",
    "reviewed_rustc_binary_sha256",
    "sealed_candidate_build_report_sha256",
    "status",
}


class AuthorizationError(ValueError):
    """A mobile production authorization or bound artifact is invalid."""


def _fail(message: str) -> NoReturn:
    raise AuthorizationError(message)


def _strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _fail(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def _canonical_json(document: dict[str, Any]) -> bytes:
    return (
        json.dumps(document, sort_keys=True, separators=(",", ":"), allow_nan=False)
        + "\n"
    ).encode("utf-8")


@contextmanager
def _open_pinned_regular(path: Path, label: str, maximum: int):
    """Open one bounded regular file and reject identity drift around its use."""

    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail(f"{label} path must be canonical and absolute")
    try:
        before = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        _fail(f"{label} is unavailable: {error}")
    if (
        resolved != path
        or stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
    ):
        _fail(f"{label} must be a canonical regular non-symbolic file")
    if before.st_nlink != 1 or not 0 < before.st_size <= maximum:
        _fail(f"{label} size or link count is invalid")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        _fail(f"{label} could not be opened: {error}")
    opened = os.fstat(descriptor)
    if (
        opened.st_dev,
        opened.st_ino,
        opened.st_mode,
        opened.st_size,
        opened.st_nlink,
    ) != (
        before.st_dev,
        before.st_ino,
        before.st_mode,
        before.st_size,
        before.st_nlink,
    ):
        os.close(descriptor)
        _fail(f"{label} changed while being opened")
    try:
        with os.fdopen(descriptor, "rb", closefd=False) as handle:
            yield handle, opened
        try:
            after = os.fstat(descriptor)
            visible = path.lstat()
        except OSError as error:
            _fail(f"{label} became unavailable while being read: {error}")
    finally:
        os.close(descriptor)
    opened_identity = (
        opened.st_dev,
        opened.st_ino,
        opened.st_mode,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        opened.st_nlink,
    )
    if opened_identity != (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
        after.st_nlink,
    ) or opened_identity != (
        visible.st_dev,
        visible.st_ino,
        visible.st_mode,
        visible.st_size,
        visible.st_mtime_ns,
        visible.st_ctime_ns,
        visible.st_nlink,
    ):
        _fail(f"{label} changed while being read")


def _snapshot_regular(path: Path, label: str, maximum: int) -> bytes:
    """Read one bounded regular file from a pinned descriptor."""

    with _open_pinned_regular(path, label, maximum) as (handle, opened):
        payload = handle.read(maximum + 1)
    if len(payload) != opened.st_size or len(payload) > maximum:
        _fail(f"{label} changed or exceeded its byte limit while being read")
    return payload


def _hash_opened_regular(handle: Any, expected_size: int, label: str) -> str:
    """Hash an already pinned file without retaining large artifacts in memory."""

    handle.seek(0)
    hasher = hashlib.sha256()
    observed_size = 0
    while chunk := handle.read(READ_CHUNK_BYTES):
        observed_size += len(chunk)
        if observed_size > expected_size:
            _fail(f"{label} grew while being hashed")
        hasher.update(chunk)
    if observed_size != expected_size:
        _fail(f"{label} changed while being hashed")
    handle.seek(0)
    return hasher.hexdigest()


def _record_for_payload(name: str, payload: bytes) -> tuple[str, int, str]:
    """Return the authenticated inventory tuple for an in-memory payload."""

    return name, len(payload), hashlib.sha256(payload).hexdigest()


def _inventory_sha256(
    domain: bytes, records: list[tuple[str, int, str]]
) -> str:
    """Hash an exact filename, size, and content-digest inventory."""

    if len(records) != len({name for name, _, _ in records}):
        _fail("artifact inventory contains duplicate filenames")
    hasher = hashlib.sha256()
    hasher.update(domain + b"\0")
    for name, size, digest in sorted(records):
        try:
            encoded_name = name.encode("ascii")
        except UnicodeEncodeError:
            _fail("artifact inventory filename is not canonical ASCII")
        if (
            not name
            or "/" in name
            or "\\" in name
            or size <= 0
            or SHA256.fullmatch(digest) is None
        ):
            _fail("artifact inventory record is invalid")
        hasher.update(encoded_name + b"\0")
        hasher.update(str(size).encode("ascii") + b"\0")
        hasher.update(bytes.fromhex(digest))
    return hasher.hexdigest()


def _parse_checksum_inventory(
    payload: bytes, label: str, *, top_level: bool
) -> dict[str, str]:
    """Parse a strict GNU-style SHA-256 inventory with no ambiguous paths."""

    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError:
        _fail(f"{label} is not ASCII")
    if not text or not text.endswith("\n") or "\r" in text:
        _fail(f"{label} must end in one canonical newline")
    result: dict[str, str] = {}
    for line in text.splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  ([A-Za-z0-9][A-Za-z0-9._/+@=-]*)", line)
        if match is None:
            _fail(f"{label} contains a noncanonical checksum line")
        digest, name = match.groups()
        path = PurePosixPath(name)
        if (
            digest == "0" * 64
            or name in result
            or name.startswith("/")
            or "\\" in name
            or any(part in {"", ".", ".."} for part in path.parts)
            or str(path) != name
            or (top_level and len(path.parts) != 1)
        ):
            _fail(f"{label} contains an unsafe or duplicate path")
        result[name] = digest
    return result


def _verify_checksum_inventory(
    payload: bytes,
    label: str,
    records: dict[str, tuple[int, str]],
    *,
    top_level: bool,
) -> None:
    """Require a checksum inventory to cover exactly the supplied records."""

    checksums = _parse_checksum_inventory(payload, label, top_level=top_level)
    if set(checksums) != set(records):
        _fail(f"{label} does not cover the exact artifact inventory")
    for name, (_, digest) in records.items():
        if checksums[name] != digest:
            _fail(f"{label} digest mismatch for {name}")


def _verify_package_manifest(
    payload: bytes,
    *,
    mode: str,
    release_tag: str,
    records: list[tuple[str, str, int, str]],
) -> None:
    """Verify the packager's exact outer manifest and payload records."""

    document = _decode_json(payload, f"{mode} package manifest")
    expected_fields = {"version", "mode", "artifacts"}
    if mode == "apple":
        expected_fields.add("apple_sdk_semver")
    if set(document) != expected_fields:
        _fail(f"{mode} package manifest field inventory is not exact")
    if (
        document.get("version") != release_tag
        or document.get("mode") != mode
        or (mode == "apple" and document.get("apple_sdk_semver") != release_tag[1:])
    ):
        _fail(f"{mode} package manifest release identity is invalid")
    artifacts = document.get("artifacts")
    if not isinstance(artifacts, list) or len(artifacts) != len(records):
        _fail(f"{mode} package manifest artifact inventory is not exact")
    for artifact, (kind, name, size, digest) in zip(artifacts, records):
        if not isinstance(artifact, dict) or set(artifact) != {
            "bytes",
            "kind",
            "name",
            "path",
            "sha256",
        }:
            _fail(f"{mode} package manifest artifact fields are not exact")
        expected = {
            "bytes": size,
            "kind": kind,
            "name": name,
            "path": name,
            "sha256": digest,
        }
        if any(
            artifact.get(field) != value
            or type(artifact.get(field)) is not type(value)
            for field, value in expected.items()
        ):
            _fail(f"{mode} package manifest artifact binding is invalid")


def release_catalog_sha256(root: Path) -> str:
    """Hash the validated catalog's release and signed-metadata identities."""

    if not root.is_absolute() or Path(os.path.abspath(root)) != root:
        _fail("release catalog root path must be canonical and absolute")
    try:
        root_metadata = root.lstat()
        releases = sorted(root.iterdir(), key=lambda path: os.fsencode(path.name))
    except OSError as error:
        _fail(f"release catalog root is unavailable: {error}")
    if stat.S_ISLNK(root_metadata.st_mode) or not stat.S_ISDIR(root_metadata.st_mode):
        _fail("release catalog root must be a non-symbolic directory")
    if not 1 <= len(releases) <= 16:
        _fail("release catalog must contain between one and sixteen releases")
    hasher = hashlib.sha256()
    hasher.update(b"iroha.kagemusha.mobile.release-catalog.v1\0")
    for release in releases:
        try:
            metadata = release.lstat()
        except OSError as error:
            _fail(f"release catalog entry is unavailable: {error}")
        if (
            SHA256.fullmatch(release.name) is None
            or release.name == "0" * 64
            or stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
        ):
            _fail("release catalog contains a noncanonical release directory")
        hasher.update(release.name.encode("ascii") + b"\0")
        for name, maximum in CATALOG_METADATA.items():
            payload = _snapshot_regular(
                release / name, f"release catalog {release.name}/{name}", maximum
            )
            hasher.update(name.encode("ascii") + b"\0")
            hasher.update(str(len(payload)).encode("ascii") + b"\0")
            hasher.update(hashlib.sha256(payload).digest())
    return hasher.hexdigest()


def selected_release_identity(root: Path, manifest_sha256: str) -> tuple[str, str]:
    """Return exact manifest and promotion-record digests for one catalog release."""

    manifest_sha256 = _sha256(manifest_sha256, "artifact manifest digest")
    if not root.is_absolute() or Path(os.path.abspath(root)) != root:
        _fail("release catalog root path must be canonical and absolute")
    release = root / manifest_sha256
    try:
        root_metadata = root.lstat()
        release_metadata = release.lstat()
    except OSError as error:
        _fail(f"selected release is unavailable: {error}")
    if stat.S_ISLNK(root_metadata.st_mode) or not stat.S_ISDIR(root_metadata.st_mode):
        _fail("release catalog root must be a non-symbolic directory")
    if stat.S_ISLNK(release_metadata.st_mode) or not stat.S_ISDIR(
        release_metadata.st_mode
    ):
        _fail("selected release must be a non-symbolic directory")
    manifest = _snapshot_regular(
        release / "manifest.norito", "selected artifact manifest", 32 * 1024 * 1024
    )
    observed_manifest_sha256 = hashlib.sha256(manifest).hexdigest()
    if observed_manifest_sha256 != manifest_sha256:
        _fail("selected release directory does not equal manifest SHA-256")
    promotion_record = _snapshot_regular(
        release / "promotion-record-v4.norito",
        "selected promotion record",
        1024 * 1024,
    )
    return observed_manifest_sha256, hashlib.sha256(promotion_record).hexdigest()


def _selected_release_context(
    root: Path,
    manifest_sha256: str,
    verification_report_path: Path,
    *,
    release_policy_sha256: str,
    source_sha: str,
    reviewed_source_closure_sha256: str,
    sealed_candidate_build_report_sha256: str,
) -> dict[str, str]:
    """Authenticate the selected release tuple projected by pinned Kagami."""

    manifest_sha256, promotion_record_sha256 = selected_release_identity(
        root, manifest_sha256
    )
    release = root / manifest_sha256
    manifest = _decode_json(
        _snapshot_regular(
            release / "manifest.json", "selected artifact manifest JSON", 32 * 1024 * 1024
        ),
        "selected artifact manifest JSON",
    )
    source_tree_sha256 = _sha256(
        manifest.get("source_tree_sha256"), "selected release source tree digest"
    )
    generation = manifest.get("generation")
    if (
        not isinstance(generation, str)
        or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", generation) is None
        or manifest.get("source_commit") != source_sha
        or manifest.get("bridge_abi_version") != NATIVE_BRIDGE_ABI_VERSION
        or manifest.get("source_repo_dirty") is not False
        or manifest.get("reviewed_source_closure_descriptor_sha256")
        != reviewed_source_closure_sha256
        or manifest.get("sealed_candidate_build_report_sha256")
        != sealed_candidate_build_report_sha256
    ):
        _fail("selected release manifest source/build binding is invalid")
    report_payload = _snapshot_regular(
        verification_report_path,
        "Kagami selected-release verification report",
        4 * 1024 * 1024,
    )
    report = _decode_json(report_payload, "Kagami selected-release verification report")
    if set(report) != KAGAMI_REPORT_FIELDS:
        _fail("Kagami selected-release verification report fields are not exact")
    candidate_sha256 = _sha256(
        report.get("candidate_sha256"), "Kagami candidate digest"
    )
    if (
        report.get("status") != "verified"
        or report.get("envelope_sha256") != manifest_sha256
        or report.get("promotion_record_sha256") != promotion_record_sha256
        or report.get("release_policy_sha256") != release_policy_sha256
        or report.get("sealed_candidate_build_report_sha256")
        != sealed_candidate_build_report_sha256
        or report.get("bridge_abi_version") != NATIVE_BRIDGE_ABI_VERSION
        or report.get("generation") != generation
    ):
        _fail("Kagami report does not verify the selected release tuple")
    return {
        "artifact_manifest_sha256": manifest_sha256,
        "candidate_sha256": candidate_sha256,
        "promotion_record_sha256": promotion_record_sha256,
        "release_generation": generation,
        "release_verification_report_sha256": hashlib.sha256(
            report_payload
        ).hexdigest(),
        "source_tree_sha256": source_tree_sha256,
    }


def _validate_platform_evidence(
    platform: str,
    payload: bytes,
    *,
    artifact_manifest_sha256: str,
    candidate_sha256: str,
    promotion_id_value: str,
    source_sha: str,
    source_tree_sha256: str,
    release_generation: str,
) -> None:
    """Require platform evidence to identify the selected release and source."""

    evidence = _decode_json(payload, f"{platform} promotion evidence")
    if platform == "apple":
        if (
            evidence.get("schema")
            != "iroha.kagemusha.ios.app_attest_catalog_revalidation_receipt.v1"
            or evidence.get("version") != 1
            or evidence.get("promotion_id") != promotion_id_value
            or evidence.get("status") != "catalog-revalidated-for-one-promotion"
        ):
            _fail("Apple evidence does not bind this protected promotion")
        statuses = evidence.get("release_statuses")
        if not isinstance(statuses, list) or not 1 <= len(statuses) <= 16:
            _fail("Apple evidence release status inventory is invalid")
        manifests = [
            status.get("release_manifest_sha256")
            for status in statuses
            if isinstance(status, dict)
        ]
        if (
            len(manifests) != len(statuses)
            or len(manifests) != len(set(manifests))
            or manifests.count(artifact_manifest_sha256) != 1
        ):
            _fail("Apple evidence does not cover the selected release exactly once")
        return
    if platform != "android":
        _fail("unsupported platform evidence")
    slots = evidence.get("slots")
    kagemusha = evidence.get("kagemusha")
    if (
        evidence.get("schema_version") != 1
        or not isinstance(slots, list)
        or not slots
        or evidence.get("ok") != len(slots)
        or evidence.get("failed") != 0
        or not isinstance(kagemusha, dict)
        or kagemusha.get("production_evidence_required") is not True
        or kagemusha.get("standard_matrix_required") is not True
        or kagemusha.get("missing_device_families") != []
        or kagemusha.get("missing_d2d_payment_transports") != []
        or kagemusha.get("missing_d2d_payment_transport_pairs") != []
        or kagemusha.get("duplicate_bindings") != {}
    ):
        _fail("Android production evidence matrix is incomplete")
    singleton_fields: dict[str, set[str]] = {
        "candidate_manifest_sha256": set(),
        "candidate_stage_manifest_sha256": set(),
        "candidate_inventory_sha256": set(),
    }
    for slot in slots:
        if not isinstance(slot, dict) or slot.get("status") != "ok":
            _fail("Android evidence contains an unsuccessful device slot")
        binding = slot.get("kagemusha")
        if (
            not isinstance(binding, dict)
            or binding.get("required") is not True
            or binding.get("candidate_record_sha256") != candidate_sha256
            or binding.get("candidate_source_commit") != source_sha
            or binding.get("candidate_source_tree_sha256") != source_tree_sha256
            or binding.get("candidate_generation") != release_generation
            or binding.get("candidate_source_repo_dirty") is not False
            or binding.get("native_bridge_abi_version") != NATIVE_BRIDGE_ABI_VERSION
            or binding.get("production_capability_observed") is not False
            or binding.get("strongbox_attestation") is not True
            or binding.get("physical_device_attestation") is not True
        ):
            _fail("Android device slot does not bind the selected release and source")
        _sha256(
            binding.get("signed_evidence_artifact_sha256"),
            "Android signed evidence digest",
        )
        _sha256(
            binding.get("signed_evidence_signer_public_key_sha256"),
            "Android evidence signer key digest",
        )
        for field, values in singleton_fields.items():
            values.add(_sha256(binding.get(field), f"Android {field}"))
    if any(len(values) != 1 for values in singleton_fields.values()):
        _fail("Android device slots disagree on candidate build identity")


def _write_new(path: Path, payload: bytes) -> None:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        _fail("authorization output path must be canonical and absolute")
    try:
        parent = path.parent.lstat()
    except OSError as error:
        _fail(f"authorization output parent is unavailable: {error}")
    if stat.S_ISLNK(parent.st_mode) or not stat.S_ISDIR(parent.st_mode):
        _fail("authorization output parent must be a non-symbolic directory")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as error:
        _fail(f"authorization output could not be created exclusively: {error}")
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as handle:
            handle.write(payload)
            handle.flush()
            os.fchmod(descriptor, 0o600)
            os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _positive_integer(value: str, label: str) -> int:
    if re.fullmatch(r"[1-9][0-9]*", value) is None:
        _fail(f"{label} must be a canonical positive integer")
    parsed = int(value, 10)
    if parsed > (1 << 63) - 1:
        _fail(f"{label} exceeds the first-release bound")
    return parsed


def _sha256(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256.fullmatch(value) is None
        or value == "0" * 64
    ):
        _fail(f"{label} must be non-zero lowercase SHA-256")
    return value


def _sha1(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA1.fullmatch(value) is None
        or value == "0" * 40
    ):
        _fail(f"{label} must be non-zero lowercase Git SHA-1")
    return value


def _release_tag(value: str) -> str:
    if SEMVER_TAG.fullmatch(value) is None:
        _fail("release tag must be canonical v-prefixed SemVer")
    return value


def promotion_id(
    repository: str,
    workflow_ref: str,
    workflow_sha: str,
    run_id: int,
    run_attempt: int,
) -> str:
    """Return the repository's deterministic protected-promotion identity."""

    fields = (
        PROMOTION_ID_DOMAIN,
        repository.encode("utf-8"),
        workflow_ref.encode("utf-8"),
        workflow_sha.encode("ascii"),
        str(run_id).encode("ascii"),
        str(run_attempt).encode("ascii"),
    )
    return hashlib.sha256(b"\0".join(fields) + b"\0").hexdigest()


def _validate_coordinates(args: argparse.Namespace) -> tuple[int, int]:
    if args.platform not in PLATFORM_TARGETS:
        _fail("platform must be exactly apple or android")
    if args.repository != EXPECTED_REPOSITORY:
        _fail("repository is not the canonical Iroha repository")
    if args.workflow_ref != EXPECTED_WORKFLOW_REF:
        _fail("workflow ref is not the protected main promotion workflow")
    source_sha = _sha1(args.source_sha, "source SHA")
    workflow_sha = _sha1(args.workflow_sha, "workflow SHA")
    if source_sha != workflow_sha:
        _fail("source SHA and promotion workflow SHA must be identical")
    _release_tag(args.release_tag)
    run_id = _positive_integer(args.run_id, "run id")
    run_attempt = _positive_integer(args.run_attempt, "run attempt")
    expected_promotion_id = promotion_id(
        args.repository, args.workflow_ref, workflow_sha, run_id, run_attempt
    )
    if args.promotion_id != expected_promotion_id:
        _fail("promotion id does not match the exact GitHub run identity")
    return run_id, run_attempt


def _authorization_document(args: argparse.Namespace) -> dict[str, Any]:
    run_id, run_attempt = _validate_coordinates(args)
    evidence = _snapshot_regular(
        Path(args.evidence), f"{args.platform} promotion evidence", MAX_EVIDENCE_BYTES
    )
    release_policy_sha256 = hashlib.sha256(
        _snapshot_regular(
            Path(args.release_policy), "Kagemusha release policy", 16 * 1024 * 1024
        )
    ).hexdigest()
    context = _selected_release_context(
        Path(args.artifact_root),
        args.artifact_manifest_sha256,
        Path(args.release_verification_report),
        release_policy_sha256=release_policy_sha256,
        source_sha=args.source_sha,
        reviewed_source_closure_sha256=args.reviewed_source_closure_sha256,
        sealed_candidate_build_report_sha256=args.sealed_candidate_build_report_sha256,
    )
    _validate_platform_evidence(
        args.platform,
        evidence,
        artifact_manifest_sha256=context["artifact_manifest_sha256"],
        candidate_sha256=context["candidate_sha256"],
        promotion_id_value=args.promotion_id,
        source_sha=args.source_sha,
        source_tree_sha256=context["source_tree_sha256"],
        release_generation=context["release_generation"],
    )
    return {
        "authorization": PRODUCTION_FEATURE,
        "artifact_manifest_sha256": context["artifact_manifest_sha256"],
        "candidate_sha256": context["candidate_sha256"],
        "cargo_features": [PRODUCTION_FEATURE],
        "kagemusha_artifact_abi_version": KAGEMUSHA_ARTIFACT_ABI_VERSION,
        "native_bridge_abi_version": NATIVE_BRIDGE_ABI_VERSION,
        "platform": args.platform,
        "platform_evidence_sha256": hashlib.sha256(evidence).hexdigest(),
        "promotion_id": args.promotion_id,
        "promotion_record_sha256": context["promotion_record_sha256"],
        "release_catalog_sha256": release_catalog_sha256(Path(args.artifact_root)),
        "release_generation": context["release_generation"],
        "release_policy_sha256": release_policy_sha256,
        "release_verification_report_sha256": context[
            "release_verification_report_sha256"
        ],
        "release_tag": args.release_tag,
        "repository": args.repository,
        "reviewed_source_closure_sha256": _sha256(
            args.reviewed_source_closure_sha256, "reviewed source closure digest"
        ),
        "run_attempt": run_attempt,
        "run_id": run_id,
        "schema": SCHEMA,
        "sealed_candidate_build_report_sha256": _sha256(
            args.sealed_candidate_build_report_sha256,
            "sealed candidate build report digest",
        ),
        "source_sha": args.source_sha,
        "source_tree_sha256": context["source_tree_sha256"],
        "target_triples": PLATFORM_TARGETS[args.platform],
        "version": 1,
        "workflow_ref": args.workflow_ref,
        "workflow_sha": args.workflow_sha,
    }


def _decode_authorization(payload: bytes) -> dict[str, Any]:
    try:
        decoded = json.loads(payload.decode("utf-8"), object_pairs_hook=_strict_object)
    except (UnicodeDecodeError, json.JSONDecodeError, AuthorizationError) as error:
        _fail(f"authorization is not strict UTF-8 JSON: {error}")
    if not isinstance(decoded, dict) or set(decoded) != AUTHORIZATION_FIELDS:
        _fail("authorization field inventory is not exact")
    if payload != _canonical_json(decoded):
        _fail("authorization JSON bytes are not canonical")
    return decoded


def _validate_authorization(
    document: dict[str, Any], args: argparse.Namespace
) -> None:
    run_id, run_attempt = _validate_coordinates(args)
    expected = {
        "authorization": PRODUCTION_FEATURE,
        "cargo_features": [PRODUCTION_FEATURE],
        "kagemusha_artifact_abi_version": KAGEMUSHA_ARTIFACT_ABI_VERSION,
        "native_bridge_abi_version": NATIVE_BRIDGE_ABI_VERSION,
        "platform": args.platform,
        "promotion_id": args.promotion_id,
        "release_tag": args.release_tag,
        "repository": args.repository,
        "run_attempt": run_attempt,
        "run_id": run_id,
        "schema": SCHEMA,
        "source_sha": args.source_sha,
        "target_triples": PLATFORM_TARGETS[args.platform],
        "version": 1,
        "workflow_ref": args.workflow_ref,
        "workflow_sha": args.workflow_sha,
    }
    for field, value in expected.items():
        if document[field] != value or type(document[field]) is not type(value):
            _fail(f"authorization {field} does not match the requested release")
    for field in (
        "artifact_manifest_sha256",
        "candidate_sha256",
        "platform_evidence_sha256",
        "promotion_record_sha256",
        "release_catalog_sha256",
        "release_policy_sha256",
        "release_verification_report_sha256",
        "reviewed_source_closure_sha256",
        "sealed_candidate_build_report_sha256",
        "source_tree_sha256",
    ):
        if not isinstance(document[field], str):
            _fail(f"authorization {field} is not text")
        _sha256(document[field], f"authorization {field}")
    if (
        not isinstance(document["release_generation"], str)
        or re.fullmatch(
            r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", document["release_generation"]
        )
        is None
    ):
        _fail("authorization release generation is invalid")
    if args.reviewed_source_closure_sha256 is not None and document[
        "reviewed_source_closure_sha256"
    ] != _sha256(
        args.reviewed_source_closure_sha256, "reviewed source closure digest"
    ):
        _fail("authorization reviewed source closure digest does not match")
    if args.sealed_candidate_build_report_sha256 is not None and document[
        "sealed_candidate_build_report_sha256"
    ] != _sha256(
        args.sealed_candidate_build_report_sha256,
        "sealed candidate build report digest",
    ):
        _fail("authorization sealed candidate build report digest does not match")
    if args.artifact_manifest_sha256 is not None and document[
        "artifact_manifest_sha256"
    ] != _sha256(args.artifact_manifest_sha256, "artifact manifest digest"):
        _fail("authorization artifact manifest digest does not match")
    report_path = getattr(args, "release_verification_report", None)
    if report_path is not None and document[
        "release_verification_report_sha256"
    ] != hashlib.sha256(
        _snapshot_regular(
            Path(report_path),
            "durable Kagami release verification report",
            4 * 1024 * 1024,
        )
    ).hexdigest():
        _fail("authorization Kagami verification report digest does not match")


def verify_authorization_pair(args: argparse.Namespace) -> dict[str, str]:
    """Verify platform separation and identical release trust bindings."""

    documents: dict[str, dict[str, Any]] = {}
    digests: dict[str, str] = {}
    for platform, argument in (
        ("apple", args.apple_authorization),
        ("android", args.android_authorization),
    ):
        payload = _snapshot_regular(
            Path(argument),
            f"{platform} production authorization",
            MAX_AUTHORIZATION_BYTES,
        )
        document = _decode_authorization(payload)
        coordinates = dict(vars(args))
        coordinates["platform"] = platform
        coordinates["release_verification_report"] = (
            args.apple_release_verification_report
            if platform == "apple"
            else args.android_release_verification_report
        )
        _validate_authorization(document, argparse.Namespace(**coordinates))
        documents[platform] = document
        digests[platform] = hashlib.sha256(payload).hexdigest()
    for field in SHARED_AUTHORIZATION_FIELDS:
        if documents["apple"][field] != documents["android"][field]:
            _fail(f"platform authorizations disagree on {field}")
    if digests["apple"] == digests["android"]:
        _fail("platform authorizations must have distinct exact bytes")
    return {
        "android_authorization_sha256": digests["android"],
        "apple_authorization_sha256": digests["apple"],
        "artifact_manifest_sha256": documents["apple"][
            "artifact_manifest_sha256"
        ],
        "candidate_sha256": documents["apple"]["candidate_sha256"],
        "promotion_record_sha256": documents["apple"]["promotion_record_sha256"],
        "release_catalog_sha256": documents["apple"]["release_catalog_sha256"],
        "release_policy_sha256": documents["apple"]["release_policy_sha256"],
        "release_verification_report_sha256": documents["apple"][
            "release_verification_report_sha256"
        ],
        "reviewed_source_closure_sha256": documents["apple"][
            "reviewed_source_closure_sha256"
        ],
        "sealed_candidate_build_report_sha256": documents["apple"][
            "sealed_candidate_build_report_sha256"
        ],
    }


def _decode_json(payload: bytes, label: str) -> dict[str, Any]:
    try:
        decoded = json.loads(payload.decode("utf-8"), object_pairs_hook=_strict_object)
    except (UnicodeDecodeError, json.JSONDecodeError, AuthorizationError) as error:
        _fail(f"{label} is not strict UTF-8 JSON: {error}")
    if not isinstance(decoded, dict):
        _fail(f"{label} root must be an object")
    return decoded


def _verify_native_provenance(
    document: dict[str, Any], authorization_sha256: str, source_sha: str, label: str
) -> None:
    if document.get("native_bridge_abi_version") != NATIVE_BRIDGE_ABI_VERSION:
        _fail(f"{label} does not bind native bridge ABI 23")
    if document.get("privacy_production_enabled") is not True:
        _fail(f"{label} is not production-enabled")
    if document.get("cargo_features") != [PRODUCTION_FEATURE]:
        _fail(f"{label} Cargo feature inventory is not exact")
    if document.get("source_commit") != source_sha:
        _fail(f"{label} source commit does not match the release")
    if document.get("kagemusha_production_authorization_sha256") != authorization_sha256:
        _fail(f"{label} does not bind its platform authorization")


def _validated_zip_entries(
    archive: zipfile.ZipFile,
    *,
    label: str,
    root_name: str,
    maximum_entry_bytes: int,
    maximum_total_bytes: int,
    compression_types: set[int],
) -> dict[str, zipfile.ZipInfo]:
    """Validate ZIP paths, types, bounds, compression, and collisions."""

    infos = archive.infolist()
    if not 1 <= len(infos) <= MAX_ZIP_ENTRIES:
        _fail(f"{label} entry count is invalid")
    entries: dict[str, zipfile.ZipInfo] = {}
    folded: set[str] = set()
    total = 0
    for info in infos:
        name = info.filename
        stripped = name.rstrip("/")
        path = PurePosixPath(stripped)
        casefolded = stripped.casefold()
        if (
            not stripped
            or "\x00" in name
            or "\\" in name
            or name.startswith("/")
            or not path.parts
            or path.parts[0] != root_name
            or any(part in {"", ".", ".."} for part in path.parts)
            or str(path) != stripped
            or name in entries
            or casefolded in folded
        ):
            _fail(f"{label} contains an unsafe, duplicate, or colliding path")
        if info.flag_bits & 0x1 or info.compress_type not in compression_types:
            _fail(f"{label} contains an encrypted or unsupported entry")
        unix_mode = info.external_attr >> 16
        file_type = stat.S_IFMT(unix_mode)
        if file_type == stat.S_IFLNK:
            _fail(f"{label} contains a symbolic link")
        if info.is_dir():
            if not name.endswith("/") or file_type not in {0, stat.S_IFDIR}:
                _fail(f"{label} contains an invalid directory entry")
        elif name.endswith("/") or file_type not in {0, stat.S_IFREG}:
            _fail(f"{label} contains an invalid regular-file entry")
        if info.file_size > maximum_entry_bytes:
            _fail(f"{label} contains an oversized entry")
        total += info.file_size
        if total > maximum_total_bytes:
            _fail(f"{label} expanded size exceeds its limit")
        entries[name] = info
        folded.add(casefolded)
    return entries


def _read_zip_entry(
    archive: zipfile.ZipFile,
    info: zipfile.ZipInfo,
    label: str,
    maximum: int,
) -> bytes:
    """Read one bounded regular ZIP member and require its declared size."""

    if info.is_dir() or not 0 < info.file_size <= maximum:
        _fail(f"{label} ZIP entry size is invalid")
    payload = archive.read(info)
    if len(payload) != info.file_size:
        _fail(f"{label} ZIP entry changed while being read")
    return payload


def _hash_zip_entry(
    archive: zipfile.ZipFile, info: zipfile.ZipInfo, label: str
) -> str:
    """Hash one ZIP member while checking its declared uncompressed size."""

    if info.is_dir():
        _fail(f"{label} unexpectedly names a directory")
    hasher = hashlib.sha256()
    observed = 0
    with archive.open(info, "r") as handle:
        while chunk := handle.read(READ_CHUNK_BYTES):
            observed += len(chunk)
            if observed > info.file_size:
                _fail(f"{label} exceeded its declared ZIP size")
            hasher.update(chunk)
    if observed != info.file_size:
        _fail(f"{label} did not match its declared ZIP size")
    return hasher.hexdigest()


def _require_one_package_directory(paths: list[Path], label: str) -> None:
    """Require every package component to have the same canonical parent."""

    parents = {path.parent for path in paths}
    if len(parents) != 1:
        _fail(f"{label} components do not share one package directory")


def verify_apple_artifact(args: argparse.Namespace) -> str:
    """Verify every byte in one exact Apple release package."""

    authorization_sha256 = _sha256(
        args.authorization_sha256, "Apple authorization digest"
    )
    source_sha = _sha1(args.source_sha, "Apple artifact source SHA")
    release_tag = _release_tag(args.release_tag)
    semver = release_tag[1:]
    archive_path = Path(args.archive)
    manifest_path = Path(args.manifest)
    podspec_path = Path(args.podspec)
    checksums_path = Path(args.checksums)
    package_manifest_path = Path(args.package_manifest)
    expected_names = {
        archive_path: f"NoritoBridge-{release_tag}.xcframework.zip",
        manifest_path: f"NoritoBridge-{release_tag}.artifacts.json",
        podspec_path: f"NoritoBridge-{semver}.podspec",
        checksums_path: f"SHA256SUMS-apple-{release_tag}.txt",
        package_manifest_path: f"mobile-sdk-apple-{release_tag}.artifacts.json",
    }
    if any(path.name != name for path, name in expected_names.items()):
        _fail("Apple package filenames do not match the release tag")
    _require_one_package_directory(list(expected_names), "Apple package")

    manifest_payload = _snapshot_regular(
        manifest_path, "Apple release artifact manifest", MAX_PACKAGE_METADATA_BYTES
    )
    podspec_payload = _snapshot_regular(
        podspec_path, "Apple CocoaPods specification", MAX_PACKAGE_METADATA_BYTES
    )
    checksums_payload = _snapshot_regular(
        checksums_path, "Apple package checksums", MAX_PACKAGE_METADATA_BYTES
    )
    package_manifest_payload = _snapshot_regular(
        package_manifest_path, "Apple package manifest", MAX_PACKAGE_METADATA_BYTES
    )
    manifest = _decode_json(manifest_payload, "Apple release artifact manifest")
    if manifest.get("version") != semver:
        _fail("Apple artifact version does not match the release tag")
    _verify_native_provenance(
        manifest, authorization_sha256, source_sha, "Apple release artifact"
    )

    try:
        with _open_pinned_regular(
            archive_path,
            "Apple XCFramework release archive",
            MAX_APPLE_ARCHIVE_BYTES,
        ) as (handle, opened):
            archive_digest = _hash_opened_regular(
                handle, opened.st_size, "Apple XCFramework release archive"
            )
            with zipfile.ZipFile(handle) as archive:
                entries = _validated_zip_entries(
                    archive,
                    label="Apple XCFramework release archive",
                    root_name="NoritoBridge.xcframework",
                    maximum_entry_bytes=256 * 1024 * 1024,
                    maximum_total_bytes=1024 * 1024 * 1024,
                    compression_types={zipfile.ZIP_STORED},
                )
                embedded_name = (
                    "NoritoBridge.xcframework/NoritoBridge.artifacts.json"
                )
                if embedded_name not in entries:
                    _fail("Apple archive is missing its embedded artifact manifest")
                embedded = _read_zip_entry(
                    archive,
                    entries[embedded_name],
                    "Apple embedded artifact manifest",
                    MAX_PACKAGE_METADATA_BYTES,
                )
                if embedded != manifest_payload:
                    _fail("Apple detached and embedded artifact manifests differ")
                corrupt = archive.testzip()
                if corrupt is not None:
                    _fail(f"Apple release archive contains a corrupt entry: {corrupt}")
            archive_record = (
                archive_path.name,
                opened.st_size,
                archive_digest,
            )
    except (
        OSError,
        RuntimeError,
        NotImplementedError,
        zipfile.BadZipFile,
        zipfile.LargeZipFile,
    ) as error:
        _fail(f"Apple release archive is unreadable: {error}")

    manifest_record = _record_for_payload(manifest_path.name, manifest_payload)
    podspec_record = _record_for_payload(podspec_path.name, podspec_payload)
    package_manifest_record = _record_for_payload(
        package_manifest_path.name, package_manifest_payload
    )
    checksums_record = _record_for_payload(checksums_path.name, checksums_payload)
    primary = [archive_record, manifest_record, podspec_record]
    _verify_package_manifest(
        package_manifest_payload,
        mode="apple",
        release_tag=release_tag,
        records=[
            ("apple-xcframework", *archive_record),
            ("apple-manifest", *manifest_record),
            ("apple-cocoapods-podspec", *podspec_record),
        ],
    )
    _verify_checksum_inventory(
        checksums_payload,
        "Apple package checksums",
        {
            name: (size, digest)
            for name, size, digest in [*primary, package_manifest_record]
        },
        top_level=True,
    )
    try:
        podspec = podspec_payload.decode("utf-8")
    except UnicodeDecodeError:
        _fail("Apple CocoaPods specification is not UTF-8")
    expected_digest_binding = f":sha256 => '{archive_digest}'"
    expected_version_binding = f"s.version          = '{semver}'"
    expected_url = (
        f"releases/download/{release_tag}/"
        f"NoritoBridge-{release_tag}.xcframework.zip"
    )
    if (
        podspec.count(expected_digest_binding) != 1
        or podspec.count(expected_version_binding) != 1
        or podspec.count(expected_url) != 1
    ):
        _fail("Apple CocoaPods specification is not bound to the exact archive")
    return _inventory_sha256(
        b"iroha.kagemusha.mobile.apple-package.v1",
        [*primary, package_manifest_record, checksums_record],
    )


def verify_android_artifact(args: argparse.Namespace) -> str:
    """Verify every byte and internal member in one Android release package."""

    authorization_sha256 = _sha256(
        args.authorization_sha256, "Android authorization digest"
    )
    source_sha = _sha1(args.source_sha, "Android artifact source SHA")
    release_tag = _release_tag(args.release_tag)
    archive_path = Path(args.archive)
    checksums_path = Path(args.checksums)
    package_manifest_path = Path(args.package_manifest)
    expected_names = {
        archive_path: f"iroha-mobile-sdk-android-{release_tag}.zip",
        checksums_path: f"SHA256SUMS-android-{release_tag}.txt",
        package_manifest_path: f"mobile-sdk-android-{release_tag}.artifacts.json",
    }
    if any(path.name != name for path, name in expected_names.items()):
        _fail("Android package filenames do not match the release tag")
    _require_one_package_directory(list(expected_names), "Android package")
    checksums_payload = _snapshot_regular(
        checksums_path, "Android package checksums", MAX_PACKAGE_METADATA_BYTES
    )
    package_manifest_payload = _snapshot_regular(
        package_manifest_path, "Android package manifest", MAX_PACKAGE_METADATA_BYTES
    )
    root_name = f"iroha-mobile-sdk-android-{release_tag}"
    try:
        with _open_pinned_regular(
            archive_path,
            "Android SDK release archive",
            MAX_ANDROID_ARCHIVE_BYTES,
        ) as (handle, opened):
            archive_digest = _hash_opened_regular(
                handle, opened.st_size, "Android SDK release archive"
            )
            archive_record = (
                archive_path.name,
                opened.st_size,
                archive_digest,
            )
            with zipfile.ZipFile(handle) as archive:
                entries = _validated_zip_entries(
                    archive,
                    label="Android SDK release archive",
                    root_name=root_name,
                    maximum_entry_bytes=MAX_ANDROID_ARCHIVE_BYTES,
                    maximum_total_bytes=MAX_ANDROID_EXPANDED_BYTES,
                    compression_types={zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED},
                )
                prefix = f"{root_name}/"
                file_entries = {
                    name.removeprefix(prefix): info
                    for name, info in entries.items()
                    if not info.is_dir()
                }
                internal_checksums_name = "SHA256SUMS.txt"
                if internal_checksums_name not in file_entries:
                    _fail("Android release archive is missing SHA256SUMS.txt")
                internal_checksums = _read_zip_entry(
                    archive,
                    file_entries[internal_checksums_name],
                    "Android internal checksums",
                    MAX_PACKAGE_METADATA_BYTES,
                )
                expected_payloads = set(file_entries) - {internal_checksums_name}
                parsed_checksums = _parse_checksum_inventory(
                    internal_checksums,
                    "Android internal checksums",
                    top_level=False,
                )
                if set(parsed_checksums) != expected_payloads:
                    _fail(
                        "Android internal checksums do not cover every payload exactly"
                    )
                mandatory = {
                    "client-android/client-android-release.aar",
                    "native/arm64-v8a/libconnect_norito_bridge.so",
                    "native/x86_64/libconnect_norito_bridge.so",
                    "native/native-build-provenance-v1.json",
                }
                core_jars = [
                    name
                    for name in expected_payloads
                    if re.fullmatch(r"core-jvm/core-jvm-[A-Za-z0-9._-]+\.jar", name)
                ]
                if (
                    not mandatory.issubset(expected_payloads)
                    or len(core_jars) != 1
                    or not any(name.startswith("maven/") for name in expected_payloads)
                ):
                    _fail("Android release archive payload inventory is incomplete")
                for name in sorted(expected_payloads):
                    digest = _hash_zip_entry(
                        archive,
                        file_entries[name],
                        f"Android payload {name}",
                    )
                    if parsed_checksums[name] != digest:
                        _fail(f"Android internal checksum mismatch for {name}")
                provenance_name = "native/native-build-provenance-v1.json"
                info = file_entries[provenance_name]
                provenance_payload = _read_zip_entry(
                    archive,
                    info,
                    "Android native provenance",
                    MAX_PACKAGE_METADATA_BYTES,
                )
            manifest = _decode_json(
                provenance_payload, "Android release native provenance"
            )
    except (
        OSError,
        RuntimeError,
        NotImplementedError,
        zipfile.BadZipFile,
        zipfile.LargeZipFile,
    ) as error:
        _fail(f"Android release archive is unreadable: {error}")
    _verify_native_provenance(
        manifest, authorization_sha256, source_sha, "Android release artifact"
    )
    package_manifest_record = _record_for_payload(
        package_manifest_path.name, package_manifest_payload
    )
    checksums_record = _record_for_payload(checksums_path.name, checksums_payload)
    _verify_package_manifest(
        package_manifest_payload,
        mode="android",
        release_tag=release_tag,
        records=[("android-sdk", *archive_record)],
    )
    _verify_checksum_inventory(
        checksums_payload,
        "Android package checksums",
        {
            name: (size, digest)
            for name, size, digest in [archive_record, package_manifest_record]
        },
        top_level=True,
    )
    return _inventory_sha256(
        b"iroha.kagemusha.mobile.android-package.v1",
        [archive_record, package_manifest_record, checksums_record],
    )


def verify_release_inventory(args: argparse.Namespace) -> str:
    """Require and hash the exact eight- or fourteen-file release inventory."""

    release_tag = _release_tag(args.release_tag)
    semver = release_tag[1:]
    root = Path(args.release_root)
    if not root.is_absolute() or Path(os.path.abspath(root)) != root:
        _fail("release asset root must be canonical and absolute")
    try:
        before = root.lstat()
        resolved = root.resolve(strict=True)
    except OSError as error:
        _fail(f"release asset root is unavailable: {error}")
    if (
        resolved != root
        or stat.S_ISLNK(before.st_mode)
        or not stat.S_ISDIR(before.st_mode)
    ):
        _fail("release asset root must be a canonical non-symbolic directory")
    expected = {
        f"NoritoBridge-{release_tag}.xcframework.zip",
        f"NoritoBridge-{release_tag}.artifacts.json",
        f"NoritoBridge-{semver}.podspec",
        f"SHA256SUMS-apple-{release_tag}.txt",
        f"mobile-sdk-apple-{release_tag}.artifacts.json",
        f"iroha-mobile-sdk-android-{release_tag}.zip",
        f"SHA256SUMS-android-{release_tag}.txt",
        f"mobile-sdk-android-{release_tag}.artifacts.json",
    }
    if args.phase == "final":
        expected.update(
            {
                "kagemusha-apple-production-authorization-v1.json",
                "kagemusha-apple-release-verification-report-v1.json",
                "kagemusha-apple-github-attestation-v1.json",
                "kagemusha-android-production-authorization-v1.json",
                "kagemusha-android-release-verification-report-v1.json",
                "kagemusha-android-github-attestation-v1.json",
            }
        )
    try:
        names = {entry.name for entry in root.iterdir()}
    except OSError as error:
        _fail(f"release asset inventory is unavailable: {error}")
    if names != expected:
        _fail(
            f"release asset inventory is not the exact {len(expected)}-file {args.phase} set"
        )
    records: list[tuple[str, int, str]] = []
    with ExitStack() as stack:
        for name in sorted(expected):
            maximum = (
                MAX_ANDROID_ARCHIVE_BYTES
                if name.endswith(".zip")
                else 64 * 1024 * 1024
            )
            label = f"release asset {name}"
            handle, opened = stack.enter_context(
                _open_pinned_regular(root / name, label, maximum)
            )
            records.append(
                (
                    name,
                    opened.st_size,
                    _hash_opened_regular(handle, opened.st_size, label),
                )
            )
        try:
            after = root.lstat()
            final_names = {entry.name for entry in root.iterdir()}
        except OSError as error:
            _fail(f"release asset inventory changed while being hashed: {error}")
        identity = (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_mtime_ns,
            before.st_ctime_ns,
            before.st_nlink,
        )
        if identity != (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_mtime_ns,
            after.st_ctime_ns,
            after.st_nlink,
        ) or final_names != expected:
            _fail("release asset inventory changed while being hashed")
    return _inventory_sha256(
        (
            "iroha.kagemusha.mobile.release-inventory.v1:"
            f"{args.phase}:{release_tag}"
        ).encode("ascii"),
        records,
    )


def _add_coordinates(
    parser: argparse.ArgumentParser,
    *,
    require_trust_digests: bool,
    include_platform: bool = True,
) -> None:
    if include_platform:
        parser.add_argument(
            "--platform", required=True, choices=sorted(PLATFORM_TARGETS)
        )
    parser.add_argument("--repository", required=True)
    parser.add_argument("--workflow-ref", required=True)
    parser.add_argument("--workflow-sha", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--run-attempt", required=True)
    parser.add_argument("--promotion-id", required=True)
    parser.add_argument("--release-tag", required=True)
    parser.add_argument(
        "--reviewed-source-closure-sha256", required=require_trust_digests
    )
    parser.add_argument(
        "--sealed-candidate-build-report-sha256", required=require_trust_digests
    )
    parser.add_argument(
        "--artifact-manifest-sha256", required=require_trust_digests
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Issue or verify an OIDC-attested Kagemusha mobile authorization."
    )
    commands = parser.add_subparsers(dest="command", required=True)
    issue = commands.add_parser("issue")
    _add_coordinates(issue, require_trust_digests=True)
    issue.add_argument("--evidence", required=True)
    issue.add_argument("--release-policy", required=True)
    issue.add_argument("--artifact-root", required=True)
    issue.add_argument("--release-verification-report", required=True)
    issue.add_argument("--output", required=True)
    verify = commands.add_parser("verify")
    _add_coordinates(verify, require_trust_digests=False)
    verify.add_argument("--authorization", required=True)
    verify.add_argument("--release-verification-report")
    pair = commands.add_parser("verify-pair")
    _add_coordinates(
        pair, require_trust_digests=False, include_platform=False
    )
    pair.add_argument("--apple-authorization", required=True)
    pair.add_argument("--android-authorization", required=True)
    pair.add_argument("--apple-release-verification-report", required=True)
    pair.add_argument("--android-release-verification-report", required=True)
    apple = commands.add_parser("verify-apple-artifact")
    apple.add_argument("--archive", required=True)
    apple.add_argument("--manifest", required=True)
    apple.add_argument("--podspec", required=True)
    apple.add_argument("--checksums", required=True)
    apple.add_argument("--package-manifest", required=True)
    apple.add_argument("--release-tag", required=True)
    apple.add_argument("--source-sha", required=True)
    apple.add_argument("--authorization-sha256", required=True)
    android = commands.add_parser("verify-android-artifact")
    android.add_argument("--archive", required=True)
    android.add_argument("--checksums", required=True)
    android.add_argument("--package-manifest", required=True)
    android.add_argument("--release-tag", required=True)
    android.add_argument("--source-sha", required=True)
    android.add_argument("--authorization-sha256", required=True)
    inventory = commands.add_parser("verify-release-inventory")
    inventory.add_argument("--release-root", required=True)
    inventory.add_argument("--release-tag", required=True)
    inventory.add_argument("--phase", required=True, choices=("artifacts", "final"))
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run the authorization issuer or one of its strict consumers."""

    args = _parser().parse_args(argv)
    try:
        if args.command == "issue":
            document = _authorization_document(args)
            _write_new(Path(args.output), _canonical_json(document))
            print(hashlib.sha256(_canonical_json(document)).hexdigest())
        elif args.command == "verify":
            payload = _snapshot_regular(
                Path(args.authorization),
                f"{args.platform} production authorization",
                MAX_AUTHORIZATION_BYTES,
            )
            document = _decode_authorization(payload)
            _validate_authorization(document, args)
            print(hashlib.sha256(payload).hexdigest())
        elif args.command == "verify-pair":
            print(
                _canonical_json(verify_authorization_pair(args)).decode("utf-8"),
                end="",
            )
        elif args.command == "verify-apple-artifact":
            print(verify_apple_artifact(args))
        elif args.command == "verify-android-artifact":
            print(verify_android_artifact(args))
        elif args.command == "verify-release-inventory":
            print(verify_release_inventory(args))
        else:  # pragma: no cover - argparse owns the closed command set.
            _fail("unsupported command")
    except AuthorizationError as error:
        print(f"[kagemusha-mobile-authorization] ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
