#!/usr/bin/env python3
"""Verify one signed archive-only, dual-target Taira release candidate.

This command is deliberately verification-only.  It never deploys, installs,
publishes, or records a receipt as consumed.  A later deployment controller
must atomically consume the returned ``receipt_id`` in the replay ledger before
mutating the testnet.

The candidate consists of a detached signed aggregate manifest and one
``.tar.gz`` archive.  The signed manifest binds the complete archive bytes.  A
canonical manifest inside that archive closes its inventory over:

* one signed native Linux/aarch64 exact-12 privacy authority tuple and its
  rollout archive; and
* one fresh macOS/arm64 receipt proving an exact four-peer cohort and binding
  the deployable reset-manifest, validator binary, supervisor, and four config
  byte identities.

Both targets must bind the same full source commit, Cargo.lock digest, and
canonical workspace-source-manifest digest.  Signatures are verified only by
an independently pinned native verifier.  Candidate scripts and binaries are
treated as data and are never executed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import tarfile
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Mapping, NoReturn, Sequence

try:
    from . import taira_release_authority
    from .release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        canonical_relative_path,
        exclusive_output_fd,
        load_canonical_release_manifest,
        load_json_object,
        parse_sha256sums,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
        stable_read_path,
        stable_read_relative,
    )
    from .release_manifest_signing import (
        ReleaseManifestSignatureError,
        verify_release_manifest,
    )
except ImportError:
    import taira_release_authority
    from release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        canonical_relative_path,
        exclusive_output_fd,
        load_canonical_release_manifest,
        load_json_object,
        parse_sha256sums,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
        stable_read_path,
        stable_read_relative,
    )
    from release_manifest_signing import (
        ReleaseManifestSignatureError,
        verify_release_manifest,
    )


ADMISSION_SCHEMA = "iroha.taira.rollout_admission"
ADMISSION_SCHEMA_VERSION = 1
ADMISSION_MANIFEST_PATH = "taira-rollout-admission-v1.json"
MACOS_RECEIPT_SCHEMA = "iroha.taira.macos_arm64_four_peer_receipt"
MACOS_RECEIPT_SCHEMA_VERSION = 1
MACOS_RECEIPT_PATH = "macos/four-peer-receipt-v1.json"
REPLAY_LEDGER_SCHEMA = "iroha.taira.rollout_admission_replay_ledger"
REPLAY_LEDGER_SCHEMA_VERSION = 1
VERIFICATION_SCHEMA = "iroha.taira.rollout_admission_verification"
VERIFICATION_SCHEMA_VERSION = 1

FINAL_AUTHORITY_FILES = (
    "release_manifest.json",
    "release_manifest.json.pub",
    "release_manifest.json.sig",
)
LINUX_AUTHORITY_DIRECTORY = "linux/authority"
LINUX_AUTHORITY_PAYLOAD = "artifacts/taira-exact12-release-authority-v1.json"
LINUX_AUTHORITY_ARTIFACTS = (
    "release_artifact_contract.py",
    "sorafs-validate",
    "taira-exact12-release-authority-v1.json",
    "taira_release_authority.py",
)
LINUX_AUTHORITY_FILES = (
    "artifacts/SHA256SUMS",
    *(f"artifacts/{name}" for name in LINUX_AUTHORITY_ARTIFACTS),
    "release_manifest.json",
    "release_manifest.json.pub",
    "release_manifest.json.sig",
)
FINAL_ARCHIVE_DIRECTORIES = frozenset(
    {
        "linux",
        "linux/authority",
        "linux/authority/artifacts",
        "macos",
    }
)

PEER_COUNT = 4
MAX_RECEIPT_LIFETIME_SECONDS = 60 * 60
MAX_FUTURE_CLOCK_SKEW_SECONDS = 5 * 60
MAX_JSON_BYTES = 1024 * 1024
MAX_FINAL_ARCHIVE_MEMBERS = 64
MAX_FINAL_ARCHIVE_LOGICAL_BYTES = (
    taira_release_authority.MAX_ARCHIVE_LOGICAL_BYTES + 64 * 1024 * 1024
)
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")


class TairaRolloutAdmissionError(RuntimeError):
    """The archive does not satisfy the first-release admission contract."""


@dataclass(frozen=True)
class SourceIdentity:
    commit: str
    cargo_lock_sha256: str
    workspace_source_manifest_sha256: str

    def as_dict(self) -> dict[str, str]:
        return {
            "cargo_lock_sha256": self.cargo_lock_sha256,
            "commit": self.commit,
            "workspace_source_manifest_sha256": (self.workspace_source_manifest_sha256),
        }


@dataclass(frozen=True)
class ExtractedFile:
    sha256: str
    size: int


@dataclass(frozen=True)
class ReplayLedgerSnapshot:
    """Canonical replay-ledger bytes and the stable file identity they came from."""

    consumed_receipt_ids: tuple[str, ...]
    file: StableFile
    payload: bytes


def _fail(message: str) -> NoReturn:
    raise TairaRolloutAdmissionError(message)


def _exact_fields(value: Mapping[str, object], expected: set[str], label: str) -> None:
    if set(value) != expected:
        missing = sorted(expected - set(value))
        extra = sorted(set(value) - expected)
        _fail(f"{label} fields are not exact: missing={missing}, extra={extra}")


def _sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    return value


def _commit(value: object, label: str) -> str:
    if not isinstance(value, str) or COMMIT_RE.fullmatch(value) is None:
        _fail(f"{label} must be one full lowercase 40-hex commit")
    return value


def _integer(value: object, label: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        _fail(f"{label} must be an integer greater than or equal to {minimum}")
    return value


def _canonical_object(payload: bytes, label: str) -> dict[str, object]:
    if len(payload) > MAX_JSON_BYTES:
        _fail(f"{label} exceeds the {MAX_JSON_BYTES}-byte limit")
    try:
        value = load_json_object(payload, label)
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(str(exc)) from exc
    try:
        canonical = canonical_json_bytes(value)
    except (TypeError, ValueError) as exc:
        raise TairaRolloutAdmissionError(
            f"{label} contains a value outside canonical JSON: {exc}"
        ) from exc
    if canonical != payload:
        _fail(f"{label} is not canonical deterministic JSON")
    return value


def _source_identity(value: object, label: str) -> SourceIdentity:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    _exact_fields(
        value,
        {
            "cargo_lock_sha256",
            "commit",
            "workspace_source_manifest_sha256",
        },
        label,
    )
    return SourceIdentity(
        commit=_commit(value["commit"], f"{label} commit"),
        cargo_lock_sha256=_sha256(
            value["cargo_lock_sha256"], f"{label} Cargo.lock digest"
        ),
        workspace_source_manifest_sha256=_sha256(
            value["workspace_source_manifest_sha256"],
            f"{label} workspace-source-manifest digest",
        ),
    )


def _require_source(
    actual: SourceIdentity, expected: SourceIdentity, label: str
) -> None:
    if actual != expected:
        _fail(
            f"{label} source identity differs from the independently pinned "
            "commit/Cargo.lock/workspace source triple"
        )


def compute_macos_receipt_id(receipt_without_id: Mapping[str, object]) -> str:
    """Return the domain-separated ID for a receipt body without ``receipt_id``."""

    if "receipt_id" in receipt_without_id:
        _fail("receipt ID input must omit receipt_id")
    digest = hashlib.sha256()
    digest.update(b"iroha.taira.macos_arm64_four_peer_receipt.v1\0")
    digest.update(canonical_json_bytes(receipt_without_id))
    return digest.hexdigest()


def canonical_replay_ledger_bytes(consumed_receipt_ids: Sequence[str]) -> bytes:
    """Render the sole canonical first-release replay-ledger representation."""

    consumed = [
        _sha256(value, "consumed receipt ID") for value in consumed_receipt_ids
    ]
    if consumed != sorted(set(consumed)):
        _fail("replay ledger receipt IDs must be unique and canonically sorted")
    return canonical_json_bytes(
        {
            "consumed_receipt_ids": consumed,
            "schema": REPLAY_LEDGER_SCHEMA,
            "schema_version": REPLAY_LEDGER_SCHEMA_VERSION,
        }
    )


def load_replay_ledger(path: Path) -> ReplayLedgerSnapshot:
    """Load one stable canonical replay ledger without consuming a receipt."""

    try:
        info, payload = stable_read_path(path, max_size=MAX_JSON_BYTES)
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(
            f"cannot read the replay ledger: {exc}"
        ) from exc
    ledger = _canonical_object(payload, "Taira admission replay ledger")
    _exact_fields(
        ledger,
        {"consumed_receipt_ids", "schema", "schema_version"},
        "replay ledger",
    )
    if ledger["schema"] != REPLAY_LEDGER_SCHEMA:
        _fail("replay ledger schema identifier is unsupported")
    if (
        _integer(ledger["schema_version"], "replay ledger schema version", minimum=1)
        != REPLAY_LEDGER_SCHEMA_VERSION
    ):
        _fail("replay ledger schema version is unsupported")
    raw_ids = ledger["consumed_receipt_ids"]
    if not isinstance(raw_ids, list):
        _fail("replay ledger consumed_receipt_ids must be an array")
    consumed = [_sha256(value, "consumed receipt ID") for value in raw_ids]
    if canonical_replay_ledger_bytes(consumed) != payload:
        _fail("replay ledger is not the sole canonical first-release representation")
    return ReplayLedgerSnapshot(tuple(consumed), info, payload)


def _validate_point(value: object, label: str) -> tuple[int, str]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    _exact_fields(value, {"block_hash", "height"}, label)
    return (
        _integer(value["height"], f"{label} height"),
        _sha256(value["block_hash"], f"{label} block hash"),
    )


def _validate_macos_receipt(
    payload: bytes,
    *,
    expected_source: SourceIdentity,
    expected_receipt_id: str,
    consumed_receipt_ids: set[str],
    now_unix: int,
) -> dict[str, object]:
    receipt = _canonical_object(payload, "macOS four-peer receipt")
    _exact_fields(
        receipt,
        {
            "end",
            "expires_at_unix",
            "issued_at_unix",
            "peer_count",
            "peers",
            "platform",
            "receipt_id",
            "restart_generation",
            "schema",
            "schema_version",
            "source",
            "start",
            "reset_manifest_sha256",
            "supervisor_sha256",
            "validator_binary_sha256",
            "validator_config_sha256",
        },
        "macOS four-peer receipt",
    )
    if receipt["schema"] != MACOS_RECEIPT_SCHEMA:
        _fail("macOS receipt schema identifier is unsupported")
    if (
        _integer(receipt["schema_version"], "macOS receipt schema version", minimum=1)
        != MACOS_RECEIPT_SCHEMA_VERSION
    ):
        _fail("macOS receipt schema version is unsupported")

    receipt_id = _sha256(receipt["receipt_id"], "macOS receipt ID")
    body = dict(receipt)
    del body["receipt_id"]
    if compute_macos_receipt_id(body) != receipt_id:
        _fail("macOS receipt ID does not match its canonical receipt body")
    if receipt_id != expected_receipt_id:
        _fail("macOS receipt ID does not match the independently expected receipt")
    if receipt_id in consumed_receipt_ids:
        _fail("macOS four-peer receipt has already been consumed")

    source = _source_identity(receipt["source"], "macOS receipt source")
    _require_source(source, expected_source, "macOS receipt")
    platform = receipt["platform"]
    if not isinstance(platform, dict):
        _fail("macOS receipt platform must be an object")
    _exact_fields(platform, {"arch", "os"}, "macOS receipt platform")
    if platform != {"arch": "arm64", "os": "macos"}:
        _fail("macOS receipt platform must be exactly macos/arm64")

    issued = _integer(receipt["issued_at_unix"], "receipt issue time")
    expires = _integer(receipt["expires_at_unix"], "receipt expiry time")
    if expires <= issued:
        _fail("macOS receipt expiry must be after its issue time")
    if expires - issued > MAX_RECEIPT_LIFETIME_SECONDS:
        _fail("macOS receipt lifetime exceeds the first-release maximum")
    if issued > now_unix + MAX_FUTURE_CLOCK_SKEW_SECONDS:
        _fail("macOS receipt issue time is implausibly far in the future")
    if now_unix > expires:
        _fail("macOS four-peer receipt is stale")

    if receipt["peer_count"] != PEER_COUNT:
        _fail("macOS receipt must declare exactly four peers")
    peers = receipt["peers"]
    if not isinstance(peers, list) or len(peers) != PEER_COUNT:
        _fail("macOS receipt must contain exactly four peer rows")
    validator_sha = _sha256(
        receipt["validator_binary_sha256"], "macOS validator binary digest"
    )
    supervisor_sha = _sha256(
        receipt["supervisor_sha256"], "macOS supervisor digest"
    )
    reset_manifest_sha = _sha256(
        receipt["reset_manifest_sha256"], "macOS reset-manifest digest"
    )
    config_digests = receipt["validator_config_sha256"]
    expected_config_names = {
        f"taira-validator-{number}" for number in range(1, PEER_COUNT + 1)
    }
    if not isinstance(config_digests, dict) or set(config_digests) != expected_config_names:
        _fail("macOS receipt must bind the exact four validator config digests")
    normalized_config_digests = {
        name: _sha256(config_digests[name], f"macOS {name} config digest")
        for name in sorted(expected_config_names)
    }
    restart_generation = _sha256(
        receipt["restart_generation"], "macOS restart generation"
    )
    start_height, start_hash = _validate_point(receipt["start"], "receipt start")
    end_height, end_hash = _validate_point(receipt["end"], "receipt end")
    if end_height <= start_height or end_hash == start_hash:
        _fail("macOS receipt must prove advancing consensus to a new block")

    for expected_number, peer in enumerate(peers, start=1):
        if not isinstance(peer, dict):
            _fail("macOS receipt peer rows must be objects")
        _exact_fields(
            peer,
            {
                "final_block_hash",
                "final_height",
                "label",
                "number",
                "restart_proof",
                "source_commit",
                "validator_binary_sha256",
                "validator_config_sha256",
            },
            f"macOS receipt peer {expected_number}",
        )
        expected_label = f"taira-validator-{expected_number}"
        if (
            _integer(
                peer["number"],
                f"macOS receipt peer {expected_number} number",
                minimum=1,
            )
            != expected_number
            or peer["label"] != expected_label
        ):
            _fail("macOS receipt peer rows must be ordered exact peers 1 through 4")
        if peer["final_height"] != end_height or peer["final_block_hash"] != end_hash:
            _fail("every macOS peer must report the exact common final block")
        if peer["source_commit"] != source.commit:
            _fail("every macOS peer must report the exact common source commit")
        if peer["validator_binary_sha256"] != validator_sha:
            _fail("every macOS peer must report the exact validator binary")
        if peer["validator_config_sha256"] != normalized_config_digests[expected_label]:
            _fail("every macOS peer must report its exact validator config")
        if peer["restart_proof"] != "passed":
            _fail("every macOS peer must carry a passed restart proof")

    return {
        "end_block_hash": end_hash,
        "end_height": end_height,
        "receipt_id": receipt_id,
        "reset_manifest_sha256": reset_manifest_sha,
        "restart_generation": restart_generation,
        "supervisor_sha256": supervisor_sha,
        "validator_binary_sha256": validator_sha,
        "validator_config_sha256": normalized_config_digests,
    }


def _safe_member_name(member: tarfile.TarInfo, prefix: str) -> str:
    name = member.name.removesuffix("/") if member.isdir() else member.name
    try:
        canonical_relative_path(name)
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(
            f"candidate archive contains an unsafe member {member.name!r}: {exc}"
        ) from exc
    parts = PurePosixPath(name).parts
    if not parts or parts[0] != prefix:
        _fail("candidate archive member is outside its exact basename prefix")
    return name


def _write_streamed_member(
    archive: tarfile.TarFile,
    member: tarfile.TarInfo,
    destination: Path,
) -> ExtractedFile:
    extracted = archive.extractfile(member)
    if extracted is None:
        _fail(f"cannot read candidate archive member {member.name!r}")
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    digest = hashlib.sha256()
    remaining = member.size
    with exclusive_output_fd(destination, mode=0o600) as descriptor:
        while remaining:
            chunk = extracted.read(min(1024 * 1024, remaining))
            if not chunk:
                _fail(f"candidate archive member {member.name!r} is truncated")
            digest.update(chunk)
            view = memoryview(chunk)
            while view:
                written = os.write(descriptor, view)
                if written <= 0:
                    _fail(f"short write extracting archive member {member.name!r}")
                view = view[written:]
            remaining -= len(chunk)
        if extracted.read(1):
            _fail(f"candidate archive member {member.name!r} exceeds its header")
    return ExtractedFile(sha256=digest.hexdigest(), size=member.size)


def _extract_final_archive(
    archive_path: Path,
    archive_info: StableFile,
    destination: Path,
) -> dict[str, ExtractedFile]:
    prefix = archive_path.name.removesuffix(".tar.gz")
    seen: set[str] = set()
    files: dict[str, ExtractedFile] = {}
    member_count = 0
    logical_bytes = 0
    try:
        with stable_open_relative(
            archive_path.parent,
            archive_path.name,
            expected=archive_info,
        ) as descriptor:
            with os.fdopen(os.dup(descriptor), "rb") as stream:
                with tarfile.open(fileobj=stream, mode="r:gz") as archive:
                    for member in archive:
                        member_count += 1
                        if member_count > MAX_FINAL_ARCHIVE_MEMBERS:
                            _fail("candidate archive exceeds its member-count bound")
                        name = _safe_member_name(member, prefix)
                        if name in seen:
                            _fail(f"candidate archive repeats member {name!r}")
                        seen.add(name)
                        relative_parts = PurePosixPath(name).parts[1:]
                        if member.isdir():
                            if member.size != 0:
                                _fail(
                                    "candidate archive directories must have zero size"
                                )
                            if not relative_parts:
                                continue
                            relative_directory = PurePosixPath(
                                *relative_parts
                            ).as_posix()
                            if relative_directory not in FINAL_ARCHIVE_DIRECTORIES:
                                _fail(
                                    "candidate archive contains a directory outside "
                                    "its exact first-release layout"
                                )
                            (destination / Path(*relative_parts)).mkdir(
                                mode=0o700, parents=True, exist_ok=True
                            )
                            continue
                        if not member.isfile() or member.issparse():
                            _fail(
                                "candidate archive must not contain links, sparse "
                                "files, devices, FIFOs, or sockets"
                            )
                        if not relative_parts or member.size <= 0:
                            _fail("candidate archive regular files must be non-empty")
                        logical_bytes += member.size
                        if logical_bytes > MAX_FINAL_ARCHIVE_LOGICAL_BYTES:
                            _fail("candidate archive exceeds its logical-size bound")
                        relative = PurePosixPath(*relative_parts).as_posix()
                        files[relative] = _write_streamed_member(
                            archive,
                            member,
                            destination / Path(*relative_parts),
                        )
    except (OSError, tarfile.TarError, ReleaseArtifactError) as exc:
        raise TairaRolloutAdmissionError(
            f"cannot safely inspect the candidate archive: {exc}"
        ) from exc
    return files


def _validate_inventory(
    manifest: Mapping[str, object], actual: Mapping[str, ExtractedFile]
) -> None:
    raw_inventory = manifest["inventory"]
    if not isinstance(raw_inventory, list) or not raw_inventory:
        _fail("admission inventory must be a non-empty array")
    rows: list[dict[str, object]] = []
    for index, raw in enumerate(raw_inventory):
        if not isinstance(raw, dict):
            _fail(f"admission inventory row {index} must be an object")
        _exact_fields(raw, {"path", "sha256", "size"}, "admission inventory row")
        path = raw["path"]
        if not isinstance(path, str):
            _fail("admission inventory path must be a string")
        try:
            path = canonical_relative_path(path)
        except ReleaseArtifactError as exc:
            raise TairaRolloutAdmissionError(str(exc)) from exc
        digest = _sha256(raw["sha256"], f"inventory digest for {path}")
        size = _integer(raw["size"], f"inventory size for {path}", minimum=1)
        rows.append({"path": path, "sha256": digest, "size": size})
    if rows != sorted(rows, key=lambda row: str(row["path"])):
        _fail("admission inventory rows must be canonically sorted")
    paths = [str(row["path"]) for row in rows]
    if len(paths) != len(set(paths)):
        _fail("admission inventory contains duplicate paths")
    actual_without_manifest = {
        path: info for path, info in actual.items() if path != ADMISSION_MANIFEST_PATH
    }
    if set(paths) != set(actual_without_manifest):
        missing = sorted(set(actual_without_manifest) - set(paths))
        extra = sorted(set(paths) - set(actual_without_manifest))
        _fail(f"admission inventory is not closed: missing={missing}, extra={extra}")
    for row in rows:
        info = actual_without_manifest[str(row["path"])]
        if row["sha256"] != info.sha256 or row["size"] != info.size:
            _fail(f"admission inventory digest/size mismatch for {row['path']!r}")


def _validate_admission_manifest(
    payload: bytes,
    *,
    actual_inventory: Mapping[str, ExtractedFile],
    expected_source: SourceIdentity,
    expected_receipt_id: str,
    trusted_signing_fingerprint: str,
    trusted_release_manifest_verifier_sha256: str,
) -> dict[str, object]:
    manifest = _canonical_object(payload, "Taira rollout admission manifest")
    _exact_fields(
        manifest,
        {
            "inventory",
            "linux_arm64",
            "macos_arm64",
            "schema",
            "schema_version",
            "source",
            "trust",
        },
        "Taira rollout admission manifest",
    )
    if manifest["schema"] != ADMISSION_SCHEMA:
        _fail("admission manifest schema identifier is unsupported")
    if (
        _integer(manifest["schema_version"], "admission schema version", minimum=1)
        != ADMISSION_SCHEMA_VERSION
    ):
        _fail("admission manifest schema version is unsupported")
    source = _source_identity(manifest["source"], "admission manifest source")
    _require_source(source, expected_source, "admission manifest")

    trust = manifest["trust"]
    if not isinstance(trust, dict):
        _fail("admission manifest trust must be an object")
    _exact_fields(
        trust,
        {"release_manifest_verifier_sha256", "signer_fingerprint_sha256"},
        "admission manifest trust",
    )
    if trust != {
        "release_manifest_verifier_sha256": (trusted_release_manifest_verifier_sha256),
        "signer_fingerprint_sha256": trusted_signing_fingerprint,
    }:
        _fail("admission manifest trust roots differ from independent pins")

    linux = manifest["linux_arm64"]
    if not isinstance(linux, dict):
        _fail("admission Linux target must be an object")
    _exact_fields(
        linux,
        {
            "arch",
            "archive_path",
            "authority_directory",
            "authority_manifest_sha256",
            "authority_native_verifier_sha256",
            "os",
        },
        "admission Linux target",
    )
    if linux["os"] != "linux" or linux["arch"] != "aarch64":
        _fail("admission Linux target must be exactly linux/aarch64")
    archive_path = linux["archive_path"]
    if not isinstance(archive_path, str):
        _fail("admission Linux archive path must be a string")
    try:
        archive_path = canonical_relative_path(archive_path)
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(str(exc)) from exc
    archive_pure = PurePosixPath(archive_path)
    if archive_pure.parent.as_posix() != "linux" or not archive_path.endswith(
        ".tar.gz"
    ):
        _fail("admission Linux archive must be one direct linux/*.tar.gz file")
    if linux["authority_directory"] != LINUX_AUTHORITY_DIRECTORY:
        _fail("admission Linux authority directory is not canonical")
    _sha256(linux["authority_manifest_sha256"], "Linux authority manifest digest")
    _sha256(
        linux["authority_native_verifier_sha256"],
        "Linux authority native verifier digest",
    )

    macos = manifest["macos_arm64"]
    if not isinstance(macos, dict):
        _fail("admission macOS target must be an object")
    _exact_fields(
        macos,
        {"arch", "os", "receipt_id", "receipt_path"},
        "admission macOS target",
    )
    if macos != {
        "arch": "arm64",
        "os": "macos",
        "receipt_id": expected_receipt_id,
        "receipt_path": MACOS_RECEIPT_PATH,
    }:
        _fail("admission macOS target is not the exact expected receipt")

    _validate_inventory(manifest, actual_inventory)
    required_paths = {
        archive_path,
        MACOS_RECEIPT_PATH,
        *(f"{LINUX_AUTHORITY_DIRECTORY}/{path}" for path in LINUX_AUTHORITY_FILES),
    }
    actual_paths = set(actual_inventory) - {ADMISSION_MANIFEST_PATH}
    if actual_paths != required_paths:
        _fail("admission archive does not contain the exact first-release inventory")
    return {
        "linux_archive_path": archive_path,
        "linux_authority_manifest_sha256": linux["authority_manifest_sha256"],
        "linux_native_verifier_sha256": linux["authority_native_verifier_sha256"],
    }


def _artifact_descriptor(path: str) -> tuple[str, str, str, str]:
    if path == "taira-exact12-release-authority-v1.json":
        return ("iroha3", "taira-exact12", "release-evidence", "json")
    if path == "sorafs-validate":
        return ("iroha3", "taira-authority", "reference-validator", "binary")
    return ("iroha3", "taira-authority", "release-evidence", "binary")


def _verify_closed_linux_authority(
    root: Path,
    *,
    expected_source: SourceIdentity,
    expected_manifest_sha256: str,
    expected_native_verifier_sha256: str,
    trusted_signing_fingerprint: str,
    release_manifest_verifier_path: Path,
    trusted_release_manifest_verifier_sha256: str,
    linux_archive_path: Path,
) -> dict[str, object]:
    authority_root = root / LINUX_AUTHORITY_DIRECTORY
    try:
        inventory = scan_inventory_paths(authority_root)
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(
            f"cannot scan nested Linux authority: {exc}"
        ) from exc
    if inventory != sorted(LINUX_AUTHORITY_FILES):
        _fail("nested Linux authority inventory is not exact")

    manifest_path = authority_root / "release_manifest.json"
    signature_path = authority_root / "release_manifest.json.sig"
    public_key_path = authority_root / "release_manifest.json.pub"
    try:
        verification = verify_release_manifest(
            manifest_path,
            signature_path,
            public_key_path,
            trusted_signing_fingerprint,
            release_manifest_verifier_path,
            trusted_release_manifest_verifier_sha256,
        )
    except ReleaseManifestSignatureError as exc:
        raise TairaRolloutAdmissionError(
            f"nested Linux authority signature verification failed: {exc}"
        ) from exc
    if verification["manifest_sha256"] != expected_manifest_sha256:
        _fail("nested Linux authority manifest differs from the admission manifest")

    try:
        _, manifest_payload = stable_read_path(manifest_path, max_size=MAX_JSON_BYTES)
        manifest = load_canonical_release_manifest(manifest_payload)
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(
            f"nested Linux authority manifest is invalid: {exc}"
        ) from exc
    if (
        manifest["commit"] != expected_source.commit
        or manifest["os"] != "linux"
        or manifest["arch"] != "aarch64"
        or manifest["version"]
        != f"taira-{expected_source.workspace_source_manifest_sha256[:16]}"
    ):
        _fail("nested Linux authority manifest has the wrong source or platform")

    artifacts_root = authority_root / "artifacts"
    try:
        checksums = parse_sha256sums(artifacts_root)
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(
            f"nested Linux authority SHA256SUMS is invalid: {exc}"
        ) from exc
    if set(checksums) != set(LINUX_AUTHORITY_ARTIFACTS):
        _fail("nested Linux authority checksum inventory is not exact")
    expected_rows: list[dict[str, object]] = []
    for name in sorted(LINUX_AUTHORITY_ARTIFACTS):
        try:
            info = stable_hash_relative(artifacts_root, name)
        except ReleaseArtifactError as exc:
            raise TairaRolloutAdmissionError(
                f"cannot hash nested Linux authority artifact {name!r}: {exc}"
            ) from exc
        if checksums[name] != info.sha256:
            _fail(f"nested Linux authority checksum mismatch for {name!r}")
        profile, target, kind, fmt = _artifact_descriptor(name)
        expected_rows.append(
            {
                "format": fmt,
                "kind": kind,
                "path": name,
                "profile": profile,
                "sha256": info.sha256,
                "size": info.size,
                "target": target,
            }
        )
    if manifest["artifacts"] != expected_rows:
        _fail("nested Linux signed manifest artifact rows are not exact")

    payload_path = artifacts_root / "taira-exact12-release-authority-v1.json"
    try:
        _, authority_payload = stable_read_path(
            payload_path, max_size=taira_release_authority.MAX_AUTHORITY_BYTES
        )
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(
            f"cannot read nested exact-12 authority: {exc}"
        ) from exc
    authority = _canonical_object(authority_payload, "nested exact-12 authority")
    if authority.get("schema") != taira_release_authority.SCHEMA:
        _fail("nested exact-12 authority schema is unsupported")
    if authority.get("schema_version") != taira_release_authority.SCHEMA_VERSION:
        _fail("nested exact-12 authority schema version is unsupported")
    if authority.get("commit") != expected_source.commit:
        _fail("nested exact-12 authority commit differs from the pinned source")
    if (
        authority.get("workspace_source_manifest_sha256")
        != expected_source.workspace_source_manifest_sha256
    ):
        _fail("nested exact-12 authority workspace source digest differs")
    if (
        authority.get("signing_authority_fingerprint_sha256")
        != trusted_signing_fingerprint
    ):
        _fail("nested exact-12 authority signer identity differs")
    if authority.get("native_verifier_sha256") != expected_native_verifier_sha256:
        _fail("nested exact-12 authority native verifier identity differs")
    if checksums["sorafs-validate"] != expected_native_verifier_sha256:
        _fail("nested Linux verifier bytes differ from their authority identity")

    evidence = authority.get("native_release_evidence")
    if not isinstance(evidence, list):
        _fail("nested exact-12 authority native evidence must be an array")
    cargo_rows = [
        row
        for row in evidence
        if isinstance(row, dict)
        and row.get("path") == taira_release_authority.EVIDENCE_PATHS["cargo_lock"]
    ]
    if len(cargo_rows) != 1:
        _fail("nested exact-12 authority must bind exactly one Cargo.lock")
    if cargo_rows[0].get("sha256") != expected_source.cargo_lock_sha256:
        _fail("nested exact-12 authority Cargo.lock digest differs")

    subject = authority.get("subject")
    if not isinstance(subject, dict):
        _fail("nested exact-12 authority subject must be an object")
    linux_info = stable_hash_path(linux_archive_path)
    if subject != {
        "kind": "taira-rollout-tar-gzip-v1",
        "name": linux_archive_path.name,
        "sha256": linux_info.sha256,
        "size": linux_info.size,
    }:
        _fail("nested exact-12 authority does not bind the exact Linux archive")

    return {
        "authority": authority,
        "native_verifier_sha256": expected_native_verifier_sha256,
        "manifest_sha256": verification["manifest_sha256"],
    }


def _extract_linux_evidence(archive_path: Path, destination: Path) -> None:
    prefix = archive_path.name.removesuffix(".tar.gz")
    expected = {
        f"{prefix}/{relative}": relative
        for relative in taira_release_authority.EVIDENCE_PATHS.values()
    }
    seen: set[str] = set()
    extracted_expected: set[str] = set()
    member_count = 0
    logical_bytes = 0
    archive_info = stable_hash_path(archive_path)
    try:
        with stable_open_relative(
            archive_path.parent, archive_path.name, expected=archive_info
        ) as descriptor:
            with os.fdopen(os.dup(descriptor), "rb") as stream:
                with tarfile.open(fileobj=stream, mode="r:gz") as archive:
                    for member in archive:
                        member_count += 1
                        if member_count > taira_release_authority.MAX_ARCHIVE_MEMBERS:
                            _fail("nested Linux archive exceeds its member-count bound")
                        name = _safe_member_name(member, prefix)
                        if name in seen:
                            _fail(f"nested Linux archive repeats member {name!r}")
                        seen.add(name)
                        if member.isdir():
                            if member.size != 0:
                                _fail(
                                    "nested Linux archive directories must have zero size"
                                )
                            if name in expected:
                                _fail("nested Linux evidence must be a regular file")
                            continue
                        if not member.isfile() or member.issparse():
                            _fail(
                                "nested Linux archive contains a link, sparse file, "
                                "device, FIFO, or socket"
                            )
                        logical_bytes += member.size
                        if (
                            logical_bytes
                            > taira_release_authority.MAX_ARCHIVE_LOGICAL_BYTES
                        ):
                            _fail("nested Linux archive exceeds its logical-size bound")
                        relative = expected.get(name)
                        if relative is None:
                            continue
                        if member.size <= 0:
                            _fail("nested Linux evidence files must be non-empty")
                        _write_streamed_member(
                            archive,
                            member,
                            destination / Path(*PurePosixPath(relative).parts),
                        )
                        extracted_expected.add(name)
    except (OSError, tarfile.TarError, ReleaseArtifactError) as exc:
        raise TairaRolloutAdmissionError(
            f"cannot safely inspect the nested Linux archive: {exc}"
        ) from exc
    missing = sorted(set(expected) - extracted_expected)
    if missing:
        _fail(f"nested Linux archive omits exact-12 evidence: {missing}")


def _verify_existing_linux_authority(
    root: Path,
    *,
    linux_archive_path: Path,
    expected_source: SourceIdentity,
    trusted_signing_fingerprint: str,
    native_verifier_sha256: str,
) -> None:
    evidence_root = root / "linux-evidence"
    evidence_root.mkdir(mode=0o700)
    _extract_linux_evidence(linux_archive_path, evidence_root)
    authority_path = root / LINUX_AUTHORITY_DIRECTORY / LINUX_AUTHORITY_PAYLOAD
    result = taira_release_authority.main(
        [
            "verify",
            "--evidence-root",
            str(evidence_root),
            "--commit",
            expected_source.commit,
            "--signing-fingerprint",
            trusted_signing_fingerprint,
            "--native-verifier-sha256",
            native_verifier_sha256,
            "--archive",
            str(linux_archive_path),
            "--authority",
            str(authority_path),
        ]
    )
    if result != 0:
        _fail("existing exact-12 release-authority verification rejected Linux")


def _verify_final_authority(
    archive_path: Path,
    archive_info: StableFile,
    authority_dir: Path,
    *,
    expected_source: SourceIdentity,
    trusted_signing_fingerprint: str,
    release_manifest_verifier_path: Path,
    trusted_release_manifest_verifier_sha256: str,
) -> tuple[dict[str, object], dict[str, StableFile]]:
    try:
        inventory = scan_inventory_paths(authority_dir)
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(
            f"cannot scan final authority directory: {exc}"
        ) from exc
    if inventory != list(FINAL_AUTHORITY_FILES):
        _fail("final authority directory must contain exactly manifest/signature/key")
    captured = {
        relative: stable_hash_relative(authority_dir, relative)
        for relative in FINAL_AUTHORITY_FILES
    }
    manifest_path = authority_dir / "release_manifest.json"
    try:
        verification = verify_release_manifest(
            manifest_path,
            authority_dir / "release_manifest.json.sig",
            authority_dir / "release_manifest.json.pub",
            trusted_signing_fingerprint,
            release_manifest_verifier_path,
            trusted_release_manifest_verifier_sha256,
        )
        _, payload = stable_read_path(manifest_path, max_size=MAX_JSON_BYTES)
        manifest = load_canonical_release_manifest(payload)
    except (ReleaseArtifactError, ReleaseManifestSignatureError) as exc:
        raise TairaRolloutAdmissionError(
            f"final archive authority verification failed: {exc}"
        ) from exc
    expected_artifact = {
        "format": "tar.gz",
        "kind": "reference-validator",
        "path": archive_path.name,
        "profile": "iroha3",
        "sha256": archive_info.sha256,
        "size": archive_info.size,
        "target": "taira-rollout-admission-v1",
    }
    if (
        manifest["commit"] != expected_source.commit
        or manifest["os"] != "macos"
        or manifest["arch"] != "arm64"
        or manifest["version"]
        != f"taira-admission-{expected_source.workspace_source_manifest_sha256[:16]}"
        or manifest["artifacts"] != [expected_artifact]
    ):
        _fail("final signed manifest does not bind the exact macOS archive candidate")
    if verification["manifest_sha256"] != captured["release_manifest.json"].sha256:
        _fail("final authority manifest changed across signature verification")
    return verification, captured


def _recheck_files(root: Path, captured: Mapping[str, StableFile], label: str) -> None:
    for relative, before in captured.items():
        try:
            after = stable_hash_relative(root, relative)
        except ReleaseArtifactError as exc:
            raise TairaRolloutAdmissionError(
                f"cannot recheck {label} {relative!r}: {exc}"
            ) from exc
        if after != before:
            _fail(f"{label} {relative!r} changed during admission verification")


def verify_admission(
    *,
    archive_path: Path,
    authority_dir: Path,
    expected_source: SourceIdentity,
    expected_receipt_id: str,
    replay_ledger_path: Path,
    trusted_signing_fingerprint: str,
    release_manifest_verifier_path: Path,
    trusted_release_manifest_verifier_sha256: str,
    now_unix: int | None = None,
) -> dict[str, object]:
    """Verify the complete dual-target candidate without applying it."""

    expected_source = SourceIdentity(
        commit=_commit(expected_source.commit, "expected source commit"),
        cargo_lock_sha256=_sha256(
            expected_source.cargo_lock_sha256, "expected Cargo.lock digest"
        ),
        workspace_source_manifest_sha256=_sha256(
            expected_source.workspace_source_manifest_sha256,
            "expected workspace-source-manifest digest",
        ),
    )
    expected_receipt_id = _sha256(expected_receipt_id, "expected receipt ID")
    trusted_signing_fingerprint = _sha256(
        trusted_signing_fingerprint, "trusted signing fingerprint"
    )
    trusted_release_manifest_verifier_sha256 = _sha256(
        trusted_release_manifest_verifier_sha256,
        "trusted release-manifest verifier digest",
    )
    if archive_path.name.removesuffix(".tar.gz") == archive_path.name:
        _fail("candidate archive must use the .tar.gz format")
    archive_path = Path(os.path.abspath(archive_path))
    authority_dir = Path(os.path.abspath(authority_dir))
    replay_ledger_path = Path(os.path.abspath(replay_ledger_path))
    release_manifest_verifier_path = Path(
        os.path.abspath(release_manifest_verifier_path)
    )
    if (
        len(
            {
                archive_path,
                authority_dir,
                replay_ledger_path,
                release_manifest_verifier_path,
            }
        )
        != 4
    ):
        _fail("archive, authority, replay ledger, and verifier paths must be distinct")

    try:
        archive_info = stable_hash_path(
            archive_path, max_size=MAX_FINAL_ARCHIVE_LOGICAL_BYTES
        )
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(
            f"cannot hash the final candidate archive: {exc}"
        ) from exc
    final_verification, final_authority_state = _verify_final_authority(
        archive_path,
        archive_info,
        authority_dir,
        expected_source=expected_source,
        trusted_signing_fingerprint=trusted_signing_fingerprint,
        release_manifest_verifier_path=release_manifest_verifier_path,
        trusted_release_manifest_verifier_sha256=(
            trusted_release_manifest_verifier_sha256
        ),
    )
    replay_ledger = load_replay_ledger(replay_ledger_path)
    current_time = int(time.time()) if now_unix is None else now_unix
    _integer(current_time, "current Unix time")

    with tempfile.TemporaryDirectory(prefix="taira-rollout-admission-") as raw_temp:
        # macOS exposes its temporary root through ``/var`` -> ``/private/var``.
        # The shared artifact helpers intentionally reject every symlink path
        # component, so pin the physical directory created by tempfile before
        # any descriptor-anchored reads or writes.
        root = Path(raw_temp).resolve(strict=True)
        actual_inventory = _extract_final_archive(archive_path, archive_info, root)
        admission_info = actual_inventory.get(ADMISSION_MANIFEST_PATH)
        if admission_info is None:
            _fail("candidate archive omits its canonical admission manifest")
        _, admission_payload = stable_read_relative(
            root,
            ADMISSION_MANIFEST_PATH,
            max_size=MAX_JSON_BYTES,
            return_payload=True,
        )
        assert admission_payload is not None
        admission = _validate_admission_manifest(
            admission_payload,
            actual_inventory=actual_inventory,
            expected_source=expected_source,
            expected_receipt_id=expected_receipt_id,
            trusted_signing_fingerprint=trusted_signing_fingerprint,
            trusted_release_manifest_verifier_sha256=(
                trusted_release_manifest_verifier_sha256
            ),
        )

        _, receipt_payload = stable_read_relative(
            root,
            MACOS_RECEIPT_PATH,
            max_size=MAX_JSON_BYTES,
            return_payload=True,
        )
        assert receipt_payload is not None
        receipt = _validate_macos_receipt(
            receipt_payload,
            expected_source=expected_source,
            expected_receipt_id=expected_receipt_id,
            consumed_receipt_ids=set(replay_ledger.consumed_receipt_ids),
            now_unix=current_time,
        )

        linux_archive_path = root / str(admission["linux_archive_path"])
        nested = _verify_closed_linux_authority(
            root,
            expected_source=expected_source,
            expected_manifest_sha256=str(admission["linux_authority_manifest_sha256"]),
            expected_native_verifier_sha256=str(
                admission["linux_native_verifier_sha256"]
            ),
            trusted_signing_fingerprint=trusted_signing_fingerprint,
            release_manifest_verifier_path=release_manifest_verifier_path,
            trusted_release_manifest_verifier_sha256=(
                trusted_release_manifest_verifier_sha256
            ),
            linux_archive_path=linux_archive_path,
        )
        _verify_existing_linux_authority(
            root,
            linux_archive_path=linux_archive_path,
            expected_source=expected_source,
            trusted_signing_fingerprint=trusted_signing_fingerprint,
            native_verifier_sha256=str(nested["native_verifier_sha256"]),
        )
        try:
            nested_inventory = scan_inventory_paths(root / LINUX_AUTHORITY_DIRECTORY)
        except ReleaseArtifactError as exc:
            raise TairaRolloutAdmissionError(
                f"cannot recheck nested Linux authority inventory: {exc}"
            ) from exc
        if nested_inventory != sorted(LINUX_AUTHORITY_FILES):
            _fail("nested Linux authority inventory changed during verification")

        for relative, extracted in actual_inventory.items():
            try:
                rechecked = stable_hash_relative(root, relative)
            except ReleaseArtifactError as exc:
                raise TairaRolloutAdmissionError(
                    f"cannot recheck extracted candidate {relative!r}: {exc}"
                ) from exc
            if rechecked.sha256 != extracted.sha256 or rechecked.size != extracted.size:
                _fail(f"extracted candidate {relative!r} changed during verification")

    if stable_hash_path(archive_path) != archive_info:
        _fail("candidate archive changed during admission verification")
    _recheck_files(authority_dir, final_authority_state, "final authority file")
    try:
        final_inventory = scan_inventory_paths(authority_dir)
    except ReleaseArtifactError as exc:
        raise TairaRolloutAdmissionError(
            f"cannot recheck final authority inventory: {exc}"
        ) from exc
    if final_inventory != list(FINAL_AUTHORITY_FILES):
        _fail("final authority inventory changed during verification")
    if stable_hash_path(replay_ledger_path) != replay_ledger.file:
        _fail("replay ledger changed during admission verification")

    return {
        "archive_sha256": archive_info.sha256,
        "deployment_performed": False,
        "linux_authority_manifest_sha256": nested["manifest_sha256"],
        "macos_end_block_hash": receipt["end_block_hash"],
        "macos_end_height": receipt["end_height"],
        "peer_count": PEER_COUNT,
        "receipt_id": receipt["receipt_id"],
        "reset_manifest_sha256": receipt["reset_manifest_sha256"],
        "release_manifest_sha256": final_verification["manifest_sha256"],
        "release_manifest_verifier_sha256": (trusted_release_manifest_verifier_sha256),
        "schema": VERIFICATION_SCHEMA,
        "schema_version": VERIFICATION_SCHEMA_VERSION,
        "signer_fingerprint_sha256": trusted_signing_fingerprint,
        "source": expected_source.as_dict(),
        "restart_generation": receipt["restart_generation"],
        "supervisor_sha256": receipt["supervisor_sha256"],
        "validator_binary_sha256": receipt["validator_binary_sha256"],
        "validator_config_sha256": receipt["validator_config_sha256"],
        "verified": True,
    }


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    subparsers = parser.add_subparsers(dest="command", required=True)
    verify = subparsers.add_parser("verify", allow_abbrev=False)
    verify.add_argument("--archive", type=Path, required=True)
    verify.add_argument("--authority-dir", type=Path, required=True)
    verify.add_argument("--expected-source-commit", required=True)
    verify.add_argument("--expected-cargo-lock-sha256", required=True)
    verify.add_argument("--expected-workspace-source-manifest-sha256", required=True)
    verify.add_argument("--expected-receipt-id", required=True)
    verify.add_argument("--replay-ledger", type=Path, required=True)
    verify.add_argument("--trusted-signing-fingerprint", required=True)
    verify.add_argument("--release-manifest-verifier", type=Path, required=True)
    verify.add_argument("--trusted-release-manifest-verifier-sha256", required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        source = SourceIdentity(
            commit=_commit(args.expected_source_commit, "expected source commit"),
            cargo_lock_sha256=_sha256(
                args.expected_cargo_lock_sha256, "expected Cargo.lock digest"
            ),
            workspace_source_manifest_sha256=_sha256(
                args.expected_workspace_source_manifest_sha256,
                "expected workspace-source-manifest digest",
            ),
        )
        result = verify_admission(
            archive_path=args.archive,
            authority_dir=args.authority_dir,
            expected_source=source,
            expected_receipt_id=args.expected_receipt_id,
            replay_ledger_path=args.replay_ledger,
            trusted_signing_fingerprint=args.trusted_signing_fingerprint,
            release_manifest_verifier_path=args.release_manifest_verifier,
            trusted_release_manifest_verifier_sha256=(
                args.trusted_release_manifest_verifier_sha256
            ),
        )
    except (
        OSError,
        ReleaseArtifactError,
        ReleaseManifestSignatureError,
        TairaRolloutAdmissionError,
        tarfile.TarError,
    ) as exc:
        print(f"Taira rollout admission refused: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
