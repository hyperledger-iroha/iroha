#!/usr/bin/env python3
"""Validate and manifest a Kagemusha production release evidence bundle."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from collections.abc import Mapping
from pathlib import Path
import stat
import sys
import tempfile
from typing import Any, Callable

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import kagemusha_production_readiness as readiness  # noqa: E402


RELEASE_BUNDLE_SCHEMA = "iroha.kagemusha.production_release_bundle.v1"
DEFAULT_READINESS_SUMMARY_PATH = "dist/kagemusha-production-readiness.json"
DEFAULT_RELEASE_BUNDLE_OUT = "dist/kagemusha-production-release-bundle.json"
MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES = 16 * 1024 * 1024
MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES = 16 * 1024 * 1024

SUMMARY_REQUIRED_SECTION_STATES: dict[str, str] = {
    "abi7_recursive_compact": "package_aware_multi_hop_composed",
    "lineage_key_release_tooling": "record_artifacts_wired",
    "lineage_proof_evidence": "production_width_proof_passed",
    "compact_key_evidence": "compact_key_artifacts_validated",
    "localnet_lifecycle_evidence": "localnet_lifecycle_validated",
}
ANDROID_SLOT_RELEASE_ARTIFACTS: tuple[tuple[str, str, str], ...] = (
    (
        "offline_wallet_apk",
        "offline_wallet_apk_path",
        "offline_wallet_apk_sha256",
    ),
    (
        "d2d_payment_transcript",
        "d2d_payment_transcript_path",
        "d2d_payment_transcript_sha256",
    ),
    (
        "wallet_integrity_transcript",
        "wallet_integrity_transcript_path",
        "wallet_integrity_transcript_sha256",
    ),
    (
        "attestation_certificate_chain",
        "attestation_certificate_chain_path",
        "attestation_certificate_chain_sha256",
    ),
)
ANDROID_D2D_TRANSCRIPT_ARTIFACT_PREFIX = "d2d_payment_transcript_"
ANDROID_SIGNED_EVIDENCE_SUMMARY_REQUIRED_FIELDS = frozenset(
    (
        "signed_at_utc",
        "device_family",
        "device_model",
        "device_codename",
        "artifact_sha256",
        "signer_public_key_sha256",
        *(
            field
            for _, path_field, digest_field in ANDROID_SLOT_RELEASE_ARTIFACTS
            for field in (path_field, digest_field)
        ),
    )
)
ANDROID_SIGNED_EVIDENCE_SUMMARY_PATH_FIELDS = frozenset(
    path_field for _, path_field, _ in ANDROID_SLOT_RELEASE_ARTIFACTS
)
ANDROID_SIGNED_EVIDENCE_SUMMARY_SHA256_FIELDS = frozenset(
    (
        "artifact_sha256",
        "signer_public_key_sha256",
        *(digest_field for _, _, digest_field in ANDROID_SLOT_RELEASE_ARTIFACTS),
    )
)
ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS = frozenset(
    ("device_family", "device_model", "device_codename")
)
ANDROID_DUPLICATE_BINDING_SUMMARY_FIELDS = frozenset(
    ("device_fingerprint_sha256", "attestation_challenge_sha256")
)


def _android_d2d_transcript_artifact_kind(transport: str) -> str:
    return f"{ANDROID_D2D_TRANSCRIPT_ARTIFACT_PREFIX}{transport}"


def _android_d2d_transcript_artifact_transport(artifact: Any) -> str | None:
    if not isinstance(artifact, str):
        return None
    if not artifact.startswith(ANDROID_D2D_TRANSCRIPT_ARTIFACT_PREFIX):
        return None
    transport = artifact[len(ANDROID_D2D_TRANSCRIPT_ARTIFACT_PREFIX):]
    return transport if transport in device_lab.D2D_PAYMENT_TRANSPORTS else None
ANDROID_DUPLICATE_BINDING_ENTRY_FIELDS = frozenset(("slots", "value_sha256"))
SUMMARY_ALLOWED_TOP_LEVEL_KEYS = frozenset(
    (
        "schema",
        "generated_at",
        "status",
        "ready",
        "blockers",
        "abi6_reserved_lineage",
        "abi7_recursive_compact",
        "lineage_key_release_tooling",
        "lineage_proof_evidence",
        "compact_key_evidence",
        "localnet_lifecycle_evidence",
        "android_device_lab",
    )
)
RELEASE_BUNDLE_ALLOWED_TOP_LEVEL_KEYS = frozenset(
    (
        "schema",
        "generated_at_utc",
        "ready",
        "evidence",
        "abi6_reserved_lineage",
        "abi7_recursive_compact",
        "lineage_key_release_tooling",
        "lineage_proof_evidence",
        "compact_key_evidence",
        "localnet_lifecycle_evidence",
        "android_device_lab",
        "blockers",
    )
)
RELEASE_BUNDLE_ALLOWED_ANDROID_SECTION_KEYS = frozenset(
    (
        "root",
        "covered_device_families",
        "missing_device_families",
        "covered_d2d_payment_transports",
        "missing_d2d_payment_transports",
        "duplicate_bindings",
        "signed_evidence",
        "trusted_signer_public_key_sha256",
    )
)
RELEASE_BUNDLE_ALLOWED_EVIDENCE_KEYS = frozenset(
    (
        "readiness_summary",
        "lineage_proof_evidence",
        "compact_key_evidence",
        "localnet_lifecycle_evidence",
        "lineage_artifacts",
        "lineage_proof_logs",
        "compact_key_artifacts",
        "compact_key_generator_log",
        "android_signed_evidence",
        "android_slot_artifacts",
    )
)
RELEASE_BUNDLE_SINGLE_EVIDENCE_KEYS = frozenset(
    (
        "readiness_summary",
        "lineage_proof_evidence",
        "compact_key_evidence",
        "localnet_lifecycle_evidence",
        "compact_key_generator_log",
    )
)
RELEASE_BUNDLE_MAP_EVIDENCE_KEYS = frozenset(
    (
        "lineage_artifacts",
        "lineage_proof_logs",
        "compact_key_artifacts",
        "android_signed_evidence",
    )
)
RELEASE_BUNDLE_EVIDENCE_ENTRY_FIELDS = frozenset(("path", "sha256", "size_bytes"))
RELEASE_BUNDLE_ALLOWED_SECTION_KEYS: dict[str, frozenset[str]] = {
    "abi6_reserved_lineage": frozenset(
        (
            "manifest_path",
            "schema",
            "native_bridge_abi_version",
            "operation_count",
            "limits",
            "modes",
        )
    ),
    "abi7_recursive_compact": frozenset(
        (
            "state",
            "circuit_id",
            "fixture_manifest_path",
            "fixture_manifest_schema",
            "fixture_manifest_sha256",
            "archive_fixture_path",
            "archive_fixture_schema",
            "archive_fixture_sha256",
            "native_bridge_abi_version",
            "operation_count",
        )
    ),
    "lineage_key_release_tooling": frozenset(("state", "checked_files")),
    "lineage_proof_evidence": frozenset(
        (
            "state",
            "generated_at_utc",
            "artifact_sha256",
            "artifact_size_bytes",
            "test_log_sha256",
        )
    ),
    "compact_key_evidence": frozenset(
        (
            "state",
            "generated_at_utc",
            "artifact_sha256",
            "artifact_size_bytes",
            "generator_log_sha256",
            "generator_log_artifact_sha256",
            "generator_log_artifact_size_bytes",
        )
    ),
    "localnet_lifecycle_evidence": frozenset(
        (
            "state",
            "generated_at_utc",
            "localnet_run_id",
            "chain_id",
            "target",
            "peer_count",
            "peer_ids",
            "artifact_sha256",
            "artifact_count",
        )
    ),
}
SUMMARY_ALLOWED_SECTION_KEYS: dict[str, frozenset[str]] = {
    "abi6_reserved_lineage": frozenset(
        (
            "manifest_path",
            "schema",
            "native_bridge_abi_version",
            "operation_count",
            "limits",
            "modes",
            "ok",
            "blockers",
        )
    ),
    "abi7_recursive_compact": frozenset(
        (
            "ok",
            "state",
            "circuit_id",
            "fixture_manifest_path",
            "fixture_manifest_schema",
            "fixture_manifest_sha256",
            "archive_fixture_path",
            "archive_fixture_schema",
            "archive_fixture_sha256",
            "native_bridge_abi_version",
            "operation_count",
            "blockers",
        )
    ),
    "lineage_key_release_tooling": frozenset(
        ("ok", "state", "checked_files", "blockers")
    ),
    "lineage_proof_evidence": frozenset(
        (
            "path",
            "schema",
            "artifact_sha256",
            "artifact_size_bytes",
            "test_log_sha256",
            "min_generated_at_utc",
            "max_generated_at_utc",
            "generated_at_utc",
            "opening_len",
            "ipa_k",
            "record_archive_proof_runtime_keygen_env",
            "circuit_ids",
            "artifact_count",
            "tests",
            "ok",
            "state",
            "blockers",
        )
    ),
    "compact_key_evidence": frozenset(
        (
            "path",
            "schema",
            "artifact_sha256",
            "artifact_size_bytes",
            "min_generated_at_utc",
            "max_generated_at_utc",
            "generated_at_utc",
            "opening_len",
            "ipa_k",
            "verifier_backend",
            "circuit_id",
            "record_namespace",
            "record_version",
            "command_validated",
            "generator_log_sha256",
            "generator_log_artifact_sha256",
            "generator_log_artifact_size_bytes",
            "artifact_count",
            "ok",
            "state",
            "blockers",
        )
    ),
    "localnet_lifecycle_evidence": frozenset(
        (
            "path",
            "schema",
            "artifact_sha256",
            "min_generated_at_utc",
            "max_generated_at_utc",
            "generated_at_utc",
            "localnet_run_id",
            "chain_id",
            "target",
            "peer_count",
            "peer_ids",
            "artifact_count",
            "ok",
            "state",
            "blockers",
        )
    ),
    "android_device_lab": frozenset(
        (
            "ok",
            "root",
            "slots",
            "covered_device_families",
            "missing_device_families",
            "covered_d2d_payment_transports",
            "missing_d2d_payment_transports",
            "duplicate_bindings",
            "signed_evidence",
            "min_signed_at_utc",
            "max_signed_at_utc",
            "trusted_signer_public_key_sha256",
            "blockers",
        )
    ),
}


def _blocker(code: str, message: str, **extra: Any) -> dict[str, Any]:
    return readiness.blocker(code, message, **extra)


def _safe_trusted_signer_public_key_sha256(
    trusted_signer_public_keys: Mapping[Any, Any] | None,
) -> list[str]:
    return sorted(
        device_lab._trusted_signer_public_key_sha256_set(  # type: ignore[attr-defined]
            trusted_signer_public_keys
        )
    )


def _blocked_release_bundle_manifest(
    blockers: list[dict[str, Any]],
    trusted_signer_public_keys: dict[str, Path],
) -> dict[str, Any]:
    return {
        "schema": RELEASE_BUNDLE_SCHEMA,
        "generated_at_utc": readiness.utc_now(),
        "ready": False,
        "evidence": {},
        "abi6_reserved_lineage": {
            "manifest_path": None,
            "schema": None,
            "native_bridge_abi_version": None,
            "operation_count": None,
            "limits": {},
            "modes": {},
        },
        "abi7_recursive_compact": {
            "state": None,
            "circuit_id": None,
            "fixture_manifest_path": None,
            "fixture_manifest_schema": None,
            "fixture_manifest_sha256": None,
            "archive_fixture_path": None,
            "archive_fixture_schema": None,
            "archive_fixture_sha256": None,
            "native_bridge_abi_version": None,
            "operation_count": None,
        },
        "lineage_key_release_tooling": {
            "state": None,
            "checked_files": [],
        },
        "lineage_proof_evidence": {
            "state": None,
            "generated_at_utc": None,
            "artifact_sha256": {},
            "artifact_size_bytes": {},
            "test_log_sha256": {},
        },
        "compact_key_evidence": {
            "state": None,
            "generated_at_utc": None,
            "artifact_sha256": {},
            "artifact_size_bytes": {},
            "generator_log_sha256": None,
            "generator_log_artifact_sha256": {},
            "generator_log_artifact_size_bytes": {},
        },
        "localnet_lifecycle_evidence": {
            "state": None,
            "generated_at_utc": None,
            "localnet_run_id": None,
            "chain_id": None,
            "target": None,
            "peer_count": None,
            "peer_ids": [],
            "artifact_sha256": {},
            "artifact_count": None,
        },
        "android_device_lab": {
            "covered_device_families": [],
            "missing_device_families": list(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
            "covered_d2d_payment_transports": [],
            "missing_d2d_payment_transports": list(
                readiness.ANDROID_REQUIRED_D2D_PAYMENT_TRANSPORTS
            ),
            "duplicate_bindings": {},
            "signed_evidence": {},
            "trusted_signer_public_key_sha256": _safe_trusted_signer_public_key_sha256(
                trusted_signer_public_keys
            ),
        },
        "blockers": blockers,
    }


def _secret_path_error(path: str | None, label: str, code: str) -> dict[str, Any] | None:
    if path is None:
        return None
    if device_lab.SECRET_RE.search(path):
        return _blocker(code, f"{label} must not contain secret-looking material")
    if device_lab._contains_control_character(path):
        return _blocker(code, f"{label} must not contain control characters")
    return None


def _bundle_path_shape_error(path: Path, label: str) -> dict[str, Any] | None:
    """Reject release-bundle path aliases before resolver normalization."""

    path_text = str(path)
    if "\\" in path_text:
        return _blocker(
            "kagemusha_release_bundle_path_invalid",
            f"{label} must not contain backslashes",
        )
    if ".." in path.parts:
        return _blocker(
            "kagemusha_release_bundle_path_invalid",
            f"{label} must be a canonical path under --bundle-root",
        )
    return None


def _bundle_root_shape_error(root: Path) -> dict[str, Any] | None:
    """Reject release-bundle root aliases before metadata preflight."""

    root_text = str(root)
    if "\\" in root_text:
        return _blocker(
            "kagemusha_release_bundle_root_invalid",
            "--bundle-root must not contain backslashes",
        )
    if ".." in root.parts:
        return _blocker(
            "kagemusha_release_bundle_root_invalid",
            "--bundle-root must be a canonical directory path",
        )
    return None


def _validate_bundle_root(root: Path) -> list[dict[str, Any]]:
    secret = _secret_path_error(
        str(root),
        "--bundle-root",
        "kagemusha_release_bundle_root_invalid",
    )
    if secret is not None:
        return [secret]
    shape = _bundle_root_shape_error(root)
    if shape is not None:
        return [shape]
    errors = []
    try:
        mode = root.lstat().st_mode
    except FileNotFoundError:
        return [
            _blocker(
                "kagemusha_release_bundle_root_invalid",
                "--bundle-root must be an existing directory",
            )
        ]
    except OSError:
        return [
            _blocker(
                "kagemusha_release_bundle_root_invalid",
                "--bundle-root metadata could not be read",
            )
        ]
    if stat.S_ISLNK(mode):
        errors.append("--bundle-root must not be a symlink")
    elif not stat.S_ISDIR(mode):
        errors.append("--bundle-root must be a directory")
    errors.extend(
        device_lab.validate_no_symlink_ancestors(
            root,
            "--bundle-root ancestor directory",
        )
    )
    return [
        _blocker("kagemusha_release_bundle_root_invalid", error) for error in errors
    ]


def _validate_local_file_for_read(
    path: Path,
    label: str,
    code: str,
) -> tuple[os.stat_result | None, list[dict[str, Any]]]:
    expected_stat, errors = readiness._validate_lineage_local_file_for_read(  # type: ignore[attr-defined]
        path,
        label,
    )
    return expected_stat, [_blocker(code, error) for error in errors]


def _validate_local_file(path: Path, label: str, code: str) -> list[dict[str, Any]]:
    _expected_stat, blockers = _validate_local_file_for_read(path, label, code)
    return blockers


def _read_local_json_text(
    path: Path,
    label: str,
    *,
    shape_code: str,
    unreadable_code: str,
) -> tuple[str | None, list[dict[str, Any]]]:
    expected_stat, file_blockers = _validate_local_file_for_read(path, label, shape_code)
    if file_blockers:
        return None, file_blockers
    assert expected_stat is not None
    chunks: list[bytes] = []
    size = 0
    release_json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [_blocker(shape_code, f"{label} must not be a symlink")]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [_blocker(shape_code, f"{label} must be a regular file")]
            release_json_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if release_json_open_identity != release_json_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != release_json_expected_identity:
                return None, [_blocker(shape_code, f"{label} changed while being read")]
            if open_stat.st_nlink > 1:
                return None, [_blocker(shape_code, f"{label} must not be hardlinked")]
            if open_stat.st_size > MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES:
                return None, [
                    _blocker(
                        shape_code,
                        f"{label} must be no more than {MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES} bytes",
                    )
                ]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES:
                    return None, [
                        _blocker(
                            shape_code,
                            f"{label} must be no more than {MAX_RELEASE_BUNDLE_LOCAL_JSON_BYTES} bytes",
                        )
                    ]
                chunks.append(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != (
                release_json_expected_identity
            ):
                return None, [_blocker(shape_code, f"{label} changed while being read")]
    except OSError:
        return None, [_blocker(unreadable_code, f"{label} could not be read")]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [_blocker(unreadable_code, f"{label} could not be read")]


def _load_local_json(path: Path, label: str, code_prefix: str) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    text, read_blockers = _read_local_json_text(
        path,
        label=label,
        shape_code=f"{code_prefix}_file_shape",
        unreadable_code=f"{code_prefix}_unreadable",
    )
    if read_blockers:
        return None, read_blockers
    assert text is not None
    try:
        payload = json.loads(
            text,
            object_pairs_hook=readiness._reject_duplicate_json_object_pairs,  # type: ignore[attr-defined]
            parse_constant=readiness._reject_nonfinite_json_constant,  # type: ignore[attr-defined]
        )
    except json.JSONDecodeError as exc:
        return None, [_blocker(f"{code_prefix}_invalid_json", f"{label} is not valid JSON: {exc}")]
    except readiness.DuplicateJsonKeyError as exc:  # type: ignore[attr-defined]
        return None, [
            _blocker(
                f"{code_prefix}_invalid_json",
                readiness._duplicate_json_key_message(label, exc),  # type: ignore[attr-defined]
            )
        ]
    except readiness.NonFiniteJsonConstantError as exc:  # type: ignore[attr-defined]
        return None, [
            _blocker(
                f"{code_prefix}_invalid_json",
                f"{label} is not strict JSON: non-finite constant {exc.constant} is not allowed",
            )
        ]
    if not isinstance(payload, dict):
        return None, [_blocker(f"{code_prefix}_not_object", f"{label} must be a JSON object")]
    return payload, []


def _sha256_file(path: Path, label: str, code: str) -> tuple[str | None, list[dict[str, Any]]]:
    expected_stat, file_blockers = _validate_local_file_for_read(path, label, code)
    if file_blockers:
        return None, file_blockers
    assert expected_stat is not None
    digest_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [_blocker(code, f"{label} must not be a symlink")]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [_blocker(code, f"{label} must be a regular file")]
            digest_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if digest_open_identity != digest_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != digest_expected_identity:
                return None, [_blocker(code, f"{label} changed while being read")]
            if open_stat.st_nlink > 1:
                return None, [_blocker(code, f"{label} must not be hardlinked")]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != digest_expected_identity:
                return None, [_blocker(code, f"{label} changed while being read")]
    except OSError:
        return None, [_blocker(code, f"{label} could not be read")]
    return digest.hexdigest(), []


def _sha256_file_with_size(
    path: Path,
    label: str,
    code: str,
) -> tuple[str | None, int | None, list[dict[str, Any]]]:
    expected_stat, file_blockers = _validate_local_file_for_read(path, label, code)
    if file_blockers:
        return None, None, file_blockers
    assert expected_stat is not None
    sized_digest_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, None, [_blocker(code, f"{label} must not be a symlink")]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, None, [_blocker(code, f"{label} must be a regular file")]
            sized_digest_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if sized_digest_open_identity != sized_digest_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != sized_digest_expected_identity:
                return None, None, [_blocker(code, f"{label} changed while being read")]
            if open_stat.st_nlink > 1:
                return None, None, [_blocker(code, f"{label} must not be hardlinked")]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != (
                sized_digest_expected_identity
            ):
                return None, None, [_blocker(code, f"{label} changed while being read")]
    except OSError:
        return None, None, [_blocker(code, f"{label} could not be read")]
    if size <= 0:
        return None, None, [
            _blocker(
                code,
                f"{label} must be non-empty",
            )
        ]
    return digest.hexdigest(), size, []


def _relative_to_bundle(path: Path, bundle_root: Path, label: str) -> tuple[str | None, list[dict[str, Any]]]:
    path_secret = _secret_path_error(
        str(path),
        label,
        "kagemusha_release_bundle_path_invalid",
    )
    if path_secret is not None:
        return None, [path_secret]
    path_shape = _bundle_path_shape_error(path, label)
    if path_shape is not None:
        return None, [path_shape]
    root_secret = _secret_path_error(
        str(bundle_root),
        "--bundle-root",
        "kagemusha_release_bundle_path_invalid",
    )
    if root_secret is not None:
        return None, [root_secret]
    root_shape = _bundle_root_shape_error(bundle_root)
    if root_shape is not None:
        return None, [root_shape]
    try:
        resolved_path = path.resolve()
        resolved_root = bundle_root.resolve()
    except OSError:
        return None, [
            _blocker(
                "kagemusha_release_bundle_path_invalid",
                f"{label} could not be resolved",
            )
        ]
    try:
        return resolved_path.relative_to(resolved_root).as_posix(), []
    except ValueError:
        return None, [
            _blocker(
                "kagemusha_release_bundle_path_outside_root",
                f"{label} must stay under --bundle-root",
            )
        ]


def _bundle_path(raw_path: str, bundle_root: Path) -> Path:
    path = Path(raw_path)
    return path if path.is_absolute() else bundle_root / path


def _preflight_bundle_input_path(
    path: Path,
    bundle_root: Path,
    label: str,
) -> tuple[bool, list[dict[str, Any]]]:
    _, blockers = _relative_to_bundle(path, bundle_root, label)
    return not blockers, blockers


def _validate_output_parent_path(parent: Path) -> list[dict[str, Any]]:
    try:
        parent_mode = parent.lstat().st_mode
    except FileNotFoundError:
        return []
    except OSError:
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out parent directory metadata could not be read",
            )
        ]
    if stat.S_ISLNK(parent_mode):
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out parent directory must not be a symlink",
            )
        ]
    if not stat.S_ISDIR(parent_mode):
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out parent must be a directory",
            )
        ]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        parent,
        "--out ancestor directory",
    )
    return [
        _blocker("kagemusha_release_bundle_out_invalid", error)
        for error in ancestor_errors
    ]


def _validate_output_path(out_path: Path, bundle_root: Path) -> list[dict[str, Any]]:
    secret = _secret_path_error(str(out_path), "--out", "kagemusha_release_bundle_out_invalid")
    if secret is not None:
        return [secret]
    _, root_errors = _relative_to_bundle(out_path, bundle_root, "--out")
    if root_errors:
        return root_errors
    parent = out_path.parent
    try:
        parent_mode = parent.lstat().st_mode
    except FileNotFoundError:
        ancestor_errors = device_lab.validate_no_symlink_ancestors(
            parent,
            "--out ancestor directory",
        )
        if ancestor_errors:
            return [
                _blocker("kagemusha_release_bundle_out_invalid", error)
                for error in ancestor_errors
            ]
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            return [
                _blocker(
                    "kagemusha_release_bundle_out_invalid",
                    "--out parent directory could not be created",
                )
            ]
        post_create_errors = _validate_output_parent_path(parent)
        if post_create_errors:
            return post_create_errors
    except OSError:
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out parent directory metadata could not be read",
            )
        ]
    else:
        parent_errors = _validate_output_parent_path(parent)
        if parent_errors:
            return parent_errors
    try:
        mode = out_path.lstat().st_mode
    except FileNotFoundError:
        return []
    except OSError:
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out file metadata could not be read",
            )
        ]
    if stat.S_ISLNK(mode):
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out must not be a symlink",
            )
        ]
    if not stat.S_ISREG(mode):
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out must be a regular file",
            )
        ]
    try:
        link_count = out_path.stat().st_nlink
    except OSError:
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out hardlink metadata could not be read",
            )
        ]
    if link_count > 1:
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out must not be hardlinked",
            )
        ]
    return []


def _parse_optional_timestamp(value: str, label: str, code: str) -> tuple[dt.datetime | None, list[dict[str, Any]]]:
    if not value:
        return None, []
    parsed, parse_blocker = readiness.parse_utc_timestamp(value, label)
    if parse_blocker is None:
        return parsed, []
    parse_blocker["code"] = code
    return None, [parse_blocker]


def _future_limit(seconds: int, label: str, code: str) -> tuple[dt.datetime | None, list[dict[str, Any]]]:
    if seconds < 0:
        return None, [_blocker(code, f"{label} must be non-negative")]
    return (
        dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
        + dt.timedelta(seconds=seconds),
        [],
    )


def _section(summary: dict[str, Any], name: str) -> dict[str, Any] | None:
    value = summary.get(name)
    return value if isinstance(value, dict) else None


def _display_summary_field(field: Any) -> str:
    text = str(field)
    if device_lab.SECRET_RE.search(text):
        return device_lab.SECRET_PATH_REDACTION
    if device_lab._contains_control_character(text):
        return device_lab.CONTROL_PATH_REDACTION
    return text


def _contains_secret_string(value: Any) -> bool:
    if isinstance(value, str):
        return device_lab.SECRET_RE.search(value) is not None
    if isinstance(value, dict):
        return any(
            _contains_secret_string(key) or _contains_secret_string(item)
            for key, item in value.items()
        )
    if isinstance(value, list):
        return any(_contains_secret_string(item) for item in value)
    return False


def _contains_control_string(value: Any) -> bool:
    if isinstance(value, str):
        return device_lab._contains_control_character(value)
    if isinstance(value, dict):
        return any(
            _contains_control_string(key) or _contains_control_string(item)
            for key, item in value.items()
        )
    if isinstance(value, list):
        return any(_contains_control_string(item) for item in value)
    return False


def _check_android_signed_evidence_summary_shape(
    android: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    signed_evidence_summary = android.get("signed_evidence")
    if not isinstance(signed_evidence_summary, dict):
        return [
            _blocker(
                "kagemusha_release_summary_android_signed_evidence_shape",
                "Android signed-evidence summary must be a JSON object",
            )
        ]
    if not signed_evidence_summary:
        return [
            _blocker(
                "kagemusha_release_summary_android_signed_evidence_shape",
                "Android signed-evidence summary must not be empty",
            )
        ]

    for raw_slot, entry in signed_evidence_summary.items():
        slot, slot_blockers = _validate_android_manifest_slot(raw_slot)
        for blocker in slot_blockers:
            blockers.append(
                {
                    **blocker,
                    "code": "kagemusha_release_summary_android_signed_evidence_slot",
                }
            )
        display_slot = _display_summary_field(raw_slot)
        if not isinstance(entry, dict):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_signed_evidence_shape",
                    "Android signed-evidence summary slot entry must be a JSON object",
                    slot=display_slot,
                )
            )
            continue

        unexpected_fields = sorted(
            set(entry) - ANDROID_SIGNED_EVIDENCE_SUMMARY_REQUIRED_FIELDS
        )
        for field in unexpected_fields:
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_signed_evidence_unexpected_field",
                    "Android signed-evidence summary slot entry contains an unexpected field",
                    slot=display_slot,
                    field=_display_summary_field(field),
                )
            )
        missing_fields = sorted(
            ANDROID_SIGNED_EVIDENCE_SUMMARY_REQUIRED_FIELDS - set(entry)
        )
        for field in missing_fields:
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_signed_evidence_missing_field",
                    "Android signed-evidence summary slot entry is missing a required field",
                    slot=display_slot,
                    field=field,
                )
            )

        for field in sorted(ANDROID_SIGNED_EVIDENCE_SUMMARY_REQUIRED_FIELDS & set(entry)):
            value = entry.get(field)
            if not isinstance(value, str) or not value:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_signed_evidence_value",
                        f"Android signed-evidence summary field {field} must be a non-empty string",
                        slot=display_slot,
                        field=field,
                    )
                )
                continue
            if field == "signed_at_utc":
                if not device_lab.SIGNED_AT_UTC_RE.fullmatch(value):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_signed_evidence_timestamp",
                            "Android signed-evidence summary timestamp must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                            slot=display_slot,
                        )
                    )
                    continue
                parsed_timestamp, timestamp_blocker = readiness.parse_utc_timestamp(
                    value,
                    "Android signed-evidence summary signed_at_utc",
                )
                if timestamp_blocker is not None:
                    timestamp_blocker["code"] = (
                        "kagemusha_release_summary_android_signed_evidence_timestamp"
                    )
                    timestamp_blocker["slot"] = display_slot
                    blockers.append(timestamp_blocker)
                elif parsed_timestamp is not None:
                    max_timestamp = dt.datetime.now(dt.timezone.utc).replace(
                        microsecond=0,
                    ) + dt.timedelta(
                        seconds=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
                    )
                    if parsed_timestamp > max_timestamp:
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_signed_evidence_future_dated",
                                "Android signed-evidence summary timestamp must not be future-dated beyond the allowed clock skew",
                                slot=display_slot,
                                max_timestamp_utc=max_timestamp.isoformat().replace(
                                    "+00:00",
                                    "Z",
                                ),
                            )
                        )
            elif field in ANDROID_SIGNED_EVIDENCE_SUMMARY_SHA256_FIELDS:
                if (
                    not device_lab.SHA256_HEX_RE.fullmatch(value)
                    or value == "0" * 64
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_signed_evidence_sha256",
                            "Android signed-evidence summary field must be a non-zero lowercase sha256 hex digest",
                            slot=display_slot,
                            field=field,
                        )
                    )
            elif field in ANDROID_SIGNED_EVIDENCE_SUMMARY_PATH_FIELDS:
                path_errors: list[str] = []
                safe_relative = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                    value,
                    path_errors,
                    f"Android signed-evidence summary {field}",
                )
                if (
                    safe_relative is None
                ):
                    blockers.extend(
                        _blocker(
                            "kagemusha_release_summary_android_signed_evidence_path",
                            error,
                            slot=display_slot,
                            field=field,
                        )
                        for error in path_errors
                    )
                elif (
                    field == "d2d_payment_transcript_path"
                    and not device_lab._safe_relative_path_is_child_of(  # type: ignore[attr-defined]
                        safe_relative,
                        "handoff",
                    )
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_signed_evidence_path",
                            "Android signed-evidence summary d2d_payment_transcript_path must stay under handoff/",
                            slot=display_slot,
                            field=field,
                        )
                    )
                elif (
                    field == "wallet_integrity_transcript_path"
                    and not device_lab._safe_relative_path_is_child_of(  # type: ignore[attr-defined]
                        safe_relative,
                        "wallet",
                    )
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_signed_evidence_path",
                            "Android signed-evidence summary wallet_integrity_transcript_path must stay under wallet/",
                            slot=display_slot,
                            field=field,
                        )
                    )
                elif (
                    field == "attestation_certificate_chain_path"
                    and not device_lab._safe_relative_path_is_child_of(  # type: ignore[attr-defined]
                        safe_relative,
                        "attestation",
                    )
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_signed_evidence_path",
                            "Android signed-evidence summary attestation_certificate_chain_path must stay under attestation/",
                            slot=display_slot,
                            field=field,
                        )
                    )
            elif field in ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:
                if (
                    value != value.strip()
                    or device_lab._contains_control_character(value)
                    or device_lab.SECRET_RE.search(value)
                    or (
                        field == "device_family"
                        and value not in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
                    )
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_signed_evidence_value",
                            "Android signed-evidence summary identity field must be an exact non-secret string",
                            slot=display_slot,
                            field=field,
                        )
                    )
        device_family = entry.get("device_family")
        device_model = entry.get("device_model")
        device_codename = entry.get("device_codename")
        if (
            isinstance(device_family, str)
            and isinstance(device_model, str)
            and isinstance(device_codename, str)
            and device_model
            and device_codename
            and device_family in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
            and device_model == device_model.strip()
            and device_codename == device_codename.strip()
            and not device_lab._contains_control_character(device_model)
            and not device_lab._contains_control_character(device_codename)
            and not device_lab.SECRET_RE.search(device_model)
            and not device_lab.SECRET_RE.search(device_codename)
        ):
            inferred_family = device_lab.infer_kagemusha_device_family(
                device_model,
                device_codename,
            )
            if inferred_family != device_family:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_signed_evidence_identity",
                        "Android signed-evidence summary model/codename must match device family",
                        slot=display_slot,
                    )
                )
    return blockers


def _check_android_duplicate_bindings_summary_shape(
    android: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    duplicate_bindings = android.get("duplicate_bindings")
    signed_evidence_summary = android.get("signed_evidence")
    slot_kagemusha_by_slot: dict[str, dict[str, Any]] = {}
    slots_summary = android.get("slots")
    if isinstance(slots_summary, list):
        for slot_entry in slots_summary:
            if not isinstance(slot_entry, dict):
                continue
            slot = slot_entry.get("slot")
            kagemusha = slot_entry.get("kagemusha")
            if isinstance(slot, str) and isinstance(kagemusha, dict):
                slot_kagemusha_by_slot[slot] = kagemusha
    if not isinstance(duplicate_bindings, dict):
        return [
            _blocker(
                "kagemusha_release_summary_android_duplicate_bindings_shape",
                "Android duplicate-bindings summary must be a JSON object",
            )
        ]

    for raw_field, entries in duplicate_bindings.items():
        field = _display_summary_field(raw_field)
        if raw_field not in ANDROID_DUPLICATE_BINDING_SUMMARY_FIELDS:
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_duplicate_bindings_unexpected_field",
                    "Android duplicate-bindings summary contains an unexpected field",
                    field=field,
                )
            )
            continue
        if not isinstance(entries, list):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_duplicate_bindings_shape",
                    "Android duplicate-bindings summary entries must be a list",
                    field=field,
                )
            )
            continue
        duplicate_value_sha256_by_value: dict[str, int] = {}
        valid_value_sha256s: list[str] = []
        for index, entry in enumerate(entries):
            if not isinstance(entry, dict):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_duplicate_bindings_shape",
                        "Android duplicate-bindings summary entry must be a JSON object",
                        field=field,
                        index=index,
                    )
                )
                continue
            unexpected_fields = sorted(
                set(entry) - ANDROID_DUPLICATE_BINDING_ENTRY_FIELDS
            )
            for entry_field in unexpected_fields:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_duplicate_bindings_unexpected_field",
                        "Android duplicate-bindings summary entry contains an unexpected field",
                        field=field,
                        entry_field=_display_summary_field(entry_field),
                        index=index,
                    )
                )
            missing_fields = sorted(
                ANDROID_DUPLICATE_BINDING_ENTRY_FIELDS - set(entry)
            )
            for entry_field in missing_fields:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_duplicate_bindings_missing_field",
                        "Android duplicate-bindings summary entry is missing a required field",
                        field=field,
                        entry_field=entry_field,
                        index=index,
                    )
                )
            value_sha256 = entry.get("value_sha256")
            value_sha256_valid = (
                isinstance(value_sha256, str)
                and device_lab.SHA256_HEX_RE.fullmatch(value_sha256) is not None
                and value_sha256 != "0" * 64
            )
            if not value_sha256_valid:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_duplicate_bindings_sha256",
                        "Android duplicate-bindings summary value must be a non-zero lowercase sha256 hex digest",
                        field=field,
                        index=index,
                    )
                )
            else:
                assert isinstance(value_sha256, str)
                prior_index = duplicate_value_sha256_by_value.get(value_sha256)
                if prior_index is not None:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_duplicate_bindings_value_inventory",
                            "Android duplicate-bindings summary values must be unique",
                            field=field,
                            index=index,
                            prior_index=prior_index,
                        )
                    )
                duplicate_value_sha256_by_value[value_sha256] = index
                valid_value_sha256s.append(value_sha256)
            slots = entry.get("slots")
            if not isinstance(slots, list) or not slots:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_duplicate_bindings_slots",
                        "Android duplicate-bindings summary slots must be a non-empty list",
                        field=field,
                        index=index,
                    )
                )
                continue
            validated_slots: list[str] = []
            for raw_slot in slots:
                slot, slot_blockers = _validate_android_manifest_slot(raw_slot)
                for blocker in slot_blockers:
                    blockers.append(
                        {
                            **blocker,
                            "code": "kagemusha_release_summary_android_duplicate_bindings_slot",
                            "field": field,
                            "index": index,
                        }
                    )
                if slot is not None:
                    validated_slots.append(slot)
                    if (
                        isinstance(signed_evidence_summary, dict)
                        and slot not in signed_evidence_summary
                    ):
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_duplicate_bindings_slot_binding",
                                "Android duplicate-bindings summary slots must be present in signed-evidence summary",
                                field=field,
                                index=index,
                                slot=_display_summary_field(slot),
                            )
                        )
            if value_sha256_valid and slot_kagemusha_by_slot:
                for slot in validated_slots:
                    kagemusha = slot_kagemusha_by_slot.get(slot)
                    if not isinstance(kagemusha, dict):
                        continue
                    if kagemusha.get(raw_field) == value_sha256:
                        continue
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_duplicate_bindings_value_binding",
                            "Android duplicate-bindings summary value must match the named signed-evidence field for every slot",
                            field=field,
                            index=index,
                            slot=_display_summary_field(slot),
                        )
                    )
            if len(set(validated_slots)) < 2:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_duplicate_bindings_slots",
                        "Android duplicate-bindings summary must name at least two distinct slots",
                        field=field,
                        index=index,
                    )
                )
            elif len(validated_slots) == len(slots) and validated_slots != sorted(
                set(validated_slots)
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_duplicate_bindings_slots",
                        "Android duplicate-bindings summary slots must be unique and sorted",
                        field=field,
                        index=index,
                    )
                )
        if (
            valid_value_sha256s
            and len(valid_value_sha256s) == len(entries)
            and valid_value_sha256s != sorted(set(valid_value_sha256s))
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_duplicate_bindings_value_inventory",
                    "Android duplicate-bindings summary values must be sorted by value_sha256",
                    field=field,
                )
            )
    return blockers


def _check_android_ready_summary_shape(android: dict[str, Any]) -> list[dict[str, Any]]:
    """Validate release-facing Android readiness summary lists."""

    blockers: list[dict[str, Any]] = []
    if android.get("root") != readiness.ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_android_root",
                "Android readiness summary root must use the canonical redacted label",
            )
        )
    list_fields_ok: dict[str, bool] = {}
    for field in (
        "covered_device_families",
        "missing_device_families",
        "covered_d2d_payment_transports",
        "missing_d2d_payment_transports",
        "trusted_signer_public_key_sha256",
    ):
        value = android.get(field)
        field_ok = isinstance(value, list) and all(
            isinstance(item, str) and item for item in value
        )
        list_fields_ok[field] = field_ok
        if not field_ok:
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_list_shape",
                    "Android readiness summary list fields must contain non-empty strings",
                    field=field,
                )
            )
    signed_evidence_summary = android.get("signed_evidence")
    slots = android.get("slots")
    validated_slots: list[str] = []
    slot_device_families: list[str] = []
    slot_d2d_payment_transports: list[str] = []
    if not isinstance(slots, list) or not slots:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_android_slots_shape",
                "Android readiness summary slots must be a non-empty list",
            )
        )
    else:
        for entry in slots:
            if not isinstance(entry, dict):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_slots_shape",
                        "Android readiness summary slots must contain JSON objects",
                    )
                )
                continue
            allowed_slot_fields = {
                "slot",
                "status",
                "kagemusha",
                "present",
                "file_counts",
                "errors",
            }
            raw_slot = entry.get("slot")
            for field in sorted(set(entry) - allowed_slot_fields):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_slots_unexpected_field",
                        "Android readiness summary slot entry contains an unexpected field",
                        slot=(
                            _display_summary_field(raw_slot)
                            if isinstance(raw_slot, str)
                            else None
                        ),
                        field=_display_summary_field(field),
                    )
                )
            if not isinstance(raw_slot, str):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_slots_slot",
                        "Android readiness summary slot entries must name a safe slot",
                    )
                )
            else:
                slot_ids, slot_errors = device_lab.validate_slot_ids([raw_slot])
                if slot_errors or slot_ids != [raw_slot]:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_slot",
                            "Android readiness summary slot entries must name a safe slot",
                            slot=_display_summary_field(raw_slot),
                        )
                    )
                else:
                    validated_slots.append(raw_slot)
            if entry.get("status") != "ok":
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_slots_status",
                        "Android readiness summary slot entries must be accepted",
                        slot=(
                            _display_summary_field(raw_slot)
                            if isinstance(raw_slot, str)
                            else None
                        ),
                    )
                )
            errors = entry.get("errors")
            if errors != []:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_slots_errors",
                        "Android readiness summary accepted slots must not contain errors",
                        slot=(
                            _display_summary_field(raw_slot)
                            if isinstance(raw_slot, str)
                            else None
                        ),
                    )
                )
            expected_present = {
                "attestation",
                "logs",
                "queue",
                "sha256sum.txt",
                "telemetry",
            }
            present = entry.get("present")
            if (
                not isinstance(present, dict)
                or set(present) != expected_present
                or any(value is not True for value in present.values())
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_slots_present",
                        "Android readiness summary accepted slots must mark every release-critical artifact group present",
                        slot=(
                            _display_summary_field(raw_slot)
                            if isinstance(raw_slot, str)
                            else None
                        ),
                    )
                )
            expected_file_counts = {"attestation", "logs", "queue", "telemetry"}
            file_counts = entry.get("file_counts")
            if (
                not isinstance(file_counts, dict)
                or set(file_counts) != expected_file_counts
                or any(
                    isinstance(value, bool)
                    or not isinstance(value, int)
                    or value <= 0
                    for value in file_counts.values()
                )
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_slots_file_counts",
                        "Android readiness summary accepted slots must carry positive file counts for every release-critical artifact group",
                        slot=(
                            _display_summary_field(raw_slot)
                            if isinstance(raw_slot, str)
                            else None
                        ),
                    )
                )
            if not isinstance(entry.get("kagemusha"), dict):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_slots_shape",
                        "Android readiness summary accepted slots must contain Kagemusha details",
                        slot=(
                            _display_summary_field(raw_slot)
                            if isinstance(raw_slot, str)
                            else None
                        ),
                    )
                )
            elif isinstance(raw_slot, str):
                kagemusha = entry["kagemusha"]
                display_slot = _display_summary_field(raw_slot)
                required_fields = {
                    "required",
                    "device_family",
                    "device_model",
                    "device_codename",
                    "native_bridge_abi_version",
                    "signed_at_utc",
                    "signed_evidence_artifact_sha256",
                    "signed_evidence_signer_public_key_sha256",
                    "device_fingerprint_sha256",
                    "attestation_challenge_sha256",
                    "d2d_payment_transport",
                    *(
                        field
                        for _, path_field, digest_field in ANDROID_SLOT_RELEASE_ARTIFACTS
                        for field in (path_field, digest_field)
                    ),
                }
                optional_fields = {
                    "d2d_payment_transports",
                    "d2d_payment_transcripts",
                }
                for field in sorted(set(kagemusha) - required_fields - optional_fields):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_kagemusha_unexpected_field",
                            "Android readiness summary Kagemusha slot details contain an unexpected field",
                            slot=display_slot,
                            field=_display_summary_field(field),
                        )
                    )
                for field in sorted(required_fields - set(kagemusha)):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_missing_field",
                            "Android readiness summary Kagemusha slot details are missing a required field",
                            slot=display_slot,
                            field=field,
                        )
                    )
                if kagemusha.get("required") is not True:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_value",
                            "Android readiness summary Kagemusha slot details must be required",
                            slot=display_slot,
                            field="required",
                        )
                    )
                device_family = kagemusha.get("device_family")
                device_family_valid = (
                    isinstance(device_family, str)
                    and device_family
                    in device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES
                )
                if (
                    not device_family_valid
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_device_family",
                            "Android readiness summary Kagemusha slot details must name a standard device family",
                            slot=display_slot,
                        )
                    )
                else:
                    assert isinstance(device_family, str)
                    slot_device_families.append(device_family)
                identity_fields_valid = True
                for field in ("device_model", "device_codename"):
                    value = kagemusha.get(field)
                    if (
                        not isinstance(value, str)
                        or not value
                        or value != value.strip()
                        or device_lab._contains_control_character(value)
                        or device_lab.SECRET_RE.search(value)
                    ):
                        identity_fields_valid = False
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_slots_device_identity",
                                f"Android readiness summary Kagemusha slot identity field {field} must be an exact non-secret string",
                                slot=display_slot,
                                field=field,
                            )
                        )
                if device_family_valid and identity_fields_valid:
                    device_model = kagemusha.get("device_model")
                    device_codename = kagemusha.get("device_codename")
                    assert isinstance(device_family, str)
                    assert isinstance(device_model, str)
                    assert isinstance(device_codename, str)
                    inferred_family = device_lab.infer_kagemusha_device_family(
                        device_model,
                        device_codename,
                    )
                    if inferred_family != device_family:
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_slots_device_identity",
                                "Android readiness summary Kagemusha slot model/codename must match device family",
                                slot=display_slot,
                            )
                        )
                abi_version = kagemusha.get("native_bridge_abi_version")
                if (
                    isinstance(abi_version, bool)
                    or not isinstance(abi_version, int)
                    or abi_version
                    != device_lab.REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_abi",
                            "Android readiness summary Kagemusha slot details must name the required native ABI version",
                            slot=display_slot,
                        )
                    )
                d2d_transport = kagemusha.get("d2d_payment_transport")
                declared_d2d_transports: set[str] = set()
                if (
                    not isinstance(d2d_transport, str)
                    or d2d_transport not in device_lab.D2D_PAYMENT_TRANSPORTS
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_d2d_transport",
                            "Android readiness summary Kagemusha slot details must name a required offline D2D transport",
                            slot=display_slot,
                        )
                    )
                else:
                    slot_d2d_payment_transports.append(d2d_transport)
                    declared_d2d_transports.add(d2d_transport)
                d2d_transports = kagemusha.get("d2d_payment_transports")
                d2d_transports_valid = False
                if d2d_transports is not None:
                    d2d_transports_all_strings = isinstance(
                        d2d_transports,
                        list,
                    ) and all(
                        isinstance(transport, str) for transport in d2d_transports
                    )
                    if (
                        not d2d_transports_all_strings
                        or not d2d_transports
                        or d2d_transports != sorted(set(d2d_transports))
                        or any(
                            transport not in device_lab.D2D_PAYMENT_TRANSPORTS
                            for transport in d2d_transports
                        )
                    ):
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_slots_d2d_transport",
                                "Android readiness summary Kagemusha slot D2D transports must be a sorted required-transport list",
                                slot=display_slot,
                            )
                        )
                    else:
                        d2d_transports_valid = True
                        slot_d2d_payment_transports.extend(d2d_transports)
                        declared_d2d_transports.update(d2d_transports)
                        if (
                            isinstance(d2d_transport, str)
                            and d2d_transport in device_lab.D2D_PAYMENT_TRANSPORTS
                            and d2d_transport not in d2d_transports
                        ):
                            blockers.append(
                                _blocker(
                                    "kagemusha_release_summary_android_slots_d2d_transport",
                                    "Android readiness summary Kagemusha slot D2D transports must include the primary transport",
                                    slot=display_slot,
                                )
                            )
                d2d_transcripts = kagemusha.get("d2d_payment_transcripts")
                d2d_transcript_paths: dict[str, str] = {}
                if d2d_transcripts is not None:
                    if not isinstance(d2d_transcripts, dict) or not d2d_transcripts:
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_slots_d2d_transcripts",
                                "Android readiness summary Kagemusha slot D2D transcripts must be a non-empty object",
                                slot=display_slot,
                            )
                        )
                    else:
                        for transport, binding in sorted(d2d_transcripts.items()):
                            if transport not in device_lab.D2D_PAYMENT_TRANSPORTS:
                                blockers.append(
                                    _blocker(
                                        "kagemusha_release_summary_android_slots_d2d_transcripts",
                                        "Android readiness summary Kagemusha slot D2D transcript keys must be required transports",
                                        slot=display_slot,
                                        field=_display_summary_field(transport),
                                    )
                                )
                                continue
                            if (
                                not isinstance(binding, dict)
                                or set(binding) != {"path", "sha256"}
                                or not isinstance(binding.get("path"), str)
                                or not isinstance(binding.get("sha256"), str)
                            ):
                                blockers.append(
                                    _blocker(
                                        "kagemusha_release_summary_android_slots_d2d_transcripts",
                                        "Android readiness summary Kagemusha slot D2D transcript bindings must contain path and sha256",
                                        slot=display_slot,
                                        field=transport,
                                    )
                                )
                                continue
                            path_errors: list[str] = []
                            safe_relative = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                                binding["path"],
                                path_errors,
                                "Android readiness summary Kagemusha slot D2D transcript path",
                            )
                            if (
                                safe_relative is None
                                or not device_lab._safe_relative_path_is_child_of(  # type: ignore[attr-defined]
                                    safe_relative,
                                    "handoff",
                                )
                            ):
                                blockers.append(
                                    _blocker(
                                        "kagemusha_release_summary_android_slots_d2d_transcripts",
                                        "Android readiness summary Kagemusha slot D2D transcript path must stay under handoff/",
                                        slot=display_slot,
                                        field=transport,
                                    )
                                )
                            else:
                                previous_transport = d2d_transcript_paths.get(
                                    safe_relative
                                )
                                if (
                                    previous_transport is not None
                                    and previous_transport != transport
                                ):
                                    blockers.append(
                                        _blocker(
                                            "kagemusha_release_summary_android_slots_d2d_transcripts",
                                            "Android readiness summary Kagemusha slot D2D transcript bindings must not reuse paths across transports",
                                            slot=display_slot,
                                            field=transport,
                                        )
                                    )
                                else:
                                    d2d_transcript_paths[safe_relative] = transport
                            digest = binding["sha256"]
                            if (
                                device_lab.SHA256_HEX_RE.fullmatch(digest) is None
                                or digest == "0" * 64
                            ):
                                blockers.append(
                                    _blocker(
                                        "kagemusha_release_summary_android_slots_d2d_transcripts",
                                        "Android readiness summary Kagemusha slot D2D transcript sha256 must be non-zero lowercase sha256 hex",
                                        slot=display_slot,
                                        field=transport,
                                    )
                                )
                if (
                    d2d_transports_valid
                    and isinstance(d2d_transcripts, dict)
                    and d2d_transcripts
                    and set(d2d_transcripts) != declared_d2d_transports
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_d2d_transcripts",
                            "Android readiness summary Kagemusha slot D2D transcript bindings must exactly match declared transports",
                            slot=display_slot,
                        )
                    )
                elif d2d_transports_valid and d2d_transcripts is None:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_d2d_transcripts",
                            "Android readiness summary Kagemusha slot D2D transcript bindings must be present when a transport list is declared",
                            slot=display_slot,
                        )
                    )
                elif (
                    d2d_transports is None
                    and isinstance(d2d_transcripts, dict)
                    and d2d_transcripts
                    and set(d2d_transcripts) != declared_d2d_transports
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_d2d_transcripts",
                            "Android readiness summary Kagemusha slot D2D transcript bindings must match the primary transport when no transport list is declared",
                            slot=display_slot,
                        )
                    )
                signed_at = kagemusha.get("signed_at_utc")
                if (
                    not isinstance(signed_at, str)
                    or not device_lab.SIGNED_AT_UTC_RE.fullmatch(signed_at)
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_summary_android_slots_timestamp",
                            "Android readiness summary Kagemusha slot timestamp must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                            slot=display_slot,
                        )
                    )
                else:
                    parsed_signed_at, timestamp_blocker = readiness.parse_utc_timestamp(
                        signed_at,
                        "Android readiness summary Kagemusha slot signed_at_utc",
                    )
                    if timestamp_blocker is not None:
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_slots_timestamp",
                                "Android readiness summary Kagemusha slot timestamp is invalid",
                                slot=display_slot,
                            )
                        )
                    elif parsed_signed_at is not None:
                        max_timestamp = dt.datetime.now(dt.timezone.utc).replace(
                            microsecond=0,
                        ) + dt.timedelta(
                            seconds=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
                        )
                        if parsed_signed_at > max_timestamp:
                            blockers.append(
                                _blocker(
                                    "kagemusha_release_summary_android_slots_future_dated",
                                    "Android readiness summary Kagemusha slot timestamp must not be future-dated beyond the allowed clock skew",
                                    slot=display_slot,
                                    max_timestamp_utc=max_timestamp.isoformat().replace(
                                        "+00:00",
                                        "Z",
                                    ),
                                )
                            )
                sha_fields = {
                    "signed_evidence_artifact_sha256",
                    "signed_evidence_signer_public_key_sha256",
                    "device_fingerprint_sha256",
                    "attestation_challenge_sha256",
                    *(
                        digest_field
                        for _, _, digest_field in ANDROID_SLOT_RELEASE_ARTIFACTS
                    ),
                }
                for field in sorted(sha_fields & set(kagemusha)):
                    value = kagemusha.get(field)
                    if (
                        not isinstance(value, str)
                        or not device_lab.SHA256_HEX_RE.fullmatch(value)
                        or value == "0" * 64
                    ):
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_slots_sha256",
                                "Android readiness summary Kagemusha slot digest fields must be non-zero lowercase sha256 hex strings",
                                slot=display_slot,
                                field=field,
                            )
                        )
                for _, path_field, _ in ANDROID_SLOT_RELEASE_ARTIFACTS:
                    if path_field not in kagemusha:
                        continue
                    value = kagemusha.get(path_field)
                    path_errors: list[str] = []
                    if not isinstance(value, str) or not value:
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_slots_path",
                                "Android readiness summary Kagemusha slot artifact paths must be non-empty strings",
                                slot=display_slot,
                                field=path_field,
                            )
                        )
                    else:
                        safe_relative = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                            value,
                            path_errors,
                            f"Android readiness summary Kagemusha slot {path_field}",
                        )
                        if safe_relative is None:
                            blockers.extend(
                                _blocker(
                                    "kagemusha_release_summary_android_slots_path",
                                    error,
                                    slot=display_slot,
                                    field=path_field,
                                )
                                for error in path_errors
                            )
                        elif (
                            path_field == "d2d_payment_transcript_path"
                            and not device_lab._safe_relative_path_is_child_of(  # type: ignore[attr-defined]
                                safe_relative,
                                "handoff",
                            )
                        ):
                            blockers.append(
                                _blocker(
                                    "kagemusha_release_summary_android_slots_path",
                                    "Android readiness summary Kagemusha slot d2d_payment_transcript_path must stay under handoff/",
                                    slot=display_slot,
                                    field=path_field,
                                )
                            )
                        elif (
                            path_field == "wallet_integrity_transcript_path"
                            and not device_lab._safe_relative_path_is_child_of(  # type: ignore[attr-defined]
                                safe_relative,
                                "wallet",
                            )
                        ):
                            blockers.append(
                                _blocker(
                                    "kagemusha_release_summary_android_slots_path",
                                    "Android readiness summary Kagemusha slot wallet_integrity_transcript_path must stay under wallet/",
                                    slot=display_slot,
                                    field=path_field,
                                )
                            )
                        elif (
                            path_field == "attestation_certificate_chain_path"
                            and not device_lab._safe_relative_path_is_child_of(  # type: ignore[attr-defined]
                                safe_relative,
                                "attestation",
                            )
                        ):
                            blockers.append(
                                _blocker(
                                    "kagemusha_release_summary_android_slots_path",
                                    "Android readiness summary Kagemusha slot attestation_certificate_chain_path must stay under attestation/",
                                    slot=display_slot,
                                    field=path_field,
                                )
                            )
                summary_entry = (
                    signed_evidence_summary.get(raw_slot)
                    if isinstance(signed_evidence_summary, dict)
                    else None
                )
                if isinstance(summary_entry, dict):
                    bindings = {
                        "device_family": "device_family",
                        "device_model": "device_model",
                        "device_codename": "device_codename",
                        "signed_at_utc": "signed_at_utc",
                        "signed_evidence_artifact_sha256": "artifact_sha256",
                        "signed_evidence_signer_public_key_sha256": (
                            "signer_public_key_sha256"
                        ),
                    }
                    for _, path_field, digest_field in ANDROID_SLOT_RELEASE_ARTIFACTS:
                        bindings[path_field] = path_field
                        bindings[digest_field] = digest_field
                    for slot_field, summary_field in bindings.items():
                        if kagemusha.get(slot_field) == summary_entry.get(
                            summary_field
                        ):
                            continue
                        blockers.append(
                            _blocker(
                                "kagemusha_release_summary_android_slots_binding",
                                "Android readiness summary Kagemusha slot details must match signed-evidence summary fields",
                                slot=display_slot,
                                field=slot_field,
                            )
                        )
        if validated_slots and validated_slots != sorted(set(validated_slots)):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_slots_inventory",
                    "Android readiness summary slots must be unique and sorted",
                )
            )
        if isinstance(signed_evidence_summary, dict) and set(validated_slots) != set(
            signed_evidence_summary
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_slots_inventory",
                    "Android readiness summary slots must match signed-evidence slots",
                )
            )
        covered_families = android.get("covered_device_families")
        if (
            isinstance(covered_families, list)
            and all(isinstance(item, str) for item in covered_families)
            and sorted(slot_device_families) != covered_families
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_slots_device_family_inventory",
                    "Android readiness summary slot device families must exactly match covered_device_families",
                )
            )
        covered_transports = android.get("covered_d2d_payment_transports")
        if (
            isinstance(covered_transports, list)
            and all(isinstance(item, str) for item in covered_transports)
            and sorted(set(slot_d2d_payment_transports)) != covered_transports
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_slots_d2d_transport_inventory",
                    "Android readiness summary slot D2D transports must exactly match covered_d2d_payment_transports",
                )
            )

    if list_fields_ok.get("covered_device_families"):
        expected_families = sorted(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES)
        if android.get("covered_device_families") != expected_families:
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_device_families",
                    "Android readiness summary covered_device_families must exactly match the standard matrix",
                )
            )
    if (
        list_fields_ok.get("missing_device_families")
        and android.get("missing_device_families") != []
    ):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_android_device_families",
                "Android readiness summary missing_device_families must be empty",
            )
        )
    if list_fields_ok.get("covered_d2d_payment_transports"):
        if android.get("covered_d2d_payment_transports") != list(
            readiness.ANDROID_REQUIRED_D2D_PAYMENT_TRANSPORTS
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_d2d_transports",
                    "Android readiness summary covered_d2d_payment_transports must cover every required offline D2D transport",
                )
            )
    if (
        list_fields_ok.get("missing_d2d_payment_transports")
        and android.get("missing_d2d_payment_transports") != []
    ):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_android_d2d_transports",
                "Android readiness summary missing_d2d_payment_transports must be empty",
            )
        )
    if list_fields_ok.get("trusted_signer_public_key_sha256"):
        signer_digests = android.get("trusted_signer_public_key_sha256")
        assert isinstance(signer_digests, list)
        if (
            not signer_digests
            or signer_digests != sorted(set(signer_digests))
            or any(
                not device_lab.SHA256_HEX_RE.fullmatch(digest)
                or digest == "0" * 64
                for digest in signer_digests
            )
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_signer_sha256",
                    "Android readiness summary trusted signer digests must be unique sorted non-zero lowercase sha256 hex strings",
                )
            )
    blockers.extend(_check_android_trusted_signer_binding(android))
    return blockers


def _check_android_trusted_signer_binding(
    android: dict[str, Any],
    *,
    code: str = "kagemusha_release_summary_android_signer_binding",
) -> list[dict[str, Any]]:
    """Reject signed Android evidence attributed to untrusted signer digests."""

    trusted_signers = android.get("trusted_signer_public_key_sha256")
    signed_evidence_summary = android.get("signed_evidence")
    if not isinstance(trusted_signers, list) or not all(
        isinstance(item, str) for item in trusted_signers
    ):
        return []
    if not isinstance(signed_evidence_summary, dict):
        return []
    trusted_signer_set = set(trusted_signers)
    blockers: list[dict[str, Any]] = []
    for raw_slot, entry in signed_evidence_summary.items():
        if not isinstance(entry, dict):
            continue
        signer = entry.get("signer_public_key_sha256")
        if (
            not isinstance(signer, str)
            or not device_lab.SHA256_HEX_RE.fullmatch(signer)
            or signer in trusted_signer_set
        ):
            continue
        blockers.append(
            _blocker(
                code,
                "Android signed-evidence signer digests must be included in trusted_signer_public_key_sha256",
                slot=_display_summary_field(raw_slot),
            )
        )
    return blockers


def _check_ready_summary_shape(summary: dict[str, Any]) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    if _contains_secret_string(summary):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_secret_material",
                "Kagemusha readiness summary must not contain secret-looking material",
            )
        )
    if _contains_control_string(summary):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_control_character",
                "Kagemusha readiness summary must not contain control characters",
            )
        )
    unexpected_fields = sorted(set(summary) - SUMMARY_ALLOWED_TOP_LEVEL_KEYS)
    for field in unexpected_fields:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_unexpected_field",
                "Kagemusha readiness summary contains an unexpected top-level field",
                field=_display_summary_field(field),
            )
        )
    for field in sorted(SUMMARY_ALLOWED_TOP_LEVEL_KEYS - set(summary)):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_missing_field",
                "Kagemusha readiness summary is missing a required top-level field",
                field=field,
            )
        )
    for section_name, allowed_fields in SUMMARY_ALLOWED_SECTION_KEYS.items():
        raw_section = summary.get(section_name)
        if not isinstance(raw_section, dict):
            if section_name in summary:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_shape",
                        "Kagemusha readiness summary section must be a JSON object",
                        section=section_name,
                    )
                )
            continue
        section = raw_section
        for field in sorted(set(section) - allowed_fields):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_unexpected_section_field",
                    "Kagemusha readiness summary section contains an unexpected field",
                    section=section_name,
                    field=_display_summary_field(field),
                )
            )
        for field in sorted(allowed_fields - set(section)):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_section_missing_field",
                    "Kagemusha readiness summary section is missing a required field",
                    section=section_name,
                    field=field,
                )
            )
        if "ok" in section and not isinstance(section.get("ok"), bool):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_section_ok_shape",
                    "Kagemusha readiness summary section ok flag must be boolean",
                    section=section_name,
                )
            )
        if "state" in section and not isinstance(section.get("state"), str):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_section_state_shape",
                    "Kagemusha readiness summary section state must be a string",
                    section=section_name,
                )
            )
        string_fields_by_section = {
            "abi6_reserved_lineage": ("manifest_path", "schema"),
            "abi7_recursive_compact": (
                "circuit_id",
                "fixture_manifest_path",
                "fixture_manifest_schema",
                "fixture_manifest_sha256",
                "archive_fixture_path",
                "archive_fixture_schema",
                "archive_fixture_sha256",
            ),
            "lineage_proof_evidence": (
                "path",
                "schema",
                "record_archive_proof_runtime_keygen_env",
            ),
            "compact_key_evidence": (
                "path",
                "schema",
                "verifier_backend",
                "circuit_id",
                "record_namespace",
                "generator_log_sha256",
            ),
            "localnet_lifecycle_evidence": (
                "path",
                "schema",
                "localnet_run_id",
                "chain_id",
                "target",
            ),
        }
        for field in string_fields_by_section.get(section_name, ()):
            value = section.get(field)
            if not isinstance(value, str) or not value:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_string",
                        "Kagemusha readiness summary section string field must be non-empty",
                        section=section_name,
                        field=field,
                    )
                )
        if section_name == "localnet_lifecycle_evidence":
            if not readiness._localnet_run_id_is_valid(section.get("localnet_run_id")):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_localnet_identity",
                        "Kagemusha readiness summary localnet_run_id must identify a production localnet run",
                        section=section_name,
                        field="localnet_run_id",
                    )
                )
            if not readiness._localnet_chain_id_is_valid(section.get("chain_id")):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_localnet_identity",
                        "Kagemusha readiness summary chain_id must identify a production localnet chain",
                        section=section_name,
                        field="chain_id",
                    )
                )
            if section.get("target") != readiness.EXPECTED_LOCALNET_TARGET:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_localnet_value",
                        "Kagemusha readiness summary localnet target must match the required value",
                        section=section_name,
                        field="target",
                    )
                )
            if section.get("peer_count") != readiness.EXPECTED_LOCALNET_PEER_COUNT:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_localnet_value",
                        "Kagemusha readiness summary localnet peer_count must match the required value",
                        section=section_name,
                        field="peer_count",
                    )
                )
            if section.get("artifact_count") != len(
                readiness.LOCALNET_LIFECYCLE_HASH_FIELDS
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_localnet_value",
                        "Kagemusha readiness summary localnet artifact_count must match the required hash inventory",
                        section=section_name,
                        field="artifact_count",
                    )
                )
        boolean_fields_by_section = {
            "compact_key_evidence": ("command_validated",),
        }
        for field in boolean_fields_by_section.get(section_name, ()):
            if not isinstance(section.get(field), bool):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_boolean",
                        "Kagemusha readiness summary section boolean field must be boolean",
                        section=section_name,
                        field=field,
                    )
                )
        object_fields_by_section = {
            "abi6_reserved_lineage": ("limits", "modes"),
            "lineage_proof_evidence": (
                "artifact_sha256",
                "artifact_size_bytes",
                "test_log_sha256",
                "circuit_ids",
            ),
            "compact_key_evidence": (
                "artifact_sha256",
                "artifact_size_bytes",
                "generator_log_artifact_sha256",
                "generator_log_artifact_size_bytes",
            ),
            "localnet_lifecycle_evidence": ("artifact_sha256",),
        }
        for field in object_fields_by_section.get(section_name, ()):
            if not isinstance(section.get(field), dict):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_object",
                        "Kagemusha readiness summary section object field must be a JSON object",
                        section=section_name,
                        field=field,
                    )
                )
        integer_fields_by_section = {
            "abi7_recursive_compact": (
                "native_bridge_abi_version",
                "operation_count",
            ),
        }
        for field in integer_fields_by_section.get(section_name, ()):
            value = section.get(field)
            if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_integer",
                        "Kagemusha readiness summary section integer field must be positive",
                        section=section_name,
                        field=field,
                    )
                )
        for field in (
            "fixture_manifest_sha256",
            "archive_fixture_sha256",
        ):
            if section_name != "abi7_recursive_compact" or field not in section:
                continue
            value = section.get(field)
            if (
                not isinstance(value, str)
                or not device_lab.SHA256_HEX_RE.fullmatch(value)
                or value == "0" * 64
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_sha256",
                        "Kagemusha readiness summary ABI-7 fixture digest must be a non-zero lowercase SHA-256 digest",
                        section=section_name,
                        field=field,
                    )
                )
        list_fields_by_section = {
            "lineage_key_release_tooling": ("checked_files",),
            "lineage_proof_evidence": ("tests",),
            "localnet_lifecycle_evidence": ("peer_ids",),
        }
        for field in list_fields_by_section.get(section_name, ()):
            value = section.get(field)
            if (
                not isinstance(value, list)
                or not value
                or any(not isinstance(item, str) or not item for item in value)
                or (
                    section_name == "localnet_lifecycle_evidence"
                    and (
                        len(value) != readiness.EXPECTED_LOCALNET_PEER_COUNT
                        or any(
                            not readiness._localnet_peer_id_is_valid(peer_id)
                            for peer_id in value
                        )
                        or len(set(value)) != readiness.EXPECTED_LOCALNET_PEER_COUNT
                        or value != sorted(value)
                    )
                )
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_list",
                        "Kagemusha readiness summary section list field must contain canonical non-empty strings",
                        section=section_name,
                        field=field,
                    )
                )
        sha256_map_fields_by_section = {
            "lineage_proof_evidence": ("artifact_sha256", "test_log_sha256"),
            "compact_key_evidence": (
                "artifact_sha256",
                "generator_log_artifact_sha256",
            ),
            "localnet_lifecycle_evidence": ("artifact_sha256",),
        }
        for field in sha256_map_fields_by_section.get(section_name, ()):
            value = section.get(field)
            expected_keys = _expected_release_bundle_section_map_keys(
                section_name,
                field,
            )
            if (
                isinstance(value, dict)
                and expected_keys is not None
                and set(value) != expected_keys
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_inventory",
                        "Kagemusha readiness summary section map must exactly match the required inventory",
                        section=section_name,
                        field=field,
                    )
                )
            if (
                not isinstance(value, dict)
                or not value
                or any(
                    not isinstance(key, str)
                    or not key
                    or not isinstance(digest, str)
                    or not device_lab.SHA256_HEX_RE.fullmatch(digest)
                    or digest == "0" * 64
                    or (
                        section_name == "localnet_lifecycle_evidence"
                        and len(set(digest)) == 1
                    )
                    for key, digest in value.items()
                )
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_sha256",
                        "Kagemusha readiness summary section SHA-256 map must contain non-placeholder lowercase hex digests",
                        section=section_name,
                        field=field,
                    )
                )
            if (
                section_name == "localnet_lifecycle_evidence"
                and isinstance(value, dict)
                and len(set(value.values())) != len(value)
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_sha256_distinct",
                        "Kagemusha readiness summary localnet artifact hashes must be distinct",
                        section=section_name,
                        field=field,
                    )
                )
        if section_name == "compact_key_evidence":
            generator_log_sha256 = section.get("generator_log_sha256")
            if (
                not isinstance(generator_log_sha256, str)
                or not device_lab.SHA256_HEX_RE.fullmatch(generator_log_sha256)
                or generator_log_sha256 == "0" * 64
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_sha256",
                        "Kagemusha readiness summary section SHA-256 field must be a non-zero lowercase hex digest",
                        section=section_name,
                        field="generator_log_sha256",
                    )
                )
        size_map_fields_by_section = {
            "lineage_proof_evidence": ("artifact_size_bytes",),
            "compact_key_evidence": (
                "artifact_size_bytes",
                "generator_log_artifact_size_bytes",
            ),
        }
        for field in size_map_fields_by_section.get(section_name, ()):
            value = section.get(field)
            expected_keys = _expected_release_bundle_section_map_keys(
                section_name,
                field,
            )
            if (
                isinstance(value, dict)
                and expected_keys is not None
                and set(value) != expected_keys
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_inventory",
                        "Kagemusha readiness summary section map must exactly match the required inventory",
                        section=section_name,
                        field=field,
                    )
                )
            if (
                not isinstance(value, dict)
                or not value
                or any(
                    not isinstance(key, str)
                    or not key
                    or isinstance(size, bool)
                    or not isinstance(size, int)
                    or size <= 0
                    for key, size in value.items()
                )
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_size",
                        "Kagemusha readiness summary section size map must contain positive integer sizes",
                        section=section_name,
                        field=field,
                    )
                )
        integer_map_fields_by_section = {
            "abi6_reserved_lineage": ("limits",),
        }
        for field in integer_map_fields_by_section.get(section_name, ()):
            value = section.get(field)
            expected_keys = _expected_release_bundle_section_map_keys(
                section_name,
                field,
            )
            if (
                isinstance(value, dict)
                and expected_keys is not None
                and set(value) != expected_keys
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_inventory",
                        "Kagemusha readiness summary section map must exactly match the required inventory",
                        section=section_name,
                        field=field,
                    )
                )
            if (
                not isinstance(value, dict)
                or not value
                or any(
                    not isinstance(key, str)
                    or not key
                    or isinstance(item, bool)
                    or not isinstance(item, int)
                    or item <= 0
                    for key, item in value.items()
                )
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_integer_map",
                        "Kagemusha readiness summary section integer map must contain positive integer values",
                        section=section_name,
                        field=field,
                    )
                )
        string_map_fields_by_section = {
            "abi6_reserved_lineage": ("modes",),
            "lineage_proof_evidence": ("circuit_ids",),
        }
        for field in string_map_fields_by_section.get(section_name, ()):
            value = section.get(field)
            expected_keys = _expected_release_bundle_section_map_keys(
                section_name,
                field,
            )
            if (
                isinstance(value, dict)
                and expected_keys is not None
                and set(value) != expected_keys
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_inventory",
                        "Kagemusha readiness summary section map must exactly match the required inventory",
                        section=section_name,
                        field=field,
                    )
                )
            if (
                not isinstance(value, dict)
                or not value
                or any(
                    not isinstance(key, str)
                    or not key
                    or not isinstance(item, str)
                    or not item
                    for key, item in value.items()
                )
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_string_map",
                        "Kagemusha readiness summary section string map must contain non-empty strings",
                        section=section_name,
                        field=field,
                    )
                )
        integer_fields_by_section = {
            "abi6_reserved_lineage": (
                "native_bridge_abi_version",
                "operation_count",
            ),
            "lineage_proof_evidence": (
                "opening_len",
                "ipa_k",
                "artifact_count",
            ),
            "compact_key_evidence": (
                "opening_len",
                "ipa_k",
                "record_version",
                "artifact_count",
            ),
            "localnet_lifecycle_evidence": (
                "peer_count",
                "artifact_count",
            ),
        }
        for field in integer_fields_by_section.get(section_name, ()):
            value = section.get(field)
            if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_integer",
                        "Kagemusha readiness summary section integer field must be a positive integer",
                        section=section_name,
                        field=field,
                    )
                )
        section_blockers = section.get("blockers")
        if not isinstance(section_blockers, list):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_section_blockers_shape",
                    "Kagemusha readiness summary section blockers must be a JSON array",
                    section=section_name,
                )
            )
        elif section_blockers:
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_section_blockers_present",
                    "Kagemusha readiness summary section must not contain blockers",
                    section=section_name,
                )
            )
    timestamp_fields_by_section = {
        "lineage_proof_evidence": (
            ("min_generated_at_utc", False, False),
            ("max_generated_at_utc", True, False),
            ("generated_at_utc", True, True),
        ),
        "compact_key_evidence": (
            ("min_generated_at_utc", False, False),
            ("max_generated_at_utc", True, False),
            ("generated_at_utc", True, True),
        ),
        "localnet_lifecycle_evidence": (
            ("min_generated_at_utc", False, False),
            ("max_generated_at_utc", True, False),
            ("generated_at_utc", True, True),
        ),
        "android_device_lab": (
            ("min_signed_at_utc", False, False),
            ("max_signed_at_utc", True, False),
        ),
    }
    for section_name, timestamp_fields in timestamp_fields_by_section.items():
        section = _section(summary, section_name)
        if section is None:
            continue
        for field, reject_future, required in timestamp_fields:
            timestamp = section.get(field)
            if timestamp is None and not required:
                continue
            if (
                not isinstance(timestamp, str)
                or not device_lab.SIGNED_AT_UTC_RE.fullmatch(timestamp)
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_timestamp",
                        "Kagemusha readiness summary section timestamp must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                        section=section_name,
                        field=field,
                    )
                )
                continue
            parsed_timestamp, parse_blocker = readiness.parse_utc_timestamp(
                timestamp,
                f"Kagemusha readiness summary {section_name} {field}",
            )
            if parse_blocker is not None:
                parse_blocker["code"] = "kagemusha_release_summary_section_timestamp"
                parse_blocker["section"] = section_name
                parse_blocker["field"] = field
                blockers.append(parse_blocker)
                continue
            if not reject_future or parsed_timestamp is None:
                continue
            max_timestamp = dt.datetime.now(dt.timezone.utc).replace(
                microsecond=0,
            ) + dt.timedelta(
                seconds=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
            )
            if parsed_timestamp > max_timestamp:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_section_future_dated",
                        "Kagemusha readiness summary section timestamp must not be future-dated beyond the allowed clock skew",
                        section=section_name,
                        field=field,
                        max_timestamp_utc=max_timestamp.isoformat().replace(
                            "+00:00",
                            "Z",
                        ),
                    )
                )
    summary_schema = summary.get("schema")
    if not isinstance(summary_schema, str):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_schema_shape",
                "Kagemusha readiness summary schema must be a string",
            )
        )
    if summary_schema != readiness.SUMMARY_SCHEMA:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_schema",
                "Kagemusha readiness summary schema mismatch",
            )
        )
    generated_at = summary.get("generated_at")
    if not isinstance(generated_at, str):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_timestamp",
                "Kagemusha readiness summary generated_at is required",
            )
        )
    elif not device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_timestamp",
                "Kagemusha readiness summary generated_at must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
            )
        )
    else:
        generated_at_timestamp, parse_blocker = readiness.parse_utc_timestamp(
            generated_at,
            "Kagemusha readiness summary generated_at",
        )
        if parse_blocker is not None:
            parse_blocker["code"] = "kagemusha_release_summary_timestamp"
            blockers.append(parse_blocker)
        elif generated_at_timestamp is not None:
            max_generated_at = dt.datetime.now(dt.timezone.utc).replace(
                microsecond=0,
            ) + dt.timedelta(
                seconds=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
            )
            if generated_at_timestamp > max_generated_at:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_future_dated",
                        "Kagemusha readiness summary generated_at must not be future-dated beyond the allowed clock skew",
                        max_generated_at_utc=max_generated_at.isoformat().replace(
                            "+00:00",
                            "Z",
                        ),
                    )
                )
    ready_value = summary.get("ready")
    if not isinstance(ready_value, bool):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_ready_shape",
                "Kagemusha readiness summary ready flag must be boolean",
            )
        )
    status_value = summary.get("status")
    if not isinstance(status_value, str):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_status_shape",
                "Kagemusha readiness summary status must be a string",
            )
        )
    if ready_value is not True or status_value != "ready":
        blockers.append(
            _blocker(
                "kagemusha_release_summary_not_ready",
                "Kagemusha readiness summary must be ready",
            )
        )
    summary_blockers = summary.get("blockers")
    if not isinstance(summary_blockers, list):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_blockers_shape",
                "Kagemusha readiness summary blockers must be a JSON array",
            )
        )
    elif summary_blockers:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_blockers_present",
                "Kagemusha readiness summary must not contain blockers",
            )
        )
    abi6 = _section(summary, "abi6_reserved_lineage")
    if abi6 is None or abi6.get("ok") is not True:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_abi6_not_ready",
                "ABI-6 Reserved-lineage summary section must be ready",
            )
        )
    android = _section(summary, "android_device_lab")
    if android is None or android.get("ok") is not True:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_android_not_ready",
                "Android device-lab summary section must be ready",
            )
        )
    else:
        blockers.extend(_check_android_ready_summary_shape(android))
        if android.get("missing_device_families") != []:
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_matrix_incomplete",
                    "Android device-lab summary must cover the full standard matrix",
                )
            )
        blockers.extend(_check_android_duplicate_bindings_summary_shape(android))
        blockers.extend(_check_android_signed_evidence_summary_shape(android))
    for name, state in SUMMARY_REQUIRED_SECTION_STATES.items():
        section = _section(summary, name)
        if section is None or section.get("ok") is not True or section.get("state") != state:
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_section_not_ready",
                    "Kagemusha readiness summary section is not ready",
                    section=name,
                    expected_state=state,
                )
            )
    return blockers


def _compare_field(
    summary_section: dict[str, Any],
    recomputed_section: dict[str, Any],
    section: str,
    field: str,
) -> list[dict[str, Any]]:
    if summary_section.get(field) == recomputed_section.get(field):
        return []
    return [
        _blocker(
            "kagemusha_release_summary_drift",
            "Kagemusha readiness summary no longer matches local release evidence",
            section=section,
            field=field,
        )
    ]


def _compare_section_evidence_fields(
    summary_section: dict[str, Any],
    recomputed_section: dict[str, Any],
    section: str,
    fields: tuple[str, ...],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    for field in fields:
        if summary_section.get(field) == recomputed_section.get(field):
            continue
        blockers.append(
            _blocker(
                "kagemusha_release_summary_section_evidence_drift",
                "Kagemusha readiness summary section evidence no longer "
                "matches local release evidence",
                section=section,
                field=field,
            )
        )
    return blockers


def _compare_section_value_fields(
    summary_section: dict[str, Any],
    recomputed_section: dict[str, Any],
    section: str,
    fields: tuple[str, ...],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    for field in fields:
        if summary_section.get(field) == recomputed_section.get(field):
            continue
        blockers.append(
            _blocker(
                "kagemusha_release_summary_section_value_drift",
                "Kagemusha readiness summary section value no longer "
                "matches local release evidence",
                section=section,
                field=field,
            )
        )
    return blockers


def _compare_android_signed_evidence_summary(
    summary_section: dict[str, Any],
    recomputed_section: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    summary_signed = summary_section.get("signed_evidence")
    recomputed_signed = recomputed_section.get("signed_evidence")
    if not isinstance(summary_signed, dict) or not isinstance(
        recomputed_signed, dict
    ):
        return blockers
    if set(summary_signed) != set(recomputed_signed):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_android_signed_evidence_inventory_drift",
                "Android signed-evidence summary slot inventory no longer "
                "matches validated device-lab evidence",
            )
        )
    for raw_slot, summary_entry in summary_signed.items():
        recomputed_entry = recomputed_signed.get(raw_slot)
        if not isinstance(summary_entry, dict) or not isinstance(
            recomputed_entry, dict
        ):
            continue
        for field in sorted(ANDROID_SIGNED_EVIDENCE_SUMMARY_REQUIRED_FIELDS):
            if summary_entry.get(field) == recomputed_entry.get(field):
                continue
            if field in ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS:
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_signed_evidence_identity_drift",
                        "Android signed-evidence identity summary no longer matches validated device-lab evidence",
                        slot=_display_summary_field(raw_slot),
                        field=field,
                    )
                )
                continue
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_signed_evidence_drift",
                    "Android signed-evidence summary no longer matches "
                    "validated device-lab evidence",
                    slot=_display_summary_field(raw_slot),
                    field=field,
                )
            )
    return blockers


def _without_android_identity_fields(value: Any) -> Any:
    if not isinstance(value, dict):
        return value
    return {
        key: item
        for key, item in value.items()
        if key not in ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS
    }


def _signed_evidence_without_android_identity_fields(value: Any) -> Any:
    if not isinstance(value, dict):
        return value
    return {
        slot: _without_android_identity_fields(entry)
        for slot, entry in value.items()
    }


def _compare_android_summary_binding(
    summary_section: dict[str, Any],
    recomputed_section: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    fields = {
        "covered_device_families": (
            "kagemusha_release_summary_android_device_families_drift",
            "Android covered device-family summary no longer matches validated device-lab evidence",
        ),
        "covered_d2d_payment_transports": (
            "kagemusha_release_summary_android_d2d_transports_drift",
            "Android D2D payment transport summary no longer matches validated device-lab evidence",
        ),
        "missing_d2d_payment_transports": (
            "kagemusha_release_summary_android_d2d_transports_drift",
            "Android D2D payment transport summary no longer matches validated device-lab evidence",
        ),
        "duplicate_bindings": (
            "kagemusha_release_summary_android_duplicate_bindings_drift",
            "Android duplicate-bindings summary no longer matches validated device-lab evidence",
        ),
        "trusted_signer_public_key_sha256": (
            "kagemusha_release_summary_android_trusted_signer_drift",
            "Android trusted signer digest summary no longer matches validated device-lab evidence",
        ),
        "min_signed_at_utc": (
            "kagemusha_release_summary_android_signed_bounds_drift",
            "Android signed-evidence minimum timestamp bound no longer matches validated device-lab evidence",
        ),
    }
    for field, (code, message) in fields.items():
        if summary_section.get(field) == recomputed_section.get(field):
            continue
        blockers.append(_blocker(code, message, field=field))
    return blockers


def _android_slots_by_name(slots: Any) -> dict[str, dict[str, Any]] | None:
    if not isinstance(slots, list):
        return None
    by_name: dict[str, dict[str, Any]] = {}
    for entry in slots:
        if not isinstance(entry, dict):
            continue
        slot = entry.get("slot")
        if isinstance(slot, str):
            by_name[slot] = entry
    return by_name


def _compare_android_slots_summary(
    summary_section: dict[str, Any],
    recomputed_section: dict[str, Any],
) -> list[dict[str, Any]]:
    """Bind release-facing Android slot metadata to freshly scanned evidence."""

    summary_slots = _android_slots_by_name(summary_section.get("slots"))
    recomputed_slots = _android_slots_by_name(recomputed_section.get("slots"))
    if summary_slots is None or recomputed_slots is None:
        return []
    blockers: list[dict[str, Any]] = []
    if set(summary_slots) != set(recomputed_slots):
        blockers.append(
            _blocker(
                "kagemusha_release_summary_android_slots_drift",
                "Android readiness summary slot inventory no longer matches validated device-lab evidence",
                field="inventory",
            )
        )
    for slot in sorted(set(summary_slots) & set(recomputed_slots)):
        summary_entry = summary_slots[slot]
        recomputed_entry = recomputed_slots[slot]
        for field in ("status", "errors", "present", "file_counts"):
            if summary_entry.get(field) == recomputed_entry.get(field):
                continue
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_android_slots_drift",
                    "Android readiness summary slot metadata no longer matches validated device-lab evidence",
                    slot=_display_summary_field(slot),
                    field=field,
                )
            )
        summary_kagemusha = summary_entry.get("kagemusha")
        recomputed_kagemusha = recomputed_entry.get("kagemusha")
        if isinstance(summary_kagemusha, dict) and isinstance(
            recomputed_kagemusha,
            dict,
        ):
            for field in sorted(ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS):
                if summary_kagemusha.get(field) == recomputed_kagemusha.get(field):
                    continue
                blockers.append(
                    _blocker(
                        "kagemusha_release_summary_android_slots_identity_drift",
                        "Android readiness summary slot identity no longer matches validated device-lab evidence",
                        slot=_display_summary_field(slot),
                        field=field,
                    )
                )
            if _without_android_identity_fields(
                summary_kagemusha
            ) == _without_android_identity_fields(recomputed_kagemusha):
                continue
        if summary_kagemusha == recomputed_kagemusha:
            continue
        blockers.append(
            _blocker(
                "kagemusha_release_summary_android_slots_drift",
                "Android readiness summary slot metadata no longer matches validated device-lab evidence",
                slot=_display_summary_field(slot),
                field="kagemusha",
            )
        )
    return blockers


def _compare_validated_sections(
    summary: dict[str, Any],
    abi6: dict[str, Any],
    abi7: dict[str, Any],
    lineage_tooling: dict[str, Any],
    lineage: dict[str, Any],
    compact: dict[str, Any],
    localnet_lifecycle: dict[str, Any],
    android: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    abi6_summary = _section(summary, "abi6_reserved_lineage") or {}
    abi7_summary = _section(summary, "abi7_recursive_compact") or {}
    lineage_tooling_summary = _section(summary, "lineage_key_release_tooling") or {}
    lineage_summary = _section(summary, "lineage_proof_evidence") or {}
    compact_summary = _section(summary, "compact_key_evidence") or {}
    localnet_lifecycle_summary = (
        _section(summary, "localnet_lifecycle_evidence") or {}
    )
    android_summary = _section(summary, "android_device_lab") or {}
    for field in (
        "manifest_path",
        "schema",
        "native_bridge_abi_version",
        "operation_count",
        "limits",
        "modes",
    ):
        blockers.extend(
            _compare_section_value_fields(
                abi6_summary,
                abi6,
                "abi6_reserved_lineage",
                (field,),
            )
        )
    blockers.extend(
        _compare_section_value_fields(
            abi7_summary,
            abi7,
            "abi7_recursive_compact",
            (
                "state",
                "circuit_id",
                "fixture_manifest_path",
                "fixture_manifest_schema",
                "fixture_manifest_sha256",
                "archive_fixture_path",
                "archive_fixture_schema",
                "archive_fixture_sha256",
                "native_bridge_abi_version",
                "operation_count",
            ),
        )
    )
    blockers.extend(
        _compare_section_value_fields(
            lineage_tooling_summary,
            lineage_tooling,
            "lineage_key_release_tooling",
            ("state", "checked_files"),
        )
    )
    blockers.extend(
        _compare_section_value_fields(
            lineage_summary,
            lineage,
            "lineage_proof_evidence",
            (
                "state",
                "schema",
                "min_generated_at_utc",
                "generated_at_utc",
                "opening_len",
                "ipa_k",
                "record_archive_proof_runtime_keygen_env",
                "circuit_ids",
                "artifact_count",
                "tests",
            ),
        )
    )
    blockers.extend(
        _compare_section_evidence_fields(
            lineage_summary,
            lineage,
            "lineage_proof_evidence",
            ("artifact_sha256", "artifact_size_bytes", "test_log_sha256"),
        )
    )
    blockers.extend(
        _compare_section_value_fields(
            compact_summary,
            compact,
            "compact_key_evidence",
            (
                "state",
                "schema",
                "min_generated_at_utc",
                "generated_at_utc",
                "opening_len",
                "ipa_k",
                "verifier_backend",
                "circuit_id",
                "record_namespace",
                "record_version",
                "command_validated",
                "artifact_count",
            ),
        )
    )
    blockers.extend(
        _compare_section_evidence_fields(
            compact_summary,
            compact,
            "compact_key_evidence",
            (
                "artifact_sha256",
                "artifact_size_bytes",
                "generator_log_sha256",
                "generator_log_artifact_sha256",
                "generator_log_artifact_size_bytes",
            ),
        )
    )
    blockers.extend(
        _compare_section_value_fields(
            localnet_lifecycle_summary,
            localnet_lifecycle,
            "localnet_lifecycle_evidence",
            (
                "state",
                "schema",
                "min_generated_at_utc",
                "generated_at_utc",
                "localnet_run_id",
                "chain_id",
                "target",
                "peer_count",
                "peer_ids",
                "artifact_count",
            ),
        )
    )
    blockers.extend(
        _compare_section_evidence_fields(
            localnet_lifecycle_summary,
            localnet_lifecycle,
            "localnet_lifecycle_evidence",
            ("artifact_sha256",),
        )
    )
    for field in (
        "missing_device_families",
        "missing_d2d_payment_transports",
    ):
        blockers.extend(
            _compare_field(android_summary, android, "android_device_lab", field)
        )
    blockers.extend(_compare_android_summary_binding(android_summary, android))
    blockers.extend(_compare_android_slots_summary(android_summary, android))
    android_signed_evidence_blockers = _compare_android_signed_evidence_summary(
        android_summary,
        android,
    )
    blockers.extend(android_signed_evidence_blockers)
    if not android_signed_evidence_blockers:
        blockers.extend(
            _compare_field(
                android_summary,
                android,
                "android_device_lab",
                "signed_evidence",
            )
        )
    return blockers


def _evidence_entry(
    path: Path,
    bundle_root: Path,
    *,
    label: str,
    code: str,
) -> tuple[dict[str, str] | None, list[dict[str, Any]]]:
    relative, relative_blockers = _relative_to_bundle(path, bundle_root, label)
    if relative_blockers:
        return None, relative_blockers
    assert relative is not None
    digest, digest_blockers = _sha256_file(path, label, code)
    if digest_blockers:
        return None, digest_blockers
    assert digest is not None
    return {"path": relative, "sha256": digest}, []


def _evidence_entry_with_size(
    path: Path,
    bundle_root: Path,
    *,
    label: str,
    code: str,
) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    relative, relative_blockers = _relative_to_bundle(path, bundle_root, label)
    if relative_blockers:
        return None, relative_blockers
    assert relative is not None
    digest, size, digest_blockers = _sha256_file_with_size(path, label, code)
    if digest_blockers:
        return None, digest_blockers
    assert digest is not None and size is not None
    return {"path": relative, "sha256": digest, "size_bytes": size}, []


def _artifact_inventory_entries(
    artifact_root: Path,
    bundle_root: Path,
    *,
    artifact_names: tuple[str, ...],
    artifact_sha256: Any,
    artifact_size_bytes: Any,
    label_prefix: str,
    code_prefix: str,
    artifact_content_validator: Callable[[Path, str], list[str]] | None = None,
) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]]]:
    entries: dict[str, dict[str, Any]] = {}
    blockers: list[dict[str, Any]] = []
    if not isinstance(artifact_sha256, dict):
        return entries, [
            _blocker(
                f"{code_prefix}_summary",
                f"{label_prefix} artifact SHA-256 summary must be a JSON object",
            )
        ]
    if not isinstance(artifact_size_bytes, dict):
        return entries, [
            _blocker(
                f"{code_prefix}_summary",
                f"{label_prefix} artifact size summary must be a JSON object",
            )
        ]
    for artifact in artifact_names:
        artifact_path = artifact_root / artifact
        entry, entry_blockers = _evidence_entry_with_size(
            artifact_path,
            bundle_root,
            label=f"{label_prefix} artifact",
            code=f"{code_prefix}_file_shape",
        )
        blockers.extend(entry_blockers)
        if entry is None:
            continue
        if artifact_content_validator is not None:
            content_errors = artifact_content_validator(artifact_path, artifact)
            if content_errors:
                for error in content_errors:
                    blockers.append(
                        _blocker(
                            f"{code_prefix}_placeholder",
                            error,
                            artifact=artifact,
                        )
                    )
                continue
        if artifact_sha256.get(artifact) != entry["sha256"]:
            blockers.append(
                _blocker(
                    f"{code_prefix}_digest_drift",
                    f"{label_prefix} artifact digest no longer matches validated readiness summary",
                    artifact=artifact,
                )
            )
            continue
        if artifact_size_bytes.get(artifact) != entry["size_bytes"]:
            blockers.append(
                _blocker(
                    f"{code_prefix}_size_drift",
                    f"{label_prefix} artifact size no longer matches validated readiness summary",
                    artifact=artifact,
                )
            )
            continue
        entries[artifact] = entry
    if set(entries) != set(artifact_names):
        blockers.append(
            _blocker(
                f"{code_prefix}_inventory",
                f"{label_prefix} artifact inventory must include every required artifact",
            )
        )
    return entries, blockers


def _lineage_proof_log_entries(
    artifact_root: Path,
    bundle_root: Path,
    lineage: dict[str, Any],
) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]]]:
    entries: dict[str, dict[str, Any]] = {}
    blockers: list[dict[str, Any]] = []
    test_log_sha256 = lineage.get("test_log_sha256", {})
    if not isinstance(test_log_sha256, dict):
        return entries, [
            _blocker(
                "kagemusha_release_lineage_proof_log_summary",
                "Reserved-lineage proof-log summary must be a JSON object",
            )
        ]
    for key, relative_log_path in readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS.items():
        entry, entry_blockers = _evidence_entry_with_size(
            artifact_root / relative_log_path,
            bundle_root,
            label="Reserved-lineage proof log",
            code="kagemusha_release_lineage_proof_log_file_shape",
        )
        blockers.extend(entry_blockers)
        if entry is None:
            continue
        if test_log_sha256.get(key) != entry["sha256"]:
            blockers.append(
                _blocker(
                    "kagemusha_release_lineage_proof_log_digest_drift",
                    "Reserved-lineage proof-log digest no longer matches validated readiness summary",
                    test=key,
                )
            )
            continue
        entries[key] = entry
    if set(entries) != set(readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS):
        blockers.append(
            _blocker(
                "kagemusha_release_lineage_proof_log_inventory",
                "Reserved-lineage proof-log inventory must include every required proof log",
            )
        )
    return entries, blockers


def _validate_android_manifest_slot(slot: Any) -> tuple[str | None, list[dict[str, Any]]]:
    if not isinstance(slot, str):
        return None, [
            _blocker(
                "kagemusha_release_android_signed_evidence_slot",
                "validated Android signed-evidence report is missing a safe slot name",
            )
        ]
    validated, errors = device_lab.validate_slot_ids([slot])
    if errors or validated != [slot]:
        return None, [
            _blocker(
                "kagemusha_release_android_signed_evidence_slot",
                "validated Android signed-evidence report has an unsafe slot name",
            )
        ]
    return slot, []


def _android_signed_evidence_entries(
    device_lab_root: Path,
    bundle_root: Path,
    android: dict[str, Any],
) -> tuple[dict[str, dict[str, Any]], list[dict[str, Any]]]:
    entries: dict[str, dict[str, Any]] = {}
    blockers: list[dict[str, Any]] = []
    signed_evidence_summary = android.get("signed_evidence", {})
    if not isinstance(signed_evidence_summary, dict):
        return entries, [
            _blocker(
                "kagemusha_release_android_signed_evidence_summary",
                "Android signed-evidence summary must be a JSON object",
            )
        ]
    for report in android.get("slots", []):
        if not isinstance(report, dict) or report.get("status") != "ok":
            continue
        slot = report.get("slot")
        kagemusha = report.get("kagemusha", {})
        slot, slot_blockers = _validate_android_manifest_slot(slot)
        blockers.extend(slot_blockers)
        if slot is None:
            continue
        if not isinstance(kagemusha, dict):
            blockers.append(
                _blocker(
                    "kagemusha_release_android_signed_evidence_slot",
                    "validated Android signed-evidence report is missing Kagemusha details",
                )
            )
            continue
        artifact_path = (
            device_lab_root
            / slot
            / device_lab.KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH
        )
        entry, entry_blockers = _evidence_entry_with_size(
            artifact_path,
            bundle_root,
            label="Android signed evidence artifact",
            code="kagemusha_release_android_signed_evidence_file_shape",
        )
        blockers.extend(entry_blockers)
        if entry is None:
            continue
        expected_digest = kagemusha.get("signed_evidence_artifact_sha256")
        if expected_digest != entry["sha256"]:
            blockers.append(
                _blocker(
                    "kagemusha_release_android_signed_evidence_digest_drift",
                    "Android signed-evidence artifact digest no longer matches validated device-lab report",
                    slot=slot,
                )
            )
            continue
        summary_entry = signed_evidence_summary.get(slot)
        if not isinstance(summary_entry, dict) or summary_entry.get("artifact_sha256") != entry["sha256"]:
            blockers.append(
                _blocker(
                    "kagemusha_release_android_signed_evidence_summary_drift",
                    "Android signed-evidence manifest entry no longer matches readiness summary",
                    slot=slot,
                )
            )
            continue
        entries[slot] = entry
    if set(entries) != set(signed_evidence_summary):
        blockers.append(
            _blocker(
                "kagemusha_release_android_signed_evidence_inventory",
                "Android signed-evidence manifest inventory must match validated readiness summary slots",
            )
        )
    return entries, blockers


def _android_slot_artifact_entries(
    device_lab_root: Path,
    bundle_root: Path,
    android: dict[str, Any],
) -> tuple[dict[str, dict[str, dict[str, Any]]], list[dict[str, Any]]]:
    entries: dict[str, dict[str, dict[str, Any]]] = {}
    blockers: list[dict[str, Any]] = []
    signed_evidence_summary = android.get("signed_evidence", {})
    if not isinstance(signed_evidence_summary, dict):
        return entries, [
            _blocker(
                "kagemusha_release_android_slot_artifact_summary",
                "Android signed-evidence summary must be a JSON object",
            )
        ]

    for report in android.get("slots", []):
        if not isinstance(report, dict) or report.get("status") != "ok":
            continue
        slot, slot_blockers = _validate_android_manifest_slot(report.get("slot"))
        blockers.extend(slot_blockers)
        if slot is None:
            continue
        kagemusha = report.get("kagemusha", {})
        summary_entry = signed_evidence_summary.get(slot)
        if not isinstance(kagemusha, dict) or not isinstance(summary_entry, dict):
            blockers.append(
                _blocker(
                    "kagemusha_release_android_slot_artifact_summary",
                    "validated Android slot artifacts are missing from the readiness summary",
                    slot=slot,
                )
            )
            continue

        slot_entries: dict[str, dict[str, Any]] = {}
        for artifact_kind, path_field, digest_field in ANDROID_SLOT_RELEASE_ARTIFACTS:
            expected_path = summary_entry.get(path_field)
            expected_digest = summary_entry.get(digest_field)
            if (
                not isinstance(expected_path, str)
                or not expected_path
                or not isinstance(expected_digest, str)
                or not expected_digest
                or kagemusha.get(path_field) != expected_path
                or kagemusha.get(digest_field) != expected_digest
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_android_slot_artifact_summary_drift",
                        "Android slot artifact summary no longer matches validated device-lab report",
                        slot=slot,
                        artifact=artifact_kind,
                    )
                )
                continue

            path_errors: list[str] = []
            safe_relative = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                expected_path,
                path_errors,
                f"Android slot artifact {artifact_kind}",
            )
            if safe_relative is None:
                blockers.extend(
                    _blocker(
                        "kagemusha_release_android_slot_artifact_path",
                        error,
                        slot=slot,
                        artifact=artifact_kind,
                    )
                    for error in path_errors
                )
                continue

            entry, entry_blockers = _evidence_entry_with_size(
                device_lab_root / slot / safe_relative,
                bundle_root,
                label=f"Android slot artifact {artifact_kind}",
                code="kagemusha_release_android_slot_artifact_file_shape",
            )
            blockers.extend(entry_blockers)
            if entry is None:
                continue
            if expected_digest != entry["sha256"]:
                blockers.append(
                    _blocker(
                        "kagemusha_release_android_slot_artifact_digest_drift",
                        "Android slot artifact digest no longer matches validated readiness summary",
                        slot=slot,
                        artifact=artifact_kind,
                    )
                )
                continue
            slot_entries[artifact_kind] = entry

        d2d_transcripts = kagemusha.get("d2d_payment_transcripts")
        if isinstance(d2d_transcripts, dict):
            primary_path = kagemusha.get("d2d_payment_transcript_path")
            for transport, binding in sorted(d2d_transcripts.items()):
                if transport not in device_lab.D2D_PAYMENT_TRANSPORTS:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_android_slot_artifact_summary_drift",
                            "Android D2D transcript summary names an unsupported transport",
                            slot=slot,
                            artifact=_display_summary_field(transport),
                        )
                    )
                    continue
                if (
                    not isinstance(binding, dict)
                    or not isinstance(binding.get("path"), str)
                    or not isinstance(binding.get("sha256"), str)
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_android_slot_artifact_summary_drift",
                            "Android D2D transcript summary binding must contain path and sha256",
                            slot=slot,
                            artifact=transport,
                        )
                    )
                    continue
                if binding["path"] == primary_path:
                    continue
                path_errors: list[str] = []
                safe_relative = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                    binding["path"],
                    path_errors,
                    f"Android slot D2D transcript {transport}",
                )
                if (
                    safe_relative is None
                    or not device_lab._safe_relative_path_is_child_of(  # type: ignore[attr-defined]
                        safe_relative,
                        "handoff",
                    )
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_android_slot_artifact_path",
                            "Android D2D transcript artifact path must stay under handoff/",
                            slot=slot,
                            artifact=transport,
                        )
                    )
                    continue
                entry, entry_blockers = _evidence_entry_with_size(
                    device_lab_root / slot / safe_relative,
                    bundle_root,
                    label=f"Android slot D2D transcript {transport}",
                    code="kagemusha_release_android_slot_artifact_file_shape",
                )
                blockers.extend(entry_blockers)
                if entry is None:
                    continue
                if binding["sha256"] != entry["sha256"]:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_android_slot_artifact_digest_drift",
                            "Android D2D transcript artifact digest no longer matches validated readiness summary",
                            slot=slot,
                            artifact=transport,
                        )
                    )
                    continue
                slot_entries[_android_d2d_transcript_artifact_kind(transport)] = entry

        if not {item[0] for item in ANDROID_SLOT_RELEASE_ARTIFACTS}.issubset(
            set(slot_entries)
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_android_slot_artifact_inventory",
                    "Android slot artifact inventory must include every release-critical artifact",
                    slot=slot,
                )
            )
            continue
        entries[slot] = slot_entries

    if set(entries) != set(signed_evidence_summary):
        blockers.append(
            _blocker(
                "kagemusha_release_android_slot_artifact_inventory",
                "Android slot artifact inventory must match validated readiness summary slots",
            )
        )
    return entries, blockers


def _bundle_evidence_paths(bundle: dict[str, Any]) -> set[str]:
    paths: set[str] = set()

    def visit(value: Any) -> None:
        if isinstance(value, dict):
            path = value.get("path")
            if isinstance(path, str):
                paths.add(path)
            for item in value.values():
                visit(item)
        elif isinstance(value, list):
            for item in value:
                visit(item)

    visit(bundle.get("evidence", {}))
    return paths


def _cleanup_temp_output(
    path: Path,
    expected_identity: tuple[int, int] | None,
) -> list[dict[str, Any]]:
    if expected_identity is None:
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out temporary file metadata could not be read",
            )
        ]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out temporary file could not be removed",
            )
        ]
    try:
        try:
            temp_stat = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            return []
        except OSError:
            return [
                _blocker(
                    "kagemusha_release_bundle_out_invalid",
                    "--out temporary file could not be removed",
                )
            ]
        if (
            not stat.S_ISREG(temp_stat.st_mode)
            or _file_identity(temp_stat) != expected_identity
        ):
            return [
                _blocker(
                    "kagemusha_release_bundle_out_invalid",
                    "--out temporary file changed before cleanup",
                )
            ]
        try:
            os.unlink(path.name, dir_fd=parent_fd)
        except FileNotFoundError:
            return []
        except OSError:
            return [
                _blocker(
                    "kagemusha_release_bundle_out_invalid",
                    "--out temporary file could not be removed",
                )
            ]
    finally:
        os.close(parent_fd)
    return []


def _file_identity(file_stat: os.stat_result) -> tuple[int, int]:
    return file_stat.st_dev, file_stat.st_ino


def _directory_open_flags() -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _sync_output_parent(
    parent: Path,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[dict[str, Any]]:
    try:
        parent_fd = os.open(parent, _directory_open_flags())
    except OSError:
        return [_release_bundle_out_blocker("--out parent directory could not be synced")]
    try:
        return _sync_output_parent_fd(parent_fd, expected_identity=expected_identity)
    finally:
        os.close(parent_fd)


def _sync_output_parent_fd(
    parent_fd: int,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[dict[str, Any]]:
    try:
        parent_stat = os.fstat(parent_fd)
        if not stat.S_ISDIR(parent_stat.st_mode):
            return [_release_bundle_out_blocker("--out parent directory could not be synced")]
        if expected_identity is not None and _file_identity(parent_stat) != expected_identity:
            return [_release_bundle_out_blocker("--out parent directory changed before sync")]
        os.fsync(parent_fd)
    except OSError:
        return [_release_bundle_out_blocker("--out parent directory could not be synced")]
    return []


def _stable_release_bundle(bundle: dict[str, Any]) -> dict[str, Any]:
    return {
        key: value
        for key, value in bundle.items()
        if key != "generated_at_utc"
    }


def _check_release_bundle_evidence_entry_shape(
    entry: Any,
    *,
    group: str,
    item: str | None = None,
    artifact: str | None = None,
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    extra = {"group": group}
    if item is not None:
        extra["item"] = _display_summary_field(item)
    if artifact is not None:
        extra["artifact"] = _display_summary_field(artifact)
    if not isinstance(entry, dict):
        return [
            _blocker(
                "kagemusha_release_bundle_manifest_evidence_entry_shape",
                "Kagemusha release bundle evidence entry must be a JSON object",
                **extra,
            )
        ]
    for field in sorted(set(entry) - RELEASE_BUNDLE_EVIDENCE_ENTRY_FIELDS):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_evidence_unexpected_field",
                "Kagemusha release bundle evidence entry contains an unexpected field",
                field=_display_summary_field(field),
                **extra,
            )
        )
    for field in sorted(RELEASE_BUNDLE_EVIDENCE_ENTRY_FIELDS - set(entry)):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_evidence_missing_field",
                "Kagemusha release bundle evidence entry is missing a required field",
                field=field,
                **extra,
            )
        )
    return blockers


def _check_release_bundle_evidence_inventory_shape(
    value: Any,
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    if not isinstance(value, dict):
        return [
            _blocker(
                "kagemusha_release_bundle_manifest_evidence_shape",
                "Kagemusha release bundle evidence inventory must be a JSON object",
            )
        ]
    for field in sorted(set(value) - RELEASE_BUNDLE_ALLOWED_EVIDENCE_KEYS):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_evidence_unexpected_field",
                "Kagemusha release bundle evidence inventory contains an unexpected field",
                field=_display_summary_field(field),
            )
        )
    for field in sorted(RELEASE_BUNDLE_ALLOWED_EVIDENCE_KEYS - set(value)):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_evidence_missing_field",
                "Kagemusha release bundle evidence inventory is missing a required field",
                field=field,
            )
        )

    for group in sorted(RELEASE_BUNDLE_SINGLE_EVIDENCE_KEYS & set(value)):
        blockers.extend(
            _check_release_bundle_evidence_entry_shape(value.get(group), group=group)
        )
    for group in sorted(RELEASE_BUNDLE_MAP_EVIDENCE_KEYS & set(value)):
        entries = value.get(group)
        expected_items = {
            "lineage_artifacts": set(readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS),
            "lineage_proof_logs": set(readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS),
            "compact_key_artifacts": set(readiness.COMPACT_KEY_REQUIRED_ARTIFACTS),
        }.get(group)
        if not isinstance(entries, dict):
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_evidence_inventory_shape",
                    "Kagemusha release bundle evidence group must be a non-empty JSON object",
                    group=group,
                )
            )
            continue
        if not entries:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_evidence_inventory_shape",
                    "Kagemusha release bundle evidence group must be a non-empty JSON object",
                    group=group,
                )
            )
        if expected_items is not None and set(entries) != expected_items:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_evidence_inventory_keys",
                    "Kagemusha release bundle evidence group does not match the required inventory",
                    group=group,
                )
            )
        if not entries:
            continue
        for item, entry in entries.items():
            if not isinstance(item, str) or not item:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_evidence_inventory_shape",
                        "Kagemusha release bundle evidence item names must be non-empty strings",
                        group=group,
                    )
                )
                continue
            if expected_items is not None and item not in expected_items:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_evidence_inventory_item",
                        "Kagemusha release bundle evidence item is not in the required inventory",
                        group=group,
                        item=_display_summary_field(item),
                    )
                )
                continue
            if group == "android_signed_evidence":
                slot, slot_blockers = _validate_android_manifest_slot(item)
                for blocker in slot_blockers:
                    blockers.append(
                        {
                            **blocker,
                            "code": "kagemusha_release_bundle_manifest_evidence_slot",
                            "group": group,
                        }
                    )
                if slot is None:
                    continue
            blockers.extend(
                _check_release_bundle_evidence_entry_shape(
                    entry,
                    group=group,
                    item=item,
                )
            )

    android_slot_artifacts = value.get("android_slot_artifacts")
    if "android_slot_artifacts" in value:
        if not isinstance(android_slot_artifacts, dict) or not android_slot_artifacts:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_evidence_inventory_shape",
                    "Kagemusha release bundle Android slot artifacts must be a non-empty JSON object",
                    group="android_slot_artifacts",
                )
            )
        else:
            expected_artifacts = {item[0] for item in ANDROID_SLOT_RELEASE_ARTIFACTS}
            for raw_slot, artifacts in android_slot_artifacts.items():
                slot, slot_blockers = _validate_android_manifest_slot(raw_slot)
                for blocker in slot_blockers:
                    blockers.append(
                        {
                            **blocker,
                            "code": "kagemusha_release_bundle_manifest_evidence_slot",
                            "group": "android_slot_artifacts",
                        }
                    )
                if not isinstance(artifacts, dict) or not artifacts:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_evidence_inventory_shape",
                            "Kagemusha release bundle Android slot artifact entry must be a non-empty JSON object",
                            group="android_slot_artifacts",
                            item=_display_summary_field(raw_slot),
                        )
                    )
                    continue
                artifact_inventory = set(artifacts)
                dynamic_d2d_artifacts = {
                    artifact
                    for artifact in artifact_inventory
                    if _android_d2d_transcript_artifact_transport(artifact) is not None
                }
                allowed_artifacts = expected_artifacts | dynamic_d2d_artifacts
                missing_artifacts = expected_artifacts - artifact_inventory
                unexpected_artifacts = artifact_inventory - allowed_artifacts
                if missing_artifacts or unexpected_artifacts:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_evidence_artifact_kind",
                            "Kagemusha release bundle Android slot artifacts must exactly match release-critical artifact kinds",
                            group="android_slot_artifacts",
                            item=_display_summary_field(raw_slot),
                        )
                    )
                for missing_artifact in sorted(missing_artifacts):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_evidence_artifact_kind",
                            "Kagemusha release bundle Android slot artifact is missing from the required inventory",
                            group="android_slot_artifacts",
                            item=_display_summary_field(raw_slot),
                            artifact=missing_artifact,
                        )
                    )
                for unexpected_artifact in sorted(
                    unexpected_artifacts,
                    key=_display_summary_field,
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_evidence_artifact_kind",
                            "Kagemusha release bundle Android slot artifact is not in the required inventory",
                            group="android_slot_artifacts",
                            item=_display_summary_field(raw_slot),
                            artifact=_display_summary_field(unexpected_artifact),
                        )
                    )
                for raw_artifact, entry in artifacts.items():
                    artifact = _display_summary_field(raw_artifact)
                    if raw_artifact not in allowed_artifacts:
                        continue
                    blockers.extend(
                        _check_release_bundle_evidence_entry_shape(
                            entry,
                            group="android_slot_artifacts",
                            item=slot if slot is not None else None,
                            artifact=artifact,
                        )
                    )
    return blockers


def _check_release_bundle_single_section_evidence_binding(
    entry: Any,
    *,
    group: str,
    expected_sha256: Any,
    expected_path: Any | None = None,
    expected_size: Any | None = None,
    item: str | None = None,
) -> list[dict[str, Any]]:
    if not isinstance(entry, dict) or not isinstance(expected_sha256, str):
        return []
    if expected_path is not None and not isinstance(expected_path, str):
        return []
    if expected_size is not None and (
        isinstance(expected_size, bool) or not isinstance(expected_size, int)
    ):
        return []
    if (
        entry.get("sha256") == expected_sha256
        and (expected_path is None or entry.get("path") == expected_path)
        and (expected_size is None or entry.get("size_bytes") == expected_size)
    ):
        return []
    blocker = _blocker(
        "kagemusha_release_bundle_manifest_section_evidence_binding",
        "Kagemusha release bundle evidence entry does not match its release section",
        group=group,
    )
    if item is not None:
        blocker["item"] = _display_summary_field(item)
    return [blocker]


def _check_release_bundle_section_evidence_map_binding(
    entries: dict[str, Any],
    section: dict[str, Any],
    *,
    group: str,
    digest_field: str,
    size_field: str,
    path_prefix: str,
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    digests = section.get(digest_field)
    sizes = section.get(size_field)
    if not isinstance(digests, dict) or not isinstance(sizes, dict):
        return blockers
    for item, entry in entries.items():
        blockers.extend(
            _check_release_bundle_single_section_evidence_binding(
                entry,
                group=group,
                expected_sha256=digests.get(item),
                expected_path=(
                    f"{path_prefix}/{item}" if isinstance(item, str) else None
                ),
                expected_size=sizes.get(item),
                item=item if isinstance(item, str) else None,
            )
        )
    return blockers


def _check_release_bundle_section_log_binding(
    entries: dict[str, Any],
    *,
    group: str,
    expected_sha256: dict[str, Any],
    expected_paths: dict[str, str],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    for item, entry in entries.items():
        blockers.extend(
            _check_release_bundle_single_section_evidence_binding(
                entry,
                group=group,
                expected_sha256=expected_sha256.get(item),
                expected_path=expected_paths.get(item),
                item=item if isinstance(item, str) else None,
            )
        )
    return blockers


def _check_release_bundle_cross_section_shape(
    bundle: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    evidence = bundle.get("evidence")
    if not isinstance(evidence, dict):
        return blockers
    expected_single_evidence_paths = {
        "readiness_summary": DEFAULT_READINESS_SUMMARY_PATH,
        "lineage_proof_evidence": (
            f"artifacts/kagemusha/{readiness.LINEAGE_PROOF_EVIDENCE_FILENAME}"
        ),
        "compact_key_evidence": (
            f"artifacts/kagemusha/{readiness.COMPACT_KEY_EVIDENCE_FILENAME}"
        ),
        "localnet_lifecycle_evidence": (
            f"artifacts/kagemusha/{readiness.LOCALNET_LIFECYCLE_EVIDENCE_FILENAME}"
        ),
    }
    for group, expected_path in expected_single_evidence_paths.items():
        entry = evidence.get(group)
        if isinstance(entry, dict) and entry.get("path") != expected_path:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_top_level_evidence_path",
                    "Kagemusha release bundle top-level evidence entry "
                    "does not use the canonical release path",
                    group=group,
                )
            )
    lineage = bundle.get("lineage_proof_evidence")
    compact = bundle.get("compact_key_evidence")
    if isinstance(lineage, dict):
        lineage_artifacts = evidence.get("lineage_artifacts")
        if isinstance(lineage_artifacts, dict):
            blockers.extend(
                _check_release_bundle_section_evidence_map_binding(
                    lineage_artifacts,
                    lineage,
                    group="lineage_artifacts",
                    digest_field="artifact_sha256",
                    size_field="artifact_size_bytes",
                    path_prefix="artifacts/kagemusha",
                )
            )
        lineage_logs = evidence.get("lineage_proof_logs")
        if isinstance(lineage_logs, dict):
            blockers.extend(
                _check_release_bundle_section_log_binding(
                    lineage_logs,
                    group="lineage_proof_logs",
                    expected_sha256=lineage.get("test_log_sha256", {}),
                    expected_paths={
                        key: f"artifacts/kagemusha/{relative}"
                        for key, relative in (
                            readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS.items()
                        )
                    },
                )
            )
    if isinstance(compact, dict):
        compact_artifacts = evidence.get("compact_key_artifacts")
        if isinstance(compact_artifacts, dict):
            blockers.extend(
                _check_release_bundle_section_evidence_map_binding(
                    compact_artifacts,
                    compact,
                    group="compact_key_artifacts",
                    digest_field="artifact_sha256",
                    size_field="artifact_size_bytes",
                    path_prefix="artifacts/kagemusha",
                )
            )
        compact_log = evidence.get("compact_key_generator_log")
        if isinstance(compact_log, dict):
            blockers.extend(
                _check_release_bundle_single_section_evidence_binding(
                    compact_log,
                    group="compact_key_generator_log",
                    expected_sha256=compact.get("generator_log_sha256"),
                    expected_path=(
                        f"artifacts/kagemusha/"
                        f"{readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME}"
                    ),
                )
            )
    android = bundle.get("android_device_lab")
    if not isinstance(android, dict):
        return blockers
    signed_evidence_summary = android.get("signed_evidence")
    if not isinstance(signed_evidence_summary, dict):
        return blockers
    expected_slots = set(signed_evidence_summary)
    for group in ("android_signed_evidence", "android_slot_artifacts"):
        entries = evidence.get(group)
        if isinstance(entries, dict) and set(entries) != expected_slots:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_evidence_inventory_keys",
                    "Kagemusha release bundle Android evidence group does not match signed-evidence slots",
                    group=group,
                )
            )
    signed_entries = evidence.get("android_signed_evidence")
    if isinstance(signed_entries, dict):
        for slot, entry in signed_entries.items():
            summary_entry = signed_evidence_summary.get(slot)
            if not isinstance(entry, dict) or not isinstance(summary_entry, dict):
                continue
            expected_path = (
                f"artifacts/android/device_lab/{slot}/"
                f"{device_lab.KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH}"
            )
            if (
                entry.get("path") != expected_path
                or entry.get("sha256") != summary_entry.get("artifact_sha256")
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_android_signed_evidence_binding",
                        "Kagemusha release bundle Android signed-evidence "
                        "entry does not match the signed-evidence summary",
                        group="android_signed_evidence",
                        item=_display_summary_field(slot),
                    )
                )
    slot_artifact_entries = evidence.get("android_slot_artifacts")
    if isinstance(slot_artifact_entries, dict):
        for slot, artifacts in slot_artifact_entries.items():
            summary_entry = signed_evidence_summary.get(slot)
            if not isinstance(artifacts, dict) or not isinstance(summary_entry, dict):
                continue
            for artifact_kind, path_field, digest_field in ANDROID_SLOT_RELEASE_ARTIFACTS:
                entry = artifacts.get(artifact_kind)
                expected_path = summary_entry.get(path_field)
                expected_digest = summary_entry.get(digest_field)
                if (
                    not isinstance(entry, dict)
                    or not isinstance(expected_path, str)
                    or not isinstance(expected_digest, str)
                ):
                    continue
                expected_bundle_path = (
                    f"artifacts/android/device_lab/{slot}/{expected_path}"
                )
                if (
                    entry.get("path") != expected_bundle_path
                    or entry.get("sha256") != expected_digest
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_android_slot_artifact_binding",
                            "Kagemusha release bundle Android slot artifact "
                            "entry does not match the signed-evidence summary",
                            group="android_slot_artifacts",
                            item=_display_summary_field(slot),
                            artifact=artifact_kind,
                        )
                    )
            report_kagemusha: dict[str, Any] | None = None
            for report in android.get("slots", []):
                if isinstance(report, dict) and report.get("slot") == slot:
                    kagemusha = report.get("kagemusha")
                    if isinstance(kagemusha, dict):
                        report_kagemusha = kagemusha
                    break
            if report_kagemusha is None:
                continue
            d2d_transcripts = report_kagemusha.get("d2d_payment_transcripts")
            if not isinstance(d2d_transcripts, dict):
                continue
            primary_path = report_kagemusha.get("d2d_payment_transcript_path")
            for transport, binding in sorted(d2d_transcripts.items()):
                if transport not in device_lab.D2D_PAYMENT_TRANSPORTS:
                    continue
                if (
                    not isinstance(binding, dict)
                    or not isinstance(binding.get("path"), str)
                    or not isinstance(binding.get("sha256"), str)
                    or binding["path"] == primary_path
                ):
                    continue
                artifact_kind = _android_d2d_transcript_artifact_kind(transport)
                entry = artifacts.get(artifact_kind)
                expected_bundle_path = (
                    f"artifacts/android/device_lab/{slot}/{binding['path']}"
                )
                if (
                    not isinstance(entry, dict)
                    or entry.get("path") != expected_bundle_path
                    or entry.get("sha256") != binding["sha256"]
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_android_slot_artifact_binding",
                            "Kagemusha release bundle Android D2D transcript entry does not match the readiness summary",
                            group="android_slot_artifacts",
                            item=_display_summary_field(slot),
                            artifact=artifact_kind,
                        )
                    )
    return blockers


def _check_release_bundle_expected_top_level_evidence_binding(
    existing: dict[str, Any],
    expected: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    existing_evidence = existing.get("evidence")
    expected_evidence = expected.get("evidence")
    if not isinstance(existing_evidence, dict) or not isinstance(
        expected_evidence, dict
    ):
        return blockers
    for group in (
        "readiness_summary",
        "lineage_proof_evidence",
        "compact_key_evidence",
        "localnet_lifecycle_evidence",
        "compact_key_generator_log",
    ):
        entry = existing_evidence.get(group)
        expected_entry = expected_evidence.get(group)
        if not isinstance(entry, dict) or not isinstance(expected_entry, dict):
            continue
        if (
            entry.get("path") == expected_entry.get("path")
            and entry.get("sha256") == expected_entry.get("sha256")
            and entry.get("size_bytes") == expected_entry.get("size_bytes")
        ):
            continue
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_top_level_evidence_binding",
                "Kagemusha release bundle top-level evidence entry does not "
                "match freshly computed release evidence",
                group=group,
            )
        )
    return blockers


def _check_release_bundle_expected_android_summary_binding(
    existing: dict[str, Any],
    expected: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    existing_android = existing.get("android_device_lab")
    expected_android = expected.get("android_device_lab")
    if not isinstance(existing_android, dict) or not isinstance(
        expected_android, dict
    ):
        return blockers
    for field in (
        "covered_device_families",
        "missing_device_families",
        "covered_d2d_payment_transports",
        "missing_d2d_payment_transports",
        "duplicate_bindings",
        "trusted_signer_public_key_sha256",
    ):
        if existing_android.get(field) == expected_android.get(field):
            continue
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_android_summary_binding",
                "Kagemusha release bundle Android summary field does not "
                "match freshly computed device-lab evidence",
                field=field,
            )
        )
    existing_signed = existing_android.get("signed_evidence")
    expected_signed = expected_android.get("signed_evidence")
    if isinstance(existing_signed, dict) and isinstance(expected_signed, dict):
        for slot in sorted(set(existing_signed) & set(expected_signed)):
            entry = existing_signed.get(slot)
            expected_entry = expected_signed.get(slot)
            if not isinstance(entry, dict) or not isinstance(expected_entry, dict):
                continue
            for field in sorted(ANDROID_SIGNED_EVIDENCE_SUMMARY_IDENTITY_FIELDS):
                if entry.get(field) == expected_entry.get(field):
                    continue
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_android_signed_evidence_identity_binding",
                        "Kagemusha release bundle Android signed-evidence identity does not match freshly computed device-lab evidence",
                        field=field,
                        item=_display_summary_field(slot),
                    )
                )
        if _signed_evidence_without_android_identity_fields(
            existing_signed
        ) == _signed_evidence_without_android_identity_fields(expected_signed):
            return blockers
    if existing_signed != expected_signed:
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_android_summary_binding",
                "Kagemusha release bundle Android summary field does not "
                "match freshly computed device-lab evidence",
                field="signed_evidence",
            )
        )
    return blockers


def _check_release_bundle_expected_section_value_binding(
    existing: dict[str, Any],
    expected: dict[str, Any],
) -> list[dict[str, Any]]:
    """Bind release manifest section values to freshly computed evidence."""

    blockers: list[dict[str, Any]] = []
    fields_by_section = {
        "abi6_reserved_lineage": (
            "manifest_path",
            "schema",
            "native_bridge_abi_version",
            "operation_count",
            "limits",
            "modes",
        ),
        "abi7_recursive_compact": (
            "state",
            "circuit_id",
            "fixture_manifest_path",
            "fixture_manifest_schema",
            "fixture_manifest_sha256",
            "archive_fixture_path",
            "archive_fixture_schema",
            "archive_fixture_sha256",
            "native_bridge_abi_version",
            "operation_count",
        ),
        "lineage_key_release_tooling": ("state", "checked_files"),
        "lineage_proof_evidence": (
            "state",
            "generated_at_utc",
            "artifact_sha256",
            "artifact_size_bytes",
            "test_log_sha256",
        ),
        "compact_key_evidence": (
            "state",
            "generated_at_utc",
            "artifact_sha256",
            "artifact_size_bytes",
            "generator_log_sha256",
            "generator_log_artifact_sha256",
            "generator_log_artifact_size_bytes",
        ),
        "localnet_lifecycle_evidence": (
            "state",
            "generated_at_utc",
            "localnet_run_id",
            "chain_id",
            "target",
            "peer_count",
            "peer_ids",
            "artifact_sha256",
            "artifact_count",
        ),
    }
    for section_name, fields in fields_by_section.items():
        existing_section = existing.get(section_name)
        expected_section = expected.get(section_name)
        if not isinstance(existing_section, dict) or not isinstance(
            expected_section, dict
        ):
            continue
        for field in fields:
            if existing_section.get(field) == expected_section.get(field):
                continue
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_value_binding",
                    "Kagemusha release bundle section value does not match "
                    "freshly computed release evidence",
                    section=section_name,
                    field=field,
                )
            )
    return blockers


def _check_release_bundle_expected_android_evidence_binding(
    existing: dict[str, Any],
    expected: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    existing_evidence = existing.get("evidence")
    expected_evidence = expected.get("evidence")
    if not isinstance(existing_evidence, dict) or not isinstance(
        expected_evidence, dict
    ):
        return blockers

    signed_entries = existing_evidence.get("android_signed_evidence")
    expected_signed_entries = expected_evidence.get("android_signed_evidence")
    if isinstance(signed_entries, dict) and isinstance(
        expected_signed_entries, dict
    ):
        for slot, entry in signed_entries.items():
            expected_entry = expected_signed_entries.get(slot)
            if not isinstance(entry, dict) or not isinstance(expected_entry, dict):
                continue
            if (
                entry.get("path") == expected_entry.get("path")
                and entry.get("sha256") == expected_entry.get("sha256")
                and entry.get("size_bytes") == expected_entry.get("size_bytes")
            ):
                continue
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_android_signed_evidence_binding",
                    "Kagemusha release bundle Android signed-evidence "
                    "entry does not match freshly computed release evidence",
                    group="android_signed_evidence",
                    item=_display_summary_field(slot),
                )
            )

    slot_artifacts = existing_evidence.get("android_slot_artifacts")
    expected_slot_artifacts = expected_evidence.get("android_slot_artifacts")
    if isinstance(slot_artifacts, dict) and isinstance(expected_slot_artifacts, dict):
        for slot, artifacts in slot_artifacts.items():
            expected_artifacts = expected_slot_artifacts.get(slot)
            if not isinstance(artifacts, dict) or not isinstance(
                expected_artifacts, dict
            ):
                continue
            if set(artifacts) != set(expected_artifacts):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_android_slot_artifact_binding",
                        "Kagemusha release bundle Android slot artifact "
                        "inventory does not match freshly computed release evidence",
                        group="android_slot_artifacts",
                        item=_display_summary_field(slot),
                    )
                )
            for artifact_kind, entry in artifacts.items():
                expected_entry = expected_artifacts.get(artifact_kind)
                if not isinstance(entry, dict) or not isinstance(
                    expected_entry, dict
                ):
                    continue
                if (
                    entry.get("path") == expected_entry.get("path")
                    and entry.get("sha256") == expected_entry.get("sha256")
                    and entry.get("size_bytes") == expected_entry.get("size_bytes")
                ):
                    continue
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_android_slot_artifact_binding",
                        "Kagemusha release bundle Android slot artifact "
                        "entry does not match freshly computed release evidence",
                        group="android_slot_artifacts",
                        item=_display_summary_field(slot),
                        artifact=artifact_kind,
                    )
                )
    return blockers


def _check_release_bundle_expected_compact_generator_log_artifact_binding(
    existing: dict[str, Any],
    expected: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    existing_compact = existing.get("compact_key_evidence")
    expected_compact = expected.get("compact_key_evidence")
    if not isinstance(existing_compact, dict) or not isinstance(
        expected_compact, dict
    ):
        return blockers
    for field in (
        "generator_log_artifact_sha256",
        "generator_log_artifact_size_bytes",
    ):
        if existing_compact.get(field) == expected_compact.get(field):
            continue
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_compact_generator_log_artifact_binding",
                "Kagemusha release bundle compact generator-log artifact "
                "summary does not match freshly computed compact evidence",
                field=field,
            )
        )
    return blockers


def _check_release_bundle_evidence_paths(value: Any) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    if not isinstance(value, dict):
        return [
            _blocker(
                "kagemusha_release_bundle_manifest_evidence_shape",
                "Kagemusha release bundle evidence inventory must be a JSON object",
            )
        ]

    def visit(item: Any) -> None:
        if isinstance(item, dict):
            path = item.get("path")
            if "path" in item:
                if not isinstance(path, str):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_evidence_path",
                            "Kagemusha release bundle evidence path must be a string",
                        )
                    )
                else:
                    path_errors: list[str] = []
                    safe_relative = device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                        path,
                        path_errors,
                        "Kagemusha release bundle evidence path",
                    )
                    if safe_relative is None or safe_relative != path:
                        for error in path_errors or [
                            "Kagemusha release bundle evidence path must be canonical"
                        ]:
                            blockers.append(
                                _blocker(
                                    "kagemusha_release_bundle_manifest_evidence_path",
                                    error,
                                )
                            )
                digest = item.get("sha256")
                if (
                    not isinstance(digest, str)
                    or device_lab.SHA256_HEX_RE.fullmatch(digest) is None
                    or digest == "0" * 64
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_evidence_sha256",
                            "Kagemusha release bundle evidence SHA-256 must be a non-zero lowercase hex digest",
                        )
                    )
                size = item.get("size_bytes")
                if (
                    "size_bytes" not in item
                    or isinstance(size, bool)
                    or not isinstance(size, int)
                    or size <= 0
                ):
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_evidence_size",
                            "Kagemusha release bundle evidence size_bytes must be a positive integer",
                        )
                    )
            for child in item.values():
                visit(child)
        elif isinstance(item, list):
            for child in item:
                visit(child)

    visit(value)
    return blockers


def _release_manifest_android_blocker(blocker: dict[str, Any]) -> dict[str, Any]:
    mapped = dict(blocker)
    code = mapped.get("code")
    if isinstance(code, str) and code.startswith("kagemusha_release_summary_android_"):
        mapped["code"] = code.replace(
            "kagemusha_release_summary_android_",
            "kagemusha_release_bundle_manifest_android_",
            1,
        )
    return mapped


def _expected_release_bundle_section_map_keys(
    section_name: str,
    field: str,
) -> set[str] | None:
    if section_name == "abi6_reserved_lineage":
        if field == "limits":
            return set(readiness.EXPECTED_ABI6_LIMIT_VALUES)
        if field == "modes":
            return {
                "preferred_when_recursive_available",
                "fallback_when_recursive_unavailable",
            }
    if section_name == "lineage_proof_evidence":
        if field == "circuit_ids":
            return set(readiness.EXPECTED_LINEAGE_CIRCUIT_IDS)
        if field in ("artifact_sha256", "artifact_size_bytes"):
            return set(readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS)
        if field == "test_log_sha256":
            return set(readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS)
    if section_name == "compact_key_evidence":
        if field in (
            "artifact_sha256",
            "artifact_size_bytes",
            "generator_log_artifact_sha256",
            "generator_log_artifact_size_bytes",
        ):
            return set(readiness.COMPACT_KEY_REQUIRED_ARTIFACTS)
    if section_name == "localnet_lifecycle_evidence":
        if field == "artifact_sha256":
            return set(readiness.LOCALNET_LIFECYCLE_HASH_FIELDS)
    return None


def _check_release_bundle_section_shapes(
    bundle: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    for section_name, allowed_fields in RELEASE_BUNDLE_ALLOWED_SECTION_KEYS.items():
        section = bundle.get(section_name)
        if not isinstance(section, dict):
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_shape",
                    "Kagemusha release bundle section must be a JSON object",
                    section=section_name,
                )
            )
            continue
        for field in sorted(set(section) - allowed_fields):
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_unexpected_field",
                    "Kagemusha release bundle section contains an unexpected field",
                    section=section_name,
                    field=_display_summary_field(field),
                )
            )
        for field in sorted(allowed_fields - set(section)):
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_missing_field",
                    "Kagemusha release bundle section is missing a required field",
                    section=section_name,
                    field=field,
                )
            )
        expected_state = SUMMARY_REQUIRED_SECTION_STATES.get(section_name)
        state = section.get("state")
        if (
            expected_state is not None
            and "state" in section
            and not isinstance(state, str)
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_state_shape",
                    "Kagemusha release bundle section state must be a string",
                    section=section_name,
                )
            )
        if expected_state is not None and state != expected_state:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_state",
                    "Kagemusha release bundle section state is not ready",
                    section=section_name,
                    expected_state=expected_state,
                )
            )

    abi6 = bundle.get("abi6_reserved_lineage")
    if isinstance(abi6, dict):
        for field in ("manifest_path", "schema"):
            value = abi6.get(field)
            if not isinstance(value, str) or not value:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_string",
                        "Kagemusha release bundle section field must be a non-empty string",
                        section="abi6_reserved_lineage",
                        field=field,
                    )
                )
        expected_abi6_values = {
            "manifest_path": readiness.ABI6_MANIFEST_PATH,
            "schema": readiness.ABI6_MANIFEST_SCHEMA,
            "native_bridge_abi_version": 6,
            "operation_count": len(readiness.ABI6_OPERATION_SYMBOLS),
        }
        for field, expected in expected_abi6_values.items():
            if abi6.get(field) != expected:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_value",
                        "Kagemusha release bundle ABI-6 section field does "
                        "not match the required value",
                        section="abi6_reserved_lineage",
                        field=field,
                    )
                )
        expected_abi6_limits = {
            key: readiness.EXPECTED_ABI6_LIMIT_VALUES[key]
            for key in sorted(readiness.EXPECTED_ABI6_LIMIT_VALUES)
        }
        if abi6.get("limits") != expected_abi6_limits:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_value",
                    "Kagemusha release bundle ABI-6 limits do not match "
                    "the required values",
                    section="abi6_reserved_lineage",
                    field="limits",
                )
            )
        expected_abi6_modes = {
            "preferred_when_recursive_available": "recursive_spend_v1",
            "fallback_when_recursive_unavailable": "checked_prefold_v1",
        }
        if abi6.get("modes") != expected_abi6_modes:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_value",
                    "Kagemusha release bundle ABI-6 modes do not match "
                    "the required values",
                    section="abi6_reserved_lineage",
                    field="modes",
                )
            )
        for field in ("native_bridge_abi_version", "operation_count"):
            value = abi6.get(field)
            if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_integer",
                        "Kagemusha release bundle section field must be a positive integer",
                        section="abi6_reserved_lineage",
                        field=field,
                    )
                )
        for field in ("limits", "modes"):
            if not isinstance(abi6.get(field), dict):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_object",
                        "Kagemusha release bundle section field must be a JSON object",
                        section="abi6_reserved_lineage",
                        field=field,
                    )
                )

    abi7 = bundle.get("abi7_recursive_compact")
    if isinstance(abi7, dict):
        expected_abi7_values = {
            "circuit_id": readiness.EXPECTED_COMPACT_KEY_CIRCUIT_ID,
            "fixture_manifest_path": readiness.ABI7_FIXTURE_MANIFEST_PATH,
            "fixture_manifest_schema": readiness.ABI7_FIXTURE_MANIFEST_SCHEMA,
            "archive_fixture_path": readiness.ABI7_ARCHIVE_FIXTURE_PATH,
            "archive_fixture_schema": readiness.ABI7_ARCHIVE_FIXTURE_SCHEMA,
            "native_bridge_abi_version": 7,
            "operation_count": len(readiness.ABI7_FIXTURE_OPERATIONS),
        }
        for field, expected in expected_abi7_values.items():
            value = abi7.get(field)
            if isinstance(expected, str) and (not isinstance(value, str) or not value):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_string",
                        "Kagemusha release bundle section field must be a non-empty string",
                        section="abi7_recursive_compact",
                        field=field,
                    )
                )
            if value != expected:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_value",
                        "Kagemusha release bundle ABI-7 section field does not match the required value",
                        section="abi7_recursive_compact",
                        field=field,
                    )
                )
        for field in ("fixture_manifest_sha256", "archive_fixture_sha256"):
            value = abi7.get(field)
            if (
                not isinstance(value, str)
                or not device_lab.SHA256_HEX_RE.fullmatch(value)
                or value == "0" * 64
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_sha256",
                        "Kagemusha release bundle ABI-7 fixture digest must be a non-zero lowercase SHA-256 digest",
                        section="abi7_recursive_compact",
                        field=field,
                    )
                )
        for field in ("native_bridge_abi_version", "operation_count"):
            value = abi7.get(field)
            if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_integer",
                        "Kagemusha release bundle section field must be a positive integer",
                        section="abi7_recursive_compact",
                        field=field,
                    )
                )

    tooling = bundle.get("lineage_key_release_tooling")
    if isinstance(tooling, dict):
        checked_files = tooling.get("checked_files")
        checked_files_ok = (
            isinstance(checked_files, list)
            and bool(checked_files)
            and all(isinstance(item, str) and item for item in checked_files)
        )
        if not checked_files_ok:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_list",
                    "Kagemusha release bundle section list field must contain non-empty strings",
                    section="lineage_key_release_tooling",
                    field="checked_files",
                )
            )
        else:
            expected_checked_files = list(
                readiness.LINEAGE_KEY_RELEASE_TOOLING_REQUIREMENTS
            )
            if checked_files != expected_checked_files:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_inventory",
                        "Kagemusha release bundle checked_files must exactly "
                        "match the required release-tooling inventory",
                        section="lineage_key_release_tooling",
                        field="checked_files",
                    )
                )

    for section_name in (
        "lineage_proof_evidence",
        "compact_key_evidence",
        "localnet_lifecycle_evidence",
    ):
        section = bundle.get(section_name)
        if not isinstance(section, dict):
            continue
        generated_at = section.get("generated_at_utc")
        if (
            not isinstance(generated_at, str)
            or not device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at)
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_section_timestamp",
                    "Kagemusha release bundle section timestamp must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                    section=section_name,
                    field="generated_at_utc",
                )
            )
        else:
            generated_at_timestamp, parse_blocker = readiness.parse_utc_timestamp(
                generated_at,
                f"Kagemusha release bundle {section_name} generated_at_utc",
            )
            if parse_blocker is not None:
                parse_blocker["code"] = (
                    "kagemusha_release_bundle_manifest_section_timestamp"
                )
                parse_blocker["section"] = section_name
                parse_blocker["field"] = "generated_at_utc"
                blockers.append(parse_blocker)
            elif generated_at_timestamp is not None:
                max_generated_at = dt.datetime.now(dt.timezone.utc).replace(
                    microsecond=0,
                ) + dt.timedelta(
                    seconds=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
                )
                if generated_at_timestamp > max_generated_at:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_section_future_dated",
                            "Kagemusha release bundle section timestamp must not be future-dated beyond the allowed clock skew",
                            section=section_name,
                            field="generated_at_utc",
                            max_generated_at_utc=max_generated_at.isoformat().replace(
                                "+00:00",
                                "Z",
                            ),
                        )
                    )
        sha256_map_fields = ["artifact_sha256"]
        if section_name == "lineage_proof_evidence":
            sha256_map_fields.append("test_log_sha256")
        elif section_name == "compact_key_evidence":
            sha256_map_fields.append("generator_log_artifact_sha256")
            generator_log_sha256 = section.get("generator_log_sha256")
            if (
                not isinstance(generator_log_sha256, str)
                or not device_lab.SHA256_HEX_RE.fullmatch(generator_log_sha256)
                or generator_log_sha256 == "0" * 64
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_sha256",
                        "Kagemusha release bundle section SHA-256 field must be a non-zero lowercase hex digest",
                        section=section_name,
                        field="generator_log_sha256",
                    )
                )
        for field in sha256_map_fields:
            value = section.get(field)
            expected_keys = _expected_release_bundle_section_map_keys(
                section_name,
                field,
            )
            if (
                isinstance(value, dict)
                and expected_keys is not None
                and set(value) != expected_keys
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_inventory",
                        "Kagemusha release bundle section map must exactly match the required inventory",
                        section=section_name,
                        field=field,
                    )
                )
            if (
                not isinstance(value, dict)
                or not value
                or any(
                    not isinstance(key, str)
                    or not key
                    or not isinstance(digest, str)
                    or not device_lab.SHA256_HEX_RE.fullmatch(digest)
                    or digest == "0" * 64
                    or (
                        section_name == "localnet_lifecycle_evidence"
                        and len(set(digest)) == 1
                    )
                    for key, digest in value.items()
                )
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_sha256",
                        "Kagemusha release bundle section SHA-256 map must contain non-placeholder lowercase hex digests",
                        section=section_name,
                        field=field,
                    )
                )
        size_map_fields = []
        if section_name in ("lineage_proof_evidence", "compact_key_evidence"):
            size_map_fields.append("artifact_size_bytes")
        if section_name == "compact_key_evidence":
            size_map_fields.append("generator_log_artifact_size_bytes")
        for field in size_map_fields:
            value = section.get(field)
            expected_keys = _expected_release_bundle_section_map_keys(
                section_name,
                field,
            )
            if (
                isinstance(value, dict)
                and expected_keys is not None
                and set(value) != expected_keys
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_inventory",
                        "Kagemusha release bundle section map must exactly match the required inventory",
                        section=section_name,
                        field=field,
                    )
                )
            if (
                not isinstance(value, dict)
                or not value
                or any(
                    not isinstance(key, str)
                    or not key
                    or isinstance(size, bool)
                    or not isinstance(size, int)
                    or size <= 0
                    for key, size in value.items()
                )
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_size",
                        "Kagemusha release bundle section size map must contain positive integer sizes",
                        section=section_name,
                        field=field,
                    )
                )
        if section_name == "localnet_lifecycle_evidence":
            artifact_hashes = section.get("artifact_sha256")
            if isinstance(artifact_hashes, dict) and len(set(artifact_hashes.values())) != len(
                artifact_hashes
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_sha256_distinct",
                        "Kagemusha release bundle localnet artifact hashes must be distinct",
                        section=section_name,
                        field="artifact_sha256",
                    )
                )
            if section.get("target") != readiness.EXPECTED_LOCALNET_TARGET:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_value",
                        "Kagemusha release bundle localnet target does not match the required value",
                        section=section_name,
                        field="target",
                    )
                )
            if section.get("peer_count") != readiness.EXPECTED_LOCALNET_PEER_COUNT:
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_value",
                        "Kagemusha release bundle localnet peer_count does not match the required value",
                        section=section_name,
                        field="peer_count",
                    )
                )
            if section.get("artifact_count") != len(
                readiness.LOCALNET_LIFECYCLE_HASH_FIELDS
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_value",
                        "Kagemusha release bundle localnet artifact_count does not match the required hash inventory",
                        section=section_name,
                        field="artifact_count",
                    )
                )
            if not readiness._localnet_run_id_is_valid(section.get("localnet_run_id")):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_localnet_identity",
                        "Kagemusha release bundle localnet_run_id must identify a production localnet run",
                        section=section_name,
                        field="localnet_run_id",
                    )
                )
            if not readiness._localnet_chain_id_is_valid(section.get("chain_id")):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_localnet_identity",
                        "Kagemusha release bundle chain_id must identify a production localnet chain",
                        section=section_name,
                        field="chain_id",
                    )
                )
            peer_ids = section.get("peer_ids")
            if (
                not isinstance(peer_ids, list)
                or len(peer_ids) != readiness.EXPECTED_LOCALNET_PEER_COUNT
                or any(
                    not readiness._localnet_peer_id_is_valid(peer_id)
                    for peer_id in peer_ids
                )
                or len(set(peer_ids)) != readiness.EXPECTED_LOCALNET_PEER_COUNT
                or peer_ids != sorted(peer_ids)
            ):
                blockers.append(
                    _blocker(
                        "kagemusha_release_bundle_manifest_section_list",
                        "Kagemusha release bundle localnet peer_ids must contain four distinct sorted production localnet peer ids",
                        section=section_name,
                        field="peer_ids",
                    )
                )
    return blockers


def _check_release_bundle_android_section_shape(
    bundle: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    android = bundle.get("android_device_lab")
    if not isinstance(android, dict):
        return [
            _blocker(
                "kagemusha_release_bundle_manifest_android_shape",
                "Kagemusha release bundle Android section must be a JSON object",
            )
        ]

    for field in sorted(set(android) - RELEASE_BUNDLE_ALLOWED_ANDROID_SECTION_KEYS):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_android_unexpected_field",
                "Kagemusha release bundle Android section contains an unexpected field",
                field=_display_summary_field(field),
            )
        )
    for field in sorted(RELEASE_BUNDLE_ALLOWED_ANDROID_SECTION_KEYS - set(android)):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_android_missing_field",
                "Kagemusha release bundle Android section is missing a required field",
                field=field,
            )
        )

    if android.get("root") != readiness.ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL:
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_android_root",
                "Kagemusha release bundle Android root must use the canonical redacted label",
            )
        )

    list_fields_ok: dict[str, bool] = {}
    for field in (
        "covered_device_families",
        "missing_device_families",
        "covered_d2d_payment_transports",
        "missing_d2d_payment_transports",
        "trusted_signer_public_key_sha256",
    ):
        value = android.get(field)
        field_ok = isinstance(value, list) and all(
            isinstance(item, str) and item for item in value
        )
        list_fields_ok[field] = field_ok
        if not field_ok:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_android_list_shape",
                    "Kagemusha release bundle Android list fields must contain non-empty strings",
                    field=field,
                )
            )

    if list_fields_ok.get("covered_device_families"):
        expected_families = sorted(device_lab.KAGEMUSHA_STANDARD_DEVICE_FAMILIES)
        if android.get("covered_device_families") != expected_families:
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_android_device_families",
                    "Kagemusha release bundle Android covered_device_families must exactly match the standard matrix",
                )
            )
    if (
        list_fields_ok.get("missing_device_families")
        and android.get("missing_device_families") != []
    ):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_android_device_families",
                "Kagemusha release bundle Android missing_device_families must be empty",
            )
        )
    if list_fields_ok.get("covered_d2d_payment_transports"):
        if android.get("covered_d2d_payment_transports") != list(
            readiness.ANDROID_REQUIRED_D2D_PAYMENT_TRANSPORTS
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_android_d2d_transports",
                    "Kagemusha release bundle Android covered_d2d_payment_transports must cover every required offline D2D transport",
                )
            )
    if (
        list_fields_ok.get("missing_d2d_payment_transports")
        and android.get("missing_d2d_payment_transports") != []
    ):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_android_d2d_transports",
                "Kagemusha release bundle Android missing_d2d_payment_transports must be empty",
            )
        )
    if list_fields_ok.get("trusted_signer_public_key_sha256"):
        signer_digests = android.get("trusted_signer_public_key_sha256")
        assert isinstance(signer_digests, list)
        if (
            not signer_digests
            or signer_digests != sorted(set(signer_digests))
            or any(
                not device_lab.SHA256_HEX_RE.fullmatch(digest)
                or digest == "0" * 64
                for digest in signer_digests
            )
        ):
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_android_signer_sha256",
                    "Kagemusha release bundle Android trusted signer digests must be unique sorted non-zero lowercase sha256 hex strings",
                )
            )
    blockers.extend(
        _release_manifest_android_blocker(item)
        for item in _check_android_duplicate_bindings_summary_shape(android)
    )
    blockers.extend(
        _release_manifest_android_blocker(item)
        for item in _check_android_signed_evidence_summary_shape(android)
    )
    blockers.extend(
        _check_android_trusted_signer_binding(
            android,
            code="kagemusha_release_bundle_manifest_android_signer_binding",
        )
    )
    return blockers


def _check_release_bundle_manifest_shape(bundle: dict[str, Any]) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    if _contains_secret_string(bundle):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_secret_material",
                "Kagemusha release bundle manifest must not contain secret-looking material",
            )
        )
    if _contains_control_string(bundle):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_control_character",
                "Kagemusha release bundle manifest must not contain control characters",
            )
        )
    for field in sorted(set(bundle) - RELEASE_BUNDLE_ALLOWED_TOP_LEVEL_KEYS):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_unexpected_field",
                "Kagemusha release bundle manifest contains an unexpected top-level field",
                field=_display_summary_field(field),
            )
        )
    for field in sorted(RELEASE_BUNDLE_ALLOWED_TOP_LEVEL_KEYS - set(bundle)):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_missing_field",
                "Kagemusha release bundle manifest is missing a required top-level field",
                field=field,
            )
        )
    bundle_schema = bundle.get("schema")
    if not isinstance(bundle_schema, str):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_schema_shape",
                "Kagemusha release bundle manifest schema must be a string",
            )
        )
    if bundle_schema != RELEASE_BUNDLE_SCHEMA:
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_schema",
                "Kagemusha release bundle manifest schema mismatch",
            )
        )
    generated_at = bundle.get("generated_at_utc")
    if not isinstance(generated_at, str):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_timestamp",
                "Kagemusha release bundle generated_at_utc is required",
            )
        )
    else:
        if not device_lab.SIGNED_AT_UTC_RE.fullmatch(generated_at):
            blockers.append(
                _blocker(
                    "kagemusha_release_bundle_manifest_timestamp",
                    "Kagemusha release bundle generated_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ",
                )
            )
        else:
            generated_at_timestamp, parse_blocker = readiness.parse_utc_timestamp(
                generated_at,
                "Kagemusha release bundle generated_at_utc",
            )
            if parse_blocker is not None:
                parse_blocker["code"] = "kagemusha_release_bundle_manifest_timestamp"
                blockers.append(parse_blocker)
            elif generated_at_timestamp is not None:
                max_generated_at = dt.datetime.now(dt.timezone.utc).replace(
                    microsecond=0,
                ) + dt.timedelta(
                    seconds=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
                )
                if generated_at_timestamp > max_generated_at:
                    blockers.append(
                        _blocker(
                            "kagemusha_release_bundle_manifest_future_dated",
                            "Kagemusha release bundle generated_at_utc must not be future-dated beyond the allowed clock skew",
                            max_generated_at_utc=max_generated_at.isoformat().replace(
                                "+00:00",
                                "Z",
                            ),
                        )
                    )
    ready_value = bundle.get("ready")
    if not isinstance(ready_value, bool):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_ready_shape",
                "Kagemusha release bundle manifest ready flag must be boolean",
            )
        )
    if ready_value is not True:
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_not_ready",
                "Kagemusha release bundle manifest must be ready",
            )
        )
    manifest_blockers = bundle.get("blockers")
    if not isinstance(manifest_blockers, list):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_blockers_shape",
                "Kagemusha release bundle manifest blockers must be a JSON array",
            )
        )
    elif manifest_blockers:
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_blockers_present",
                "Kagemusha release bundle manifest must not contain blockers",
            )
        )
    evidence = bundle.get("evidence")
    blockers.extend(_check_release_bundle_evidence_inventory_shape(evidence))
    blockers.extend(_check_release_bundle_evidence_paths(evidence))
    blockers.extend(_check_release_bundle_section_shapes(bundle))
    blockers.extend(_check_release_bundle_android_section_shape(bundle))
    blockers.extend(_check_release_bundle_cross_section_shape(bundle))
    return blockers


def build_release_bundle(
    *,
    repo_root: Path,
    bundle_root: Path,
    readiness_summary_path: Path,
    lineage_proof_evidence_path: Path,
    compact_key_evidence_path: Path,
    device_lab_root: Path,
    trusted_signer_public_keys: dict[str, Path],
    min_signed_at: dt.datetime | None,
    max_signed_at: dt.datetime | None,
    min_lineage_proof_evidence_at: dt.datetime | None,
    max_lineage_proof_evidence_at: dt.datetime | None,
    min_compact_key_evidence_at: dt.datetime | None,
    max_compact_key_evidence_at: dt.datetime | None,
    localnet_lifecycle_evidence_path: Path | None = None,
    min_localnet_lifecycle_evidence_at: dt.datetime | None = None,
    max_localnet_lifecycle_evidence_at: dt.datetime | None = None,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Return the release bundle manifest and blockers."""

    signer_map_blockers = [
        _blocker("android_trusted_signer_invalid", error)
        for error in device_lab.validate_trusted_signer_public_key_map(
            trusted_signer_public_keys
        )
    ]
    if signer_map_blockers:
        return (
            _blocked_release_bundle_manifest(
                signer_map_blockers,
                trusted_signer_public_keys,
            ),
            signer_map_blockers,
        )

    repo_root_blockers = readiness.validate_repo_root_path(repo_root)
    if repo_root_blockers:
        return (
            _blocked_release_bundle_manifest(
                repo_root_blockers,
                trusted_signer_public_keys,
            ),
            repo_root_blockers,
        )

    blockers: list[dict[str, Any]] = []
    if localnet_lifecycle_evidence_path is None:
        localnet_lifecycle_evidence_path = (
            lineage_proof_evidence_path.parent
            / readiness.LOCALNET_LIFECYCLE_EVIDENCE_FILENAME
        )
    root_blockers = _validate_bundle_root(bundle_root)
    blockers.extend(root_blockers)
    summary_path_ok, summary_path_blockers = _preflight_bundle_input_path(
        readiness_summary_path,
        bundle_root,
        "Kagemusha readiness summary",
    )
    lineage_path_ok, lineage_path_blockers = _preflight_bundle_input_path(
        lineage_proof_evidence_path,
        bundle_root,
        "Reserved-lineage proof evidence",
    )
    compact_path_ok, compact_path_blockers = _preflight_bundle_input_path(
        compact_key_evidence_path,
        bundle_root,
        "ABI-7 recursive compact key evidence",
    )
    localnet_path_ok, localnet_path_blockers = _preflight_bundle_input_path(
        localnet_lifecycle_evidence_path,
        bundle_root,
        "Kagemusha localnet lifecycle evidence",
    )
    android_path_ok, android_path_blockers = _preflight_bundle_input_path(
        device_lab_root,
        bundle_root,
        "Android device-lab root",
    )
    input_path_blockers = [
        *summary_path_blockers,
        *lineage_path_blockers,
        *compact_path_blockers,
        *localnet_path_blockers,
        *android_path_blockers,
    ]
    blockers.extend(input_path_blockers)
    input_paths_ok = not root_blockers and not input_path_blockers
    summary = None
    if input_paths_ok and summary_path_ok:
        summary, summary_blockers = _load_local_json(
            readiness_summary_path,
            "Kagemusha readiness summary",
            "kagemusha_release_summary",
        )
        blockers.extend(summary_blockers)
        if summary is not None:
            blockers.extend(_check_ready_summary_shape(summary))

    abi6 = readiness.check_abi6_reserved_lineage(repo_root)
    abi7 = readiness.check_abi7_fail_closed(repo_root)
    lineage_tooling = readiness.check_lineage_key_release_tooling(repo_root)
    lineage: dict[str, Any] = {"blockers": []}
    if input_paths_ok and lineage_path_ok:
        lineage = readiness.check_lineage_proof_evidence(
            lineage_proof_evidence_path,
            min_generated_at=min_lineage_proof_evidence_at,
            max_generated_at=max_lineage_proof_evidence_at,
        )
    compact: dict[str, Any] = {"blockers": []}
    if input_paths_ok and compact_path_ok:
        compact = readiness.check_compact_key_evidence(
            compact_key_evidence_path,
            min_generated_at=min_compact_key_evidence_at,
            max_generated_at=max_compact_key_evidence_at,
        )
    localnet_lifecycle: dict[str, Any] = {"blockers": []}
    if input_paths_ok and localnet_path_ok:
        localnet_lifecycle = readiness.check_localnet_lifecycle_evidence(
            localnet_lifecycle_evidence_path,
            min_generated_at=min_localnet_lifecycle_evidence_at,
            max_generated_at=max_localnet_lifecycle_evidence_at,
        )
    android: dict[str, Any] = {"blockers": []}
    if input_paths_ok and android_path_ok:
        android = readiness.check_android_device_lab(
            device_lab_root,
            trusted_signer_public_keys,
            min_signed_at=min_signed_at,
            max_signed_at=max_signed_at,
        )
    blockers.extend(abi6["blockers"])
    blockers.extend(abi7["blockers"])
    blockers.extend(lineage_tooling["blockers"])
    blockers.extend(lineage["blockers"])
    blockers.extend(compact["blockers"])
    blockers.extend(localnet_lifecycle["blockers"])
    blockers.extend(android["blockers"])
    if (
        summary is not None
        and input_paths_ok
        and not abi6["blockers"]
        and not abi7["blockers"]
        and not lineage_tooling["blockers"]
        and not lineage["blockers"]
        and not compact["blockers"]
        and not localnet_lifecycle["blockers"]
        and not android["blockers"]
    ):
        blockers.extend(
            _compare_validated_sections(
                summary,
                abi6,
                abi7,
                lineage_tooling,
                lineage,
                compact,
                localnet_lifecycle,
                android,
            )
        )

    evidence_entries: dict[str, Any] = {}
    for key, path, label, path_ok in (
        (
            "readiness_summary",
            readiness_summary_path,
            "Kagemusha readiness summary",
            summary_path_ok,
        ),
        (
            "lineage_proof_evidence",
            lineage_proof_evidence_path,
            "Reserved-lineage proof evidence",
            lineage_path_ok,
        ),
        (
            "compact_key_evidence",
            compact_key_evidence_path,
            "ABI-7 recursive compact key evidence",
            compact_path_ok,
        ),
        (
            "localnet_lifecycle_evidence",
            localnet_lifecycle_evidence_path,
            "Kagemusha localnet lifecycle evidence",
            localnet_path_ok,
        ),
    ):
        if not input_paths_ok or not path_ok:
            continue
        entry, entry_blockers = _evidence_entry_with_size(
            path,
            bundle_root,
            label=label,
            code=f"kagemusha_release_{key}_file_shape",
        )
        blockers.extend(entry_blockers)
        if entry is not None:
            evidence_entries[key] = entry

    if input_paths_ok and lineage_path_ok:
        lineage_artifact_entries, lineage_artifact_blockers = _artifact_inventory_entries(
            lineage_proof_evidence_path.parent,
            bundle_root,
            artifact_names=readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS,
            artifact_sha256=lineage.get("artifact_sha256"),
            artifact_size_bytes=lineage.get("artifact_size_bytes"),
            label_prefix="Reserved-lineage proof evidence",
            code_prefix="kagemusha_release_lineage_artifact",
            artifact_content_validator=readiness.validate_lineage_artifact_content,
        )
        blockers.extend(lineage_artifact_blockers)
        if lineage_artifact_entries:
            evidence_entries["lineage_artifacts"] = lineage_artifact_entries

        lineage_log_entries, lineage_log_blockers = _lineage_proof_log_entries(
            lineage_proof_evidence_path.parent,
            bundle_root,
            lineage,
        )
        blockers.extend(lineage_log_blockers)
        if lineage_log_entries:
            evidence_entries["lineage_proof_logs"] = lineage_log_entries

    if input_paths_ok and compact_path_ok:
        compact_artifact_entries, compact_artifact_blockers = _artifact_inventory_entries(
            compact_key_evidence_path.parent,
            bundle_root,
            artifact_names=readiness.COMPACT_KEY_REQUIRED_ARTIFACTS,
            artifact_sha256=compact.get("artifact_sha256"),
            artifact_size_bytes=compact.get("artifact_size_bytes"),
            label_prefix="ABI-7 recursive compact key evidence",
            code_prefix="kagemusha_release_compact_artifact",
            artifact_content_validator=readiness.validate_compact_key_artifact_content,
        )
        blockers.extend(compact_artifact_blockers)
        if compact_artifact_entries:
            evidence_entries["compact_key_artifacts"] = compact_artifact_entries
        compact_log_entry, compact_log_blockers = _evidence_entry_with_size(
            compact_key_evidence_path.parent / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME,
            bundle_root,
            label="ABI-7 recursive compact key generator log",
            code="kagemusha_release_compact_generator_log_file_shape",
        )
        blockers.extend(compact_log_blockers)
        if compact_log_entry is not None:
            if compact.get("generator_log_sha256") != compact_log_entry["sha256"]:
                blockers.append(
                    _blocker(
                        "kagemusha_release_compact_generator_log_digest_drift",
                        "ABI-7 recursive compact key generator log digest no longer matches validated readiness summary",
                    )
                )
            else:
                evidence_entries["compact_key_generator_log"] = compact_log_entry

    if input_paths_ok and android_path_ok:
        android_signed_entries, android_entry_blockers = _android_signed_evidence_entries(
            device_lab_root,
            bundle_root,
            android,
        )
        blockers.extend(android_entry_blockers)
        if android_signed_entries:
            evidence_entries["android_signed_evidence"] = android_signed_entries

        android_slot_entries, android_slot_blockers = _android_slot_artifact_entries(
            device_lab_root,
            bundle_root,
            android,
        )
        blockers.extend(android_slot_blockers)
        if android_slot_entries:
            evidence_entries["android_slot_artifacts"] = android_slot_entries

    manifest: dict[str, Any] = {
        "schema": RELEASE_BUNDLE_SCHEMA,
        "generated_at_utc": readiness.utc_now(),
        "ready": not blockers,
        "evidence": evidence_entries,
        "abi6_reserved_lineage": {
            "manifest_path": abi6.get("manifest_path"),
            "schema": abi6.get("schema"),
            "native_bridge_abi_version": abi6.get("native_bridge_abi_version"),
            "operation_count": abi6.get("operation_count"),
            "limits": abi6.get("limits", {}),
            "modes": abi6.get("modes", {}),
        },
        "abi7_recursive_compact": {
            "state": abi7.get("state"),
            "circuit_id": abi7.get("circuit_id"),
            "fixture_manifest_path": abi7.get("fixture_manifest_path"),
            "fixture_manifest_schema": abi7.get("fixture_manifest_schema"),
            "fixture_manifest_sha256": abi7.get("fixture_manifest_sha256"),
            "archive_fixture_path": abi7.get("archive_fixture_path"),
            "archive_fixture_schema": abi7.get("archive_fixture_schema"),
            "archive_fixture_sha256": abi7.get("archive_fixture_sha256"),
            "native_bridge_abi_version": abi7.get("native_bridge_abi_version"),
            "operation_count": abi7.get("operation_count"),
        },
        "lineage_key_release_tooling": {
            "state": lineage_tooling.get("state"),
            "checked_files": lineage_tooling.get("checked_files", []),
        },
        "lineage_proof_evidence": {
            "state": lineage.get("state"),
            "generated_at_utc": lineage.get("generated_at_utc"),
            "artifact_sha256": lineage.get("artifact_sha256", {}),
            "artifact_size_bytes": lineage.get("artifact_size_bytes", {}),
            "test_log_sha256": lineage.get("test_log_sha256", {}),
        },
        "compact_key_evidence": {
            "state": compact.get("state"),
            "generated_at_utc": compact.get("generated_at_utc"),
            "artifact_sha256": compact.get("artifact_sha256", {}),
            "artifact_size_bytes": compact.get("artifact_size_bytes", {}),
            "generator_log_sha256": compact.get("generator_log_sha256"),
            "generator_log_artifact_sha256": compact.get(
                "generator_log_artifact_sha256",
                {},
            ),
            "generator_log_artifact_size_bytes": compact.get(
                "generator_log_artifact_size_bytes",
                {},
            ),
        },
        "localnet_lifecycle_evidence": {
            "state": localnet_lifecycle.get("state"),
            "generated_at_utc": localnet_lifecycle.get("generated_at_utc"),
            "localnet_run_id": localnet_lifecycle.get("localnet_run_id"),
            "chain_id": localnet_lifecycle.get("chain_id"),
            "target": localnet_lifecycle.get("target"),
            "peer_count": localnet_lifecycle.get("peer_count"),
            "peer_ids": localnet_lifecycle.get("peer_ids", []),
            "artifact_sha256": localnet_lifecycle.get("artifact_sha256", {}),
            "artifact_count": localnet_lifecycle.get("artifact_count"),
        },
        "android_device_lab": {
            "root": readiness.ANDROID_DEVICE_LAB_ROOT_SUMMARY_LABEL,
            "covered_device_families": android.get("covered_device_families", []),
            "missing_device_families": android.get("missing_device_families", []),
            "covered_d2d_payment_transports": android.get(
                "covered_d2d_payment_transports",
                [],
            ),
            "missing_d2d_payment_transports": android.get(
                "missing_d2d_payment_transports",
                [],
            ),
            "duplicate_bindings": android.get("duplicate_bindings", {}),
            "signed_evidence": android.get("signed_evidence", {}),
            "trusted_signer_public_key_sha256": android.get(
                "trusted_signer_public_key_sha256",
                [],
            ),
        },
        "blockers": blockers,
    }
    return manifest, blockers


def verify_release_bundle(
    *,
    repo_root: Path,
    bundle_root: Path,
    readiness_summary_path: Path,
    lineage_proof_evidence_path: Path,
    compact_key_evidence_path: Path,
    device_lab_root: Path,
    trusted_signer_public_keys: dict[str, Path],
    existing_bundle_path: Path,
    min_signed_at: dt.datetime | None,
    max_signed_at: dt.datetime | None,
    min_lineage_proof_evidence_at: dt.datetime | None,
    max_lineage_proof_evidence_at: dt.datetime | None,
    min_compact_key_evidence_at: dt.datetime | None,
    max_compact_key_evidence_at: dt.datetime | None,
    localnet_lifecycle_evidence_path: Path | None = None,
    min_localnet_lifecycle_evidence_at: dt.datetime | None = None,
    max_localnet_lifecycle_evidence_at: dt.datetime | None = None,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Verify an existing release bundle manifest against local evidence."""

    signer_map_blockers = [
        _blocker("android_trusted_signer_invalid", error)
        for error in device_lab.validate_trusted_signer_public_key_map(
            trusted_signer_public_keys
        )
    ]
    if signer_map_blockers:
        return (
            _blocked_release_bundle_manifest(
                signer_map_blockers,
                trusted_signer_public_keys,
            ),
            signer_map_blockers,
        )

    repo_root_blockers = readiness.validate_repo_root_path(repo_root)
    if repo_root_blockers:
        return (
            _blocked_release_bundle_manifest(
                repo_root_blockers,
                trusted_signer_public_keys,
            ),
            repo_root_blockers,
        )

    blockers: list[dict[str, Any]] = []
    root_blockers = _validate_bundle_root(bundle_root)
    blockers.extend(root_blockers)
    path_ok = False
    path_blockers: list[dict[str, Any]] = []
    if not root_blockers:
        path_ok, path_blockers = _preflight_bundle_input_path(
            existing_bundle_path,
            bundle_root,
            "Kagemusha release bundle manifest",
        )
    blockers.extend(path_blockers)
    existing = None
    load_blockers: list[dict[str, Any]] = []
    if path_ok:
        existing, load_blockers = _load_local_json(
            existing_bundle_path,
            "Kagemusha release bundle manifest",
            "kagemusha_release_bundle_manifest",
        )
        blockers.extend(load_blockers)
    shape_blockers: list[dict[str, Any]] = []
    if existing is not None:
        shape_blockers = _check_release_bundle_manifest_shape(existing)
        blockers.extend(shape_blockers)
    expected: dict[str, Any] | None = None
    build_blockers: list[dict[str, Any]] = []
    if (
        existing is not None
        and not path_blockers
        and not load_blockers
        and not shape_blockers
    ):
        expected, build_blockers = build_release_bundle(
            repo_root=repo_root,
            bundle_root=bundle_root,
            readiness_summary_path=readiness_summary_path,
            lineage_proof_evidence_path=lineage_proof_evidence_path,
            compact_key_evidence_path=compact_key_evidence_path,
            device_lab_root=device_lab_root,
            trusted_signer_public_keys=trusted_signer_public_keys,
            min_signed_at=min_signed_at,
            max_signed_at=max_signed_at,
            min_lineage_proof_evidence_at=min_lineage_proof_evidence_at,
            max_lineage_proof_evidence_at=max_lineage_proof_evidence_at,
            min_compact_key_evidence_at=min_compact_key_evidence_at,
            max_compact_key_evidence_at=max_compact_key_evidence_at,
            localnet_lifecycle_evidence_path=localnet_lifecycle_evidence_path,
            min_localnet_lifecycle_evidence_at=min_localnet_lifecycle_evidence_at,
            max_localnet_lifecycle_evidence_at=max_localnet_lifecycle_evidence_at,
        )
        blockers.extend(build_blockers)
    top_level_binding_blockers: list[dict[str, Any]] = []
    if expected is not None and existing is not None and not build_blockers:
        top_level_binding_blockers = (
            _check_release_bundle_expected_top_level_evidence_binding(
                existing,
                expected,
            )
        )
        blockers.extend(top_level_binding_blockers)
    android_summary_binding_blockers: list[dict[str, Any]] = []
    if expected is not None and existing is not None and not build_blockers:
        android_summary_binding_blockers = (
            _check_release_bundle_expected_android_summary_binding(
                existing,
                expected,
            )
        )
        blockers.extend(android_summary_binding_blockers)
    section_value_binding_blockers: list[dict[str, Any]] = []
    if expected is not None and existing is not None and not build_blockers:
        section_value_binding_blockers = (
            _check_release_bundle_expected_section_value_binding(
                existing,
                expected,
            )
        )
        blockers.extend(section_value_binding_blockers)
    android_evidence_binding_blockers: list[dict[str, Any]] = []
    if expected is not None and existing is not None and not build_blockers:
        android_evidence_binding_blockers = (
            _check_release_bundle_expected_android_evidence_binding(
                existing,
                expected,
            )
        )
        blockers.extend(android_evidence_binding_blockers)
    compact_generator_log_artifact_binding_blockers: list[dict[str, Any]] = []
    if expected is not None and existing is not None and not build_blockers:
        compact_generator_log_artifact_binding_blockers = (
            _check_release_bundle_expected_compact_generator_log_artifact_binding(
                existing,
                expected,
            )
        )
        blockers.extend(compact_generator_log_artifact_binding_blockers)
    if (
        expected is not None
        and existing is not None
        and not build_blockers
        and not top_level_binding_blockers
        and not android_summary_binding_blockers
        and not section_value_binding_blockers
        and not android_evidence_binding_blockers
        and not compact_generator_log_artifact_binding_blockers
        and _stable_release_bundle(existing) != _stable_release_bundle(expected)
    ):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_drift",
                "Kagemusha release bundle manifest no longer matches local release evidence",
            )
        )
    if expected is None:
        expected = {
            "schema": RELEASE_BUNDLE_SCHEMA,
            "generated_at_utc": readiness.utc_now(),
            "ready": False,
            "evidence": {},
            "blockers": blockers,
        }
    verification = {
        **expected,
        "ready": not blockers,
        "blockers": blockers,
    }
    return verification, blockers


def _release_bundle_out_blocker(message: str) -> dict[str, Any]:
    return _blocker("kagemusha_release_bundle_out_invalid", message)


def _read_output_text(
    path: Path,
    expected_stat: os.stat_result,
) -> tuple[str | None, list[dict[str, Any]]]:
    """Read release-bundle output text without trusting a stale path."""

    chunks: list[bytes] = []
    output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [_release_bundle_out_blocker("--out must not be a symlink")]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [_release_bundle_out_blocker("--out must be a regular file")]
            output_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if output_open_identity != output_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != output_expected_identity:
                return None, [_release_bundle_out_blocker("--out changed while being read")]
            if open_stat.st_nlink > 1:
                return None, [_release_bundle_out_blocker("--out must not be hardlinked")]
            if stat.S_IMODE(open_stat.st_mode) != 0o600:
                return None, [_release_bundle_out_blocker("--out permissions must be 0600")]
            if open_stat.st_size > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:
                return None, [
                    _release_bundle_out_blocker(
                        f"--out must be no more than {MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES} bytes"
                    )
                ]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:
                    return None, [
                        _release_bundle_out_blocker(
                            f"--out must be no more than {MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES} bytes"
                        )
                    ]
                chunks.append(chunk)
            final_path_stat = path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != output_expected_identity:
                return None, [_release_bundle_out_blocker("--out changed while being read")]
    except OSError:
        return None, [
            _release_bundle_out_blocker("--out could not be read back after writing")
        ]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [
            _release_bundle_out_blocker("--out could not be read back after writing")
        ]


def write_release_bundle(path: Path, bundle: dict[str, Any], bundle_root: Path) -> list[dict[str, Any]]:
    """Write a validated release bundle manifest."""

    output_blockers = _validate_output_path(path, bundle_root)
    if output_blockers:
        return output_blockers
    try:
        parent_stat = path.parent.lstat()
    except OSError:
        return [_release_bundle_out_blocker("--out parent directory metadata could not be read")]
    if stat.S_ISLNK(parent_stat.st_mode) or not stat.S_ISDIR(parent_stat.st_mode):
        return [_release_bundle_out_blocker("--out parent directory could not be synced")]
    parent_identity = _file_identity(parent_stat)
    relative_path, relative_blockers = _relative_to_bundle(path, bundle_root, "--out")
    if relative_blockers:
        return relative_blockers
    if relative_path in _bundle_evidence_paths(bundle):
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out must not overwrite bundled evidence input",
            )
        ]
    try:
        manifest_text = json.dumps(
            bundle,
            indent=2,
            sort_keys=True,
            allow_nan=False,
        ) + "\n"
    except ValueError:
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "release bundle manifest is not strict JSON",
            )
        ]
    if len(manifest_text.encode("utf-8")) > MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES:
        return [
            _release_bundle_out_blocker(
                f"--out must be no more than {MAX_RELEASE_BUNDLE_OUTPUT_JSON_BYTES} bytes"
            )
        ]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return [_release_bundle_out_blocker("--out parent directory metadata could not be read")]
    try:
        try:
            opened_parent_stat = os.fstat(parent_fd)
        except OSError:
            return [_release_bundle_out_blocker("--out parent directory metadata could not be read")]
        if (
            not stat.S_ISDIR(opened_parent_stat.st_mode)
            or _file_identity(opened_parent_stat) != parent_identity
        ):
            return [_release_bundle_out_blocker("--out parent directory changed before sync")]
        return _write_release_bundle_with_parent_fd(
            path,
            manifest_text,
            bundle_root,
            parent_fd=parent_fd,
            parent_identity=parent_identity,
        )
    finally:
        os.close(parent_fd)


def _write_release_bundle_with_parent_fd(
    path: Path,
    manifest_text: str,
    bundle_root: Path,
    *,
    parent_fd: int,
    parent_identity: tuple[int, int],
) -> list[dict[str, Any]]:
    tmp_path: Path | None = None
    tmp_identity: tuple[int, int] | None = None
    write_blockers: list[dict[str, Any]] = []
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            dir=path.parent,
            encoding="utf-8",
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            tmp_path = Path(handle.name)
            os.fchmod(handle.fileno(), 0o600)
            tmp_identity = _file_identity(os.fstat(handle.fileno()))
            handle.write(manifest_text)
            handle.flush()
            os.fsync(handle.fileno())
        temp_relative, temp_relative_blockers = _relative_to_bundle(
            tmp_path,
            bundle_root,
            "--out temporary file",
        )
        if temp_relative_blockers:
            write_blockers.extend(temp_relative_blockers)
        else:
            assert temp_relative is not None
            output_blockers = _validate_output_path(path, bundle_root)
            if output_blockers:
                write_blockers.extend(output_blockers)
            else:
                os.replace(
                    tmp_path.name,
                    path.name,
                    src_dir_fd=parent_fd,
                    dst_dir_fd=parent_fd,
                )
                tmp_path = None
    except OSError:
        write_blockers.append(
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out could not be written",
            )
        )
    finally:
        if tmp_path is not None:
            write_blockers.extend(_cleanup_temp_output(tmp_path, tmp_identity))
    if write_blockers:
        return write_blockers
    try:
        expected_stat = os.stat(path.name, dir_fd=parent_fd, follow_symlinks=False)
    except (FileNotFoundError, OSError):
        return [
            _release_bundle_out_blocker("--out could not be read back after writing")
        ]
    if stat.S_ISLNK(expected_stat.st_mode):
        return [_release_bundle_out_blocker("--out must not be a symlink")]
    if not stat.S_ISREG(expected_stat.st_mode):
        return [
            _release_bundle_out_blocker("--out could not be read back after writing")
        ]
    output_identity = _file_identity(expected_stat)
    try:
        current_parent_stat = path.parent.lstat()
    except OSError:
        cleanup_blockers = _unlink_output_if_identity_at(
            parent_fd,
            path.name,
            output_identity,
        )
        return [
            _release_bundle_out_blocker(
                "--out parent directory metadata could not be read"
            ),
            *cleanup_blockers,
        ]
    if _file_identity(current_parent_stat) != parent_identity:
        cleanup_blockers = _unlink_output_if_identity_at(
            parent_fd,
            path.name,
            output_identity,
        )
        return [
            _release_bundle_out_blocker("--out parent directory changed before sync"),
            *cleanup_blockers,
        ]
    sync_blockers = _sync_output_parent_fd(parent_fd, expected_identity=parent_identity)
    if sync_blockers:
        cleanup_blockers = _unlink_output_if_identity_at(
            parent_fd,
            path.name,
            output_identity,
        )
        return [*sync_blockers, *cleanup_blockers]
    output_blockers = _validate_output_path(path, bundle_root)
    if output_blockers:
        return output_blockers
    readback, readback_blockers = _read_output_text(path, expected_stat)
    if readback_blockers:
        return readback_blockers
    if readback != manifest_text:
        return [
            _release_bundle_out_blocker(
                "--out readback did not match the generated manifest",
            )
        ]
    return []


def _unlink_output_if_identity_at(
    parent_fd: int,
    name: str,
    expected_identity: tuple[int, int],
) -> list[dict[str, Any]]:
    try:
        file_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return []
    except OSError:
        return [
            _release_bundle_out_blocker("--out rollback cleanup metadata could not be read")
        ]
    if not stat.S_ISREG(file_stat.st_mode) or _file_identity(file_stat) != expected_identity:
        return []
    try:
        os.unlink(name, dir_fd=parent_fd)
    except FileNotFoundError:
        return []
    except OSError:
        return [
            _release_bundle_out_blocker(
                "--out could not be removed after parent sync failure"
            )
        ]
    return []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate and manifest a Kagemusha production release evidence bundle."
    )
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Repository root used to recheck checked-in Kagemusha trust roots.",
    )
    parser.add_argument("--bundle-root", default=".", help="Root that contains all bundled evidence files.")
    parser.add_argument(
        "--readiness-summary",
        default=DEFAULT_READINESS_SUMMARY_PATH,
        help="Ready Kagemusha production-readiness summary JSON.",
    )
    parser.add_argument(
        "--lineage-proof-evidence",
        default=readiness.DEFAULT_LINEAGE_PROOF_EVIDENCE_PATH,
        help="Reserved-lineage proof evidence JSON included in the release bundle.",
    )
    parser.add_argument(
        "--compact-key-evidence",
        default=readiness.DEFAULT_COMPACT_KEY_EVIDENCE_PATH,
        help="ABI-7 recursive compact key evidence JSON included in the release bundle.",
    )
    parser.add_argument(
        "--localnet-lifecycle-evidence",
        default=readiness.DEFAULT_LOCALNET_LIFECYCLE_EVIDENCE_PATH,
        help="Kagemusha 4-peer localnet lifecycle evidence JSON included in the release bundle.",
    )
    parser.add_argument(
        "--device-lab-root",
        default="artifacts/android/device_lab",
        help="Android device-lab root included in the release bundle.",
    )
    parser.add_argument(
        "--trusted-signer-public-key",
        action="append",
        dest="trusted_signer_public_keys",
        default=None,
        help="PEM public key for a trusted Android lab evidence signer.",
    )
    parser.add_argument(
        "--min-signed-at-utc",
        default=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
        help="Minimum signed_at_utc accepted for Android lab evidence.",
    )
    parser.add_argument(
        "--max-signed-at-future-skew-seconds",
        type=int,
        default=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help="Maximum Android signed evidence future clock skew in seconds.",
    )
    parser.add_argument(
        "--min-lineage-proof-evidence-at-utc",
        default=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
        help="Minimum generated_at_utc accepted for Reserved-lineage proof evidence.",
    )
    parser.add_argument(
        "--max-lineage-proof-evidence-future-skew-seconds",
        type=int,
        default=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help="Maximum Reserved-lineage proof evidence future clock skew in seconds.",
    )
    parser.add_argument(
        "--min-compact-key-evidence-at-utc",
        default=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
        help="Minimum generated_at_utc accepted for ABI-7 compact key evidence.",
    )
    parser.add_argument(
        "--max-compact-key-evidence-future-skew-seconds",
        type=int,
        default=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help="Maximum ABI-7 compact key evidence future clock skew in seconds.",
    )
    parser.add_argument(
        "--min-localnet-lifecycle-evidence-at-utc",
        default=readiness.DEFAULT_MIN_SIGNED_AT_UTC,
        help="Minimum generated_at_utc accepted for Kagemusha localnet lifecycle evidence.",
    )
    parser.add_argument(
        "--max-localnet-lifecycle-evidence-future-skew-seconds",
        type=int,
        default=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help="Maximum Kagemusha localnet lifecycle evidence future clock skew in seconds.",
    )
    parser.add_argument("--out", default=DEFAULT_RELEASE_BUNDLE_OUT, help="Output release bundle JSON path.")
    parser.add_argument(
        "--verify-existing",
        default=None,
        help="Verify an existing release bundle manifest instead of writing --out.",
    )
    args = parser.parse_args(argv)

    path_blockers = [
        item
        for item in (
            _secret_path_error(args.repo_root, "--repo-root", "kagemusha_repo_root_path_invalid"),
            _secret_path_error(args.bundle_root, "--bundle-root", "kagemusha_release_bundle_root_invalid"),
            _secret_path_error(args.readiness_summary, "--readiness-summary", "kagemusha_release_summary_path_invalid"),
            _secret_path_error(args.lineage_proof_evidence, "--lineage-proof-evidence", "lineage_proof_evidence_path_invalid"),
            _secret_path_error(args.compact_key_evidence, "--compact-key-evidence", "compact_key_evidence_path_invalid"),
            _secret_path_error(args.localnet_lifecycle_evidence, "--localnet-lifecycle-evidence", "localnet_lifecycle_evidence_path_invalid"),
            _secret_path_error(args.device_lab_root, "--device-lab-root", "android_device_lab_root_path_invalid"),
            _secret_path_error(args.out, "--out", "kagemusha_release_bundle_out_invalid"),
            _secret_path_error(args.verify_existing, "--verify-existing", "kagemusha_release_bundle_manifest_path_invalid"),
        )
        if item is not None
    ]
    for index, key_path in enumerate(args.trusted_signer_public_keys or []):
        secret = _secret_path_error(
            key_path,
            f"--trusted-signer-public-key[{index}]",
            "android_trusted_signer_path_invalid",
        )
        if secret is not None:
            path_blockers.append(secret)

    trusted: dict[str, Path] = {}
    if not path_blockers:
        trusted, signer_errors = device_lab.load_trusted_signer_public_keys(
            args.trusted_signer_public_keys
        )
        path_blockers.extend(
            _blocker("android_trusted_signer_invalid", error) for error in signer_errors
        )
    min_signed_at, min_signed_blockers = _parse_optional_timestamp(
        args.min_signed_at_utc,
        "--min-signed-at-utc",
        "android_min_signed_at_invalid",
    )
    min_lineage_at, min_lineage_blockers = _parse_optional_timestamp(
        args.min_lineage_proof_evidence_at_utc,
        "--min-lineage-proof-evidence-at-utc",
        "lineage_proof_evidence_min_timestamp_invalid",
    )
    min_compact_at, min_compact_blockers = _parse_optional_timestamp(
        args.min_compact_key_evidence_at_utc,
        "--min-compact-key-evidence-at-utc",
        "compact_key_evidence_min_timestamp_invalid",
    )
    min_localnet_at, min_localnet_blockers = _parse_optional_timestamp(
        args.min_localnet_lifecycle_evidence_at_utc,
        "--min-localnet-lifecycle-evidence-at-utc",
        "localnet_lifecycle_evidence_min_timestamp_invalid",
    )
    max_signed_at, max_signed_blockers = _future_limit(
        args.max_signed_at_future_skew_seconds,
        "--max-signed-at-future-skew-seconds",
        "android_max_signed_at_invalid",
    )
    max_lineage_at, max_lineage_blockers = _future_limit(
        args.max_lineage_proof_evidence_future_skew_seconds,
        "--max-lineage-proof-evidence-future-skew-seconds",
        "lineage_proof_evidence_max_timestamp_invalid",
    )
    max_compact_at, max_compact_blockers = _future_limit(
        args.max_compact_key_evidence_future_skew_seconds,
        "--max-compact-key-evidence-future-skew-seconds",
        "compact_key_evidence_max_timestamp_invalid",
    )
    max_localnet_at, max_localnet_blockers = _future_limit(
        args.max_localnet_lifecycle_evidence_future_skew_seconds,
        "--max-localnet-lifecycle-evidence-future-skew-seconds",
        "localnet_lifecycle_evidence_max_timestamp_invalid",
    )
    path_blockers.extend(
        [
            *min_signed_blockers,
            *min_lineage_blockers,
            *min_compact_blockers,
            *min_localnet_blockers,
            *max_signed_blockers,
            *max_lineage_blockers,
            *max_compact_blockers,
            *max_localnet_blockers,
        ]
    )

    if path_blockers:
        bundle = {
            "schema": RELEASE_BUNDLE_SCHEMA,
            "generated_at_utc": readiness.utc_now(),
            "ready": False,
            "evidence": {},
            "blockers": path_blockers,
        }
        blockers = path_blockers
    else:
        bundle_root = Path(args.bundle_root)
        common_kwargs = {
            "repo_root": Path(args.repo_root),
            "bundle_root": bundle_root,
            "readiness_summary_path": _bundle_path(args.readiness_summary, bundle_root),
            "lineage_proof_evidence_path": _bundle_path(
                args.lineage_proof_evidence,
                bundle_root,
            ),
            "compact_key_evidence_path": _bundle_path(
                args.compact_key_evidence,
                bundle_root,
            ),
            "localnet_lifecycle_evidence_path": _bundle_path(
                args.localnet_lifecycle_evidence,
                bundle_root,
            ),
            "device_lab_root": _bundle_path(args.device_lab_root, bundle_root),
            "trusted_signer_public_keys": trusted,
            "min_signed_at": min_signed_at,
            "max_signed_at": max_signed_at,
            "min_lineage_proof_evidence_at": min_lineage_at,
            "max_lineage_proof_evidence_at": max_lineage_at,
            "min_compact_key_evidence_at": min_compact_at,
            "max_compact_key_evidence_at": max_compact_at,
            "min_localnet_lifecycle_evidence_at": min_localnet_at,
            "max_localnet_lifecycle_evidence_at": max_localnet_at,
        }
        if args.verify_existing:
            bundle, blockers = verify_release_bundle(
                **common_kwargs,
                existing_bundle_path=_bundle_path(args.verify_existing, bundle_root),
            )
        else:
            bundle, blockers = build_release_bundle(**common_kwargs)

    if bundle["ready"] and args.verify_existing:
        print("[kagemusha-release-bundle] verified")
        return 0

    if bundle["ready"]:
        try:
            resolved_bundle_root = Path(args.bundle_root).resolve()
        except OSError:
            write_blockers = [
                _blocker(
                    "kagemusha_release_bundle_root_invalid",
                    "--bundle-root could not be resolved",
                )
            ]
        else:
            out_path = _bundle_path(args.out, resolved_bundle_root)
            write_blockers = write_release_bundle(
                out_path,
                bundle,
                resolved_bundle_root,
            )
        if write_blockers:
            bundle["ready"] = False
            bundle["blockers"].extend(write_blockers)
            blockers = [*blockers, *write_blockers]
        else:
            print("[kagemusha-release-bundle] wrote manifest")

    if bundle["ready"]:
        print("[kagemusha-release-bundle] ready")
        return 0
    for item in blockers:
        print(
            f"[kagemusha-release-bundle] blocked: {item['code']}: {item['message']}",
            file=sys.stderr,
        )
    return 1


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
