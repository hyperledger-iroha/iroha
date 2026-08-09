"""Candidate-stage manifest validation for Android device-lab evidence."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
from typing import Any


_SHA256_HEX_RE = re.compile(r"^[0-9a-f]{64}$")


@dataclass(frozen=True)
class CandidateStageContract:
    """Constants and callbacks that define the exact candidate-stage contract."""

    stage_manifest_path: str
    stage_manifest_schema: str
    stage_manifest_fields: frozenset[str]
    validation_report_path: str
    validation_report_schema: str
    validation_report_fields: frozenset[str]
    qualification_receipt_file_name: str
    generation_memory_enforcement_profile: str
    generation_memory_limit_max_bytes: int
    stage_entry_fields: frozenset[str]
    stage_validator_fields: frozenset[str]
    stage_validator_schema: str
    scenario_inventory_domain: bytes
    scenario_files: tuple[str, ...]
    artifact_roles: tuple[str, ...]
    artifact_file_names: tuple[str, ...]
    max_json_bytes: int
    derive_qualified_candidate_sha256: Callable[[str, str], str]


def validate_candidate_stage_manifest_v2(
    stage_root: Path,
    *,
    contract: CandidateStageContract,
    candidate_sha256: str,
    stage_sha256: str,
    source_commit: str,
    source_tree_sha256: str,
    verify_entry_digests: bool = True,
) -> dict[str, Any]:
    """Verify the canonical stage manifest and every one of its 45 files.

    ``verify_entry_digests=False`` is reserved for streaming consumers which
    authenticate every byte against the returned catalog while copying it. The
    default remains the full promotion-grade validation path.
    """

    def reject_duplicate_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"stage manifest repeats JSON key {key!r}")
            result[key] = value
        return result

    def file_digest(
        path: Path,
        expected: os.stat_result,
        relative: str,
        capture: list[bytes] | None = None,
    ) -> str:
        digest = hashlib.sha256()
        flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
        try:
            descriptor = os.open(path, flags)
        except OSError as error:
            raise ValueError(
                f"candidate stage entry could not be securely opened: {relative}"
            ) from error
        try:
            opened = os.fstat(descriptor)
            identity = (expected.st_dev, expected.st_ino)
            if (
                not stat.S_ISREG(opened.st_mode)
                or opened.st_nlink != 1
                or (opened.st_dev, opened.st_ino) != identity
                or opened.st_size != expected.st_size
            ):
                raise ValueError(
                    f"candidate stage entry changed while opening: {relative}"
                )
            size = 0
            while chunk := os.read(descriptor, 1024 * 1024):
                size += len(chunk)
                if size > expected.st_size:
                    raise ValueError(
                        f"candidate stage entry grew while hashing: {relative}"
                    )
                digest.update(chunk)
                if capture is not None:
                    capture.append(chunk)
            final_opened = os.fstat(descriptor)
            final_path = path.lstat()
            if (
                (final_opened.st_dev, final_opened.st_ino) != identity
                or (final_path.st_dev, final_path.st_ino) != identity
                or not stat.S_ISREG(final_opened.st_mode)
                or not stat.S_ISREG(final_path.st_mode)
                or final_opened.st_nlink != 1
                or final_path.st_nlink != 1
                or stat.S_IMODE(final_opened.st_mode) != 0o600
                or stat.S_IMODE(final_path.st_mode) != 0o600
                or final_opened.st_uid != expected.st_uid
                or final_path.st_uid != expected.st_uid
                or size != expected.st_size
                or final_opened.st_size != expected.st_size
                or final_path.st_size != expected.st_size
                or final_path.st_mtime_ns != expected.st_mtime_ns
                or final_path.st_ctime_ns != expected.st_ctime_ns
            ):
                raise ValueError(
                    f"candidate stage entry changed while hashing: {relative}"
                )
        finally:
            os.close(descriptor)
        return digest.hexdigest()

    if not _SHA256_HEX_RE.fullmatch(candidate_sha256) or candidate_sha256 == "0" * 64:
        raise ValueError("candidate_sha256 must be non-zero lowercase SHA-256")
    if not _SHA256_HEX_RE.fullmatch(stage_sha256) or stage_sha256 == "0" * 64:
        raise ValueError("stage_sha256 must be non-zero lowercase SHA-256")
    if not re.fullmatch(r"[0-9a-f]{40}", source_commit):
        raise ValueError("source_commit must be lowercase git hex")
    if not _SHA256_HEX_RE.fullmatch(source_tree_sha256) or source_tree_sha256 == "0" * 64:
        raise ValueError("source_tree_sha256 must be non-zero lowercase SHA-256")

    root = stage_root.resolve()
    manifest_path = root / contract.stage_manifest_path
    manifest_stat = manifest_path.lstat()
    if not stat.S_ISREG(manifest_stat.st_mode) or manifest_stat.st_nlink != 1:
        raise ValueError("candidate stage manifest must be one singly-linked regular file")
    if stat.S_IMODE(manifest_stat.st_mode) != 0o600:
        raise ValueError("candidate stage manifest mode must be 0600")
    maximum_manifest_bytes = 1024 * 1024
    if manifest_stat.st_size <= 0 or manifest_stat.st_size > maximum_manifest_bytes:
        raise ValueError("candidate stage manifest is empty or oversized")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(manifest_path, flags)
    chunks: list[bytes] = []
    try:
        open_stat = os.fstat(descriptor)
        expected_identity = (manifest_stat.st_dev, manifest_stat.st_ino)
        if (
            not stat.S_ISREG(open_stat.st_mode)
            or open_stat.st_nlink != 1
            or (open_stat.st_dev, open_stat.st_ino) != expected_identity
            or open_stat.st_size != manifest_stat.st_size
        ):
            raise ValueError("candidate stage manifest changed while being opened")
        size = 0
        while True:
            chunk = os.read(
                descriptor,
                min(1024 * 1024, maximum_manifest_bytes + 1 - size),
            )
            if not chunk:
                break
            size += len(chunk)
            if size > maximum_manifest_bytes:
                raise ValueError("candidate stage manifest is empty or oversized")
            chunks.append(chunk)
        final_open_stat = os.fstat(descriptor)
        final_path_stat = manifest_path.lstat()
        if (
            (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity
            or (final_open_stat.st_dev, final_open_stat.st_ino) != expected_identity
            or final_path_stat.st_size != size
            or final_path_stat.st_mtime_ns != manifest_stat.st_mtime_ns
            or final_path_stat.st_ctime_ns != manifest_stat.st_ctime_ns
        ):
            raise ValueError("candidate stage manifest changed while being read")
    finally:
        os.close(descriptor)
    payload_bytes = b"".join(chunks)
    if hashlib.sha256(payload_bytes).hexdigest() != stage_sha256:
        raise ValueError("candidate stage manifest digest is not the stage identity")
    try:
        manifest = json.loads(
            payload_bytes.decode("utf-8"),
            object_pairs_hook=reject_duplicate_pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(
                ValueError(f"non-finite JSON value {value}")
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise ValueError(f"candidate stage manifest is not strict JSON: {error}") from error
    if not isinstance(manifest, dict) or set(manifest) != contract.stage_manifest_fields:
        raise ValueError("candidate stage manifest must have the exact V2 fields")
    canonical = (
        json.dumps(manifest, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("utf-8")
    if canonical != payload_bytes:
        raise ValueError("candidate stage manifest bytes are not canonical JSON")
    exact_top = {
        "schema": contract.stage_manifest_schema,
        "version": 2,
        "stage_manifest_path": contract.stage_manifest_path,
        "stage_manifest_mode": "0600",
        "stage_manifest_size_bytes": len(payload_bytes),
        "candidate_record_sha256": candidate_sha256,
        "source_commit": source_commit,
        "source_tree_sha256": source_tree_sha256,
        "source_repo_dirty": False,
        "entry_count": 45,
        "scenario_entry_count": 33,
    }
    for key, expected in exact_top.items():
        if manifest.get(key) != expected or isinstance(manifest.get(key), bool) != isinstance(expected, bool):
            raise ValueError(f"candidate stage manifest {key} is not exact")

    validator = manifest.get("validator")
    if not isinstance(validator, dict) or set(validator) != contract.stage_validator_fields:
        raise ValueError("candidate stage manifest validator must have the exact V1 fields")
    validator_exact = {
        "schema": contract.stage_validator_schema,
        "candidate_binary_name": "kagemusha_recursive_spend_v4_bundle",
        "scenario_binary_name": "kagemusha_candidate_scenario_validator",
        "locked": True,
        "offline": True,
        "isolated_target": True,
        "build_jobs": 2,
        "candidate_package": "iroha_core",
        "scenario_package": "connect_norito_bridge",
        "features": ["kagemusha-candidate-evidence-lab"],
        "profile": "debug",
    }
    for key, expected in validator_exact.items():
        if validator.get(key) != expected or isinstance(validator.get(key), bool) != isinstance(expected, bool):
            raise ValueError(f"candidate stage manifest validator.{key} is not exact")
    for key in (
        "candidate_binary_sha256",
        "scenario_binary_sha256",
        "cargo_binary_sha256",
        "rustc_binary_sha256",
    ):
        value = validator.get(key)
        if not isinstance(value, str) or not _SHA256_HEX_RE.fullmatch(value) or value == "0" * 64:
            raise ValueError(f"candidate stage manifest validator.{key} is invalid")
    for key in ("cargo_version_verbose", "rustc_version_verbose"):
        value = validator.get(key)
        if (
            not isinstance(value, str)
            or not value
            or len(value.encode("utf-8")) > 64 * 1024
            or not value.endswith("\n")
            or "\x00" in value
            or "\r" in value
        ):
            raise ValueError(f"candidate stage manifest validator.{key} is invalid")

    expected_paths = {
        "evidence/candidate/candidate-v4.norito",
        "evidence/candidate/manifest-v4.norito",
        contract.validation_report_path,
        f"evidence/candidate/{contract.qualification_receipt_file_name}",
        *(
            f"evidence/candidate/artifacts/{name}"
            for name in contract.artifact_file_names
        ),
        *(f"scenario/{name}" for name in contract.scenario_files),
    }
    parent_paths = sorted(
        {PurePosixPath(relative).parent.as_posix() for relative in expected_paths},
        key=lambda path: path.encode("utf-8"),
    )
    for relative_parent in parent_paths:
        parent = root / relative_parent
        parent_stat = parent.lstat()
        if not stat.S_ISDIR(parent_stat.st_mode) or parent.resolve(strict=True) != parent:
            raise ValueError(
                f"candidate stage entry parent is not a real directory: {relative_parent}"
            )
    entries = manifest.get("entries")
    if not isinstance(entries, list) or len(entries) != 45:
        raise ValueError("candidate stage manifest entries must contain exactly 45 objects")
    paths: list[str] = []
    measured: dict[str, tuple[int, str]] = {}
    validation_report_bytes: bytes | None = None
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict) or set(entry) != contract.stage_entry_fields:
            raise ValueError(f"candidate stage manifest entries[{index}] has wrong fields")
        relative = entry.get("path")
        if not isinstance(relative, str) or relative not in expected_paths:
            raise ValueError(f"candidate stage manifest entries[{index}] path is not canonical")
        paths.append(relative)
        if entry.get("mode") != "0600":
            raise ValueError(f"candidate stage manifest entry {relative} mode must be 0600")
        path = root / relative
        current = path.lstat()
        if not stat.S_ISREG(current.st_mode) or current.st_nlink != 1:
            raise ValueError(f"candidate stage entry is not singly-linked regular: {relative}")
        if stat.S_IMODE(current.st_mode) != 0o600:
            raise ValueError(f"candidate stage entry mode is not 0600: {relative}")
        size = entry.get("size_bytes")
        digest = entry.get("sha256")
        if not isinstance(size, int) or isinstance(size, bool) or size <= 0 or size != current.st_size:
            raise ValueError(f"candidate stage entry size is not exact: {relative}")
        if not isinstance(digest, str) or not _SHA256_HEX_RE.fullmatch(digest):
            raise ValueError(f"candidate stage entry digest is invalid: {relative}")
        capture = [] if relative == contract.validation_report_path else None
        if verify_entry_digests or capture is not None:
            measured_digest = file_digest(path, current, relative, capture)
            if measured_digest != digest:
                raise ValueError(f"candidate stage entry digest is not exact: {relative}")
        if capture is not None:
            validation_report_bytes = b"".join(capture)
            if len(validation_report_bytes) > contract.max_json_bytes:
                raise ValueError("candidate validation report is oversized")
        measured[relative] = (size, digest)
    expected_order = sorted(expected_paths, key=lambda path: path.encode("utf-8"))
    if paths != expected_order or set(paths) != expected_paths:
        raise ValueError("candidate stage entries are not the exact byte-lexicographic inventory")

    digest_bindings = {
        "candidate_record_sha256": "evidence/candidate/candidate-v4.norito",
        "candidate_manifest_sha256": "evidence/candidate/manifest-v4.norito",
        "candidate_validation_report_sha256": (
            contract.validation_report_path
        ),
    }
    for key, path in digest_bindings.items():
        if manifest.get(key) != measured[path][1]:
            raise ValueError(f"candidate stage manifest {key} does not bind {path}")
    receipt_path = f"evidence/candidate/{contract.qualification_receipt_file_name}"
    receipt_sha256 = measured[receipt_path][1]
    if manifest.get("qualification_receipt_sha256") != receipt_sha256:
        raise ValueError(
            "candidate stage manifest qualification_receipt_sha256 does not bind the receipt"
        )
    qualified_candidate_sha256 = contract.derive_qualified_candidate_sha256(
        candidate_sha256,
        receipt_sha256,
    )
    if manifest.get("qualified_candidate_sha256") != qualified_candidate_sha256:
        raise ValueError("candidate stage manifest qualified_candidate_sha256 is invalid")

    if validation_report_bytes is None:
        raise ValueError("candidate validation report was not securely read")
    try:
        validation_report = json.loads(
            validation_report_bytes.decode("utf-8"),
            object_pairs_hook=reject_duplicate_pairs,
            parse_constant=lambda value: (_ for _ in ()).throw(
                ValueError(f"non-finite JSON value {value}")
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise ValueError(f"candidate validation report is not strict JSON: {error}") from error
    if (
        not isinstance(validation_report, dict)
        or set(validation_report) != contract.validation_report_fields
    ):
        raise ValueError("candidate validation report must have the exact V2 fields")
    exact_validation = {
        "schema": contract.validation_report_schema,
        "candidate_record_sha256": candidate_sha256,
        "candidate_manifest_sha256": manifest.get("candidate_manifest_sha256"),
        "qualification_receipt_file_name": contract.qualification_receipt_file_name,
        "qualification_receipt_sha256": receipt_sha256,
        "qualified_candidate_sha256": qualified_candidate_sha256,
        "source_commit": source_commit,
        "source_tree_sha256": source_tree_sha256,
        "bridge_abi_version": 22,
        "artifact_count": len(contract.artifact_file_names),
        "topup_finality_roster_file_name": "topup-finality-roster-v4.norito",
    }
    for key, expected in exact_validation.items():
        if validation_report.get(key) != expected:
            raise ValueError(f"candidate validation report {key} is not exact")
    if not isinstance(validation_report.get("source_repo_dirty"), bool):
        raise ValueError("candidate validation report source_repo_dirty must be boolean")
    generation = validation_report.get("generation")
    if (
        not isinstance(generation, str)
        or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", generation) is None
    ):
        raise ValueError("candidate validation report generation is invalid")
    generation_memory_limit = validation_report.get("generation_memory_limit_bytes")
    if (
        isinstance(generation_memory_limit, bool)
        or not isinstance(generation_memory_limit, int)
        or generation_memory_limit <= 0
        or generation_memory_limit > contract.generation_memory_limit_max_bytes
    ):
        raise ValueError("candidate validation report generation memory limit is invalid")
    if (
        validation_report.get("generation_memory_enforcement_profile")
        != contract.generation_memory_enforcement_profile
    ):
        raise ValueError(
            "candidate validation report generation memory enforcement profile is invalid"
        )
    artifacts = validation_report.get("artifacts")
    if not isinstance(artifacts, list) or len(artifacts) != len(
        contract.artifact_file_names
    ):
        raise ValueError("candidate validation report artifact inventory is invalid")
    for index, (artifact, expected_role, expected_name) in enumerate(
        zip(
            artifacts,
            contract.artifact_roles,
            contract.artifact_file_names,
        )
    ):
        if not isinstance(artifact, dict) or set(artifact) != {
            "role",
            "file_name",
            "framed_size_bytes",
            "framed_sha256",
            "payload_size_bytes",
            "payload_sha256",
        }:
            raise ValueError(f"candidate validation artifact {index} fields are invalid")
        if artifact.get("role") != expected_role or artifact.get("file_name") != expected_name:
            raise ValueError(f"candidate validation artifact {index} identity is invalid")
        for key in ("framed_sha256", "payload_sha256"):
            value = artifact.get(key)
            if not isinstance(value, str) or not _SHA256_HEX_RE.fullmatch(value):
                raise ValueError(f"candidate validation artifact {index} {key} is invalid")
        for key in ("framed_size_bytes", "payload_size_bytes"):
            value = artifact.get(key)
            if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
                raise ValueError(f"candidate validation artifact {index} {key} is invalid")
    roster_size = validation_report.get("topup_finality_roster_size_bytes")
    roster_sha256 = validation_report.get("topup_finality_roster_sha256")
    if (
        isinstance(roster_size, bool)
        or not isinstance(roster_size, int)
        or roster_size <= 0
        or not isinstance(roster_sha256, str)
        or not _SHA256_HEX_RE.fullmatch(roster_sha256)
    ):
        raise ValueError("candidate validation report roster metadata is invalid")
    scenario_paths = sorted(
        (f"scenario/{name}" for name in contract.scenario_files),
        key=lambda path: path.encode("utf-8"),
    )
    scenario_digest = hashlib.sha256()
    scenario_digest.update(contract.scenario_inventory_domain)
    scenario_digest.update(len(scenario_paths).to_bytes(4, "big"))
    for relative in scenario_paths:
        path_bytes = relative.encode("utf-8")
        size, digest = measured[relative]
        scenario_digest.update(len(path_bytes).to_bytes(4, "big"))
        scenario_digest.update(path_bytes)
        scenario_digest.update(size.to_bytes(8, "big"))
        scenario_digest.update(bytes.fromhex(digest))
    if manifest.get("scenario_inventory_sha256") != scenario_digest.hexdigest():
        raise ValueError("candidate stage manifest scenario_inventory_sha256 is not exact")
    return manifest
