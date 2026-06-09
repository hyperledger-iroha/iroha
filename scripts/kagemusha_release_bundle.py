#!/usr/bin/env python3
"""Validate and manifest a Kagemusha production release evidence bundle."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
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
ANDROID_SIGNED_EVIDENCE_SUMMARY_REQUIRED_FIELDS = frozenset(
    (
        "signed_at_utc",
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
        "android_device_lab",
        "blockers",
    )
)
SUMMARY_ALLOWED_SECTION_KEYS: dict[str, frozenset[str]] = {
    "abi6_reserved_lineage": frozenset(
        (
            "manifest_path",
            "schema",
            "bridge_abi_version",
            "operation_count",
            "limits",
            "modes",
            "ok",
            "blockers",
        )
    ),
    "abi7_recursive_compact": frozenset(("ok", "state", "circuit_id", "blockers")),
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
    "android_device_lab": frozenset(
        (
            "ok",
            "root",
            "slots",
            "covered_device_families",
            "missing_device_families",
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


def _secret_path_error(path: str | None, label: str, code: str) -> dict[str, Any] | None:
    if path is not None and device_lab.SECRET_RE.search(path):
        return _blocker(code, f"{label} must not contain secret-looking material")
    return None


def _validate_bundle_root(root: Path) -> list[dict[str, Any]]:
    secret = _secret_path_error(str(root), "--bundle-root", "kagemusha_release_bundle_root_invalid")
    if secret is not None:
        return [secret]
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
    return device_lab.SECRET_PATH_REDACTION if device_lab.SECRET_RE.search(text) else text


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
                        "Android signed-evidence summary field must be a non-empty string",
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
                _, timestamp_blocker = readiness.parse_utc_timestamp(
                    value,
                    "Android signed-evidence summary signed_at_utc",
                )
                if timestamp_blocker is not None:
                    timestamp_blocker["code"] = (
                        "kagemusha_release_summary_android_signed_evidence_timestamp"
                    )
                    timestamp_blocker["slot"] = display_slot
                    blockers.append(timestamp_blocker)
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
                if (
                    device_lab._normalise_safe_relative_path(  # type: ignore[attr-defined]
                        value,
                        path_errors,
                        f"Android signed-evidence summary {field}",
                    )
                    is None
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
    unexpected_fields = sorted(set(summary) - SUMMARY_ALLOWED_TOP_LEVEL_KEYS)
    for field in unexpected_fields:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_unexpected_field",
                "Kagemusha readiness summary contains an unexpected top-level field",
                field=_display_summary_field(field),
            )
        )
    for section_name, allowed_fields in SUMMARY_ALLOWED_SECTION_KEYS.items():
        section = _section(summary, section_name)
        if section is None:
            continue
        for field in sorted(set(section) - allowed_fields):
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_unexpected_section_field",
                    "Kagemusha readiness summary section contains an unexpected field",
                    section=section_name,
                    field=_display_summary_field(field),
                )
            )
        if section.get("blockers") != []:
            blockers.append(
                _blocker(
                    "kagemusha_release_summary_section_blockers_present",
                    "Kagemusha readiness summary section must not contain blockers",
                    section=section_name,
                )
            )
    if summary.get("schema") != readiness.SUMMARY_SCHEMA:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_schema",
                "Kagemusha readiness summary schema mismatch",
            )
        )
    if summary.get("ready") is not True or summary.get("status") != "ready":
        blockers.append(
            _blocker(
                "kagemusha_release_summary_not_ready",
                "Kagemusha readiness summary must be ready",
            )
        )
    summary_blockers = summary.get("blockers")
    if summary_blockers != []:
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
    elif android.get("missing_device_families") != []:
        blockers.append(
            _blocker(
                "kagemusha_release_summary_android_matrix_incomplete",
                "Android device-lab summary must cover the full standard matrix",
            )
        )
    else:
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


def _compare_validated_sections(
    summary: dict[str, Any],
    abi6: dict[str, Any],
    abi7: dict[str, Any],
    lineage_tooling: dict[str, Any],
    lineage: dict[str, Any],
    compact: dict[str, Any],
    android: dict[str, Any],
) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    abi6_summary = _section(summary, "abi6_reserved_lineage") or {}
    abi7_summary = _section(summary, "abi7_recursive_compact") or {}
    lineage_tooling_summary = _section(summary, "lineage_key_release_tooling") or {}
    lineage_summary = _section(summary, "lineage_proof_evidence") or {}
    compact_summary = _section(summary, "compact_key_evidence") or {}
    android_summary = _section(summary, "android_device_lab") or {}
    for field in (
        "manifest_path",
        "schema",
        "bridge_abi_version",
        "operation_count",
        "limits",
        "modes",
    ):
        blockers.extend(_compare_field(abi6_summary, abi6, "abi6_reserved_lineage", field))
    for field in ("state", "circuit_id"):
        blockers.extend(_compare_field(abi7_summary, abi7, "abi7_recursive_compact", field))
    for field in ("state", "checked_files"):
        blockers.extend(
            _compare_field(
                lineage_tooling_summary,
                lineage_tooling,
                "lineage_key_release_tooling",
                field,
            )
        )
    for field in (
        "state",
        "generated_at_utc",
        "artifact_sha256",
        "artifact_size_bytes",
        "test_log_sha256",
        "record_archive_proof_runtime_keygen_env",
    ):
        blockers.extend(
            _compare_field(lineage_summary, lineage, "lineage_proof_evidence", field)
        )
    for field in (
        "state",
        "generated_at_utc",
        "artifact_sha256",
        "artifact_size_bytes",
        "generator_log_sha256",
        "generator_log_artifact_sha256",
        "generator_log_artifact_size_bytes",
        "command_validated",
    ):
        blockers.extend(
            _compare_field(compact_summary, compact, "compact_key_evidence", field)
        )
    for field in (
        "covered_device_families",
        "missing_device_families",
        "signed_evidence",
        "trusted_signer_public_key_sha256",
    ):
        blockers.extend(
            _compare_field(android_summary, android, "android_device_lab", field)
        )
    return blockers


def _evidence_entry(
    path: Path,
    bundle_root: Path,
    *,
    label: str,
    code: str,
) -> tuple[dict[str, str] | None, list[dict[str, Any]]]:
    digest, digest_blockers = _sha256_file(path, label, code)
    relative, relative_blockers = _relative_to_bundle(path, bundle_root, label)
    blockers = [*digest_blockers, *relative_blockers]
    if blockers:
        return None, blockers
    assert digest is not None and relative is not None
    return {"path": relative, "sha256": digest}, []


def _evidence_entry_with_size(
    path: Path,
    bundle_root: Path,
    *,
    label: str,
    code: str,
) -> tuple[dict[str, Any] | None, list[dict[str, Any]]]:
    digest, size, digest_blockers = _sha256_file_with_size(path, label, code)
    relative, relative_blockers = _relative_to_bundle(path, bundle_root, label)
    blockers = [*digest_blockers, *relative_blockers]
    if blockers:
        return None, blockers
    assert digest is not None and size is not None and relative is not None
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

        if set(slot_entries) != {item[0] for item in ANDROID_SLOT_RELEASE_ARTIFACTS}:
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


def _cleanup_temp_output(path: Path) -> None:
    try:
        path.unlink()
    except FileNotFoundError:
        return
    except OSError:
        return


def _stable_release_bundle(bundle: dict[str, Any]) -> dict[str, Any]:
    return {
        key: value
        for key, value in bundle.items()
        if key != "generated_at_utc"
    }


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


def _check_release_bundle_manifest_shape(bundle: dict[str, Any]) -> list[dict[str, Any]]:
    blockers: list[dict[str, Any]] = []
    if _contains_secret_string(bundle):
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_secret_material",
                "Kagemusha release bundle manifest must not contain secret-looking material",
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
    if bundle.get("schema") != RELEASE_BUNDLE_SCHEMA:
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
            _, parse_blocker = readiness.parse_utc_timestamp(
                generated_at,
                "Kagemusha release bundle generated_at_utc",
            )
            if parse_blocker is not None:
                parse_blocker["code"] = "kagemusha_release_bundle_manifest_timestamp"
                blockers.append(parse_blocker)
    if bundle.get("ready") is not True:
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_not_ready",
                "Kagemusha release bundle manifest must be ready",
            )
        )
    if bundle.get("blockers") != []:
        blockers.append(
            _blocker(
                "kagemusha_release_bundle_manifest_blockers_present",
                "Kagemusha release bundle manifest must not contain blockers",
            )
        )
    blockers.extend(_check_release_bundle_evidence_paths(bundle.get("evidence")))
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
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Return the release bundle manifest and blockers."""

    blockers: list[dict[str, Any]] = []
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
    android_path_ok, android_path_blockers = _preflight_bundle_input_path(
        device_lab_root,
        bundle_root,
        "Android device-lab root",
    )
    input_path_blockers = [
        *summary_path_blockers,
        *lineage_path_blockers,
        *compact_path_blockers,
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
    blockers.extend(android["blockers"])
    if (
        summary is not None
        and input_paths_ok
        and not abi6["blockers"]
        and not abi7["blockers"]
        and not lineage_tooling["blockers"]
        and not lineage["blockers"]
        and not compact["blockers"]
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
            "bridge_abi_version": abi6.get("bridge_abi_version"),
            "operation_count": abi6.get("operation_count"),
            "limits": abi6.get("limits", {}),
            "modes": abi6.get("modes", {}),
        },
        "abi7_recursive_compact": {
            "state": abi7.get("state"),
            "circuit_id": abi7.get("circuit_id"),
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
        "android_device_lab": {
            "covered_device_families": android.get("covered_device_families", []),
            "missing_device_families": android.get("missing_device_families", []),
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
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Verify an existing release bundle manifest against local evidence."""

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
        )
        blockers.extend(build_blockers)
    if (
        expected is not None
        and existing is not None
        and not build_blockers
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
    tmp_path: Path | None = None
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
            handle.write(manifest_text)
            handle.flush()
            os.fsync(handle.fileno())
        temp_relative, temp_relative_blockers = _relative_to_bundle(
            tmp_path,
            bundle_root,
            "--out temporary file",
        )
        if temp_relative_blockers:
            return temp_relative_blockers
        assert temp_relative is not None
        output_blockers = _validate_output_path(path, bundle_root)
        if output_blockers:
            return output_blockers
        os.replace(tmp_path, path)
        tmp_path = None
    except OSError:
        return [
            _blocker(
                "kagemusha_release_bundle_out_invalid",
                "--out could not be written",
            )
        ]
    finally:
        if tmp_path is not None:
            _cleanup_temp_output(tmp_path)
    output_blockers = _validate_output_path(path, bundle_root)
    if output_blockers:
        return output_blockers
    try:
        parent_fd = os.open(path.parent, os.O_RDONLY)
    except OSError:
        parent_fd = None
    if parent_fd is not None:
        try:
            os.fsync(parent_fd)
        except OSError:
            pass
        finally:
            os.close(parent_fd)
    try:
        expected_stat = path.lstat()
    except (FileNotFoundError, OSError):
        return [
            _release_bundle_out_blocker("--out could not be read back after writing")
        ]
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
    path_blockers.extend(
        [
            *min_signed_blockers,
            *min_lineage_blockers,
            *min_compact_blockers,
            *max_signed_blockers,
            *max_lineage_blockers,
            *max_compact_blockers,
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
            "device_lab_root": _bundle_path(args.device_lab_root, bundle_root),
            "trusted_signer_public_keys": trusted,
            "min_signed_at": min_signed_at,
            "max_signed_at": max_signed_at,
            "min_lineage_proof_evidence_at": min_lineage_at,
            "max_lineage_proof_evidence_at": max_lineage_at,
            "min_compact_key_evidence_at": min_compact_at,
            "max_compact_key_evidence_at": max_compact_at,
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
