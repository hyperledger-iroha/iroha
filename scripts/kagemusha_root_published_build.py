#!/usr/bin/env python3
"""Admit root-published Kagemusha build artifacts.

Production compilation runs under the dedicated no-login ``boi-build`` UID.
The privileged supervisor copies worker outputs from stable descriptors into a
root-only staging directory and atomically publishes this three-file set:

* one executable;
* one canonical sealed-build report;
* one independently pinned publication receipt.

The receipt is excluded from the directory tree digest to avoid a digest cycle.
It is still root-owned, non-writable, singly linked, canonical JSON, and bound
by the independently supplied receipt SHA-256.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import stat
from typing import Any, Mapping

from scripts import kagemusha_source_tree_seal as source_seal


PUBLICATION_PROTOCOL = "iroha.kagemusha.distinct_uid_root_atomic_publish.v1"
CANDIDATE_RECEIPT_SCHEMA = "iroha.kagemusha.root_published_candidate_build.v1"
PROMOTION_VERIFIER_RECEIPT_SCHEMA = (
    "iroha.kagemusha.root_published_promotion_verifier.v1"
)
CANDIDATE_REPORT_SCHEMA = "iroha.kagemusha.sealed_candidate_build.v1"
PROMOTION_VERIFIER_REPORT_SCHEMA = (
    "iroha.kagemusha.sealed_promotion_verifier_build.v1"
)
CANDIDATE_RECEIPT_FILE_NAME = "root-published-candidate-build.json"
PROMOTION_VERIFIER_RECEIPT_FILE_NAME = (
    "root-published-promotion-verifier.json"
)
CANDIDATE_BINARY_FILE_NAME = "kagemusha_recursive_spend_v4_bundle"
PROMOTION_VERIFIER_FILE_NAME = "kagami"
CANDIDATE_REPORT_FILE_NAME = "sealed-kagemusha-candidate-build.json"
PROMOTION_VERIFIER_REPORT_FILE_NAME = (
    "sealed-kagemusha-promotion-verifier-build.json"
)
BUILD_USER_NAME = "boi-build"
ARTIFACT_TREE_DOMAIN = b"iroha.kagemusha.root-published-build-artifact.v1\0"
MAX_RECEIPT_BYTES = 64 * 1024
MAX_REPORT_BYTES = 16 * 1024 * 1024
TRUSTED_OWNER_UID = 0
MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES = 24 * 1024**3
MAX_RECORDED_PHYSICAL_MEMORY_BYTES = 1024**6

CANDIDATE_RECEIPT_KEYS = {
    "artifact_root",
    "artifact_tree_sha256",
    "binary_path",
    "binary_sha256",
    "binary_size_bytes",
    "build_uid",
    "build_user_name",
    "production_closure_tree_sha256",
    "publication_protocol",
    "reviewed_source_closure_descriptor_sha256",
    "schema",
    "sealed_build_report_path",
    "sealed_build_report_sha256",
    "source_commit",
    "source_tree_sha256",
    "toolchain_provenance_sha256",
}
PROMOTION_VERIFIER_RECEIPT_KEYS = {
    "artifact_root",
    "artifact_tree_sha256",
    "build_uid",
    "build_user_name",
    "production_closure_tree_sha256",
    "promotion_verifier_path",
    "promotion_verifier_sha256",
    "promotion_verifier_size_bytes",
    "publication_protocol",
    "reviewed_source_closure_descriptor_sha256",
    "schema",
    "sealed_build_report_path",
    "sealed_build_report_sha256",
    "source_commit",
    "source_tree_sha256",
    "toolchain_provenance_sha256",
}
CANDIDATE_REPORT_KEYS = {
    "apple_developer_dir_path",
    "apple_sdk_path",
    "binary_path",
    "binary_sha256",
    "binary_size_bytes",
    "build_profile",
    "build_uid",
    "build_user_name",
    "cargo_home_path",
    "cargo_path",
    "cargo_sha256",
    "cargo_vendor_path",
    "clang_resource_dir_path",
    "git_exec_path",
    "git_path",
    "git_sha256",
    "gpg_path",
    "gpg_sha256",
    "linker_path",
    "linker_sha256",
    "minimum_build_physical_memory_bytes",
    "physical_memory_bytes_at_admission",
    "production_closure_root",
    "production_closure_tree_sha256",
    "publication_status",
    "python_path",
    "python_sha256",
    "reviewed_source_closure",
    "reviewed_source_closure_descriptor_sha256",
    "rustc_path",
    "rustc_sha256",
    "rustc_sysroot_path",
    "schema",
    "source_commit",
    "source_repo_dirty",
    "source_signing_key_fingerprint",
    "source_tree_sha256",
    "target_dir",
    "toolchain_provenance_sha256",
}
PROMOTION_VERIFIER_REPORT_KEYS = {
    "build_uid",
    "build_user_name",
    "production_closure_tree_sha256",
    "promotion_verifier_path",
    "promotion_verifier_sha256",
    "promotion_verifier_size_bytes",
    "publication_status",
    "reviewed_source_closure_descriptor_sha256",
    "schema",
    "source_commit",
    "source_tree_sha256",
    "toolchain_provenance_sha256",
}


class PublishedBuildError(RuntimeError):
    """A root-published build artifact failed closed."""


@dataclass(frozen=True)
class AdmittedPublishedBuild:
    """One immutable executable and its root publication evidence."""

    receipt: Path
    receipt_sha256: str
    artifact_root: Path
    artifact_tree_sha256: str
    executable: Path
    executable_sha256: str
    executable_size_bytes: int
    sealed_build_report: Path
    sealed_build_report_sha256: str
    build_user_name: str
    build_uid: int
    production_closure_tree_sha256: str
    toolchain_provenance_sha256: str
    reviewed_source_closure_descriptor_sha256: str
    source_commit: str
    source_tree_sha256: str


@dataclass(frozen=True)
class _ArtifactContract:
    receipt_schema: str
    report_schema: str
    receipt_file_name: str
    executable_file_name: str
    report_file_name: str
    executable_path_key: str
    executable_sha256_key: str
    executable_size_key: str
    receipt_keys: frozenset[str]
    report_keys: frozenset[str]


_CANDIDATE_CONTRACT = _ArtifactContract(
    receipt_schema=CANDIDATE_RECEIPT_SCHEMA,
    report_schema=CANDIDATE_REPORT_SCHEMA,
    receipt_file_name=CANDIDATE_RECEIPT_FILE_NAME,
    executable_file_name=CANDIDATE_BINARY_FILE_NAME,
    report_file_name=CANDIDATE_REPORT_FILE_NAME,
    executable_path_key="binary_path",
    executable_sha256_key="binary_sha256",
    executable_size_key="binary_size_bytes",
    receipt_keys=frozenset(CANDIDATE_RECEIPT_KEYS),
    report_keys=frozenset(CANDIDATE_REPORT_KEYS),
)
_PROMOTION_VERIFIER_CONTRACT = _ArtifactContract(
    receipt_schema=PROMOTION_VERIFIER_RECEIPT_SCHEMA,
    report_schema=PROMOTION_VERIFIER_REPORT_SCHEMA,
    receipt_file_name=PROMOTION_VERIFIER_RECEIPT_FILE_NAME,
    executable_file_name=PROMOTION_VERIFIER_FILE_NAME,
    report_file_name=PROMOTION_VERIFIER_REPORT_FILE_NAME,
    executable_path_key="promotion_verifier_path",
    executable_sha256_key="promotion_verifier_sha256",
    executable_size_key="promotion_verifier_size_bytes",
    receipt_keys=frozenset(PROMOTION_VERIFIER_RECEIPT_KEYS),
    report_keys=frozenset(PROMOTION_VERIFIER_REPORT_KEYS),
)


def _lower_hex(value: object, length: int) -> bool:
    return (
        isinstance(value, str)
        and re.fullmatch(rf"[0-9a-f]{{{length}}}", value) is not None
    )


def _nonzero_lower_hex(value: object, length: int) -> bool:
    return _lower_hex(value, length) and value != "0" * length


def _canonical_absolute_path(value: object) -> Path:
    if not isinstance(value, str):
        raise PublishedBuildError("published build path is not a string")
    path = Path(value)
    if not path.is_absolute() or os.path.normpath(value) != value:
        raise PublishedBuildError("published build path is not canonical")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise PublishedBuildError("published build path is unavailable") from error
    if resolved != path:
        raise PublishedBuildError("published build path is a symlink or alias")
    return path


def _canonical_lexical_absolute(value: object) -> bool:
    return (
        isinstance(value, str)
        and Path(value).is_absolute()
        and os.path.normpath(value) == value
    )


def _exact_positive_int(value: object) -> bool:
    return not isinstance(value, bool) and isinstance(value, int) and value > 0


def _stat_identity(value: os.stat_result) -> tuple[int, ...]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_uid,
        value.st_gid,
        value.st_nlink,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def _stable_regular_bytes(
    path: Path,
    *,
    maximum_bytes: int,
    require_root: bool = True,
) -> tuple[bytes, str, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise PublishedBuildError("published build file is unavailable") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum_bytes
        or before.st_mode & 0o222 != 0
        or before.st_mode & 0o7000 != 0
        or (require_root and before.st_uid != TRUSTED_OWNER_UID)
    ):
        raise PublishedBuildError("published build file has unsafe metadata")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(os.fsencode(path), flags)
    except OSError as error:
        raise PublishedBuildError("published build file cannot be opened safely") from error
    try:
        opened_before = os.fstat(descriptor)
        payload = bytearray()
        digest = hashlib.sha256()
        while chunk := os.read(descriptor, 1024 * 1024):
            payload.extend(chunk)
            digest.update(chunk)
            if len(payload) > maximum_bytes:
                raise PublishedBuildError("published build file is oversized")
        opened_after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = path.lstat()
    if not (
        _stat_identity(before)
        == _stat_identity(opened_before)
        == _stat_identity(opened_after)
        == _stat_identity(after)
        and len(payload) == before.st_size
    ):
        raise PublishedBuildError("published build file changed while read")
    return bytes(payload), digest.hexdigest(), after


def _stable_regular_sha256(
    path: Path,
    *,
    maximum_bytes: int | None = None,
) -> tuple[str, int, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise PublishedBuildError("published executable is unavailable") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != TRUSTED_OWNER_UID
        or before.st_nlink != 1
        or before.st_size <= 0
        or (maximum_bytes is not None and before.st_size > maximum_bytes)
        or before.st_mode & 0o222 != 0
        or before.st_mode & 0o7000 != 0
    ):
        raise PublishedBuildError("published executable has unsafe metadata")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(os.fsencode(path), flags)
    except OSError as error:
        raise PublishedBuildError("published executable cannot be opened safely") from error
    try:
        opened_before = os.fstat(descriptor)
        digest = hashlib.sha256()
        size = 0
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
            size += len(chunk)
            if maximum_bytes is not None and size > maximum_bytes:
                raise PublishedBuildError("published executable is oversized")
        opened_after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    after = path.lstat()
    if not (
        _stat_identity(before)
        == _stat_identity(opened_before)
        == _stat_identity(opened_after)
        == _stat_identity(after)
        and size == before.st_size
    ):
        raise PublishedBuildError("published executable changed while hashed")
    return digest.hexdigest(), size, after


def _strict_canonical_json(payload: bytes, description: str) -> Mapping[str, Any]:
    def object_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key: {key}")
            result[key] = value
        return result

    def reject_nonfinite(constant: str) -> None:
        raise ValueError(f"non-finite JSON number: {constant}")

    try:
        value = json.loads(
            payload,
            object_pairs_hook=object_pairs,
            parse_constant=reject_nonfinite,
        )
    except (UnicodeError, json.JSONDecodeError, ValueError) as error:
        raise PublishedBuildError(f"{description} is not strict JSON") from error
    if not isinstance(value, dict):
        raise PublishedBuildError(f"{description} is not a JSON object")
    try:
        canonical = (
            json.dumps(
                value,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, UnicodeError) as error:
        raise PublishedBuildError(f"{description} is not canonical JSON") from error
    if canonical != payload:
        raise PublishedBuildError(f"{description} is not canonical JSON")
    return value


def _validate_root_and_parent_chain(root: Path) -> None:
    try:
        metadata = root.lstat()
        resolved = root.resolve(strict=True)
    except OSError as error:
        raise PublishedBuildError("published artifact root is unavailable") from error
    if (
        resolved != root
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != TRUSTED_OWNER_UID
        or metadata.st_mode & 0o222 != 0
        or metadata.st_mode & 0o7000 != 0
    ):
        raise PublishedBuildError(
            "published artifact root is not canonical root-owned read-only storage"
        )
    ancestor = root.parent
    while True:
        try:
            metadata = ancestor.lstat()
            resolved = ancestor.resolve(strict=True)
        except OSError as error:
            raise PublishedBuildError(
                "published artifact parent chain is unavailable"
            ) from error
        if (
            resolved != ancestor
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != TRUSTED_OWNER_UID
            or metadata.st_mode & 0o022 != 0
        ):
            raise PublishedBuildError(
                "published artifact parent chain is not root-controlled"
            )
        if ancestor == ancestor.parent:
            break
        ancestor = ancestor.parent


def _artifact_tree_sha256(
    root: Path,
    receipt: Path,
    executable: Path,
    report: Path,
) -> str:
    _validate_root_and_parent_chain(root)
    expected_names = {
        os.fsencode(receipt.name),
        os.fsencode(executable.name),
        os.fsencode(report.name),
    }
    try:
        entries = sorted(
            os.scandir(os.fsencode(root)),
            key=lambda entry: entry.name,
        )
    except OSError as error:
        raise PublishedBuildError(
            "published artifact inventory cannot be enumerated"
        ) from error
    if {entry.name for entry in entries} != expected_names:
        raise PublishedBuildError("published artifact inventory is not exact")
    digest = hashlib.sha256()
    digest.update(ARTIFACT_TREE_DOMAIN)

    def frame(payload: bytes) -> None:
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)

    for entry in entries:
        path = Path(os.fsdecode(entry.path))
        metadata = entry.stat(follow_symlinks=False)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_uid != TRUSTED_OWNER_UID
            or metadata.st_nlink != 1
            or metadata.st_mode & 0o222 != 0
            or metadata.st_mode & 0o7000 != 0
        ):
            raise PublishedBuildError(
                "published artifact inventory contains an unsafe entry"
            )
        if path == receipt:
            continue
        observed_sha256, size, stable_metadata = _stable_regular_sha256(path)
        digest.update(b"F")
        frame(entry.name)
        digest.update(stat.S_IMODE(stable_metadata.st_mode).to_bytes(4, "big"))
        digest.update(size.to_bytes(8, "big"))
        digest.update(bytes.fromhex(observed_sha256))
    return digest.hexdigest()


def _validate_report_semantics(
    report: Mapping[str, Any],
    contract: _ArtifactContract,
) -> None:
    if (
        report.get("build_user_name") != BUILD_USER_NAME
        or not _exact_positive_int(report.get("build_uid"))
        or report.get("publication_status")
        != "provisional_boi_build_worker_output"
        or not _canonical_lexical_absolute(
            report.get(contract.executable_path_key)
        )
        or Path(report[contract.executable_path_key]).name
        != contract.executable_file_name
        or not _nonzero_lower_hex(
            report.get(contract.executable_sha256_key),
            64,
        )
        or not _exact_positive_int(
            report.get(contract.executable_size_key)
        )
        or not _nonzero_lower_hex(
            report.get("production_closure_tree_sha256"),
            64,
        )
        or not _nonzero_lower_hex(
            report.get("toolchain_provenance_sha256"),
            64,
        )
        or not _nonzero_lower_hex(
            report.get("reviewed_source_closure_descriptor_sha256"),
            64,
        )
        or not _nonzero_lower_hex(report.get("source_commit"), 40)
        or not _nonzero_lower_hex(report.get("source_tree_sha256"), 64)
    ):
        raise PublishedBuildError(
            "published sealed-build report field semantics are malformed"
        )
    if contract is _PROMOTION_VERIFIER_CONTRACT:
        return

    path_fields = (
        "apple_developer_dir_path",
        "apple_sdk_path",
        "cargo_home_path",
        "cargo_path",
        "cargo_vendor_path",
        "clang_resource_dir_path",
        "git_exec_path",
        "git_path",
        "gpg_path",
        "linker_path",
        "production_closure_root",
        "python_path",
        "rustc_path",
        "rustc_sysroot_path",
        "target_dir",
    )
    digest_fields = (
        "cargo_sha256",
        "git_sha256",
        "gpg_sha256",
        "linker_sha256",
        "python_sha256",
        "rustc_sha256",
    )
    if (
        report.get("build_profile") != "release"
        or any(
            not _canonical_lexical_absolute(report.get(key))
            for key in path_fields
        )
        or any(
            not _nonzero_lower_hex(report.get(key), 64)
            for key in digest_fields
        )
        or not isinstance(report.get("source_repo_dirty"), bool)
        or not isinstance(
            report.get("source_signing_key_fingerprint"),
            str,
        )
        or re.fullmatch(
            r"(?:[0-9A-F]{40}|[0-9A-F]{64})",
            report["source_signing_key_fingerprint"],
        )
        is None
        or set(report["source_signing_key_fingerprint"]) == {"0"}
        or not _exact_positive_int(
            report.get("minimum_build_physical_memory_bytes")
        )
        or not _exact_positive_int(
            report.get("physical_memory_bytes_at_admission")
        )
        or report["minimum_build_physical_memory_bytes"]
        != MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES
        or report["physical_memory_bytes_at_admission"]
        < report["minimum_build_physical_memory_bytes"]
        or report["physical_memory_bytes_at_admission"]
        > MAX_RECORDED_PHYSICAL_MEMORY_BYTES
    ):
        raise PublishedBuildError(
            "sealed candidate-build report types or memory bounds are malformed"
        )

    closure_root = Path(report["production_closure_root"])
    target_dir = Path(report["target_dir"])
    closure_bound_paths = (
        "apple_developer_dir_path",
        "apple_sdk_path",
        "cargo_home_path",
        "cargo_path",
        "cargo_vendor_path",
        "clang_resource_dir_path",
        "git_exec_path",
        "git_path",
        "gpg_path",
        "linker_path",
        "python_path",
        "rustc_path",
        "rustc_sysroot_path",
    )
    try:
        for key in closure_bound_paths:
            Path(report[key]).relative_to(closure_root)
        Path(report[contract.executable_path_key]).relative_to(target_dir)
        if (
            target_dir.is_relative_to(closure_root)
            or closure_root.is_relative_to(target_dir)
        ):
            raise ValueError(
                "target and immutable closure must be disjoint"
            )
    except ValueError as error:
        raise PublishedBuildError(
            "sealed candidate-build report paths escape their admitted roots"
        ) from error

    reviewed_source_closure = report.get("reviewed_source_closure")
    try:
        validated_closure = source_seal._validate_descriptor(
            reviewed_source_closure,
            report["source_commit"],
        )
        descriptor_sha256 = hashlib.sha256(
            source_seal._canonical_json_bytes(validated_closure)
        ).hexdigest()
    except (TypeError, ValueError, source_seal.SourceSealError) as error:
        raise PublishedBuildError(
            "sealed candidate-build reviewed source closure is malformed"
        ) from error
    if (
        descriptor_sha256
        != report["reviewed_source_closure_descriptor_sha256"]
        or validated_closure["source_commit"] != report["source_commit"]
        or validated_closure["source_tree_sha256"]
        != report["source_tree_sha256"]
        or validated_closure["source_repo_dirty"]
        is not report["source_repo_dirty"]
    ):
        raise PublishedBuildError(
            "sealed candidate-build reviewed source closure does not agree"
        )


def _admit(
    receipt_path: Path,
    receipt_sha256: str,
    contract: _ArtifactContract,
) -> AdmittedPublishedBuild:
    if (
        not receipt_path.is_absolute()
        or os.path.normpath(os.fspath(receipt_path)) != os.fspath(receipt_path)
        or receipt_path.name != contract.receipt_file_name
        or not _nonzero_lower_hex(receipt_sha256, 64)
    ):
        raise PublishedBuildError(
            "published build receipt path or independent pin is malformed"
        )
    receipt = _canonical_absolute_path(os.fspath(receipt_path))
    receipt_payload, observed_receipt_sha256, _ = _stable_regular_bytes(
        receipt,
        maximum_bytes=MAX_RECEIPT_BYTES,
    )
    if observed_receipt_sha256 != receipt_sha256:
        raise PublishedBuildError(
            "published build receipt differs from its independent pin"
        )
    document = _strict_canonical_json(receipt_payload, "published build receipt")
    if (
        set(document) != set(contract.receipt_keys)
        or document.get("schema") != contract.receipt_schema
        or document.get("publication_protocol") != PUBLICATION_PROTOCOL
        or document.get("build_user_name") != BUILD_USER_NAME
    ):
        raise PublishedBuildError("published build receipt contract is not exact")
    build_uid = document.get("build_uid")
    if (
        isinstance(build_uid, bool)
        or not isinstance(build_uid, int)
        or build_uid < 1
    ):
        raise PublishedBuildError("published build UID is not a non-root account")
    for key, length in (
        ("artifact_tree_sha256", 64),
        (contract.executable_sha256_key, 64),
        ("sealed_build_report_sha256", 64),
        ("production_closure_tree_sha256", 64),
        ("toolchain_provenance_sha256", 64),
        ("reviewed_source_closure_descriptor_sha256", 64),
        ("source_commit", 40),
        ("source_tree_sha256", 64),
    ):
        if not _nonzero_lower_hex(document.get(key), length):
            raise PublishedBuildError(f"published build {key} is malformed")
    executable_size = document.get(contract.executable_size_key)
    if (
        isinstance(executable_size, bool)
        or not isinstance(executable_size, int)
        or executable_size <= 0
    ):
        raise PublishedBuildError("published executable size is malformed")

    root = _canonical_absolute_path(document.get("artifact_root"))
    executable = _canonical_absolute_path(document.get(contract.executable_path_key))
    report = _canonical_absolute_path(document.get("sealed_build_report_path"))
    expected_tree_sha256 = document["artifact_tree_sha256"]
    assert isinstance(expected_tree_sha256, str)
    if (
        root.name != expected_tree_sha256
        or receipt != root / contract.receipt_file_name
        or executable != root / contract.executable_file_name
        or report != root / contract.report_file_name
    ):
        raise PublishedBuildError(
            "published build paths are not the exact content-addressed inventory"
        )
    observed_tree_sha256 = _artifact_tree_sha256(
        root,
        receipt,
        executable,
        report,
    )
    if observed_tree_sha256 != expected_tree_sha256:
        raise PublishedBuildError(
            "published build tree differs from its content address"
        )

    executable_sha256, observed_size, executable_metadata = (
        _stable_regular_sha256(executable)
    )
    if (
        executable_sha256 != document[contract.executable_sha256_key]
        or observed_size != executable_size
        or executable_metadata.st_mode & 0o111 == 0
    ):
        raise PublishedBuildError(
            "published executable differs from its receipt"
        )
    report_payload, report_sha256, report_metadata = _stable_regular_bytes(
        report,
        maximum_bytes=MAX_REPORT_BYTES,
    )
    if (
        report_sha256 != document["sealed_build_report_sha256"]
        or report_metadata.st_mode & 0o111 != 0
    ):
        raise PublishedBuildError(
            "published sealed-build report differs from its receipt"
        )
    report_document = _strict_canonical_json(
        report_payload,
        "published sealed-build report",
    )
    if (
        set(report_document) != set(contract.report_keys)
        or report_document.get("schema") != contract.report_schema
    ):
        raise PublishedBuildError(
            "published sealed-build report contract is not exact"
        )
    _validate_report_semantics(report_document, contract)
    agreement = {
        contract.executable_sha256_key: document[contract.executable_sha256_key],
        contract.executable_size_key: executable_size,
        "build_uid": build_uid,
        "build_user_name": BUILD_USER_NAME,
        "production_closure_tree_sha256": document[
            "production_closure_tree_sha256"
        ],
        "toolchain_provenance_sha256": document[
            "toolchain_provenance_sha256"
        ],
        "reviewed_source_closure_descriptor_sha256": document[
            "reviewed_source_closure_descriptor_sha256"
        ],
        "source_commit": document["source_commit"],
        "source_tree_sha256": document["source_tree_sha256"],
    }
    if any(
        type(report_document.get(key)) is not type(value)
        or report_document.get(key) != value
        for key, value in agreement.items()
    ):
        raise PublishedBuildError(
            "published receipt and sealed-build report do not agree"
        )

    return AdmittedPublishedBuild(
        receipt=receipt,
        receipt_sha256=receipt_sha256,
        artifact_root=root,
        artifact_tree_sha256=observed_tree_sha256,
        executable=executable,
        executable_sha256=executable_sha256,
        executable_size_bytes=observed_size,
        sealed_build_report=report,
        sealed_build_report_sha256=report_sha256,
        build_user_name=BUILD_USER_NAME,
        build_uid=build_uid,
        production_closure_tree_sha256=document[
            "production_closure_tree_sha256"
        ],
        toolchain_provenance_sha256=document["toolchain_provenance_sha256"],
        reviewed_source_closure_descriptor_sha256=document[
            "reviewed_source_closure_descriptor_sha256"
        ],
        source_commit=document["source_commit"],
        source_tree_sha256=document["source_tree_sha256"],
    )


def admit_candidate(
    receipt_path: Path,
    receipt_sha256: str,
) -> AdmittedPublishedBuild:
    """Admit one independently pinned root-published candidate executable."""

    return _admit(receipt_path, receipt_sha256, _CANDIDATE_CONTRACT)


def admit_promotion_verifier(
    receipt_path: Path,
    receipt_sha256: str,
) -> AdmittedPublishedBuild:
    """Admit one independently pinned root-published Kagami verifier."""

    return _admit(
        receipt_path,
        receipt_sha256,
        _PROMOTION_VERIFIER_CONTRACT,
    )
